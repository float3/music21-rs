use crate::{
    chord::{Chord, root::pitch_class},
    chordsymbol::{ChordQuality, ChordSymbol},
    defaults::IntegerType,
    error::{Error, Result},
    interval::Interval,
    key::Key,
    pitch::Pitch,
};
use std::fmt;
use std::sync::LazyLock;

/// Parsed once: inverting a chord walks this interval one pitch at a time.
static OCTAVE_UP: LazyLock<Interval> =
    LazyLock::new(|| Interval::from_name("P8").expect("P8 is a valid interval"));

/// How a figure on the sixth or seventh degree of a minor key decides
/// between the natural and the raised degree: music21's `Minor67Default`.
///
/// Minor is two scales at once, and a `vi` might mean either of two chords.
/// music21 lets a caller say which reading to use, and the readings differ
/// in what they do with an accidental the figure already carries.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Minor67Default {
    /// Read it from the chord the figure asks for: a minor triad on the
    /// sixth or a diminished chord on the seventh needs the raised degree,
    /// and a major one needs the natural. This is music21's default and the
    /// reading most scores assume.
    #[default]
    Quality,
    /// Always the natural degree, whatever the figure asks for. A sharp
    /// written in front still raises it.
    Flat,
    /// Always the raised degree. A flat written in front still lowers it.
    Sharp,
    /// As `Quality`, but an accidental already written in front is read as a
    /// caution rather than as a further change — so `#vi` and `vi` are the
    /// same chord, and so are `bVI` and `VI`.
    Cautionary,
}

/// A parsed Roman numeral in a key.
#[derive(Clone, Debug)]
pub struct RomanNumeral {
    figure: String,
    key: Key,
    degree: u8,
    accidental: i8,
    inversion: u8,
    seventh: bool,
    quality: RomanQuality,
    secondary: Option<String>,
    kind: RomanKind,
    /// How the sixth and seventh degrees of a minor key are read.
    #[cfg_attr(feature = "serde", serde(default))]
    sixth_minor: Minor67Default,
    #[cfg_attr(feature = "serde", serde(default))]
    seventh_minor: Minor67Default,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RomanKind {
    Diatonic,
    AugmentedSixth(AugmentedSixthKind),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AugmentedSixthKind {
    Italian,
    French,
    German,
    Swiss,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RomanQuality {
    Major,
    Minor,
    Diminished,
    HalfDiminished,
    Augmented,
}

impl AugmentedSixthKind {
    fn from_figure(figure: &str) -> Option<Self> {
        match figure.trim() {
            // music21 writes these with or without the `+6`, and with the
            // inversion figure in place of it — `Ger65` is the German sixth
            // in the position it is nearly always used in.
            "It" | "It+6" | "It6" | "It53" | "It63" | "It64" => Some(Self::Italian),
            "Fr" | "Fr+6" | "Fr6" | "Fr43" | "Fr7" | "Fr42" | "Fr65" => Some(Self::French),
            "Ger" | "Ger+6" | "Ger6" | "Ger65" | "Ger7" | "Ger43" | "Ger42" => Some(Self::German),
            "Sw" | "Sw+6" | "Sw6" | "Sw43" | "Sw7" | "Sw65" | "Sw42" => Some(Self::Swiss),
            _ => None,
        }
    }

    fn from_common_name(name: &str) -> Option<Self> {
        if name.contains("Italian augmented sixth chord") {
            Some(Self::Italian)
        } else if name.contains("French augmented sixth chord") {
            Some(Self::French)
        } else if name.contains("German augmented sixth chord") {
            Some(Self::German)
        } else if name.contains("Swiss augmented sixth chord") {
            Some(Self::Swiss)
        } else {
            None
        }
    }

    fn figure(self) -> &'static str {
        match self {
            Self::Italian => "It+6",
            Self::French => "Fr+6",
            Self::German => "Ger+6",
            Self::Swiss => "Sw+6",
        }
    }

    fn interval_names(self) -> Vec<&'static str> {
        match self {
            Self::Italian => vec!["P1", "M3", "a6"],
            Self::French => vec!["P1", "M3", "a4", "a6"],
            Self::German => vec!["P1", "M3", "P5", "a6"],
            Self::Swiss => vec!["P1", "M3", "aa4", "a6"],
        }
    }
}

impl RomanNumeral {
    /// Parses a Roman numeral figure in a key.
    ///
    /// Supports ordinary figures such as `V7/V` and augmented-sixth figures
    /// such as `It+6`, `Fr+6`, `Ger+6`, and `Sw+6`.
    pub fn new(figure: impl Into<String>, key: Key) -> Result<Self> {
        Self::with_minor_defaults(
            figure,
            key,
            Minor67Default::default(),
            Minor67Default::default(),
        )
    }

    /// The same, saying how the sixth and seventh degrees of a minor key are
    /// to be read: music21's `sixthMinor` and `seventhMinor`.
    pub fn with_minor_defaults(
        figure: impl Into<String>,
        key: Key,
        sixth_minor: Minor67Default,
        seventh_minor: Minor67Default,
    ) -> Result<Self> {
        let figure = figure.into();
        let trimmed = figure.trim();
        if trimmed.is_empty() {
            return Err(Error::Chord("roman numeral cannot be empty".to_string()));
        }

        if let Some(kind) = AugmentedSixthKind::from_figure(trimmed) {
            return Ok(Self {
                // The figure is kept as written, since music21 names these
                // several ways and reports back the one it was given.
                figure: trimmed.to_string(),
                key,
                degree: 6,
                accidental: -1,
                inversion: 0,
                seventh: false,
                quality: RomanQuality::Augmented,
                secondary: None,
                kind: RomanKind::AugmentedSixth(kind),
                sixth_minor,
                seventh_minor,
            });
        }

        let (primary, secondary) = match trimmed.split_once('/') {
            Some((primary, secondary)) => (primary, Some(secondary.to_string())),
            None => (trimmed, None),
        };

        // music21 writes a few chords by name rather than by numeral: the
        // Neapolitan and the cadential six-four. Each is read as the figure
        // it stands for, while the numeral keeps the name it was given.
        let primary = &named_figure(primary, &key);

        let (accidental, primary) = split_roman_accidental_prefix(primary);
        let (roman, suffix) = split_roman_prefix(primary)?;
        let degree = roman_degree(roman)?;
        let quality = roman_quality(roman, suffix);
        let inversion = parse_inversion(suffix);
        let seventh = suffix_has_seventh(suffix);

        let mut numeral = Self {
            figure: trimmed.to_string(),
            key,
            degree,
            accidental,
            inversion,
            seventh,
            quality,
            secondary,
            kind: RomanKind::Diatonic,
            sixth_minor,
            seventh_minor,
        };
        numeral.raise_minor_sixth_and_seventh()?;
        Ok(numeral)
    }

