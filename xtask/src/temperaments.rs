//! `data/temperaments.toml` -> `src/tuningsystem/temperaments_generated.rs`.
//!
//! The odd one out among the pipelines here: there is no `regenerate` half.
//! Every other generated table is read out of a pinned submodule, so a machine
//! can rebuild it; this one comes from the Xenharmonic Wiki, which answers 403
//! to every client that is not a browser. So the TOML is collected by hand
//! every few months and committed, and this module only turns it into Rust and
//! checks that the two still agree.
//!
//! Each entry carries the `revid` of the wiki page it was read from, which is
//! what a later collection diffs against to see what actually changed. That is
//! the same job `music21_commit` does for the submodule-derived fixtures.

use std::error::Error;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Everything `data/temperaments.toml` holds.
#[derive(Debug, Deserialize)]
pub struct Temperaments {
    /// The day the wiki was last read, as a plain date.
    pub collected: String,
    /// The wiki this was collected from.
    pub source: String,
    /// The temperaments the crate can build.
    #[serde(default, rename = "temperament")]
    pub temperaments: Vec<NamedTemperament>,
    /// The pages deliberately left alone, and why.
    #[serde(default, rename = "excluded")]
    pub excluded: Vec<Excluded>,
}

/// One regular temperament, as its infobox gives it.
#[derive(Debug, Deserialize)]
pub struct NamedTemperament {
    /// The Rust constant's name.
    pub name: String,
    /// The wiki page it was read from.
    pub page: String,
    /// The revision of that page, so a later collection can diff.
    pub revision: u64,
    /// The primes it is written over, lowest first; the first is the equave.
    pub subgroup: Vec<i32>,
    /// How many periods there are to an equave.
    pub periods_per_equave: u32,
    /// One row per generator, each saying how many of it every prime after the
    /// equave is worth.
    pub generator_rows: Vec<Vec<i32>>,
    /// Each generator written as the ratio it approximates.
    pub generator_ratios: Vec<String>,
    /// Each generator's width in cents, under `optimization`.
    pub generator_cents: Vec<f64>,
    /// Which optimum those widths are — `CWE`, `CTE`, and so on.
    pub optimization: String,
    /// The infobox's own `Title`, where a page names more than one temperament.
    #[serde(default)]
    pub titles: Option<String>,
    /// The commas it tempers out, as the wiki lists them for this subgroup.
    #[serde(default)]
    pub commas: Vec<String>,
    /// The moment-of-symmetry scales the wiki lists for it, if any.
    #[serde(default)]
    pub moments: Vec<String>,
}

/// A page carrying an infobox that the crate does not model.
#[derive(Debug, Deserialize)]
pub struct Excluded {
    /// The wiki page.
    pub page: String,
    /// The revision it was judged at.
    pub revision: u64,
    /// Why it is not here.
    pub reason: String,
}

/// Where the hand-collected TOML lives.
pub fn data_path(workspace_root: &Path) -> PathBuf {
    workspace_root.join("data/temperaments.toml")
}

/// Where the Rust emitted from it lives.
pub fn generated_path(workspace_root: &Path) -> PathBuf {
    workspace_root.join("src/tuningsystem/temperaments_generated.rs")
}

/// Reads the collected TOML, refusing anything the emitter could not render honestly.
pub fn read(path: &Path) -> Result<Temperaments, Box<dyn Error>> {
    let text = fs::read_to_string(path)
        .map_err(|err| format!("could not read {}: {err}", path.display()))?;
    let data: Temperaments =
        toml::from_str(&text).map_err(|err| format!("{} does not parse: {err}", path.display()))?;

    // The names become Rust constants, so a duplicate would not compile; say
    // so here instead, where the message can name the file.
    let mut names: Vec<&str> = data.temperaments.iter().map(|t| t.name.as_str()).collect();
    names.sort_unstable();
    if let Some(pair) = names.windows(2).find(|pair| pair[0] == pair[1]) {
        return Err(format!("{} names two temperaments {}", path.display(), pair[0]).into());
    }
    for entry in &data.temperaments {
        if entry.generator_rows.len() != entry.generator_cents.len()
            || entry.generator_rows.len() != entry.generator_ratios.len()
        {
            return Err(format!(
                "{}: {} mapping rows, {} tunings and {} ratios do not go together",
                entry.name,
                entry.generator_rows.len(),
                entry.generator_cents.len(),
                entry.generator_ratios.len()
            )
            .into());
        }
        for row in &entry.generator_rows {
            if row.len() + 1 != entry.subgroup.len() {
                return Err(format!(
                    "{}: a mapping row of {} steps does not map a subgroup of {} primes",
                    entry.name,
                    row.len(),
                    entry.subgroup.len()
                )
                .into());
            }
        }
    }
    Ok(data)
}

