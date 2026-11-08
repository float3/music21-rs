"""Deals the parity tests out to CI's shards by how long each one takes.

The tests' times, measured on CI, are kept in `test-times.toml`. The known
tests are dealt slowest first, each to the shard with the least time so far,
so the shards finish together; a test the file does not know yet goes to a
shard by a hash of its name, so adding one moves no other. Within a shard
the slowest start first, as nextest priorities, so that no long test starts
last.

    python3 shards.py run SHARD COUNT --list tests.json --config shard.toml
        prints the nextest filterset of shard SHARD of COUNT, from
        `cargo nextest list --message-format json`, and writes the
        priorities to shard.toml for `--config-file`.
    python3 shards.py times LOG...
        prints a new `test-times.toml` from the logs of nextest runs.
    python3 shards.py estimate COUNT THREADS
        prints how long each shard would take, from the times alone.
"""

import argparse
import heapq
import json
import pathlib
import re
import sys
import tomllib
import zlib

TIMES = pathlib.Path(__file__).with_name("test-times.toml")

# nextest's line for a test that passed, colours taken off:
# `PASS [   3.007s] ( 3/57) music21-rs-python-parity::parity capella_parity::...`
PASSED = re.compile(r"\bPASS\s+\[\s*([0-9.]+)s\]\s+\(\s*\d+/\d+\)\s+\S+\s+(\S+)")
COLOURS = re.compile(r"\x1b\[[0-9;]*m")


def load_times(path=TIMES):
    """Each known test's seconds, by its name."""
    with open(path, "rb") as file:
        return tomllib.load(file)["seconds"]


def listed_tests(listing):
    """The names of the tests in `cargo nextest list --message-format json`."""
    names = set()
    for suite in listing["rust-suites"].values():
        names.update(suite["testcases"])
    return sorted(names)


def hashed_shard(name, count):
    """The shard a test with no time goes to, the same on every run."""
    return zlib.crc32(name.encode()) % count


def slowest_first(names, times):
    """The names, the slowest first, ties by name."""
    return sorted(names, key=lambda name: (-times[name], name))


def deal(tests, times, count):
    """The tests of each of `count` shards: the known ones slowest first,
    each to the shard with the least time so far (the first such), and the
    unknown ones by hash."""
    shards = [[] for _ in range(count)]
    loads = [(0.0, index) for index in range(count)]
    known = slowest_first([name for name in tests if name in times], times)
    for name in known:
        load, index = heapq.heappop(loads)
        shards[index].append(name)
        heapq.heappush(loads, (load + times[name], index))
    for name in tests:
        if name not in times:
            shards[hashed_shard(name, count)].append(name)
    return [sorted(shard) for shard in shards]


def filterset(names):
    """The nextest filterset running exactly these tests."""
    if not names:
        return "none()"
    return " | ".join(f"test(={name})" for name in names)


def priorities(names, times):
    """A nextest config starting the shard's tests slowest first; tests with
    no time start after them."""
    known = slowest_first([name for name in names if name in times], times)
    lines = []
    for rank, name in enumerate(known):
        lines.append("[[profile.default.overrides]]")
        lines.append(f"filter = 'test(={name})'")
        lines.append(f"priority = {max(100 - rank, -100)}")
        lines.append("")
    return "\n".join(lines)


def wall_time(seconds, threads):
    """How long tests of these times take on `threads` threads, the slowest
    started first."""
    finishing = [0.0] * threads
    for length in sorted(seconds, reverse=True):
        heapq.heapreplace(finishing, finishing[0] + length)
    return max(finishing)


def times_from_logs(texts):
    """Each passing test's seconds, from nextest's output."""
    seconds = {}
    for text in texts:
        for line in COLOURS.sub("", text).splitlines():
            found = PASSED.search(line)
            if found:
                seconds[found.group(2)] = float(found.group(1))
    return seconds


def times_toml(seconds):
    """`test-times.toml`'s text for these times."""
    lines = [
        "# How long each parity test takes on CI, in seconds, as `shards.py`",
        "# deals the tests out to the shards by. A test missing here goes to a",
        "# shard by a hash of its name. Remake it from the logs of a CI run's",
        "# parity shards: `python3 shards.py times shard*.log > test-times.toml`.",
        "",
        "[seconds]",
    ]
    lines += [f'"{name}" = {seconds[name]:.1f}' for name in sorted(seconds)]
    return "\n".join(lines) + "\n"


def main(arguments):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    commands = parser.add_subparsers(dest="command", required=True)
    run = commands.add_parser("run")
    run.add_argument("shard", type=int)
    run.add_argument("count", type=int)
    run.add_argument("--list", required=True)
    run.add_argument("--config", required=True)
    times = commands.add_parser("times")
    times.add_argument("logs", nargs="+")
    estimate = commands.add_parser("estimate")
    estimate.add_argument("count", type=int)
    estimate.add_argument("threads", type=int)
    said = parser.parse_args(arguments)

    if said.command == "times":
        texts = [
            pathlib.Path(log).read_text(encoding="utf-8-sig", errors="replace")
            for log in said.logs
        ]
        sys.stdout.write(times_toml(times_from_logs(texts)))
        return
    known = load_times()
    if said.command == "estimate":
        for index, shard in enumerate(deal(sorted(known), known, said.count)):
            seconds = [known[name] for name in shard]
            print(f"{index + 1}: {len(shard)} tests, {sum(seconds) / 60:.1f} min of tests, "
                  f"{wall_time(seconds, said.threads) / 60:.1f} min on {said.threads} threads")
        return
    if not 1 <= said.shard <= said.count:
        parser.error(f"shard {said.shard} is not one of 1 to {said.count}")
    with open(said.list, encoding="utf-8") as file:
        tests = listed_tests(json.load(file))
    names = deal(tests, known, said.count)[said.shard - 1]
    pathlib.Path(said.config).write_text(priorities(names, known), encoding="utf-8")
    print(filterset(names))


if __name__ == "__main__":
    main(sys.argv[1:])