    /// Raises the sixth and seventh degrees of a minor key where the figure
    /// asks for a chord that only the raised degree gives.
    ///
    /// This is music21's `_adjustMinorVIandVIIByQuality` at its default
    /// setting, and it is what makes `viio7` in A minor the diminished
    /// seventh on `G#` rather than a chord on `G` with three flats in it,
    /// and `vi` the minor triad on `F#`. The natural degrees are what `VI`
    /// and `VII` ask for, and those are left alone.
    fn raise_minor_sixth_and_seventh(&mut self) -> Result<()> {
        if !matches!(self.degree, 6 | 7) {
            return Ok(());
        }
        // Against the key the figure is actually read in, so the `vi` of a
        // secondary numeral is judged in the key that numeral establishes.
        if self.effective_key()?.mode() != "minor" {
            return Ok(());
        }
        let reading = if self.degree == 6 {
            self.sixth_minor
        } else {
            self.seventh_minor
        };
        // A chord that only the raised degree gives.
        let wants_raised = matches!(
            self.quality,
            RomanQuality::Minor | RomanQuality::Diminished | RomanQuality::HalfDiminished
        );
        let raise = match reading {
            Minor67Default::Flat => false,
            Minor67Default::Sharp => true,
            Minor67Default::Quality => wants_raised,
            // An accidental already written is a caution, not a further
            // change: `#vi` and `vi` are the same chord, and so are `bVI`
            // and `VI`.
            Minor67Default::Cautionary => match self.accidental {
                0 => wants_raised,
                // A sharp already written is the caution, and says nothing
                // more; a flat is a caution against the raised degree, so
                // the raise it cancels is applied and the two meet at the
                // natural one.
                sharps if sharps >= 1 => false,
                _ => true,
            },
        };
        if raise {
            self.accidental += 1;
        }
        Ok(())
    }

    /// How this numeral reads the sixth degree of a minor key.
    pub fn sixth_minor(&self) -> Minor67Default {
        self.sixth_minor
    }

    /// How it reads the seventh.
    pub fn seventh_minor(&self) -> Minor67Default {
        self.seventh_minor
    }

    /// Returns the original figure.
    pub fn figure(&self) -> &str {
        &self.figure
    }

    /// Returns the one-based scale degree.
    pub fn degree(&self) -> u8 {
        self.degree
    }

    /// Returns the chromatic alteration of the scale degree in semitones.
    ///
    /// Negative values are flats and positive values are sharps, so `bII`
    /// returns `-1` and `#iv` returns `1`.
    pub fn accidental(&self) -> i8 {
        self.accidental
    }

    /// Returns the inversion number, where root position is `0`.
    pub fn inversion(&self) -> u8 {
        self.inversion
    }

    /// Returns the secondary/applied target figure, if any.
    pub fn secondary(&self) -> Option<&str> {
        self.secondary.as_deref()
    }

    /// Returns the key context.
    pub fn key(&self) -> &Key {
        &self.key
    }

    /// The numeral with its front alteration and nothing else: music21's
    /// `romanNumeral`, so `bII6` is `bII` and `V65/V` is `V`. An augmented
    /// sixth answers its nationality, `It`, `Fr`, `Ger` or `Sw`.
    pub fn roman_numeral(&self) -> String {
        let numeral = self.roman_numeral_alone();
        if matches!(self.kind, RomanKind::AugmentedSixth(_)) {
            return numeral;
        }
        let prefix = match self.accidental {
            0 => String::new(),
            sharps if sharps > 0 => "#".repeat(sharps.unsigned_abs() as usize),
            flats => "b".repeat(flats.unsigned_abs() as usize),
        };
        format!("{prefix}{numeral}")
    }

    /// The numeral with nothing written in front of it: music21's
    /// `romanNumeralAlone`, so `bVII65/V` is `VII`.
    pub fn roman_numeral_alone(&self) -> String {
        if let RomanKind::AugmentedSixth(kind) = self.kind {
            return kind.figure().trim_end_matches(['+', '6']).to_string();
        }
        let primary = self.figure.split('/').next().unwrap_or_default();
        let (_, unaltered) = split_roman_accidental_prefix(primary);
        unaltered
            .chars()
            .take_while(|c| matches!(c, 'i' | 'v' | 'x' | 'I' | 'V' | 'X'))
            .collect()
    }

    /// The figure and its key together: music21's `figureAndKey`,
    /// `bII6 in a minor`.
    pub fn figure_and_key(&self) -> String {
        format!(
            "{} in {} {}",
            self.figure,
            self.key.tonic_pitch_name_with_case(),
            self.key.mode()
        )
    }

    /// The scale degree with the alteration in front of it, as sharps
    /// (positive) or flats (negative): music21's `scaleDegreeWithAlteration`.
    pub fn scale_degree_with_alteration(&self) -> (u8, i8) {
        (self.degree, self.accidental)
    }

    /// How strongly the figure implies its function, from music21's
    /// `functionalityScores` table: `I` is 100, `V7` 80, an unknown figure 0.
    /// A secondary figure multiplies the scores of its halves.
    /// An augmented sixth is looked up by music21's spelling, `It6` rather
    /// than the `It+6` this crate writes.
    pub fn functionality_score(&self) -> u8 {
        if self.secondary.is_some() {
            let score = self.figure.split('/').fold(100.0, |score, part| {
                score * f64::from(functionality_score_of(part).unwrap_or(0)) / 100.0
            });
            return score as u8;
        }
        functionality_score_of(&self.figure.replace('+', "")).unwrap_or(0)
    }

    /// Whether this is a Neapolitan chord: a major triad on the flattened
    /// second degree, in first inversion unless `require_first_inversion` is
    /// off.
    pub fn is_neapolitan(&self, require_first_inversion: bool) -> bool {
        self.degree == 2
            && self.accidental == -1
            && self.quality == RomanQuality::Major
            && (!require_first_inversion || self.inversion == 1)
    }

    /// Whether the chord is borrowed from the parallel mode: music21's
    /// `isMixture`, so `iv` and `bVI` in a major key are mixture and `IV` in a
    /// minor key is. With `evaluate_secondary` a secondary figure is judged by
    /// the numeral after the slash.
    pub fn is_mixture(&self, evaluate_secondary: bool) -> Result<bool> {
        if evaluate_secondary && let Some(secondary) = &self.secondary {
            return RomanNumeral::new(secondary.clone(), self.key.clone())?.is_mixture(true);
        }
        if self.kind != RomanKind::Diatonic || !(1..=7).contains(&self.degree) {
            return Ok(false);
        }
        let quality = match self.quality {
            RomanQuality::Diminished => "diminished",
            RomanQuality::Minor => "minor",
            RomanQuality::Major => "major",
            RomanQuality::HalfDiminished | RomanQuality::Augmented => return Ok(false),
        };
        let front = match self.accidental {
            0 => "natural",
            1 => "sharp",
            -1 => "flat",
            _ => "other",
        };
        let entry = (self.degree, quality, front);
        Ok(match self.key.mode() {
            "major" => {
                MAJOR_KEY_MIXTURES.contains(&entry)
                    || (self.degree == 7
                        && self.seventh
                        && self.quality == RomanQuality::Diminished)
            }
            "minor" => {
                MINOR_KEY_MIXTURES.contains(&entry)
                    || (self.degree == 7
                        && self.seventh
                        && self.quality == RomanQuality::HalfDiminished)
            }
            _ => false,
        })
    }

    /// The same figure in the key transposed by `interval`.
    pub fn transpose(&self, interval: &Interval) -> Result<Self> {
        RomanNumeral::new(self.figure.clone(), self.key.transpose(interval)?)
    }

