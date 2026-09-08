//! Regular temperaments, generated from `data/temperaments.toml`.
//!
//! Do not edit by hand: run `cargo run -p xtask -- emit-temperaments`.
//! The TOML itself is collected from the Xenharmonic Wiki by hand, which
//! is why there is no `regenerate` command to run instead.

use crate::defaults::{FloatType, IntegerType, UnsignedIntegerType};

/// The day the wiki was last read for this table.
pub const COLLECTED: &str = "2026-09-08";

/// The wiki this table was collected from.
pub const SOURCE: &str = "https://en.xen.wiki";

/// A regular temperament the Xenharmonic Wiki names and publishes a mapping for.
///
/// This is the published *description*; [`NamedTemperament::temperament`]
/// turns it into a [`crate::tuningsystem::Temperament`] that can answer
/// questions.
///
/// No `Eq` or `Hash`: a generator is a width in cents, and a float is not
/// a thing to compare exactly or to key a map by.
#[derive(Clone, Copy, Debug, PartialEq)]
#[must_use]
pub struct NamedTemperament {
    /// The name it goes by.
    pub name: &'static str,
    /// The wiki page it was read from.
    pub page: &'static str,
    /// The revision of that page, so a later collection can diff.
    pub revision: u64,
    /// The primes it is written over; the first is the equave.
    pub subgroup: &'static [IntegerType],
    /// How many periods there are to an equave.
    pub periods_per_equave: UnsignedIntegerType,
    /// One row per generator, over the primes after the equave.
    pub generator_rows: &'static [&'static [IntegerType]],
    /// Each generator written as the ratio it approximates.
    pub generator_ratios: &'static [&'static str],
    /// Each generator's width in cents, under `optimization`.
    pub generator_cents: &'static [FloatType],
    /// Which optimum those widths are.
    pub optimization: &'static str,
    /// The infobox's own `Title`, empty unless the page names more
    /// than one temperament.
    ///
    /// The constant is named after the *page*, not after this: which
    /// of several names goes with which of the subgroups a page lists
    /// is not something the infobox states, so it is recorded here
    /// rather than guessed at.
    pub titles: &'static str,
    /// The commas it tempers out, as the wiki lists them.
    pub commas: &'static [&'static str],
    /// The moment-of-symmetry scales the wiki lists for it, if any.
    ///
    /// Empty above rank 2: a moment of symmetry comes of stacking one
    /// generator, and a rank-3 temperament has two.
    pub moments: &'static [&'static str],
}

impl NamedTemperament {
    /// How many rows the mapping has: one for the period, one per generator.
    #[must_use]
    pub const fn rank(&self) -> usize {
        1 + self.generator_rows.len()
    }

    /// The prime it repeats at — 2 for an octave, 3 for a tritave.
    #[must_use]
    pub const fn equave(&self) -> IntegerType {
        self.subgroup[0]
    }
}

/// Every regular temperament collected from the wiki, by name.
///
/// A `static` rather than a `const`: at this size a const would be
/// copied into every place that reads it.
pub static WIKI_TEMPERAMENTS: [NamedTemperament; 95] = [
    ABERSCHISMIC,
    AMITY,
    ANTONIAN,
    ARCHYTAS,
    AUGMENTED,
    BLACKWOOD,
    BOHPIER,
    BPS,
    BUG,
    BUNYA,
    BUZZARD,
    CANOPUS,
    CATAKLEISMIC,
    COMPTON,
    COTONEUM,
    DECIMAL,
    DEEPTONE,
    DIASCHISMIC,
    DICOT,
    DIDACUS,
    DIMINISHED,
    DOMINANT,
    ENNEALIMMAL,
    FATHER,
    FLATTONE,
    GAMELISMIC,
    GARIBALDI,
    GRAVITY,
    GUNN,
    HARRY,
    HEMIFIFTHS,
    JOVE,
    KEEMUN,
    KLEISMIC,
    LAKA,
    LEAPDAY,
    LEMBA,
    LUNA,
    MAGIC,
    MARVEL,
    MAVILA,
    MEANTONE,
    MINTAKA,
    MIRACLE,
    MISTY,
    MODUS,
    MOHAJIRA,
    MONKEY,
    MOTHRA,
    MUGGLES,
    MYNA,
    MYSTERY,
    NEGRI,
    NEUTROMINANT,
    OCTOID,
    OOLONG,
    OPOSSUM,
    ORWELL,
    PAJARA,
    PARAKLEISMIC,
    PARAPYTH,
    PELE,
    PENTADACUS,
    PONTIAC,
    PORCUPINE,
    QUARTKEENLIG,
    QUASISUPER,
    RODAN,
    SCHISMIC,
    SEMAPHORE,
    SENSAMAGIC,
    SENSI,
    SEPTISCHISMIC,
    SHALLOWTONE,
    SIRIUS,
    SLENDRIC,
    SQUARES,
    STARLING,
    SUBMERGED,
    SUPERKLEISMIC,
    SUPERPYTH,
    TETRACOT,
    TRISECTED,
    TRISMEGISTUS,
    TRITIKLEISMIC,
    ULTRAPYTH,
    UNIDEC,
    VALENTINE,
    VENGEANCE,
    WHITEWOOD,
    WIZARD,
    WOLLEMIA,
    WURSCHMIDT,
    XENIAL,
    ZEUS,
];

/// A wiki page carrying a temperament infobox that this crate does not model.
///
/// Listed rather than dropped, so a later collection can tell a page it has
/// never handled from one deliberately left alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[must_use]
pub struct UnmodelledTemperament {
    /// The wiki page.
    pub page: &'static str,
    /// The revision it was judged at.
    pub revision: u64,
    /// Why the crate does not carry it.
    pub reason: &'static str,
}

/// The wiki's temperaments this crate does not model, and why.
pub const UNMODELLED_TEMPERAMENTS: [UnmodelledTemperament; 0] = [];

/// Aberschismic (2.3.5.7), rank 3, generator 3/2, 5/4 at 702.8, 386.5 cents.
pub const ABERSCHISMIC: NamedTemperament = NamedTemperament {
    name: "ABERSCHISMIC",
    page: "Aberschismic",
    revision: 236716,
    subgroup: &[2, 3, 5, 7],
    periods_per_equave: 1,
    generator_rows: &[&[1, 0, -6], &[0, 1, 1]],
    generator_ratios: &["3/2", "5/4"],
    generator_cents: &[702.8, 386.5],
    optimization: "CWE",
    titles: "",
    commas: &["5120/5103"],
    moments: &[],
};

/// Amity (2.3.5.7), rank 2, generator 243/200 at 339.4 cents.
pub const AMITY: NamedTemperament = NamedTemperament {
    name: "AMITY",
    page: "Amity",
    revision: 231767,
    subgroup: &[2, 3, 5, 7],
    periods_per_equave: 1,
    generator_rows: &[&[-5, -13, 17]],
    generator_ratios: &["243/200"],
    generator_cents: &[339.4],
    optimization: "CWE",
    titles: "",
    commas: &["4375/4374", "5120/5103"],
    moments: &["7L 4s", "7L 11s", "7L 18s", "7L 25s"],
};

/// Antonian (2.3.5.7), rank 2, generator 3/2 at 743.086 cents.
pub const ANTONIAN: NamedTemperament = NamedTemperament {
    name: "ANTONIAN",
    page: "Antonian",
    revision: 225047,
    subgroup: &[2, 3, 5, 7],
    periods_per_equave: 1,
    generator_rows: &[&[1, 2, 3]],
    generator_ratios: &["3/2"],
    generator_cents: &[743.086],
    optimization: "CWE",
    titles: "",
    commas: &["10/9", "10/9", "15/14"],
    moments: &["1L 1s", "2L 1s", "3L 2s"],
};

/// Archytas and ares (2.3.5.7.11), rank 3, generator 3/2, 5/4 at 709.7, 390.0 cents.
pub const ARCHYTAS: NamedTemperament = NamedTemperament {
    name: "ARCHYTAS",
    page: "Archytas and ares",
    revision: 226335,
    subgroup: &[2, 3, 5, 7, 11],
    periods_per_equave: 1,
    generator_rows: &[&[1, 0, -2, -2], &[0, 1, 0, 2]],
    generator_ratios: &["3/2", "5/4"],
    generator_cents: &[709.7, 390.0],
    optimization: "CWE",
    titles: "Archytas; ares",
    commas: &["64/63", "100/99"],
    moments: &[],
};