/// Renders the Rust, before rustfmt sees it.
pub fn render(data: &Temperaments) -> String {
    let strings = |values: &[String]| {
        values
            .iter()
            .map(|value| format!("{value:?}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let numbers = |values: &[i32]| {
        values
            .iter()
            .map(i32::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    };
    let rows = |values: &[Vec<i32>]| {
        values
            .iter()
            .map(|row| format!("&[{}]", numbers(row)))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let floats = |values: &[f64]| {
        values
            .iter()
            .map(|value| format!("{value:?}"))
            .collect::<Vec<_>>()
            .join(", ")
    };

    let mut out = String::new();
    out.push_str(
        "//! Regular temperaments, generated from `data/temperaments.toml`.\n\
         //!\n\
         //! Do not edit by hand: run `cargo run -p xtask -- emit-temperaments`.\n\
         //! The TOML itself is collected from the Xenharmonic Wiki by hand, which\n\
         //! is why there is no `regenerate` command to run instead.\n\n\
         use crate::defaults::{FloatType, IntegerType, UnsignedIntegerType};\n\n",
    );
    let _ = write!(
        out,
        "/// The day the wiki was last read for this table.\n\
         pub const COLLECTED: &str = {:?};\n\n\
         /// The wiki this table was collected from.\n\
         pub const SOURCE: &str = {:?};\n\n",
        data.collected, data.source
    );
    out.push_str(
        "/// A regular temperament the Xenharmonic Wiki names and publishes a mapping for.\n\
         ///\n\
         /// This is the published *description*; [`NamedTemperament::temperament`]\n\
         /// turns it into a [`crate::tuningsystem::Temperament`] that can answer\n\
         /// questions.\n\
         ///\n\
         /// No `Eq` or `Hash`: a generator is a width in cents, and a float is not\n\
         /// a thing to compare exactly or to key a map by.\n\
         #[derive(Clone, Copy, Debug, PartialEq)]\n\
         #[must_use]\n\
         pub struct NamedTemperament {\n    \
             /// The name it goes by.\n    \
             pub name: &'static str,\n    \
             /// The wiki page it was read from.\n    \
             pub page: &'static str,\n    \
             /// The revision of that page, so a later collection can diff.\n    \
             pub revision: u64,\n    \
             /// The primes it is written over; the first is the equave.\n    \
             pub subgroup: &'static [IntegerType],\n    \
             /// How many periods there are to an equave.\n    \
             pub periods_per_equave: UnsignedIntegerType,\n    \
             /// One row per generator, over the primes after the equave.\n    \
             pub generator_rows: &'static [&'static [IntegerType]],\n    \
             /// Each generator written as the ratio it approximates.\n    \
             pub generator_ratios: &'static [&'static str],\n    \
             /// Each generator's width in cents, under `optimization`.\n    \
             pub generator_cents: &'static [FloatType],\n    \
             /// Which optimum those widths are.\n    \
             pub optimization: &'static str,\n    \
             /// The infobox's own `Title`, empty unless the page names more\n    \
             /// than one temperament.\n    \
             ///\n    \
             /// The constant is named after the *page*, not after this: which\n    \
             /// of several names goes with which of the subgroups a page lists\n    \
             /// is not something the infobox states, so it is recorded here\n    \
             /// rather than guessed at.\n    \
             pub titles: &'static str,\n    \
             /// The commas it tempers out, as the wiki lists them.\n    \
             pub commas: &'static [&'static str],\n    \
             /// The moment-of-symmetry scales the wiki lists for it, if any.\n    \
             ///\n    \
             /// Empty above rank 2: a moment of symmetry comes of stacking one\n    \
             /// generator, and a rank-3 temperament has two.\n    \
             pub moments: &'static [&'static str],\n\
         }\n\n\
         impl NamedTemperament {\n    \
             /// How many rows the mapping has: one for the period, one per generator.\n    \
             #[must_use]\n    \
             pub const fn rank(&self) -> usize {\n        \
                 1 + self.generator_rows.len()\n    \
             }\n\n    \
             /// The prime it repeats at — 2 for an octave, 3 for a tritave.\n    \
             #[must_use]\n    \
             pub const fn equave(&self) -> IntegerType {\n        \
                 self.subgroup[0]\n    \
             }\n\
         }\n\n",
    );
    let _ = write!(
        out,
        "/// Every regular temperament collected from the wiki, by name.\n\
         ///\n\
         /// A `static` rather than a `const`: at this size a const would be\n\
         /// copied into every place that reads it.\n\
         pub static WIKI_TEMPERAMENTS: [NamedTemperament; {}] = [\n",
        data.temperaments.len()
    );
    for entry in &data.temperaments {
        // Named, not spelled out: the constant it refers to is written below.
        let _ = writeln!(out, "    {},", entry.name);
    }
    out.push_str("];\n\n");

    out.push_str(
        "/// A wiki page carrying a temperament infobox that this crate does not model.\n\
         ///\n\
         /// Listed rather than dropped, so a later collection can tell a page it has\n\
         /// never handled from one deliberately left alone.\n\
         #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]\n\
         #[must_use]\n\
         pub struct UnmodelledTemperament {\n    \
             /// The wiki page.\n    \
             pub page: &'static str,\n    \
             /// The revision it was judged at.\n    \
             pub revision: u64,\n    \
             /// Why the crate does not carry it.\n    \
             pub reason: &'static str,\n\
         }\n\n",
    );
    let _ = write!(
        out,
        "/// The wiki's temperaments this crate does not model, and why.\n\
         pub const UNMODELLED_TEMPERAMENTS: [UnmodelledTemperament; {}] = [\n",
        data.excluded.len()
    );
    for entry in &data.excluded {
        let _ = writeln!(
            out,
            "    UnmodelledTemperament {{ page: {:?}, revision: {}, reason: {:?} }},",
            entry.page, entry.revision, entry.reason
        );
    }
    out.push_str("];\n\n");

    for entry in &data.temperaments {
        let _ = write!(
            out,
            "/// {} ({}), rank {}, generator {} at {} cents.\n\
             pub const {}: NamedTemperament = NamedTemperament {{\n    \
                 name: {:?},\n    \
                 page: {:?},\n    \
                 revision: {},\n    \
                 subgroup: &[{}],\n    \
                 periods_per_equave: {},\n    \
                 generator_rows: &[{}],\n    \
                 generator_ratios: &[{}],\n    \
                 generator_cents: &[{}],\n    \
                 optimization: {:?},\n    \
                 titles: {:?},
    \
                 commas: &[{}],\n    \
                 moments: &[{}],\n\
             }};\n\n",
            entry.page,
            entry
                .subgroup
                .iter()
                .map(i32::to_string)
                .collect::<Vec<_>>()
                .join("."),
            1 + entry.generator_rows.len(),
            entry.generator_ratios.join(", "),
            floats(&entry.generator_cents),
            entry.name,
            entry.name,
            entry.page,
            entry.revision,
            numbers(&entry.subgroup),
            entry.periods_per_equave,
            rows(&entry.generator_rows),
            strings(&entry.generator_ratios),
            floats(&entry.generator_cents),
            entry.optimization,
            entry.titles.as_deref().unwrap_or_default(),
            strings(&entry.commas),
            strings(&entry.moments),
        );
    }
    out
}