    /// Realizes the Roman numeral as a chord.
    pub fn to_chord(&self) -> Result<Chord> {
        if let RomanKind::AugmentedSixth(kind) = self.kind {
            return self.augmented_sixth_chord(kind);
        }

        let effective_key = self.effective_key()?;
        let mut root = effective_key.pitch_from_degree(self.degree as usize)?;
        if self.accidental != 0 {
            // The alteration in front of a numeral moves the note without
            // renaming it — music21 transposes by an augmented unison — so a
            // `bII` in C is `D-`, not the `C#` a semitone step would give.
            let altered = root.accidental().alter() + f64::from(self.accidental);
            root.set_accidental(Some(crate::pitch::Accidental::new(altered)?));
        }
        let mut pitches = self
            .interval_names()
            .into_iter()
            .map(|name| Interval::from_name(name)?.transpose_pitch(&root))
            .collect::<Result<Vec<_>>>()?;

        for _ in 0..self.inversion.min(pitches.len().saturating_sub(1) as u8) {
            let pitch = pitches.remove(0);
            let transposed = OCTAVE_UP.transpose_pitch(&pitch)?;
            pitches.push(transposed);
        }

        Chord::new(pitches.as_slice())
    }

    fn augmented_sixth_chord(&self, kind: AugmentedSixthKind) -> Result<Chord> {
        let mut lowered_sixth = self.key.pitch_from_degree(6)?;
        if self.key.mode() != "minor" {
            lowered_sixth = Interval::from_semitones(-1)?.transpose_pitch(&lowered_sixth)?;
        }
        let pitches = kind
            .interval_names()
            .into_iter()
            .map(|name| Interval::from_name(name)?.transpose_pitch(&lowered_sixth))
            .collect::<Result<Vec<_>>>()?;
        Chord::new(pitches.as_slice())
    }

    /// Performs functional Roman-numeral analysis in a key.
    pub fn analyze(chord: &Chord, key: Key) -> Result<Option<Self>> {
        let Some(root_name) = chord.root_pitch_name() else {
            return Ok(None);
        };
        let root = Pitch::from_name(normalize_pitch_name(&root_name))?;
        Self::analyze_with_root(chord, key, &root)
    }

    /// Performs Roman-numeral analysis using an explicit harmonic root.
    ///
    /// This is useful for pitch-class-set browser views where the caller has
    /// already chosen a transposition root and does not want inversion or root
    /// inference to pick a different chord member.
    pub fn analyze_with_root(chord: &Chord, key: Key, root: &Pitch) -> Result<Option<Self>> {
        if let Some(kind) = augmented_sixth_kind_for_key(chord, &key)? {
            return Self::new(kind.figure(), key).map(Some);
        }

        let root_pc = pitch_class(root);
        let intervals = intervals_above_root(chord, root_pc);
        if !intervals.contains(&0) {
            return Ok(None);
        }

        let Some((degree, accidental)) = degree_for_root(&key, root)? else {
            return Ok(None);
        };

        let symbol = chord
            .chord_symbols_with_root(root_pc)?
            .into_iter()
            .find_map(|figure| ChordSymbol::parse(figure).ok());
        let quality = symbol
            .as_ref()
            .map(symbol_quality)
            .unwrap_or_else(|| quality_from_intervals(&intervals));

        let figure = roman_figure(
            degree,
            accidental,
            quality,
            symbol.as_ref(),
            &intervals,
            roman_inversion(chord),
        );

        Self::new(figure, key).map(Some)
    }

    /// The key the figure is actually read in: the key it was given, or the
    /// one a secondary numeral establishes — the `V` of `V/V` in G major is
    /// read in D major.
    pub fn effective_key_of(&self) -> Result<Key> {
        self.effective_key()
    }

    fn effective_key(&self) -> Result<Key> {
        let Some(secondary) = &self.secondary else {
            return Ok(self.key.clone());
        };

        let (accidental, secondary) = split_roman_accidental_prefix(secondary);
        let (roman, _) = split_roman_prefix(secondary)?;
        let degree = roman_degree(roman)?;
        let mut tonic = self.key.pitch_from_degree(degree as usize)?;
        if accidental != 0 {
            tonic = Interval::from_semitones(accidental as IntegerType)?.transpose_pitch(&tonic)?;
        }
        let mode = if roman.chars().next().is_some_and(char::is_uppercase) {
            "major"
        } else {
            "minor"
        };
        Key::from_tonic_mode(&tonic.name(), mode)
    }

    fn interval_names(&self) -> Vec<&'static str> {
        match (self.quality, self.seventh) {
            (RomanQuality::Major, false) => vec!["P1", "M3", "P5"],
            (RomanQuality::Major, true) => vec!["P1", "M3", "P5", "m7"],
            (RomanQuality::Minor, false) => vec!["P1", "m3", "P5"],
            (RomanQuality::Minor, true) => vec!["P1", "m3", "P5", "m7"],
            (RomanQuality::Diminished, false) => vec!["P1", "m3", "d5"],
            (RomanQuality::Diminished, true) => vec!["P1", "m3", "d5", "d7"],
            (RomanQuality::HalfDiminished, false) => vec!["P1", "m3", "d5"],
            (RomanQuality::HalfDiminished, true) => vec!["P1", "m3", "d5", "m7"],
            (RomanQuality::Augmented, false) => vec!["P1", "M3", "a5"],
            (RomanQuality::Augmented, true) => vec!["P1", "M3", "a5", "m7"],
        }
    }
}

impl fmt::Display for RomanNumeral {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.figure())
    }
}

/// music21's `functionalityScores`: how strongly a figure implies its
/// harmonic function, on a hundred-point scale, in music21's order.
pub const FUNCTIONALITY_SCORES: [(&str, u8); 48] = [
    ("I", 100),
    ("i", 90),
    ("V7", 80),
    ("V", 70),
    ("V65", 68),
    ("I6", 65),
    ("V6", 63),
    ("V43", 61),
    ("I64", 60),
    ("IV", 59),
    ("i6", 58),
    ("viio7", 57),
    ("V42", 55),
    ("viio65", 53),
    ("viio6", 52),
    ("#viio65", 51),
    ("ii", 50),
    ("#viio6", 49),
    ("ii65", 48),
    ("ii43", 47),
    ("ii42", 46),
    ("IV6", 45),
    ("ii6", 43),
    ("VI", 42),
    ("#VI", 41),
    ("vi", 40),
    ("viio", 39),
    ("#viio", 38),
    ("iio", 37),
    ("iio42", 36),
    ("bII6", 35),
    ("It6", 34),
    ("Ger65", 33),
    ("iio43", 32),
    ("iio65", 31),
    ("Fr43", 30),
    ("#vio", 28),
    ("#vio6", 27),
    ("III", 22),
    ("Sw43", 21),
    ("v", 20),
    ("VII", 19),
    ("VII7", 18),
    ("IV65", 17),
    ("IV7", 16),
    ("iii", 15),
    ("iii6", 12),
    ("vi6", 10),
];

fn functionality_score_of(figure: &str) -> Option<u8> {
    FUNCTIONALITY_SCORES
        .iter()
        .find(|(known, _)| *known == figure)
        .map(|(_, score)| *score)
}