/// Augmented (temperament) (2.3.5.7.11), rank 2, generator 3/2 at 711.6 cents.
pub const AUGMENTED: NamedTemperament = NamedTemperament {
    name: "AUGMENTED",
    page: "Augmented (temperament)",
    revision: 235640,
    subgroup: &[2, 3, 5, 7, 11],
    periods_per_equave: 3,
    generator_rows: &[&[1, 0, -2, -2]],
    generator_ratios: &["3/2"],
    generator_cents: &[711.6],
    optimization: "CWE",
    titles: "",
    commas: &["56/55", "64/63", "100/99"],
    moments: &["3L 3s", "3L 6s", "3L 9s", "12L 3s"],
};

/// Blackwood (2.3.5.7), rank 2, generator 5/4 at 391.1 cents.
pub const BLACKWOOD: NamedTemperament = NamedTemperament {
    name: "BLACKWOOD",
    page: "Blackwood",
    revision: 232912,
    subgroup: &[2, 3, 5, 7],
    periods_per_equave: 5,
    generator_rows: &[&[0, 1, 0]],
    generator_ratios: &["5/4"],
    generator_cents: &[391.1],
    optimization: "CWE",
    titles: "",
    commas: &["28/27", "49/48"],
    moments: &["5L 5s", "10L 5s"],
};

/// Bohpier (2.3.5.7.11.13), rank 2, generator 12/11 at 146.5 cents.
pub const BOHPIER: NamedTemperament = NamedTemperament {
    name: "BOHPIER",
    page: "Bohpier",
    revision: 234402,
    subgroup: &[2, 3, 5, 7, 11, 13],
    periods_per_equave: 1,
    generator_rows: &[&[13, 19, 23, 12, 14]],
    generator_ratios: &["12/11"],
    generator_cents: &[146.5],
    optimization: "CWE",
    titles: "",
    commas: &["100/99", "144/143", "196/195", "275/273"],
    moments: &["1L 7s", "8L 1s", "8L 9s", "8L 17s"],
};

/// BPS (3.5.7), rank 2, generator 9/7 at 440.7 cents.
pub const BPS: NamedTemperament = NamedTemperament {
    name: "BPS",
    page: "BPS",
    revision: 236047,
    subgroup: &[3, 5, 7],
    periods_per_equave: 1,
    generator_rows: &[&[2, -1]],
    generator_ratios: &["9/7"],
    generator_cents: &[440.7],
    optimization: "CWE",
    titles: "",
    commas: &["245/243"],
    moments: &[],
};

/// Bug and beep (2.3.5.7), rank 2, generator 5/3 at 938.0 cents.
pub const BUG: NamedTemperament = NamedTemperament {
    name: "BUG",
    page: "Bug and beep",
    revision: 230456,
    subgroup: &[2, 3, 5, 7],
    periods_per_equave: 1,
    generator_rows: &[&[2, 3, 1]],
    generator_ratios: &["5/3"],
    generator_cents: &[938.0],
    optimization: "CWE",
    titles: "Bug / Beep",
    commas: &["21/20", "27/25"],
    moments: &["1L 3s", "4L 1s", "5L 4s"],
};

/// Bunya (2.3.5.7.11.13), rank 2, generator 10/9 at 175.9 cents.
pub const BUNYA: NamedTemperament = NamedTemperament {
    name: "BUNYA",
    page: "Bunya",
    revision: 232103,
    subgroup: &[2, 3, 5, 7, 11, 13],
    periods_per_equave: 1,
    generator_rows: &[&[4, 9, 26, 10, -2]],
    generator_ratios: &["10/9"],
    generator_cents: &[175.9],
    optimization: "CWE",
    titles: "",
    commas: &[
        "100/99", "225/224", "243/242", "100/99", "144/143", "225/224", "243/242",
    ],
    moments: &["6L 1s", "7L 6s", "7L 13s", "7L 20s"],
};

/// Buzzard (2.3.5.7.11.13), rank 2, generator 21/16 at 475.7 cents.
pub const BUZZARD: NamedTemperament = NamedTemperament {
    name: "BUZZARD",
    page: "Buzzard",
    revision: 233828,
    subgroup: &[2, 3, 5, 7, 11, 13],
    periods_per_equave: 1,
    generator_rows: &[&[4, 21, -3, 39, 27]],
    generator_ratios: &["21/16"],
    generator_cents: &[475.7],
    optimization: "CWE",
    titles: "",
    commas: &["176/175", "351/350", "540/539", "676/675"],
    moments: &["3L 2s"],
};

/// Canopus (3.5.7), rank 2, generator 7/5 at 583.986 cents.
pub const CANOPUS: NamedTemperament = NamedTemperament {
    name: "CANOPUS",
    page: "Canopus",
    revision: 231671,
    subgroup: &[3, 5, 7],
    periods_per_equave: 1,
    generator_rows: &[&[-5, -4]],
    generator_ratios: &["7/5"],
    generator_cents: &[583.986],
    optimization: "CWE",
    titles: "",
    commas: &["16875/16807"],
    moments: &[],
};

/// Catakleismic (2.3.5.7.11.13), rank 2, generator 6/5 at 316.7 cents.
pub const CATAKLEISMIC: NamedTemperament = NamedTemperament {
    name: "CATAKLEISMIC",
    page: "Catakleismic",
    revision: 235730,
    subgroup: &[2, 3, 5, 7, 11, 13],
    periods_per_equave: 1,
    generator_rows: &[&[6, 5, 22, -21, 14]],
    generator_ratios: &["6/5"],
    generator_cents: &[316.7],
    optimization: "CWE",
    titles: "",
    commas: &["169/168", "225/224", "325/324", "385/384"],
    moments: &["4L 7s", "4L 11s", "15L 4s", "15L 19s"],
};

/// Compton (2.3.5.7), rank 2, generator 5/4 at 384.1 cents.
pub const COMPTON: NamedTemperament = NamedTemperament {
    name: "COMPTON",
    page: "Compton",
    revision: 225555,
    subgroup: &[2, 3, 5, 7],
    periods_per_equave: 12,
    generator_rows: &[&[0, 1, 2]],
    generator_ratios: &["5/4"],
    generator_cents: &[384.1],
    optimization: "CWE",
    titles: "",
    commas: &["225/224", "250047/250000"],
    moments: &["12L 12s", "12L 24s"],
};

/// Cotoneum (2.3.5.7.11.13.17.19), rank 2, generator 3/2 at 702.31 cents.
pub const COTONEUM: NamedTemperament = NamedTemperament {
    name: "COTONEUM",
    page: "Cotoneum",
    revision: 235863,
    subgroup: &[2, 3, 5, 7, 11, 13, 17, 19],
    periods_per_equave: 1,
    generator_rows: &[&[1, -49, -14, 23, 61, 89, -44]],
    generator_ratios: &["3/2"],
    generator_cents: &[702.31],
    optimization: "CWE",
    titles: "",
    commas: &[
        "343/342",
        "364/363",
        "441/440",
        "595/594",
        "1216/1215",
        "1729/1728",
    ],
    moments: &["12L 17s", "12L 29s", "41L 12s", "41L 53s"],
};

/// Decimal (2.3.5.7), rank 2, generator 7/4 at 951.0 cents.
pub const DECIMAL: NamedTemperament = NamedTemperament {
    name: "DECIMAL",
    page: "Decimal",
    revision: 223634,
    subgroup: &[2, 3, 5, 7],
    periods_per_equave: 2,
    generator_rows: &[&[2, 1, 1]],
    generator_ratios: &["7/4"],
    generator_cents: &[951.0],
    optimization: "CWE",
    titles: "",
    commas: &["25/24", "49/48"],
    moments: &["4L 2s", "4L 6s", "10L 4s"],
};

/// Deeptone (2.3.5.13), rank 2, generator 3/2 at 689.6 cents.
pub const DEEPTONE: NamedTemperament = NamedTemperament {
    name: "DEEPTONE",
    page: "Deeptone",
    revision: 223673,
    subgroup: &[2, 3, 5, 13],
    periods_per_equave: 1,
    generator_rows: &[&[1, 11, -4]],
    generator_ratios: &["3/2"],
    generator_cents: &[689.6],
    optimization: "CWE",
    titles: "",
    commas: &["1053/1024", "2187/2080"],
    moments: &["5L 2s", "7L 5s", "7L 12s"],
};

