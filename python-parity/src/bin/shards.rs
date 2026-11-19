//! Deals the parity tests out to CI's shards by how long each one takes.
//!
//! The tests' times, measured on CI, are kept in `test-times.toml`. The known
//! tests are dealt slowest first, each to the shard with the least time so
//! far, so the shards finish together; a test the file does not know yet goes
//! to a shard by a hash of its name, so adding one moves no other. Within a
//! shard the slowest start first, as nextest priorities, so that no long test
//! starts last.
//!
//! ```text
//! shards run SHARD COUNT --list tests.json --config shard.toml
//!     prints the nextest filterset of shard SHARD of COUNT, from
//!     `cargo nextest list --message-format json`, and writes the
//!     priorities to shard.toml for `--config-file`.
//! shards times LOG...
//!     prints a new `test-times.toml` from the logs of nextest runs.
//! shards estimate COUNT THREADS
//!     prints how long each shard would take, from the times alone.
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fs;
use std::process::ExitCode;
use std::sync::LazyLock;

use regex::Regex;

/// The tests' times, beside this crate's manifest.
const TIMES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/test-times.toml");

/// The highest priority nextest takes; the slowest test gets it.
const TOP_PRIORITY: i64 = 100;

/// The lowest priority nextest takes.
const BOTTOM_PRIORITY: i64 = -100;

/// What a usage error exits with, as `argparse` exits.
const USAGE_ERROR: u8 = 2;

/// nextest's line for a test that passed, colours taken off:
/// `PASS [   3.007s] ( 3/57) music21-rs-python-parity::parity capella_parity::...`
static PASSED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\bPASS\s+\[\s*([0-9.]+)s\]\s+\(\s*\d+/\d+\)\s+\S+\s+(\S+)").expect("a pattern")
});
static COLOURS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("\x1b\\[[0-9;]*m").expect("a pattern"));

/// Each known test's seconds, by its name.
type Times = BTreeMap<String, f64>;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("shards: {error}");
            ExitCode::from(USAGE_ERROR)
        }
    }
}

fn run(args: &[String]) -> Result<(), Box<dyn Error>> {
    let usage = "usage: shards run SHARD COUNT --list FILE --config FILE | times LOG... | \
                 estimate COUNT THREADS";
    match args.first().map(String::as_str) {
        Some("times") if args.len() > 1 => {
            let mut texts = Vec::new();
            for log in &args[1..] {
                let bytes = fs::read(log)?;
                let text = String::from_utf8_lossy(&bytes);
                texts.push(text.trim_start_matches('\u{feff}').to_owned());
            }
            print!("{}", times_toml(&times_from_logs(&texts)));
            Ok(())
        }
        Some("estimate") if args.len() == 3 => {
            let count: usize = args[1].parse()?;
            let threads: usize = args[2].parse()?;
            estimate(&load_times(&fs::read_to_string(TIMES)?)?, count, threads);
            Ok(())
        }
        Some("run") => run_shard(&args[1..]),
        _ => Err(usage.into()),
    }
}

/// `run SHARD COUNT --list FILE --config FILE`.
fn run_shard(args: &[String]) -> Result<(), Box<dyn Error>> {
    let flag = |name: &str| -> Result<&String, Box<dyn Error>> {
        let at = args
            .iter()
            .position(|arg| arg == name)
            .ok_or(format!("run needs {name}"))?;
        Ok(args.get(at + 1).ok_or(format!("{name} needs a value"))?)
    };
    let shard: usize = args.first().ok_or("run needs SHARD")?.parse()?;
    let count: usize = args.get(1).ok_or("run needs COUNT")?.parse()?;
    if !(1..=count).contains(&shard) {
        return Err(format!("shard {shard} is not one of 1 to {count}").into());
    }
    let listing: serde_json::Value = serde_json::from_str(&fs::read_to_string(flag("--list")?)?)?;
    let known = load_times(&fs::read_to_string(TIMES)?)?;
    let names = deal(&listed_tests(&listing), &known, count).swap_remove(shard - 1);
    fs::write(flag("--config")?, priorities(&names, &known))?;
    println!("{}", filterset(&names));
    Ok(())
}

/// Each known test's seconds, from `test-times.toml`'s text.
fn load_times(text: &str) -> Result<Times, Box<dyn Error>> {
    let table: toml::Table = text.parse()?;
    let seconds = table
        .get("seconds")
        .and_then(toml::Value::as_table)
        .ok_or("test-times.toml has no [seconds] table")?;
    let mut times = Times::new();
    for (name, value) in seconds {
        let value = value
            .as_float()
            .or_else(|| value.as_integer().map(|whole| whole as f64))
            .ok_or(format!("{name} has no number of seconds"))?;
        times.insert(name.clone(), value);
    }
    Ok(times)
}

/// The names of the tests in `cargo nextest list --message-format json`.
fn listed_tests(listing: &serde_json::Value) -> Vec<String> {
    let mut names = BTreeSet::new();
    let suites = listing["rust-suites"].as_object().into_iter().flatten();
    for (_, suite) in suites {
        let cases = suite["testcases"].as_object().into_iter().flatten();
        names.extend(cases.map(|(name, _)| name.clone()));
    }
    names.into_iter().collect()
}