const MAJOR_KEY_MIXTURES: [(u8, &str, &str); 7] = [
    (1, "minor", "natural"),
    (2, "diminished", "natural"),
    (3, "major", "flat"),
    (4, "minor", "natural"),
    (5, "minor", "natural"),
    (6, "major", "flat"),
    (7, "major", "flat"),
];

const MINOR_KEY_MIXTURES: [(u8, &str, &str); 5] = [
    (1, "major", "natural"),
    (2, "minor", "natural"),
    (3, "minor", "sharp"),
    (4, "major", "natural"),
    (6, "minor", "sharp"),
];

/// The figured-bass suffix a Roman numeral takes for a chord's inversion:
/// music21's `romanInversionName`, `6` and `64` for a triad, `7`, `65`, `43`
/// and `42` for a seventh chord, and nothing otherwise. The inversion is the
/// chord's own unless one is given.
pub fn roman_inversion_name(chord: &Chord, inversion: Option<u8>) -> String {
    match chord.root() {
        Some(root) => inversion_name_from_root(chord, root, inversion),
        None => String::new(),
    }
}

fn inversion_name_from_root(chord: &Chord, root: &Pitch, inversion: Option<u8>) -> String {
    let Some(inversion) = inversion.or_else(|| chord.inversion_with_root(root)) else {
        return String::new();
    };
    let has = |step: u8| chord.chord_step_from(step, root).is_some();
    let suffix = if has(7) {
        match inversion {
            0 => "7",
            1 => "65",
            2 => "43",
            3 => "42",
            _ => "",
        }
    } else if has(1) && has(3) {
        match inversion {
            1 => "6",
            2 => "64",
            _ => "",
        }
    } else {
        ""
    };
    suffix.to_string()
}

/// Reads a chord as the tonic or the dominant of a key, with its inversion:
/// music21's `identifyAsTonicOrDominant`, so `G B D F` in C major is `V7`
/// and `A C E` in A minor is `i`. A chord containing the tonic is the tonic,
/// one containing the dominant is the dominant, and anything else is judged
/// by how many of its notes the tonic and dominant sevenths share; `None`
/// when neither wins.
pub fn identify_as_tonic_or_dominant(chord: &Chord, key: &Key) -> Result<Option<String>> {
    let names = chord.pitch_names();
    let tonic = key.pitch_from_degree(1)?;
    let dominant = key.pitch_from_degree(5)?;
    let overlap = |figure: &str| -> Result<usize> {
        let members = RomanNumeral::new(figure, key.clone())?
            .to_chord()?
            .pitch_names();
        Ok(members.iter().filter(|name| names.contains(name)).count())
    };
    let is_tonic = if names.contains(&tonic.name()) {
        true
    } else if names.contains(&dominant.name()) {
        false
    } else {
        let (one, five) = (overlap("I7")?, overlap("V7")?);
        if one == five {
            return Ok(None);
        }
        one > five
    };
    let (numeral, root) = if is_tonic {
        (if key.mode() == "minor" { "i" } else { "I" }, &tonic)
    } else {
        ("V", &dominant)
    };
    Ok(Some(format!(
        "{numeral}{}",
        inversion_name_from_root(chord, root, None)
    )))
}

/// Performs functional Roman-numeral analysis in a key.
pub fn analyze_chord(chord: &Chord, key: Key) -> Result<Option<RomanNumeral>> {
    RomanNumeral::analyze(chord, key)
}

/// Performs Roman-numeral analysis in a key using an explicit harmonic root.
pub fn analyze_chord_with_root(
    chord: &Chord,
    key: Key,
    root: &Pitch,
) -> Result<Option<RomanNumeral>> {
    RomanNumeral::analyze_with_root(chord, key, root)
}

fn split_roman_accidental_prefix(value: &str) -> (i8, &str) {
    let mut accidental = 0;
    let mut end = 0;
    for (idx, ch) in value.char_indices() {
        match ch {
            '#' => {
                accidental += 1;
                end = idx + ch.len_utf8();
            }
            'b' | '-' => {
                accidental -= 1;
                end = idx + ch.len_utf8();
            }
            _ => break,
        }
    }
    (accidental, &value[end..])
}

/// The chords music21 writes by name, as the figures they stand for.
///
/// The Neapolitan is the flattened second degree, written `N` and — the way
/// it is nearly always used — `N6` in first inversion. The cadential
/// six-four is the tonic triad in second inversion, and takes the case of
/// the key it stands in.
fn named_figure(figure: &str, key: &Key) -> String {
    let tonic = if key.mode() == "minor" { "i" } else { "I" };
    match figure {
        "N" | "N6" => "bII6".to_string(),
        "N53" => "bII".to_string(),
        "Cad64" => format!("{tonic}64"),
        other => other.to_string(),
    }
}

fn split_roman_prefix(value: &str) -> Result<(&str, &str)> {
    let end = value
        .char_indices()
        .find_map(|(idx, ch)| (!matches!(ch, 'I' | 'V' | 'X' | 'i' | 'v' | 'x')).then_some(idx))
        .unwrap_or(value.len());

    if end == 0 {
        return Err(Error::Chord(format!("No roman numeral found in '{value}'")));
    }

    Ok((&value[..end], &value[end..]))
}

fn roman_degree(roman: &str) -> Result<u8> {
    match roman.to_ascii_uppercase().as_str() {
        "I" => Ok(1),
        "II" => Ok(2),
        "III" => Ok(3),
        "IV" => Ok(4),
        "V" => Ok(5),
        "VI" => Ok(6),
        "VII" => Ok(7),
        _ => Err(Error::Chord(format!("unsupported roman numeral {roman:?}"))),
    }
}

fn roman_quality(roman: &str, suffix: &str) -> RomanQuality {
    let lower = suffix.to_ascii_lowercase();
    if suffix.contains('\u{00f8}') || lower.contains("m7b5") {
        RomanQuality::HalfDiminished
    } else if lower.contains('o') || lower.contains("dim") {
        RomanQuality::Diminished
    } else if lower.contains('+') || lower.contains("aug") {
        RomanQuality::Augmented
    } else if roman.chars().next().is_some_and(char::is_lowercase) {
        RomanQuality::Minor
    } else {
        RomanQuality::Major
    }
}

fn suffix_has_seventh(suffix: &str) -> bool {
    let suffix = strip_roman_addition_groups(suffix);
    suffix.contains('7')
        || suffix.contains('9')
        || suffix.contains("11")
        || suffix.contains("13")
        || suffix.contains("65")
        || suffix.contains("43")
        || suffix.contains("42")
}

/// music21's `roman.figureShorthands`, mapping a full figured-bass string to
/// the abbreviation musicians actually write.
///
/// This is the table the inversion logic used to approximate with
/// `suffix.contains("64")`-style probes, which read `642` as a second inversion
/// because `64` matched before `42` was ever tested. Normalizing through the
/// real table removes that ordering hazard rather than reshuffling the probes.
const FIGURE_SHORTHANDS: [(&str, &str); 20] = [
    ("53", ""),
    ("3", ""),
    ("63", "6"),
    ("753", "7"),
    ("75", "7"),
    ("73", "7"),
    ("9753", "9"),
    ("975", "9"),
    ("953", "9"),
    ("97", "9"),
    ("95", "9"),
    ("93", "9"),
    ("653", "65"),
    ("6b53", "6b5"),
    ("643", "43"),
    ("642", "42"),
    ("bb7b5b3", "o7"),
    ("b7b5b3", "\u{00f8}7"),
    ("bb7b53", "o7"),
    ("b7b53", "\u{00f8}7"),
];