/// Diaschismic (2.3.5.7.11.13.17), rank 2, generator 3/2 at 703.9 cents.
pub const DIASCHISMIC: NamedTemperament = NamedTemperament {
    name: "DIASCHISMIC",
    page: "Diaschismic",
    revision: 232513,
    subgroup: &[2, 3, 5, 7, 11, 13, 17],
    periods_per_equave: 2,
    generator_rows: &[&[1, -2, -8, -12, -15, 1]],
    generator_ratios: &["3/2"],
    generator_cents: &[703.9],
    optimization: "CWE",
    titles: "",
    commas: &["126/125", "136/135", "176/175", "196/195", "256/255"],
    moments: &["2L 8s", "10L 2s", "12L 10s"],
};

/// Dicot (2.3.5.11), rank 2, generator 6/5 at 351.1 cents.
pub const DICOT: NamedTemperament = NamedTemperament {
    name: "DICOT",
    page: "Dicot",
    revision: 232876,
    subgroup: &[2, 3, 5, 11],
    periods_per_equave: 1,
    generator_rows: &[&[2, 1, 5]],
    generator_ratios: &["6/5"],
    generator_cents: &[351.1],
    optimization: "CWE",
    titles: "",
    commas: &["25/24", "45/44"],
    moments: &["3L 1s", "3L 4s", "7L 3s"],
};

/// Didacus (2.5.7.11), rank 2, generator 28/25 at 194.4 cents.
pub const DIDACUS: NamedTemperament = NamedTemperament {
    name: "DIDACUS",
    page: "Didacus",
    revision: 223688,
    subgroup: &[2, 5, 7, 11],
    periods_per_equave: 1,
    generator_rows: &[&[2, 5, 9]],
    generator_ratios: &["28/25"],
    generator_cents: &[194.4],
    optimization: "CWE",
    titles: "",
    commas: &["176/175", "1375/1372"],
    moments: &["1L 5s", "6L 1s", "6L 7s", "6L 13s", "6L 19s"],
};

/// Diminished (temperament) (2.3.5.7), rank 2, generator 3/2 at 696.0 cents.
pub const DIMINISHED: NamedTemperament = NamedTemperament {
    name: "DIMINISHED",
    page: "Diminished (temperament)",
    revision: 226097,
    subgroup: &[2, 3, 5, 7],
    periods_per_equave: 4,
    generator_rows: &[&[1, 1, 1]],
    generator_ratios: &["3/2"],
    generator_cents: &[696.0],
    optimization: "CWE",
    titles: "",
    commas: &["36/35", "50/49"],
    moments: &["4L 4s", "4L 8s", "12L 4s"],
};

/// Dominant (temperament) (2.3.5.7), rank 2, generator 3/2 at 701.1 cents.
pub const DOMINANT: NamedTemperament = NamedTemperament {
    name: "DOMINANT",
    page: "Dominant (temperament)",
    revision: 223637,
    subgroup: &[2, 3, 5, 7],
    periods_per_equave: 1,
    generator_rows: &[&[1, 4, -2]],
    generator_ratios: &["3/2"],
    generator_cents: &[701.1],
    optimization: "CWE",
    titles: "",
    commas: &["36/35", "64/63"],
    moments: &["2L 3s", "5L 2s", "5L 7s"],
};

/// Ennealimmal (2.3.5.7), rank 2, generator 5/3 at 884.322 cents.
pub const ENNEALIMMAL: NamedTemperament = NamedTemperament {
    name: "ENNEALIMMAL",
    page: "Ennealimmal",
    revision: 231237,
    subgroup: &[2, 3, 5, 7],
    periods_per_equave: 9,
    generator_rows: &[&[2, 3, 2]],
    generator_ratios: &["5/3"],
    generator_cents: &[884.322],
    optimization: "CWE",
    titles: "",
    commas: &["2401/2400", "4375/4374"],
    moments: &["18L 9s", "27L 18s", "27L 45s"],
};

/// Father (2.3.5.7), rank 2, generator 3/2 at 738.443 cents.
pub const FATHER: NamedTemperament = NamedTemperament {
    name: "FATHER",
    page: "Father",
    revision: 223994,
    subgroup: &[2, 3, 5, 7],
    periods_per_equave: 1,
    generator_rows: &[&[1, -1, 3]],
    generator_ratios: &["3/2"],
    generator_cents: &[738.443],
    optimization: "CWE",
    titles: "",
    commas: &["16/15", "16/15", "28/27"],
    moments: &["1L 1s", "2L 1s", "3L 2s"],
};

/// Flattone (2.3.5.7.11.13), rank 2, generator 3/2 at 693.1 cents.
pub const FLATTONE: NamedTemperament = NamedTemperament {
    name: "FLATTONE",
    page: "Flattone",
    revision: 223639,
    subgroup: &[2, 3, 5, 7, 11, 13],
    periods_per_equave: 1,
    generator_rows: &[&[1, 4, -9, 6, -4]],
    generator_ratios: &["3/2"],
    generator_cents: &[693.1],
    optimization: "CWE",
    titles: "",
    commas: &["45/44", "65/64", "78/77", "81/80"],
    moments: &["5L 2s", "7L 5s", "7L 12s"],
};

/// Gamelismic and portent (2.3.5.7.11), rank 3, generator 8/7, 5/4 at 233.8, 385.3 cents.
pub const GAMELISMIC: NamedTemperament = NamedTemperament {
    name: "GAMELISMIC",
    page: "Gamelismic and portent",
    revision: 231995,
    subgroup: &[2, 3, 5, 7, 11],
    periods_per_equave: 1,
    generator_rows: &[&[3, 0, -1, 4], &[0, 1, 0, -1]],
    generator_ratios: &["8/7", "5/4"],
    generator_cents: &[233.8, 385.3],
    optimization: "CWE",
    titles: "Gamelismic; portent",
    commas: &["385/384", "441/440"],
    moments: &[],
};

/// Garibaldi (2.3.5.7.19), rank 2, generator 3/2 at 702.1 cents.
pub const GARIBALDI: NamedTemperament = NamedTemperament {
    name: "GARIBALDI",
    page: "Garibaldi",
    revision: 231769,
    subgroup: &[2, 3, 5, 7, 19],
    periods_per_equave: 1,
    generator_rows: &[&[1, -8, -14, -3]],
    generator_ratios: &["3/2"],
    generator_cents: &[702.1],
    optimization: "CWE",
    titles: "",
    commas: &["190/189", "225/224", "361/360"],
    moments: &["5L 2s", "5L 7s", "12L 5s", "12L 17s"],
};

/// Gravity (2.3.5.11), rank 2, generator 27/20 at 516.8 cents.
pub const GRAVITY: NamedTemperament = NamedTemperament {
    name: "GRAVITY",
    page: "Gravity",
    revision: 227055,
    subgroup: &[2, 3, 5, 11],
    periods_per_equave: 1,
    generator_rows: &[&[6, 17, 15]],
    generator_ratios: &["27/20"],
    generator_cents: &[516.8],
    optimization: "CWE",
    titles: "Gravity; Larry",
    commas: &["243/242", "4000/3993"],
    moments: &["2L 5s", "7L 2s", "7L 9s", "7L 51s"],
};

/// Gunn (2.3.5.7.11.13.17.19), rank 3, generator 5/4, 7/4 at 384.84, 965.14 cents.
pub const GUNN: NamedTemperament = NamedTemperament {
    name: "GUNN",
    page: "Gunn",
    revision: 235544,
    subgroup: &[2, 3, 5, 7, 11, 13, 17, 19],
    periods_per_equave: 24,
    generator_rows: &[&[0, 1, 0, 0, 0, 0, 0], &[0, 0, 1, 0, -1, 0, 0]],
    generator_ratios: &["5/4", "7/4"],
    generator_cents: &[384.84, 965.14],
    optimization: "CWE",
    titles: "",
    commas: &["729/728", "273/272", "153/152", "364/363", "1729/1728"],
    moments: &[],
};

/// Harry (2.3.5.7.11.13), rank 2, generator 21/20 at 83.1 cents.
pub const HARRY: NamedTemperament = NamedTemperament {
    name: "HARRY",
    page: "Harry",
    revision: 227469,
    subgroup: &[2, 3, 5, 7, 11, 13],
    periods_per_equave: 2,
    generator_rows: &[&[-6, -17, -10, -15, -26]],
    generator_ratios: &["21/20"],
    generator_cents: &[83.1],
    optimization: "CWE",
    titles: "",
    commas: &["243/242", "351/350", "364/363", "441/440"],
    moments: &["2L 12s", "14L 2s", "14L 16s", "14L 30s"],
};