/// The shard a test with no time goes to, the same on every run.
fn hashed_shard(name: &str, count: usize) -> usize {
    crc32(name.as_bytes()) as usize % count
}

/// zlib's CRC-32 of `bytes`.
fn crc32(bytes: &[u8]) -> u32 {
    // The reversed IEEE 802.3 polynomial zlib uses.
    const POLYNOMIAL: u32 = 0xEDB8_8320;
    let mut crc = !0u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (POLYNOMIAL & mask);
        }
    }
    !crc
}

/// The names, the slowest first, ties by name.
fn slowest_first(names: &[String], times: &Times) -> Vec<String> {
    let mut known: Vec<String> = names
        .iter()
        .filter(|name| times.contains_key(*name))
        .cloned()
        .collect();
    known.sort_by(|a, b| times[b].total_cmp(&times[a]).then(a.cmp(b)));
    known
}

/// The tests of each of `count` shards: the known ones slowest first, each
/// to the shard with the least time so far (the first such), and the
/// unknown ones by hash.
fn deal(tests: &[String], times: &Times, count: usize) -> Vec<Vec<String>> {
    let mut shards = vec![Vec::new(); count];
    let mut loads = vec![0.0f64; count];
    for name in slowest_first(tests, times) {
        let lightest = (0..count)
            .min_by(|a, b| loads[*a].total_cmp(&loads[*b]).then(a.cmp(b)))
            .expect("at least one shard");
        loads[lightest] += times[&name];
        shards[lightest].push(name);
    }
    for name in tests.iter().filter(|name| !times.contains_key(*name)) {
        shards[hashed_shard(name, count)].push(name.clone());
    }
    for shard in &mut shards {
        shard.sort();
    }
    shards
}

/// The nextest filterset running exactly these tests.
fn filterset(names: &[String]) -> String {
    if names.is_empty() {
        return "none()".to_owned();
    }
    let each: Vec<String> = names.iter().map(|name| format!("test(={name})")).collect();
    each.join(" | ")
}

/// A nextest config starting the shard's tests slowest first; tests with no
/// time start after them.
fn priorities(names: &[String], times: &Times) -> String {
    let mut lines = Vec::new();
    for (rank, name) in slowest_first(names, times).iter().enumerate() {
        let priority = (TOP_PRIORITY - rank as i64).max(BOTTOM_PRIORITY);
        lines.push("[[profile.default.overrides]]".to_owned());
        lines.push(format!("filter = 'test(={name})'"));
        lines.push(format!("priority = {priority}"));
        lines.push(String::new());
    }
    lines.join("\n")
}

/// How long tests of these times take on `threads` threads, the slowest
/// started first.
fn wall_time(seconds: &[f64], threads: usize) -> f64 {
    let mut finishing = vec![0.0f64; threads];
    let mut sorted = seconds.to_vec();
    sorted.sort_by(|a, b| b.total_cmp(a));
    for length in sorted {
        let first = (0..threads)
            .min_by(|a, b| finishing[*a].total_cmp(&finishing[*b]))
            .expect("at least one thread");
        finishing[first] += length;
    }
    finishing.into_iter().fold(0.0, f64::max)
}

/// Prints how long each shard would take.
fn estimate(known: &Times, count: usize, threads: usize) {
    let names: Vec<String> = known.keys().cloned().collect();
    for (index, shard) in deal(&names, known, count).iter().enumerate() {
        let seconds: Vec<f64> = shard.iter().map(|name| known[name]).collect();
        let total: f64 = seconds.iter().sum();
        println!(
            "{}: {} tests, {:.1} min of tests, {:.1} min on {threads} threads",
            index + 1,
            shard.len(),
            total / 60.0,
            wall_time(&seconds, threads) / 60.0
        );
    }
}

/// Each passing test's seconds, from nextest's output.
fn times_from_logs(texts: &[String]) -> Times {
    let mut seconds = Times::new();
    for text in texts {
        let plain = COLOURS.replace_all(text, "");
        for line in plain.lines() {
            let Some(found) = PASSED.captures(line) else {
                continue;
            };
            if let Ok(time) = found[1].parse::<f64>() {
                seconds.insert(found[2].to_owned(), time);
            }
        }
    }
    seconds
}