/// Returns the shorthand for a figure, or the figure itself when it has none.
fn normalize_figure(figure: &str) -> &str {
    FIGURE_SHORTHANDS
        .iter()
        .find(|(full, _)| *full == figure)
        .map_or(figure, |(_, short)| *short)
}

fn parse_inversion(suffix: &str) -> u8 {
    let suffix = strip_roman_addition_groups(suffix);
    // Only the figured-bass digits decide the inversion; quality marks such as
    // `o`, `+` and the half-diminished sign ride along in the suffix.
    let digits: String = suffix.chars().filter(char::is_ascii_digit).collect();

    match normalize_figure(&digits) {
        "42" => 3,
        "43" | "64" => 2,
        "65" | "6" => 1,
        _ => 0,
    }
}

fn strip_roman_addition_groups(suffix: &str) -> String {
    let mut stripped = String::with_capacity(suffix.len());
    let mut rest = suffix;
    while let Some(index) = rest.find("add(") {
        stripped.push_str(&rest[..index]);
        let addition = &rest[index + 4..];
        let Some(end) = addition.find(')') else {
            rest = addition;
            continue;
        };
        rest = &addition[end + 1..];
    }
    stripped.push_str(rest);
    stripped
}

fn degree_for_root(key: &Key, root: &Pitch) -> Result<Option<(u8, i8)>> {
    let root_pc = pitch_class(root);
    let root_step = root.step();
    let mut best: Option<(u8, i8, bool)> = None;

    for degree in 1..=7 {
        let degree_pitch = key.pitch_from_degree(degree)?;
        let diff = ((root_pc as i16 - pitch_class(&degree_pitch) as i16).rem_euclid(12)) as u8;
        let Some(accidental) = chromatic_diff_to_accidental(diff) else {
            continue;
        };
        let same_step = degree_pitch.step() == root_step;

        let replace = match best {
            None => true,
            Some((_, best_accidental, best_same_step)) => {
                (same_step && !best_same_step)
                    || (same_step == best_same_step && accidental.abs() < best_accidental.abs())
            }
        };
        if replace {
            best = Some((degree as u8, accidental, same_step));
        }
    }

    Ok(best.map(|(degree, accidental, _)| (degree, accidental)))
}

fn chromatic_diff_to_accidental(diff: u8) -> Option<i8> {
    match diff {
        0 => Some(0),
        1 => Some(1),
        2 => Some(2),
        10 => Some(-2),
        11 => Some(-1),
        _ => None,
    }
}

fn intervals_above_root(chord: &Chord, root_pc: u8) -> Vec<u8> {
    let mut intervals = chord
        .pitch_classes()
        .into_iter()
        .map(|pc| (pc + 12 - root_pc) % 12)
        .collect::<Vec<_>>();
    intervals.sort_unstable();
    intervals.dedup();
    intervals
}

fn augmented_sixth_kind_for_key(chord: &Chord, key: &Key) -> Result<Option<AugmentedSixthKind>> {
    let kind = std::iter::once(chord.common_name())
        .chain(chord.common_names())
        .find_map(|name| AugmentedSixthKind::from_common_name(&name));
    let Some(kind) = kind else {
        return Ok(None);
    };

    let pitch_classes = chord.pitch_classes();
    let tonic = key_degree_pitch_class(key, 1, 0)?;
    let lowered_sixth_adjust = if key.mode() == "minor" { 0 } else { -1 };
    let lowered_sixth = key_degree_pitch_class(key, 6, lowered_sixth_adjust)?;
    let raised_fourth = key_degree_pitch_class(key, 4, 1)?;

    if !pitch_classes.contains(&tonic)
        || !pitch_classes.contains(&lowered_sixth)
        || !pitch_classes.contains(&raised_fourth)
    {
        return Ok(None);
    }

    let required_extra = match kind {
        AugmentedSixthKind::Italian => None,
        AugmentedSixthKind::French => Some(key_degree_pitch_class(key, 2, 0)?),
        AugmentedSixthKind::German => {
            let lowered_third_adjust = if key.mode() == "minor" { 0 } else { -1 };
            Some(key_degree_pitch_class(key, 3, lowered_third_adjust)?)
        }
        AugmentedSixthKind::Swiss => Some(key_degree_pitch_class(key, 2, 1)?),
    };

    if required_extra.is_some_and(|pitch_class| !pitch_classes.contains(&pitch_class)) {
        return Ok(None);
    }

    Ok(Some(kind))
}

fn key_degree_pitch_class(key: &Key, degree: usize, semitones: IntegerType) -> Result<u8> {
    let mut pitch = key.pitch_from_degree(degree)?;
    if semitones != 0 {
        pitch = Interval::from_semitones(semitones)?.transpose_pitch(&pitch)?;
    }
    Ok(pitch_class(&pitch))
}

fn roman_inversion(chord: &Chord) -> u8 {
    if chord.pitches().iter().any(|pitch| pitch.octave().is_some()) {
        chord.inversion().unwrap_or(0)
    } else {
        0
    }
}

fn symbol_quality(symbol: &ChordSymbol) -> RomanQuality {
    match symbol.quality() {
        ChordQuality::Major
        | ChordQuality::Dominant
        | ChordQuality::Suspended2
        | ChordQuality::Suspended4
        | ChordQuality::Power
        | ChordQuality::Pedal => RomanQuality::Major,
        ChordQuality::Minor => RomanQuality::Minor,
        ChordQuality::Diminished => RomanQuality::Diminished,
        ChordQuality::HalfDiminished => RomanQuality::HalfDiminished,
        ChordQuality::Augmented => RomanQuality::Augmented,
    }
}

fn quality_from_intervals(intervals: &[u8]) -> RomanQuality {
    if intervals.contains(&3) && intervals.contains(&6) {
        if intervals.contains(&10) {
            RomanQuality::HalfDiminished
        } else {
            RomanQuality::Diminished
        }
    } else if intervals.contains(&4) && intervals.contains(&8) {
        RomanQuality::Augmented
    } else if intervals.contains(&3) && intervals.contains(&7) {
        RomanQuality::Minor
    } else {
        RomanQuality::Major
    }
}

fn roman_figure(
    degree: u8,
    accidental: i8,
    quality: RomanQuality,
    symbol: Option<&ChordSymbol>,
    intervals: &[u8],
    inversion: u8,
) -> String {
    let base = degree_to_roman(degree);
    let prefix = roman_accidental_prefix(accidental);
    let body = roman_body_for_quality(base, quality);
    let suffix = functional_suffix(symbol, intervals, inversion, quality);
    format!("{prefix}{body}{suffix}")
}

fn roman_accidental_prefix(accidental: i8) -> String {
    match accidental.cmp(&0) {
        std::cmp::Ordering::Less => "b".repeat(accidental.unsigned_abs() as usize),
        std::cmp::Ordering::Equal => String::new(),
        std::cmp::Ordering::Greater => "#".repeat(accidental as usize),
    }
}