/// Hemififths (2.3.5.7.11.13), rank 2, generator 49/40 at 351.5 cents.
pub const HEMIFIFTHS: NamedTemperament = NamedTemperament {
    name: "HEMIFIFTHS",
    page: "Hemififths",
    revision: 231760,
    subgroup: &[2, 3, 5, 7, 11, 13],
    periods_per_equave: 1,
    generator_rows: &[&[2, 25, 13, 5, -1]],
    generator_ratios: &["49/40"],
    generator_cents: &[351.5],
    optimization: "CWE",
    titles: "",
    commas: &["144/143", "196/195", "243/242", "364/363"],
    moments: &["3L 4s", "7L 3s", "7L 10s", "17L 7s", "17L 24s"],
};

/// Jove (2.3.5.7.11), rank 3, generator 11/9, 10/7 at 350.5, 617.9 cents.
pub const JOVE: NamedTemperament = NamedTemperament {
    name: "JOVE",
    page: "Jove",
    revision: 231992,
    subgroup: &[2, 3, 5, 7, 11],
    periods_per_equave: 1,
    generator_rows: &[&[2, 1, 1, 5], &[0, 2, 1, 0]],
    generator_ratios: &["11/9", "10/7"],
    generator_cents: &[350.5, 617.9],
    optimization: "CWE",
    titles: "",
    commas: &["243/242", "441/440"],
    moments: &[],
};

/// Keemun (2.3.5.7.11), rank 2, generator 6/5 at 317.6 cents.
pub const KEEMUN: NamedTemperament = NamedTemperament {
    name: "KEEMUN",
    page: "Keemun",
    revision: 227969,
    subgroup: &[2, 3, 5, 7, 11],
    periods_per_equave: 1,
    generator_rows: &[&[6, 5, 3, -2]],
    generator_ratios: &["6/5"],
    generator_cents: &[317.6],
    optimization: "CWE",
    titles: "",
    commas: &["49/48", "126/125", "49/48", "56/55", "100/99"],
    moments: &["4L 3s", "4L 7s", "4L 11s", "15L 4s"],
};

/// Kleismic (2.3.5.13), rank 2, generator 6/5 at 317.1 cents.
pub const KLEISMIC: NamedTemperament = NamedTemperament {
    name: "KLEISMIC",
    page: "Kleismic",
    revision: 226929,
    subgroup: &[2, 3, 5, 13],
    periods_per_equave: 1,
    generator_rows: &[&[6, 5, 14]],
    generator_ratios: &["6/5"],
    generator_cents: &[317.1],
    optimization: "CWE",
    titles: "",
    commas: &["325/324", "625/624"],
    moments: &["3L 1s", "4L 3s", "4L 7s", "4L 11s", "15L 4s"],
};

/// Laka (2.3.5.7.11.13), rank 3, generator 3/2, 5/4 at 702.6, 386.8 cents.
pub const LAKA: NamedTemperament = NamedTemperament {
    name: "LAKA",
    page: "Laka",
    revision: 234653,
    subgroup: &[2, 3, 5, 7, 11, 13],
    periods_per_equave: 1,
    generator_rows: &[&[1, 0, -6, 15, 12], &[0, 1, 1, -1, -1]],
    generator_ratios: &["3/2", "5/4"],
    generator_cents: &[702.6, 386.8],
    optimization: "CWE",
    titles: "",
    commas: &[],
    moments: &[],
};

/// Leapday (2.3.5.7.11.13), rank 2, generator 3/2 at 704.2 cents.
pub const LEAPDAY: NamedTemperament = NamedTemperament {
    name: "LEAPDAY",
    page: "Leapday",
    revision: 235394,
    subgroup: &[2, 3, 5, 7, 11, 13],
    periods_per_equave: 1,
    generator_rows: &[&[1, 21, 15, 11, 8]],
    generator_ratios: &["3/2"],
    generator_cents: &[704.2],
    optimization: "CWE",
    titles: "",
    commas: &["91/90", "121/120", "169/168", "352/351"],
    moments: &["2L 3s", "5L 2s", "5L 7s", "12L 5s"],
};

/// Lemba (2.3.5.7.11.13), rank 2, generator 8/7 at 231.2 cents.
pub const LEMBA: NamedTemperament = NamedTemperament {
    name: "LEMBA",
    page: "Lemba",
    revision: 224710,
    subgroup: &[2, 3, 5, 7, 11, 13],
    periods_per_equave: 2,
    generator_rows: &[&[3, -1, -1, 5, 1]],
    generator_ratios: &["8/7"],
    generator_cents: &[231.2],
    optimization: "CWE",
    titles: "",
    commas: &["45/44", "50/49", "65/64", "78/77"],
    moments: &["4L 2s", "6L 4s", "10L 6s"],
};

/// Luna and hemithirds (2.3.5.7), rank 2, generator 28/25 at 193.2 cents.
pub const LUNA: NamedTemperament = NamedTemperament {
    name: "LUNA",
    page: "Luna and hemithirds",
    revision: 223644,
    subgroup: &[2, 3, 5, 7],
    periods_per_equave: 1,
    generator_rows: &[&[-15, 2, 5]],
    generator_ratios: &["28/25"],
    generator_cents: &[193.2],
    optimization: "CWE",
    titles: "Hemithirds",
    commas: &["1029/1024", "3136/3125"],
    moments: &["1L 5s", "6L 1s", "6L 7s", "6L 13s", "6L 19s", "25L 6s"],
};

/// Magic (2.3.5.7), rank 2, generator 5/4 at 380.5 cents.
pub const MAGIC: NamedTemperament = NamedTemperament {
    name: "MAGIC",
    page: "Magic",
    revision: 231155,
    subgroup: &[2, 3, 5, 7],
    periods_per_equave: 1,
    generator_rows: &[&[5, 1, 12]],
    generator_ratios: &["5/4"],
    generator_cents: &[380.5],
    optimization: "CWE",
    titles: "",
    commas: &["225/224", "245/243"],
    moments: &["3L 4s", "3L 7s", "3L 16s", "19L 3s"],
};

/// Marvel (2.3.5.7.11), rank 3, generator 3/2, 5/4 at 700.6, 383.5 cents.
pub const MARVEL: NamedTemperament = NamedTemperament {
    name: "MARVEL",
    page: "Marvel",
    revision: 231983,
    subgroup: &[2, 3, 5, 7, 11],
    periods_per_equave: 1,
    generator_rows: &[&[1, 0, 2, -1], &[0, 1, 2, -3]],
    generator_ratios: &["3/2", "5/4"],
    generator_cents: &[700.6, 383.5],
    optimization: "CWE",
    titles: "",
    commas: &["225/224", "385/384"],
    moments: &[],
};

/// Mavila (2.3.5.11), rank 2, generator 3/2 at 679.0 cents.
pub const MAVILA: NamedTemperament = NamedTemperament {
    name: "MAVILA",
    page: "Mavila",
    revision: 225302,
    subgroup: &[2, 3, 5, 11],
    periods_per_equave: 1,
    generator_rows: &[&[1, -3, -1]],
    generator_ratios: &["3/2"],
    generator_cents: &[679.0],
    optimization: "CWE",
    titles: "",
    commas: &["135/128", "33/32", "45/44"],
    moments: &["2L 3s", "2L 5s", "7L 2s"],
};

/// Meantone (2.3.5.7), rank 2, generator 3/2 at 696.7 cents.
pub const MEANTONE: NamedTemperament = NamedTemperament {
    name: "MEANTONE",
    page: "Meantone",
    revision: 234286,
    subgroup: &[2, 3, 5, 7],
    periods_per_equave: 1,
    generator_rows: &[&[1, 4, 10]],
    generator_ratios: &["3/2"],
    generator_cents: &[696.7],
    optimization: "CWE",
    titles: "",
    commas: &["81/80", "126/125"],
    moments: &["2L 3s", "5L 2s", "7L 5s", "12L 7s"],
};

/// Mintaka (3.7.11), rank 2, generator 11/7 at 778.7 cents.
pub const MINTAKA: NamedTemperament = NamedTemperament {
    name: "MINTAKA",
    page: "Mintaka",
    revision: 223712,
    subgroup: &[3, 7, 11],
    periods_per_equave: 1,
    generator_rows: &[&[-3, -2]],
    generator_ratios: &["11/7"],
    generator_cents: &[778.7],
    optimization: "CWE",
    titles: "",
    commas: &["1331/1323"],
    moments: &["2L 3s", "5L 2s", "5L 7s", "5L 12s"],
};

/// Miracle (2.3.5.7.11), rank 2, generator 15/14 at 116.7 cents.
pub const MIRACLE: NamedTemperament = NamedTemperament {
    name: "MIRACLE",
    page: "Miracle",
    revision: 233546,
    subgroup: &[2, 3, 5, 7, 11],
    periods_per_equave: 1,
    generator_rows: &[&[6, -7, -2, 15]],
    generator_ratios: &["15/14"],
    generator_cents: &[116.7],
    optimization: "CTE",
    titles: "",
    commas: &["225/224", "243/242", "385/384"],
    moments: &["1L 9s", "10L 1s", "10L 11s", "10L 21s"],
};