/// `test-times.toml`'s text for these times.
fn times_toml(seconds: &Times) -> String {
    let mut lines = vec![
        "# How long each parity test takes on CI, in seconds, as the `shards`".to_owned(),
        "# binary deals the tests out to the shards by. A test missing here goes".to_owned(),
        "# to a shard by a hash of its name. Remake it from the logs of a CI run's".to_owned(),
        "# parity shards: `cargo run --manifest-path python-parity/Cargo.toml".to_owned(),
        "# --bin shards -- times shard*.log > python-parity/test-times.toml`.".to_owned(),
        String::new(),
        "[seconds]".to_owned(),
    ];
    for (name, time) in seconds {
        lines.push(format!("\"{name}\" = {time:.1}"));
    }
    lines.join("\n") + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|name| (*name).to_owned()).collect()
    }

    fn times(list: &[(&str, f64)]) -> Times {
        list.iter()
            .map(|(name, time)| ((*name).to_owned(), *time))
            .collect()
    }

    #[test]
    fn every_test_runs_on_exactly_one_shard() {
        let known: Times = (0..40)
            .map(|index| (format!("slow_{index}"), f64::from(index)))
            .collect();
        let mut tests: Vec<String> = known.keys().cloned().collect();
        tests.extend(names(&["new_one", "new_two"]));
        let mut everything: Vec<String> = deal(&tests, &known, 5).concat();
        everything.sort();
        tests.sort();
        assert_eq!(everything, tests);
    }

    #[test]
    fn known_tests_are_dealt_slowest_first_to_the_lightest_shard() {
        let known = times(&[("a", 10.0), ("b", 9.0), ("c", 6.0), ("d", 4.0), ("e", 1.0)]);
        let tests: Vec<String> = known.keys().cloned().collect();
        // a, then b, c on b, d and e on a: fifteen seconds each.
        assert_eq!(
            deal(&tests, &known, 2),
            [names(&["a", "d", "e"]), names(&["b", "c"])]
        );
    }

    #[test]
    fn a_new_test_moves_no_other() {
        let known: Times = (0..30)
            .map(|index| (format!("t{index}"), f64::from(index % 7 + 1)))
            .collect();
        let tests: Vec<String> = known.keys().cloned().collect();
        let before = deal(&tests, &known, 5);
        let mut more = tests.clone();
        more.push("brand_new".to_owned());
        let after = deal(&more, &known, 5);
        let placed = hashed_shard("brand_new", 5);
        for (index, (old, new)) in before.iter().zip(&after).enumerate() {
            let mut expected = old.clone();
            if index == placed {
                expected.push("brand_new".to_owned());
                expected.sort();
            }
            assert_eq!(new, &expected);
        }
    }

    #[test]
    fn a_test_no_longer_listed_is_left_out() {
        let known = times(&[("kept", 1.0), ("gone", 100.0)]);
        assert_eq!(
            deal(&names(&["kept"]), &known, 2),
            [names(&["kept"]), vec![]]
        );
    }

    #[test]
    fn the_dealing_is_the_same_every_run() {
        assert_eq!(hashed_shard("some_parity::a_test", 5), 1);
    }

    #[test]
    fn a_filterset_names_each_test_exactly() {
        assert_eq!(filterset(&names(&["a::b", "c"])), "test(=a::b) | test(=c)");
        assert_eq!(filterset(&[]), "none()");
    }

    #[test]
    fn the_slowest_start_first() {
        let config = priorities(
            &names(&["quick", "slow", "unknown"]),
            &times(&[("quick", 1.0), ("slow", 9.0)]),
        );
        assert_eq!(
            config,
            "[[profile.default.overrides]]\nfilter = 'test(=slow)'\npriority = 100\n\n\
             [[profile.default.overrides]]\nfilter = 'test(=quick)'\npriority = 99\n"
        );
    }

    #[test]
    fn wall_time_runs_the_slowest_first_on_each_thread() {
        assert_eq!(wall_time(&[5.0, 3.0, 3.0, 2.0], 2), 7.0);
        assert_eq!(wall_time(&[], 4), 0.0);
    }

    #[test]
    fn times_are_read_from_nextest_output() {
        let log = "2026-11-07T23:32:28.1124321Z \x1b[32;1m        PASS\x1b[0m [   3.007s] ( 3/57) \
                   \x1b[35;1mmusic21-rs-python-parity::parity\x1b[0m \x1b[36mcapella_parity\x1b[0m\
                   \x1b[36m::\x1b[0m\x1b[34;1mthe_crate_reads_capella\x1b[0m\n\
                   \x20       FAIL [   1.000s] ( 4/57) music21-rs-python-parity::parity broken::test\n"
            .to_owned();
        assert_eq!(
            times_from_logs(&[log]),
            times(&[("capella_parity::the_crate_reads_capella", 3.007)])
        );
        let written = times_toml(&times(&[("b", 2.04), ("a", 1.0)]));
        let last: Vec<&str> = written.lines().rev().take(2).collect();
        assert_eq!(last, ["\"b\" = 2.0", "\"a\" = 1.0"]);
    }

    #[test]
    fn the_listing_gives_every_binary_s_tests() {
        let listing = serde_json::json!({
            "rust-suites": {
                "lib": {"testcases": {"doctest::tests::x": {}}},
                "lib::parity": {"testcases": {"abc::y": {}, "abc::z": {}}},
            }
        });
        assert_eq!(
            listed_tests(&listing),
            names(&["abc::y", "abc::z", "doctest::tests::x"])
        );
    }

    #[test]
    fn the_checked_in_times_are_read() {
        let known =
            load_times(&fs::read_to_string(TIMES).expect("test-times.toml")).expect("times");
        assert!(known.len() > 200);
        assert!(known.values().all(|seconds| *seconds >= 0.0));
    }
}