fn roman_body_for_quality(base: &str, quality: RomanQuality) -> String {
    match quality {
        RomanQuality::Major => base.to_string(),
        RomanQuality::Minor => base.to_ascii_lowercase(),
        RomanQuality::Diminished => format!("{}o", base.to_ascii_lowercase()),
        RomanQuality::HalfDiminished => format!("{}\u{00f8}", base.to_ascii_lowercase()),
        RomanQuality::Augmented => format!("{base}+"),
    }
}

fn functional_suffix(
    symbol: Option<&ChordSymbol>,
    intervals: &[u8],
    inversion: u8,
    quality: RomanQuality,
) -> String {
    if let Some(symbol) = symbol
        && needs_chord_symbol_suffix(symbol)
    {
        return chord_symbol_suffix_for_roman(symbol, quality);
    }
    figured_bass_suffix(intervals, inversion, quality)
}

fn needs_chord_symbol_suffix(symbol: &ChordSymbol) -> bool {
    matches!(
        symbol.quality(),
        ChordQuality::Suspended2 | ChordQuality::Suspended4 | ChordQuality::Power
    ) || !symbol.additions().is_empty()
        || symbol.alterations().iter().any(|alteration| {
            !(matches!(symbol.quality(), ChordQuality::HalfDiminished)
                && alteration.degree() == 5
                && alteration.semitones() == -1)
        })
        || symbol.extensions().iter().any(|degree| *degree != 7)
        || chord_symbol_suffix(symbol).contains("maj7")
}

fn chord_symbol_suffix_for_roman(symbol: &ChordSymbol, quality: RomanQuality) -> String {
    let suffix = chord_symbol_suffix(symbol);
    let converted = match quality {
        RomanQuality::Major => suffix.to_string(),
        RomanQuality::Minor => suffix
            .strip_prefix('m')
            .filter(|rest| !rest.starts_with("aj"))
            .unwrap_or(suffix)
            .to_string(),
        RomanQuality::Diminished => suffix.strip_prefix("dim").unwrap_or(suffix).to_string(),
        RomanQuality::HalfDiminished => {
            suffix.strip_prefix('m').unwrap_or(suffix).replace("b5", "")
        }
        RomanQuality::Augmented => suffix
            .strip_prefix("aug")
            .or_else(|| suffix.strip_prefix('+'))
            .unwrap_or(suffix)
            .to_string(),
    };

    if converted == "6" {
        " add(13)".to_string()
    } else {
        converted
    }
}

fn chord_symbol_suffix(symbol: &ChordSymbol) -> &str {
    let body = symbol
        .figure()
        .split_once('/')
        .map_or(symbol.figure(), |(body, _)| body);
    let root_name = normalize_symbol_root_name(&symbol.root().name());
    body.strip_prefix(&root_name).unwrap_or(body)
}

fn normalize_symbol_root_name(name: &str) -> String {
    name.replace('-', "b")
}

fn figured_bass_suffix(intervals: &[u8], inversion: u8, quality: RomanQuality) -> String {
    if has_seventh(intervals) {
        let suffix = match inversion {
            1 => "65",
            2 => "43",
            3 => "42",
            _ => "7",
        };
        if matches!(quality, RomanQuality::Major) && intervals.contains(&11) {
            format!("maj{suffix}")
        } else {
            suffix.to_string()
        }
    } else if has_triad_shape(intervals) {
        match inversion {
            1 => "6".to_string(),
            2 => "64".to_string(),
            _ => String::new(),
        }
    } else {
        String::new()
    }
}

fn has_seventh(intervals: &[u8]) -> bool {
    intervals.contains(&10) || intervals.contains(&11) || intervals.contains(&9)
}

fn has_triad_shape(intervals: &[u8]) -> bool {
    (intervals.contains(&3) || intervals.contains(&4))
        && intervals.iter().any(|interval| matches!(interval, 6..=8))
}

fn degree_to_roman(degree: u8) -> &'static str {
    match degree {
        1 => "I",
        2 => "II",
        3 => "III",
        4 => "IV",
        5 => "V",
        6 => "VI",
        7 => "VII",
        _ => "I",
    }
}