/// Misty (2.3.5.7.17.19), rank 2, generator 3/2 at 703.1 cents.
pub const MISTY: NamedTemperament = NamedTemperament {
    name: "MISTY",
    page: "Misty",
    revision: 231776,
    subgroup: &[2, 3, 5, 7, 17, 19],
    periods_per_equave: 3,
    generator_rows: &[&[1, -4, -10, 3, 1]],
    generator_ratios: &["3/2"],
    generator_cents: &[703.1],
    optimization: "CWE",
    titles: "",
    commas: &["256/255", "324/323", "400/399", "476/475"],
    moments: &["3L 9s", "12L 3s", "12L 15s", "12L 27s"],
};

/// Modus (2.3.5.7.11.13), rank 2, generator 10/9 at 176.8 cents.
pub const MODUS: NamedTemperament = NamedTemperament {
    name: "MODUS",
    page: "Modus",
    revision: 234650,
    subgroup: &[2, 3, 5, 7, 11, 13],
    periods_per_equave: 1,
    generator_rows: &[&[4, 9, -8, 10, -2]],
    generator_ratios: &["10/9"],
    generator_cents: &[176.8],
    optimization: "CWE",
    titles: "",
    commas: &[
        "64/63", "100/99", "243/242", "64/63", "78/77", "100/99", "144/143",
    ],
    moments: &["6L 1s", "7L 6s", "7L 13s", "7L 20s"],
};

/// Mohajira (2.3.5.7.11), rank 2, generator 11/9 at 348.5 cents.
pub const MOHAJIRA: NamedTemperament = NamedTemperament {
    name: "MOHAJIRA",
    page: "Mohajira",
    revision: 227468,
    subgroup: &[2, 3, 5, 7, 11],
    periods_per_equave: 1,
    generator_rows: &[&[2, 8, -11, 5]],
    generator_ratios: &["11/9"],
    generator_cents: &[348.5],
    optimization: "CWE",
    titles: "",
    commas: &["81/80", "121/120", "176/175"],
    moments: &["3L 4s", "7L 3s", "7L 10s", "7L 17s"],
};

/// Monkey (2.3.5.7.11.13), rank 2, generator 10/9 at 175.6 cents.
pub const MONKEY: NamedTemperament = NamedTemperament {
    name: "MONKEY",
    page: "Monkey",
    revision: 231770,
    subgroup: &[2, 3, 5, 7, 11, 13],
    periods_per_equave: 1,
    generator_rows: &[&[4, 9, -15, 10, -2]],
    generator_ratios: &["10/9"],
    generator_cents: &[175.6],
    optimization: "CWE",
    titles: "",
    commas: &[
        "100/99", "243/242", "385/384", "100/99", "144/143", "243/242", "385/384",
    ],
    moments: &["6L 1s", "7L 6s", "7L 13s", "7L 20s"],
};

/// Mothra (2.3.5.7), rank 2, generator 8/7 at 232.3 cents.
pub const MOTHRA: NamedTemperament = NamedTemperament {
    name: "MOTHRA",
    page: "Mothra",
    revision: 226452,
    subgroup: &[2, 3, 5, 7],
    periods_per_equave: 1,
    generator_rows: &[&[3, 12, -1]],
    generator_ratios: &["8/7"],
    generator_cents: &[232.3],
    optimization: "CWE",
    titles: "",
    commas: &["81/80", "1029/1024"],
    moments: &["1L 4s", "5L 1s", "5L 6s", "5L 21s"],
};

/// Muggles (2.3.5.7.11.13), rank 2, generator 5/4 at 377.7 cents.
pub const MUGGLES: NamedTemperament = NamedTemperament {
    name: "MUGGLES",
    page: "Muggles",
    revision: 227466,
    subgroup: &[2, 3, 5, 7, 11, 13],
    periods_per_equave: 1,
    generator_rows: &[&[5, 1, -7, 11, -1]],
    generator_ratios: &["5/4"],
    generator_cents: &[377.7],
    optimization: "CWE",
    titles: "",
    commas: &["45/44", "65/64", "78/77", "126/125"],
    moments: &["3L 7s", "3L 10s", "3L 13s", "16L 3s"],
};

/// Myna (2.3.5.7.11), rank 2, generator 6/5 at 310.1 cents.
pub const MYNA: NamedTemperament = NamedTemperament {
    name: "MYNA",
    page: "Myna",
    revision: 230321,
    subgroup: &[2, 3, 5, 7, 11],
    periods_per_equave: 1,
    generator_rows: &[&[10, 9, 7, 25]],
    generator_ratios: &["6/5"],
    generator_cents: &[310.1],
    optimization: "CWE",
    titles: "",
    commas: &["126/125", "176/175", "243/242"],
    moments: &["3L 1s", "4L 3s", "4L 7s", "4L 23s", "27L 4s"],
};

/// Mystery (2.3.5.7.11.13), rank 2, generator 5/4 at 387.9 cents.
pub const MYSTERY: NamedTemperament = NamedTemperament {
    name: "MYSTERY",
    page: "Mystery",
    revision: 231766,
    subgroup: &[2, 3, 5, 7, 11, 13],
    periods_per_equave: 29,
    generator_rows: &[&[0, 1, 1, 1, 1]],
    generator_ratios: &["5/4"],
    generator_cents: &[387.9],
    optimization: "CWE",
    titles: "",
    commas: &["196/195", "352/351", "364/363", "676/675"],
    moments: &["29L 29s", "58L 29s"],
};

/// Negri (2.3.5.7.13), rank 2, generator 16/15 at 125.4 cents.
pub const NEGRI: NamedTemperament = NamedTemperament {
    name: "NEGRI",
    page: "Negri",
    revision: 233544,
    subgroup: &[2, 3, 5, 7, 13],
    periods_per_equave: 1,
    generator_rows: &[&[-4, 3, -2, -3]],
    generator_ratios: &["16/15"],
    generator_cents: &[125.4],
    optimization: "CWE",
    titles: "",
    commas: &["49/48", "65/64", "91/90"],
    moments: &["1L 8s", "9L 1s", "10L 9s"],
};

/// Neutrominant (2.3.5.7.11.13), rank 2, generator 11/9 at 350.7 cents.
pub const NEUTROMINANT: NamedTemperament = NamedTemperament {
    name: "NEUTROMINANT",
    page: "Neutrominant",
    revision: 227457,
    subgroup: &[2, 3, 5, 7, 11, 13],
    periods_per_equave: 1,
    generator_rows: &[&[2, 8, -4, 5, -1]],
    generator_ratios: &["11/9"],
    generator_cents: &[350.7],
    optimization: "CWE",
    titles: "",
    commas: &["36/35", "64/63", "66/65", "121/120"],
    moments: &["3L 4s", "7L 3s", "7L 10s"],
};

/// Octoid (2.3.5.7.11), rank 2, generator 7/5 at 583.948 cents.
pub const OCTOID: NamedTemperament = NamedTemperament {
    name: "OCTOID",
    page: "Octoid",
    revision: 231672,
    subgroup: &[2, 3, 5, 7, 11],
    periods_per_equave: 8,
    generator_rows: &[&[3, 4, 5, 3]],
    generator_ratios: &["7/5"],
    generator_cents: &[583.948],
    optimization: "CWE",
    titles: "",
    commas: &["540/539", "1375/1372", "4000/3993"],
    moments: &["8L 64s", "72L 8s"],
};

/// Oolong (2.3.5.7.13), rank 2, generator 6/5 at 311.7 cents.
pub const OOLONG: NamedTemperament = NamedTemperament {
    name: "OOLONG",
    page: "Oolong",
    revision: 229456,
    subgroup: &[2, 3, 5, 7, 13],
    periods_per_equave: 1,
    generator_rows: &[&[-17, -18, -20, -5]],
    generator_ratios: &["6/5"],
    generator_cents: &[311.7],
    optimization: "CWE",
    titles: "",
    commas: &["126/125", "196/195", "15379/15360"],
    moments: &["4L 3s", "4L 7s", "4L 11s", "4L 15s", "4L 19s", "23L 4s"],
};