fn normalize_pitch_name(name: &str) -> String {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut normalized = first.to_string();
    for ch in chars {
        if ch == 'b' {
            normalized.push('-');
        } else {
            normalized.push(ch);
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_minor_key_raises_its_sixth_and_seventh_where_the_figure_asks() {
        let minor = Key::from_tonic_mode("a", Some("minor")).unwrap();
        // The diminished seventh on the leading note is the raised seventh
        // degree: G#, not G with three flats hung off it.
        let leading = RomanNumeral::new("viio7", minor.clone()).unwrap();
        assert_eq!(
            leading.to_chord().unwrap().pitch_names(),
            ["G#", "B", "D", "F"]
        );
        assert_eq!(leading.roman_numeral(), "#vii");
        assert_eq!(leading.roman_numeral_alone(), "vii");
        assert_eq!(leading.scale_degree_with_alteration(), (7, 1));

        // A minor triad on the sixth degree is the raised one too.
        assert_eq!(
            RomanNumeral::new("vi", minor.clone())
                .unwrap()
                .to_chord()
                .unwrap()
                .pitch_names(),
            ["F#", "A", "C#"]
        );
        // The natural degrees are what the upper-case figures ask for.
        assert_eq!(
            RomanNumeral::new("VI", minor.clone())
                .unwrap()
                .to_chord()
                .unwrap()
                .pitch_names(),
            ["F", "A", "C"]
        );
        // And a major key is left alone entirely.
        let major = Key::from_tonic_mode("C", Some("major")).unwrap();
        assert_eq!(
            RomanNumeral::new("vi", major)
                .unwrap()
                .to_chord()
                .unwrap()
                .pitch_names(),
            ["A", "C", "E"]
        );
    }

    #[test]
    fn the_four_readings_of_a_minor_sixth_and_seventh() {
        let minor = Key::from_tonic_mode("c", Some("minor")).unwrap();
        let read = |figure: &str, sixth: Minor67Default| {
            RomanNumeral::with_minor_defaults(figure, minor.clone(), sixth, sixth)
                .unwrap()
                .to_chord()
                .unwrap()
                .pitch_names()
                .join(" ")
        };

        // By quality, which is the default: the chord the figure asks for
        // says which sixth it is built on.
        assert_eq!(read("vi", Minor67Default::Quality), "A C E");
        assert_eq!(read("VI", Minor67Default::Quality), "A- C E-");

        // Flat is always the natural degree, whatever the figure asks for,
        // and a sharp written in front still raises it.
        assert_eq!(read("vi", Minor67Default::Flat), "A- C- E-");
        assert_eq!(read("#vi", Minor67Default::Flat), "A C E");

        // Sharp is always the raised one, and a flat still lowers it.
        assert_eq!(read("VI", Minor67Default::Sharp), "A C# E");
        assert_eq!(read("bVI", Minor67Default::Sharp), "A- C E-");

        // Cautionary reads the quality, but an accidental already written is
        // a caution rather than a further change, so `#vi` is `vi` and
        // `bVI` is `VI`.
        assert_eq!(read("#vi", Minor67Default::Cautionary), "A C E");
        assert_eq!(read("vi", Minor67Default::Cautionary), "A C E");
        assert_eq!(read("bVI", Minor67Default::Cautionary), "A- C E-");
        assert_eq!(read("VI", Minor67Default::Cautionary), "A- C E-");
    }

    #[test]
    fn the_neapolitan_can_be_written_by_name() {
        // music21 writes the flattened second degree as `N`, and in first
        // inversion — the way it is nearly always used — as `N6`.
        let key = Key::from_tonic_mode("c#", Some("minor")).unwrap();
        let named = RomanNumeral::new("N6", key.clone()).unwrap();
        assert_eq!(named.figure(), "N6");
        assert_eq!(named.degree(), 2);
        assert_eq!(named.accidental(), -1);
        assert_eq!(named.inversion(), 1);
        assert!(named.is_neapolitan(true));

        let spelled = RomanNumeral::new("bII6", key.clone()).unwrap();
        assert_eq!(
            named.to_chord().unwrap().pitch_names(),
            spelled.to_chord().unwrap().pitch_names()
        );

        // `N53` is the same chord in root position.
        let root_position = RomanNumeral::new("N53", key).unwrap();
        assert_eq!(root_position.inversion(), 0);
    }

    #[test]
    fn inversion_names_and_tonic_dominant_reading_match_music21() {
        let chord = |notes: &str| Chord::new(notes).unwrap();
        assert_eq!(roman_inversion_name(&chord("C4 E4 G4"), None), "");
        assert_eq!(roman_inversion_name(&chord("E4 G4 C5"), None), "6");
        assert_eq!(roman_inversion_name(&chord("G3 C4 E4"), None), "64");
        assert_eq!(roman_inversion_name(&chord("C4 E4 G4 B-4"), None), "7");
        assert_eq!(roman_inversion_name(&chord("E4 G4 B-4 C5"), None), "65");
        assert_eq!(roman_inversion_name(&chord("G3 B-3 C4 E4"), None), "43");
        assert_eq!(roman_inversion_name(&chord("B-3 C4 E4 G4"), None), "42");
        assert_eq!(roman_inversion_name(&chord("C4 E4 G4"), Some(2)), "64");
        assert_eq!(roman_inversion_name(&chord("C4 E4 G4"), Some(3)), "");
        assert_eq!(roman_inversion_name(&Chord::empty(), None), "");

        let cases = [
            ("C4 E4 G4", "C", Some("I")),
            ("E4 G4 C5", "C", Some("I6")),
            ("G3 B3 D4 F4", "C", Some("V7")),
            ("D4 F4 A4", "C", Some("V43")),
            ("A3 C4 E4", "a", Some("i")),
            ("E3 G#3 B3", "a", Some("V")),
            ("C4 E4", "C", Some("I")),
            ("G4 D5", "C", Some("V")),
            ("B3 D4", "C", Some("V")),
            ("F#4 A4", "C", None),
        ];
        for (notes, key, expected) in cases {
            let reading =
                identify_as_tonic_or_dominant(&chord(notes), &Key::from_tonic(key).unwrap())
                    .unwrap();
            assert_eq!(reading.as_deref(), expected, "{notes} in {key}");
        }
    }

    #[test]
    #[allow(clippy::type_complexity)]
    fn numeral_names_scores_and_mixture_match_music21() {
        let cases: [(&str, &str, &str, &str, (u8, i8), u8, bool, bool, bool); 18] = [
            (
                "V7",
                "C",
                "V",
                "V7 in C major",
                (5, 0),
                80,
                false,
                false,
                false,
            ),
            (
                "bII6",
                "C",
                "bII",
                "bII6 in C major",
                (2, -1),
                35,
                true,
                true,
                false,
            ),
            (
                "bII",
                "C",
                "bII",
                "bII in C major",
                (2, -1),
                0,
                false,
                true,
                false,
            ),
            (
                "bII6",
                "a",
                "bII",
                "bII6 in a minor",
                (2, -1),
                35,
                true,
                true,
                false,
            ),
            (
                "viio7",
                "C",
                "vii",
                "viio7 in C major",
                (7, 0),
                57,
                false,
                false,
                true,
            ),
            (
                "I",
                "C",
                "I",
                "I in C major",
                (1, 0),
                100,
                false,
                false,
                false,
            ),
            (
                "i",
                "a",
                "i",
                "i in a minor",
                (1, 0),
                90,
                false,
                false,
                false,
            ),
            (
                "V65/V",
                "C",
                "V",
                "V65/V in C major",
                (5, 0),
                47,
                false,
                false,
                false,
            ),
            (
                "iv",
                "C",
                "iv",
                "iv in C major",
                (4, 0),
                0,
                false,
                false,
                true,
            ),
            (
                "bVI",
                "C",
                "bVI",
                "bVI in C major",
                (6, -1),
                0,
                false,
                false,
                true,
            ),
            (
                "I",
                "a",
                "I",
                "I in a minor",
                (1, 0),
                100,
                false,
                false,
                true,
            ),
            (
                "III",
                "a",
                "III",
                "III in a minor",
                (3, 0),
                22,
                false,
                false,
                false,
            ),
            (
                "#iii",
                "a",
                "#iii",
                "#iii in a minor",
                (3, 1),
                0,
                false,
                false,
                true,
            ),
            (
                "ii",
                "C",
                "ii",
                "ii in C major",
                (2, 0),
                50,
                false,
                false,
                false,
            ),
            (
                "iio",
                "C",
                "ii",
                "iio in C major",
                (2, 0),
                37,
                false,
                false,
                true,
            ),
            (
                "IV",
                "a",
                "IV",
                "IV in a minor",
                (4, 0),
                59,
                false,
                false,
                true,
            ),
            (
                "bVII",
                "C",
                "bVII",
                "bVII in C major",
                (7, -1),
                0,
                false,
                false,
                true,
            ),
            (
                "v",
                "C",
                "v",
                "v in C major",
                (5, 0),
                20,
                false,
                false,
                true,
            ),
        ];
        for (figure, key, numeral, figure_and_key, degree, score, neapolitan, loose, mixture) in
            cases
        {
            let rn = RomanNumeral::new(figure, Key::from_tonic(key).unwrap()).unwrap();
            assert_eq!(rn.roman_numeral(), numeral, "{figure} in {key}");
            assert_eq!(rn.figure_and_key(), figure_and_key, "{figure} in {key}");
            assert_eq!(
                rn.scale_degree_with_alteration(),
                degree,
                "{figure} in {key}"
            );
            assert_eq!(rn.functionality_score(), score, "{figure} in {key}");
            assert_eq!(rn.is_neapolitan(true), neapolitan, "{figure} in {key}");
            assert_eq!(rn.is_neapolitan(false), loose, "{figure} in {key}");
            assert_eq!(rn.is_mixture(false).unwrap(), mixture, "{figure} in {key}");
            assert_eq!(rn.is_mixture(true).unwrap(), mixture, "{figure} in {key}");
        }
        let italian = RomanNumeral::new("It6", Key::from_tonic("C").unwrap()).unwrap();
        assert_eq!(italian.roman_numeral(), "It");
        assert_eq!(italian.functionality_score(), 34);
        assert!(!italian.is_mixture(false).unwrap());
        let transposed = RomanNumeral::new("bII6", Key::from_tonic("a").unwrap())
            .unwrap()
            .transpose(&Interval::from_name("M2").unwrap())
            .unwrap();
        assert_eq!(transposed.figure_and_key(), "bII6 in b minor");
    }

    #[test]
    fn functionality_scores_table_has_no_duplicates() {
        let mut figures: Vec<&str> = FUNCTIONALITY_SCORES.iter().map(|(f, _)| *f).collect();
        figures.sort_unstable();
        figures.dedup();
        assert_eq!(figures.len(), FUNCTIONALITY_SCORES.len());
    }
    use super::*;

    #[test]
    fn secondary_dominant_resolves_to_chord() {
        let key = Key::from_tonic_mode("C", "major").unwrap();
        let rn = RomanNumeral::new("V7/V", key).unwrap();
        assert_eq!(rn.degree(), 5);
        assert_eq!(rn.secondary(), Some("V"));
        assert_eq!(
            rn.to_chord().unwrap().pitched_common_name(),
            "D-dominant seventh chord"
        );
    }

    #[test]
    fn analyzes_chord_in_key() {
        let key = Key::from_tonic_mode("C", "major").unwrap();
        let chord = Chord::new("G B D F").unwrap();
        let rn = RomanNumeral::analyze(&chord, key).unwrap().unwrap();
        assert_eq!(rn.figure(), "V7");
    }

    #[test]
    fn analyzes_accidentals_inversions_and_half_diminished_quality() {
        let key = Key::from_tonic_mode("C", "major").unwrap();

        let neapolitan = Chord::new("D- F A-").unwrap();
        let rn = RomanNumeral::analyze(&neapolitan, key.clone())
            .unwrap()
            .unwrap();
        assert_eq!(rn.figure(), "bII");
        assert_eq!(rn.degree(), 2);
        assert_eq!(rn.accidental(), -1);

        let first_inversion = Chord::new("E4 G4 C5").unwrap();
        let rn = RomanNumeral::analyze(&first_inversion, key.clone())
            .unwrap()
            .unwrap();
        assert_eq!(rn.figure(), "I6");

        let leading_tone = Chord::new("B D F A").unwrap();
        let rn = RomanNumeral::analyze(&leading_tone, key).unwrap().unwrap();
        assert_eq!(rn.figure(), "vii\u{00f8}7");
    }

    #[test]
    fn analyzes_with_explicit_root_for_browser_style_sets() {
        let key = Key::from_tonic_mode("C", "major").unwrap();
        let root = Pitch::from_name("C").unwrap();
        let chord = Chord::new("C E G").unwrap();
        let rn = RomanNumeral::analyze_with_root(&chord, key.clone(), &root)
            .unwrap()
            .unwrap();
        assert_eq!(rn.figure(), "I");

        let seventh = Chord::new("C E G B-").unwrap();
        let rn = RomanNumeral::analyze_with_root(&seventh, key, &root)
            .unwrap()
            .unwrap();
        assert_eq!(rn.figure(), "I7");
    }

    #[test]
    fn analyzes_augmented_sixth_chords_functionally() {
        let key = Key::from_tonic_mode("C", "minor").unwrap();
        let root = Pitch::from_name("C").unwrap();
        let french = Chord::new("C D F# A-").unwrap();
        let rn = RomanNumeral::analyze_with_root(&french, key.clone(), &root)
            .unwrap()
            .unwrap();
        assert_eq!(rn.figure(), "Fr+6");

        let german = Chord::new("A- C E- F#").unwrap();
        let rn = RomanNumeral::analyze(&german, key).unwrap().unwrap();
        assert_eq!(rn.figure(), "Ger+6");
    }

    #[test]
    fn figured_bass_shorthands_give_music21_inversions() {
        // Captured from music21's RomanNumeral(...).inversion(). `V642` is the
        // case the old `contains("64")` probe got wrong: it matched `64` and
        // reported a second inversion where music21 reports a third.
        let key = Key::from_tonic_mode("C", "major").unwrap();
        for (figure, expected) in [
            ("V", 0),
            ("V6", 1),
            ("V64", 2),
            ("V7", 0),
            ("V65", 1),
            ("V43", 2),
            ("V42", 3),
            ("V642", 3),
            ("V653", 1),
            ("V643", 2),
            ("V63", 1),
            ("V53", 0),
            ("V9", 0),
        ] {
            let numeral = RomanNumeral::new(figure, key.clone())
                .unwrap_or_else(|err| panic!("{figure} should parse: {err}"));
            assert_eq!(numeral.inversion(), expected, "{figure}");
        }
    }

    #[test]
    fn roman_numerals_parse_inversions_and_qualities() {
        let key = Key::from_tonic_mode("C", "major").unwrap();
        let first_inversion = RomanNumeral::new("I6", key.clone()).unwrap();
        assert_eq!(first_inversion.inversion(), 1);
        assert_eq!(
            first_inversion
                .to_chord()
                .unwrap()
                .pitches()
                .into_iter()
                .map(|pitch| pitch.name())
                .collect::<Vec<_>>(),
            vec!["E", "G", "C"]
        );

        let diminished = RomanNumeral::new("viio7", key.clone()).unwrap();
        assert_eq!(diminished.degree(), 7);
        assert!(
            diminished
                .to_chord()
                .unwrap()
                .common_name()
                .contains("diminished")
        );

        let half_diminished = RomanNumeral::new("vii\u{00f8}7", key.clone()).unwrap();
        assert_eq!(half_diminished.degree(), 7);
        assert_eq!(half_diminished.accidental(), 0);
        assert!(
            half_diminished
                .to_chord()
                .unwrap()
                .common_name()
                .contains("half-diminished")
        );

        let borrowed = RomanNumeral::new("bII", key.clone()).unwrap();
        assert_eq!(borrowed.degree(), 2);
        assert_eq!(borrowed.accidental(), -1);

        let added_thirteenth = RomanNumeral::new("I add(13)", key.clone()).unwrap();
        assert_eq!(added_thirteenth.inversion(), 0);
        assert_eq!(
            added_thirteenth.to_chord().unwrap().common_name(),
            "major triad"
        );

        let augmented = RomanNumeral::new("III+", key).unwrap();
        assert_eq!(
            augmented
                .to_chord()
                .unwrap()
                .pitches()
                .into_iter()
                .map(|pitch| pitch.name())
                .collect::<Vec<_>>(),
            vec!["E", "G#", "B#"]
        );
    }

    #[test]
    fn roman_numerals_parse_augmented_sixth_figures() {
        let key = Key::from_tonic_mode("C", "minor").unwrap();
        let french = RomanNumeral::new("Fr+6", key).unwrap();
        assert_eq!(french.degree(), 6);
        assert_eq!(french.accidental(), -1);
        assert_eq!(
            french
                .to_chord()
                .unwrap()
                .pitches()
                .into_iter()
                .map(|pitch| pitch.name())
                .collect::<Vec<_>>(),
            vec!["A-", "C", "D", "F#"]
        );
    }

    #[test]
    fn roman_numerals_report_invalid_figures_and_empty_analysis() {
        let key = Key::from_tonic_mode("C", "major").unwrap();

        assert!(RomanNumeral::new("", key.clone()).is_err());
        assert!(RomanNumeral::new("Q", key.clone()).is_err());
        assert!(analyze_chord(&Chord::empty(), key).unwrap().is_none());
    }
}