/// Opossum (2.3.5.7.11), rank 2, generator 11/10 at 160.5 cents.
pub const OPOSSUM: NamedTemperament = NamedTemperament {
    name: "OPOSSUM",
    page: "Opossum",
    revision: 227460,
    subgroup: &[2, 3, 5, 7, 11],
    periods_per_equave: 1,
    generator_rows: &[&[-3, -5, -9, -4]],
    generator_ratios: &["11/10"],
    generator_cents: &[160.5],
    optimization: "CWE",
    titles: "",
    commas: &["28/27", "55/54", "77/75"],
    moments: &["1L 6s", "7L 1s"],
};

/// Orwell (2.3.5.7.11), rank 2, generator 7/6 at 271.5 cents.
pub const ORWELL: NamedTemperament = NamedTemperament {
    name: "ORWELL",
    page: "Orwell",
    revision: 233021,
    subgroup: &[2, 3, 5, 7, 11],
    periods_per_equave: 1,
    generator_rows: &[&[7, -3, 8, 2]],
    generator_ratios: &["7/6"],
    generator_cents: &[271.5],
    optimization: "CWE",
    titles: "",
    commas: &["99/98", "121/120", "176/175"],
    moments: &["4L 1s", "4L 5s", "9L 4s", "9L 13s"],
};

/// Pajara (2.3.5.7.11.17), rank 2, generator 3/2 at 707.4 cents.
pub const PAJARA: NamedTemperament = NamedTemperament {
    name: "PAJARA",
    page: "Pajara",
    revision: 231604,
    subgroup: &[2, 3, 5, 7, 11, 17],
    periods_per_equave: 2,
    generator_rows: &[&[1, -2, -2, -6, 1]],
    generator_ratios: &["3/2"],
    generator_cents: &[707.4],
    optimization: "CWE",
    titles: "",
    commas: &["50/49", "64/63", "85/84", "99/98"],
    moments: &["2L 8s", "10L 2s", "12L 10s"],
};

/// Parakleismic (2.3.5.7), rank 2, generator 5/3 at 884.81 cents.
pub const PARAKLEISMIC: NamedTemperament = NamedTemperament {
    name: "PARAKLEISMIC",
    page: "Parakleismic",
    revision: 235440,
    subgroup: &[2, 3, 5, 7],
    periods_per_equave: 1,
    generator_rows: &[&[13, 14, 35]],
    generator_ratios: &["5/3"],
    generator_cents: &[884.81],
    optimization: "CWE",
    titles: "",
    commas: &["3136/3125", "4375/4374"],
    moments: &["4L 15s", "19L 4s", "19L 23s"],
};

/// Parapyth (2.3.7.11.13), rank 3, generator 3/2, 7/4 at 703.8, 969.2 cents.
pub const PARAPYTH: NamedTemperament = NamedTemperament {
    name: "PARAPYTH",
    page: "Parapyth",
    revision: 231575,
    subgroup: &[2, 3, 7, 11, 13],
    periods_per_equave: 1,
    generator_rows: &[&[1, 0, -4, -7], &[0, 1, 1, 1]],
    generator_ratios: &["3/2", "7/4"],
    generator_cents: &[703.8, 969.2],
    optimization: "CWE",
    titles: "",
    commas: &["352/351", "364/363"],
    moments: &[],
};

/// Pele (2.3.5.7.11.13), rank 3, generator 3/2, 5/4 at 703.4, 387.8 cents.
pub const PELE: NamedTemperament = NamedTemperament {
    name: "PELE",
    page: "Pele",
    revision: 234652,
    subgroup: &[2, 3, 5, 7, 11, 13],
    periods_per_equave: 1,
    generator_rows: &[&[1, 0, -6, -10, -13], &[0, 1, 1, 1, 1]],
    generator_ratios: &["3/2", "5/4"],
    generator_cents: &[703.4, 387.8],
    optimization: "CWE",
    titles: "",
    commas: &[],
    moments: &[],
};

/// Pentadacus (5.7.11), rank 2, generator 55/49 at 194.8 cents.
pub const PENTADACUS: NamedTemperament = NamedTemperament {
    name: "PENTADACUS",
    page: "Pentadacus",
    revision: 236333,
    subgroup: &[5, 7, 11],
    periods_per_equave: 1,
    generator_rows: &[&[3, 7]],
    generator_ratios: &["55/49"],
    generator_cents: &[194.8],
    optimization: "CWE",
    titles: "",
    commas: &["831875/823543"],
    moments: &[],
};

/// Pontiac (2.3.5.7), rank 2, generator 3/2 at 701.758 cents.
pub const PONTIAC: NamedTemperament = NamedTemperament {
    name: "PONTIAC",
    page: "Pontiac",
    revision: 223671,
    subgroup: &[2, 3, 5, 7],
    periods_per_equave: 1,
    generator_rows: &[&[1, -8, 39]],
    generator_ratios: &["3/2"],
    generator_cents: &[701.758],
    optimization: "CWE",
    titles: "",
    commas: &["4375/4374", "32805/32768"],
    moments: &["12L 17s", "12L 29s", "12L 41s", "53L 12s"],
};

/// Porcupine (2.3.5.7.11), rank 2, generator 10/9 at 163.0 cents.
pub const PORCUPINE: NamedTemperament = NamedTemperament {
    name: "PORCUPINE",
    page: "Porcupine",
    revision: 229003,
    subgroup: &[2, 3, 5, 7, 11],
    periods_per_equave: 1,
    generator_rows: &[&[-3, -5, 6, -4]],
    generator_ratios: &["10/9"],
    generator_cents: &[163.0],
    optimization: "CWE",
    titles: "",
    commas: &["55/54", "64/63", "100/99"],
    moments: &["1L 6s", "7L 1s", "7L 8s"],
};

/// Quartkeenlig (2.3.5.7.11), rank 2, generator 36/35 at 52.845 cents.
pub const QUARTKEENLIG: NamedTemperament = NamedTemperament {
    name: "QUARTKEENLIG",
    page: "Quartkeenlig",
    revision: 235979,
    subgroup: &[2, 3, 5, 7, 11],
    periods_per_equave: 1,
    generator_rows: &[&[36, 30, 41, -35]],
    generator_ratios: &["36/35"],
    generator_cents: &[52.845],
    optimization: "CWE",
    titles: "",
    commas: &["385/384", "6250/6237", "67228/66825"],
    moments: &["22L 1s"],
};

/// Quasisuper (2.3.5.7.11), rank 2, generator 3/2 at 708.3 cents.
pub const QUASISUPER: NamedTemperament = NamedTemperament {
    name: "QUASISUPER",
    page: "Quasisuper",
    revision: 224309,
    subgroup: &[2, 3, 5, 7, 11],
    periods_per_equave: 1,
    generator_rows: &[&[1, -13, -2, -6]],
    generator_ratios: &["3/2"],
    generator_cents: &[708.3],
    optimization: "CWE",
    titles: "Quasisuper; quasisupra",
    commas: &["64/63", "99/98", "121/120"],
    moments: &["5L 2s", "5L 7s", "5L 12s", "17L 5s"],
};

/// Rodan (2.3.5.7.11), rank 2, generator 8/7 at 234.4 cents.
pub const RODAN: NamedTemperament = NamedTemperament {
    name: "RODAN",
    page: "Rodan",
    revision: 231761,
    subgroup: &[2, 3, 5, 7, 11],
    periods_per_equave: 1,
    generator_rows: &[&[3, 17, -1, -13]],
    generator_ratios: &["8/7"],
    generator_cents: &[234.4],
    optimization: "CWE",
    titles: "",
    commas: &["245/243", "385/384", "441/440"],
    moments: &["1L 4s", "5L 1s", "5L 6s", "5L 36s", "41L 5s"],
};

/// Schismic (2.3.5), rank 2, generator 3/2 at 701.731 cents.
pub const SCHISMIC: NamedTemperament = NamedTemperament {
    name: "SCHISMIC",
    page: "Schismic",
    revision: 235694,
    subgroup: &[2, 3, 5],
    periods_per_equave: 1,
    generator_rows: &[&[1, -8]],
    generator_ratios: &["3/2"],
    generator_cents: &[701.731],
    optimization: "CWE",
    titles: "",
    commas: &["32805/32768"],
    moments: &["2L 3s", "5L 2s", "5L 7s", "12L 5s"],
};

/// Semaphore and godzilla (2.3.5.7.13), rank 2, generator 7/4 at 948.0 cents.
pub const SEMAPHORE: NamedTemperament = NamedTemperament {
    name: "SEMAPHORE",
    page: "Semaphore and godzilla",
    revision: 233995,
    subgroup: &[2, 3, 5, 7, 13],
    periods_per_equave: 1,
    generator_rows: &[&[2, 8, 1, 11]],
    generator_ratios: &["7/4"],
    generator_cents: &[948.0],
    optimization: "CWE",
    titles: "",
    commas: &[],
    moments: &["4L 1s", "5L 4s", "5L 9s", "5L 14s"],
};

/// Sensamagic (2.3.5.7.11), rank 3, generator 3/2, 9/7 at 703.8, 440.9 cents.
pub const SENSAMAGIC: NamedTemperament = NamedTemperament {
    name: "SENSAMAGIC",
    page: "Sensamagic",
    revision: 231982,
    subgroup: &[2, 3, 5, 7, 11],
    periods_per_equave: 1,
    generator_rows: &[&[1, 1, 2, -2], &[0, 2, -1, -1]],
    generator_ratios: &["3/2", "9/7"],
    generator_cents: &[703.8, 440.9],
    optimization: "CWE",
    titles: "",
    commas: &["245/243", "385/384"],
    moments: &[],
};

/// Sensi (2.3.5.7.13), rank 2, generator 9/7 at 443.3 cents.
pub const SENSI: NamedTemperament = NamedTemperament {
    name: "SENSI",
    page: "Sensi",
    revision: 229838,
    subgroup: &[2, 3, 5, 7, 13],
    periods_per_equave: 1,
    generator_rows: &[&[7, 9, 13, 10]],
    generator_ratios: &["9/7"],
    generator_cents: &[443.3],
    optimization: "CWE",
    titles: "",
    commas: &["91/90", "126/125", "169/168"],
    moments: &["3L 2s", "3L 5s", "8L 3s", "8L 11s"],
};

/// Septischismic (2.3.5.7.11.13.19), rank 3, generator 3/2, 5/4 at 702.2307, 386.3245 cents.
pub const SEPTISCHISMIC: NamedTemperament = NamedTemperament {
    name: "SEPTISCHISMIC",
    page: "Septischismic",
    revision: 235909,
    subgroup: &[2, 3, 5, 7, 11, 13, 19],
    periods_per_equave: 1,
    generator_rows: &[&[1, 0, -14, 23, 12, 5], &[0, 1, 0, 0, -1, 1]],
    generator_ratios: &["3/2", "5/4"],
    generator_cents: &[702.2307, 386.3245],
    optimization: "CWE",
    titles: "",
    commas: &["1216/1215", "1540/1539", "1729/1728", "2080/2079"],
    moments: &[],
};

/// Shallowtone (2.3.5.7), rank 2, generator 3/2 at 681.2 cents.
pub const SHALLOWTONE: NamedTemperament = NamedTemperament {
    name: "SHALLOWTONE",
    page: "Shallowtone",
    revision: 233303,
    subgroup: &[2, 3, 5, 7],
    periods_per_equave: 1,
    generator_rows: &[&[1, -10, 12]],
    generator_ratios: &["3/2"],
    generator_cents: &[681.2],
    optimization: "CWE",
    titles: "",
    commas: &["295245/262144", "36/35", "295245/262144"],
    moments: &["2L 3s", "2L 5s", "7L 2s"],
};

/// Sirius (3.5.7), rank 2, generator 25/21 at 293.759 cents.
pub const SIRIUS: NamedTemperament = NamedTemperament {
    name: "SIRIUS",
    page: "Sirius",
    revision: 225422,
    subgroup: &[3, 5, 7],
    periods_per_equave: 1,
    generator_rows: &[&[3, 5]],
    generator_ratios: &["25/21"],
    generator_cents: &[293.759],
    optimization: "CWE",
    titles: "",
    commas: &["3125/3087"],
    moments: &[],
};

/// Slendric (2.3.7), rank 2, generator 8/7 at 233.7 cents.
pub const SLENDRIC: NamedTemperament = NamedTemperament {
    name: "SLENDRIC",
    page: "Slendric",
    revision: 223660,
    subgroup: &[2, 3, 7],
    periods_per_equave: 1,
    generator_rows: &[&[3, -1]],
    generator_ratios: &["8/7"],
    generator_cents: &[233.7],
    optimization: "CWE",
    titles: "",
    commas: &["1029/1024"],
    moments: &["1L 4s", "5L 1s", "5L 6s", "5L 11s"],
};

/// Squares (2.3.5.7.11), rank 2, generator 9/7 at 426.0 cents.
pub const SQUARES: NamedTemperament = NamedTemperament {
    name: "SQUARES",
    page: "Squares",
    revision: 223661,
    subgroup: &[2, 3, 5, 7, 11],
    periods_per_equave: 1,
    generator_rows: &[&[-4, -16, -9, -10]],
    generator_ratios: &["9/7"],
    generator_cents: &[426.0],
    optimization: "CWE",
    titles: "Skwares; Squares",
    commas: &["81/80", "99/98", "121/120"],
    moments: &["3L 2s", "3L 5s", "3L 8s", "3L 11s", "14L 3s"],
};

/// Starling and thrush (2.3.5.7.11), rank 3, generator 3/2, 5/4 at 701.6, 390.9 cents.
pub const STARLING: NamedTemperament = NamedTemperament {
    name: "STARLING",
    page: "Starling and thrush",
    revision: 231979,
    subgroup: &[2, 3, 5, 7, 11],
    periods_per_equave: 1,
    generator_rows: &[&[1, 0, -2, -2], &[0, 1, 3, 5]],
    generator_ratios: &["3/2", "5/4"],
    generator_cents: &[701.6, 390.9],
    optimization: "CWE",
    titles: "Starling; thrush",
    commas: &["126/125", "176/175"],
    moments: &[],
};

/// Submerged (2.3.5.7.11.13), rank 2, generator 8/5 at 827.0 cents.
pub const SUBMERGED: NamedTemperament = NamedTemperament {
    name: "SUBMERGED",
    page: "Submerged",
    revision: 235409,
    subgroup: &[2, 3, 5, 7, 11, 13],
    periods_per_equave: 1,
    generator_rows: &[&[11, -1, -9, 5, 1]],
    generator_ratios: &["8/5"],
    generator_cents: &[827.0],
    optimization: "CWE",
    titles: "",
    commas: &["65/64", "105/104", "121/120", "441/440"],
    moments: &["3L 7s", "3L 10s", "13L 3s"],
};

/// Superkleismic (2.3.5.7.11.19), rank 2, generator 5/3 at 878.2 cents.
pub const SUPERKLEISMIC: NamedTemperament = NamedTemperament {
    name: "SUPERKLEISMIC",
    page: "Superkleismic",
    revision: 225040,
    subgroup: &[2, 3, 5, 7, 11, 19],
    periods_per_equave: 1,
    generator_rows: &[&[9, 10, -3, 2, 14]],
    generator_ratios: &["5/3"],
    generator_cents: &[878.2],
    optimization: "CWE",
    titles: "",
    commas: &[],
    moments: &["3L 1s", "4L 3s", "4L 7s", "11L 4s", "15L 11s"],
};

/// Superpyth (2.3.5.7.11), rank 2, generator 3/2 at 710.1 cents.
pub const SUPERPYTH: NamedTemperament = NamedTemperament {
    name: "SUPERPYTH",
    page: "Superpyth",
    revision: 228424,
    subgroup: &[2, 3, 5, 7, 11],
    periods_per_equave: 1,
    generator_rows: &[&[1, 9, -2, 16]],
    generator_ratios: &["3/2"],
    generator_cents: &[710.1],
    optimization: "CWE",
    titles: "Archy; superpyth",
    commas: &["64/63", "100/99", "245/243"],
    moments: &["2L 3s", "5L 2s", "5L 7s", "5L 12s", "5L 17s"],
};

/// Tetracot (2.3.5.11.13), rank 2, generator 10/9 at 176.1 cents.
pub const TETRACOT: NamedTemperament = NamedTemperament {
    name: "TETRACOT",
    page: "Tetracot",
    revision: 232444,
    subgroup: &[2, 3, 5, 11, 13],
    periods_per_equave: 1,
    generator_rows: &[&[4, 9, 10, -2]],
    generator_ratios: &["10/9"],
    generator_cents: &[176.1],
    optimization: "CWE",
    titles: "",
    commas: &["100/99", "243/242", "100/99", "144/143", "243/242"],
    moments: &["6L 1s", "7L 6s", "7L 13s"],
};

/// Trisected (2.3.5.7.11.13), rank 2, generator 10/7 at 635.0 cents.
pub const TRISECTED: NamedTemperament = NamedTemperament {
    name: "TRISECTED",
    page: "Trisected",
    revision: 233786,
    subgroup: &[2, 3, 5, 7, 11, 13],
    periods_per_equave: 3,
    generator_rows: &[&[3, 0, -1, -1, 7]],
    generator_ratios: &["10/7"],
    generator_cents: &[635.0],
    optimization: "CWE",
    titles: "",
    commas: &["56/55", "91/90", "128/125", "1029/1000"],
    moments: &["6L 9s", "15L 6s", "15L 21s"],
};

/// Mabilic and trismegistus (2.3.5.7), rank 2, generator 175/128 at 526.7 cents.
pub const TRISMEGISTUS: NamedTemperament = NamedTemperament {
    name: "TRISMEGISTUS",
    page: "Mabilic and trismegistus",
    revision: 232517,
    subgroup: &[2, 3, 5, 7],
    periods_per_equave: 1,
    generator_rows: &[&[-15, -3, 5]],
    generator_ratios: &["175/128"],
    generator_cents: &[526.7],
    optimization: "CWE",
    titles: "Mabilic; Trismegistus",
    commas: &["1029/1024", "3125/3072"],
    moments: &["7L 2s", "9L 7s"],
};

/// Tritikleismic (2.3.5.7.11.13.17), rank 2, generator 6/5 at 316.938 cents.
pub const TRITIKLEISMIC: NamedTemperament = NamedTemperament {
    name: "TRITIKLEISMIC",
    page: "Tritikleismic",
    revision: 230636,
    subgroup: &[2, 3, 5, 7, 11, 13, 17],
    periods_per_equave: 3,
    generator_rows: &[&[6, 5, -2, 3, 14, 18]],
    generator_ratios: &["6/5"],
    generator_cents: &[316.938],
    optimization: "CWE",
    titles: "",
    commas: &[],
    moments: &["3L 6s", "9L 3s", "12L 3s", "15L 12s"],
};

/// Ultrapyth (2.3.5.7.13), rank 2, generator 3/2 at 713.6 cents.
pub const ULTRAPYTH: NamedTemperament = NamedTemperament {
    name: "ULTRAPYTH",
    page: "Ultrapyth",
    revision: 223678,
    subgroup: &[2, 3, 5, 7, 13],
    periods_per_equave: 1,
    generator_rows: &[&[1, 14, -2, 18]],
    generator_ratios: &["3/2"],
    generator_cents: &[713.6],
    optimization: "CWE",
    titles: "",
    commas: &["64/63", "6860/6561", "64/63", "91/90", "6125/6084"],
    moments: &["5L 7s", "5L 12s", "5L 17s", "5L 22s"],
};

/// Unidec (2.3.5.7.11), rank 2, generator 14/11 at 416.9 cents.
pub const UNIDEC: NamedTemperament = NamedTemperament {
    name: "UNIDEC",
    page: "Unidec",
    revision: 236473,
    subgroup: &[2, 3, 5, 7, 11],
    periods_per_equave: 2,
    generator_rows: &[&[6, 11, -2, -3]],
    generator_ratios: &["14/11"],
    generator_cents: &[416.9],
    optimization: "CWE",
    titles: "",
    commas: &["385/384", "441/440", "4375/4374"],
    moments: &["6L 2s", "6L 8s", "6L 14s", "20L 6s"],
};

/// Valentine (2.3.5.7.11), rank 2, generator 22/21 at 77.9 cents.
pub const VALENTINE: NamedTemperament = NamedTemperament {
    name: "VALENTINE",
    page: "Valentine",
    revision: 236084,
    subgroup: &[2, 3, 5, 7, 11],
    periods_per_equave: 1,
    generator_rows: &[&[9, 5, -3, 7]],
    generator_ratios: &["22/21"],
    generator_cents: &[77.9],
    optimization: "CWE",
    titles: "",
    commas: &["121/120", "126/125", "176/175"],
    moments: &["1L 14s", "15L 1s", "15L 16s"],
};

/// Vengeance (2.5.7.17), rank 2, generator 34/25 at 527.718 cents.
pub const VENGEANCE: NamedTemperament = NamedTemperament {
    name: "VENGEANCE",
    page: "Vengeance",
    revision: 231841,
    subgroup: &[2, 5, 7, 17],
    periods_per_equave: 1,
    generator_rows: &[&[3, -5, 7]],
    generator_ratios: &["34/25"],
    generator_cents: &[527.718],
    optimization: "CWE",
    titles: "",
    commas: &["78608/78125", "2023/2000", "4165/4096"],
    moments: &["2L 5s", "7L 2s"],
};

/// Whitewood (2.3.5.7), rank 2, generator 5/4 at 392.7 cents.
pub const WHITEWOOD: NamedTemperament = NamedTemperament {
    name: "WHITEWOOD",
    page: "Whitewood",
    revision: 232189,
    subgroup: &[2, 3, 5, 7],
    periods_per_equave: 7,
    generator_rows: &[&[0, 1, -1]],
    generator_ratios: &["5/4"],
    generator_cents: &[392.7],
    optimization: "CWE",
    titles: "",
    commas: &["36/35", "2187/2048"],
    moments: &["7L 7s", "7L 14s"],
};

/// Wizard (2.3.5.7.11.17), rank 2, generator 17/15 at 216.8 cents.
pub const WIZARD: NamedTemperament = NamedTemperament {
    name: "WIZARD",
    page: "Wizard",
    revision: 226921,
    subgroup: &[2, 3, 5, 7, 11, 17],
    periods_per_equave: 2,
    generator_rows: &[&[6, -1, 10, -3, 6]],
    generator_ratios: &["17/15"],
    generator_cents: &[216.8],
    optimization: "CWE",
    titles: "",
    commas: &["225/224", "289/288", "385/384", "561/560"],
    moments: &["6L 4s", "6L 10s", "6L 16s", "22L 6s"],
};

/// Wollemia (2.3.5.7.11.13), rank 2, generator 10/9 at 177.1 cents.
pub const WOLLEMIA: NamedTemperament = NamedTemperament {
    name: "WOLLEMIA",
    page: "Wollemia",
    revision: 231333,
    subgroup: &[2, 3, 5, 7, 11, 13],
    periods_per_equave: 1,
    generator_rows: &[&[4, 9, 19, 10, -2]],
    generator_ratios: &["10/9"],
    generator_cents: &[177.1],
    optimization: "CWE",
    titles: "",
    commas: &[
        "56/55", "100/99", "243/242", "56/55", "91/90", "100/99", "243/242",
    ],
    moments: &["6L 1s", "7L 6s", "7L 13s", "7L 20s"],
};

/// Würschmidt (2.3.5.23), rank 2, generator 5/4 at 387.8 cents.
pub const WURSCHMIDT: NamedTemperament = NamedTemperament {
    name: "WURSCHMIDT",
    page: "Würschmidt",
    revision: 232954,
    subgroup: &[2, 3, 5, 23],
    periods_per_equave: 1,
    generator_rows: &[&[8, 1, 14]],
    generator_ratios: &["5/4"],
    generator_cents: &[387.8],
    optimization: "CWE",
    titles: "",
    commas: &["576/575", "12167/12150"],
    moments: &["3L 1s", "3L 4s", "3L 28s", "31L 3s"],
};

/// Xenial (2.3.5.7.11.13.17.19.23), rank 2, generator 10/9 at 188.8 cents.
pub const XENIAL: NamedTemperament = NamedTemperament {
    name: "XENIAL",
    page: "Xenial",
    revision: 233320,
    subgroup: &[2, 3, 5, 7, 11, 13, 17, 19, 23],
    periods_per_equave: 1,
    generator_rows: &[&[-9, -17, -33, 22, -21, 26, 27, -3]],
    generator_ratios: &["10/9"],
    generator_cents: &[188.8],
    optimization: "CWE",
    titles: "",
    commas: &[
        "126/125", "162/161", "169/168", "171/170", "208/207", "221/220", "231/230",
    ],
    moments: &["6L 1s", "6L 7s", "13L 6s", "19L 13s", "19L 32s", "19L 51s"],
};

/// Zeus (2.3.5.7.11.13), rank 3, generator 3/2, 12/11 at 701.9, 157.0 cents.
pub const ZEUS: NamedTemperament = NamedTemperament {
    name: "ZEUS",
    page: "Zeus",
    revision: 233799,
    subgroup: &[2, 3, 5, 7, 11, 13],
    periods_per_equave: 1,
    generator_rows: &[&[1, 1, -1, 1, -2], &[0, -2, 3, -1, -1]],
    generator_ratios: &["3/2", "12/11"],
    generator_cents: &[701.9, 157.0],
    optimization: "CWE",
    titles: "",
    commas: &["121/120", "176/175", "351/350"],
    moments: &[],
};
