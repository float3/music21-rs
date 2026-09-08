pub(crate) mod chromaticinterval;
pub(crate) mod diatonicinterval;
pub(crate) mod direction;
pub(crate) mod genericinterval;
pub(crate) mod specifier;

pub use chromaticinterval::ChromaticInterval;
pub use diatonicinterval::DiatonicInterval;
pub use genericinterval::{GenericInterval, convert_generic};
pub use specifier::Specifier;

use direction::Direction;

use std::fmt;
use std::str::FromStr;
use std::{cmp::Ordering, sync::LazyLock};

use crate::common::numbertools::{MUSICAL_ORDINAL_STRINGS, MUSICAL_ORDINAL_STRINGS_LOWER};
use crate::common::stringtools::get_num_from_str;
use crate::error::{Error, Result};
use crate::{
    defaults::{FloatType, FractionType, IntegerType},
    fraction_pow::FractionPow,
    note::Note,
    pitch::Pitch,
};

/// Direction of a directed interval.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum IntervalDirection {
    /// The end pitch is lower than the start pitch.
    Descending = -1,
    /// The interval is an oblique unison.
    Oblique = 0,
    /// The end pitch is higher than the start pitch.
    Ascending = 1,
}

impl IntervalDirection {
    /// Returns `-1`, `0`, or `1` for descending, oblique, or ascending.
    pub fn as_int(self) -> IntegerType {
        self as IntegerType
    }

    /// Returns a display label for the direction.
    pub fn name(self) -> &'static str {
        match self {
            Self::Descending => "Descending",
            Self::Oblique => "Oblique",
            Self::Ascending => "Ascending",
        }
    }
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// A directed musical interval with diatonic spelling and chromatic size.
pub struct Interval {
    pub(crate) implicit_diatonic: bool,
    pub(crate) diatonic: DiatonicInterval,
    pub(crate) chromatic: ChromaticInterval,
    pitch_start: Option<Pitch>,
    pitch_end: Option<Pitch>,
}

impl PartialEq for Interval {
    /// Two intervals are the same when they are the same written distance
    /// and the same sounding one, which is music21's own comparison — the
    /// pitches an interval was built between are not part of what it is.
    fn eq(&self, other: &Self) -> bool {
        self.diatonic == other.diatonic && self.chromatic == other.chromatic
    }
}

pub(crate) enum PitchOrNote {
    Pitch(Pitch),
    Note(Note),
}

/// The pure fifths the Pythagorean walk steps by, parsed once rather than
/// re-parsed from "P5"/"-P5" on every call.
static PERFECT_FIFTH_UP: LazyLock<Interval> =
    LazyLock::new(|| Interval::from_name("P5").expect("P5 is a valid interval"));
static PERFECT_FIFTH_DOWN: LazyLock<Interval> =
    LazyLock::new(|| Interval::from_name("-P5").expect("-P5 is a valid interval"));

fn extract_pitch(arg: PitchOrNote) -> Pitch {
    match arg {
        PitchOrNote::Pitch(pitch) => pitch,
        PitchOrNote::Note(note) => note.pitch,
    }
}

fn strip_direction_word(value: &str, word: &str) -> (String, bool) {
    replace_case_insensitive(value, word, "", false, true)
}

fn replace_music_ordinal(value: &str, ordinal: &str, replacement: &str) -> (String, bool) {
    replace_case_insensitive(value, ordinal, replacement, true, true)
}

fn replace_case_insensitive(
    value: &str,
    needle: &str,
    replacement: &str,
    consume_leading_whitespace: bool,
    consume_trailing_whitespace: bool,
) -> (String, bool) {
    let needle_lower = needle.to_ascii_lowercase();
    let value_lower = value.to_ascii_lowercase();
    let mut output = String::with_capacity(value.len());
    let mut pos = 0;
    let mut replaced = false;

    while let Some(relative_start) = value_lower[pos..].find(&needle_lower) {
        let match_start = pos + relative_start;
        let match_end = match_start + needle.len();
        let mut copy_end = match_start;
        let mut next_pos = match_end;

        if consume_leading_whitespace {
            while copy_end > pos {
                let Some(ch) = value[pos..copy_end].chars().next_back() else {
                    break;
                };
                if !ch.is_whitespace() {
                    break;
                }
                copy_end -= ch.len_utf8();
            }
        }

        if consume_trailing_whitespace {
            while next_pos < value.len() {
                let Some(ch) = value[next_pos..].chars().next() else {
                    break;
                };
                if !ch.is_whitespace() {
                    break;
                }
                next_pos += ch.len_utf8();
            }
        }

        output.push_str(&value[pos..copy_end]);
        output.push_str(replacement);
        pos = next_pos;
        replaced = true;
    }

    if !replaced {
        return (value.to_string(), false);
    }

    output.push_str(&value[pos..]);
    (output, true)
}

fn convert_staff_distance_to_interval(staff_dist: IntegerType) -> IntegerType {
    match staff_dist.cmp(&0) {
        Ordering::Equal => 1,
        Ordering::Greater => staff_dist + 1,
        Ordering::Less => staff_dist - 1,
    }
}

/// music21's `convertStaffDistanceToInterval`: a signed count of staff steps
/// as a signed generic interval number, so `0` is a unison, `2` a third and
/// `-1` a descending second.
pub fn staff_distance_to_generic_number(staff_distance: IntegerType) -> IntegerType {
    convert_staff_distance_to_interval(staff_distance)
}

fn diatonic_note_number(pitch: &Pitch) -> IntegerType {
    pitch.step().step_to_dnn_offset() + (7 * pitch.octave().unwrap_or(4))
}

/// The pitch written higher on the staff, whatever it sounds like: music21's
/// `getWrittenHigherNote`, so `C4` over `B#3`. Ties in staff position are
/// settled by sound, and a complete tie returns the first.
pub fn written_higher_pitch<'a>(first: &'a Pitch, second: &'a Pitch) -> &'a Pitch {
    match diatonic_note_number(first).cmp(&diatonic_note_number(second)) {
        Ordering::Greater => first,
        Ordering::Less => second,
        Ordering::Equal => absolute_higher_pitch(first, second),
    }
}

/// The pitch written lower on the staff: music21's `getWrittenLowerNote`,
/// so `B#3` under `C4`.
pub fn written_lower_pitch<'a>(first: &'a Pitch, second: &'a Pitch) -> &'a Pitch {
    match diatonic_note_number(first).cmp(&diatonic_note_number(second)) {
        Ordering::Less => first,
        Ordering::Greater => second,
        Ordering::Equal => absolute_lower_pitch(first, second),
    }
}

/// The pitch that sounds higher: music21's `getAbsoluteHigherNote`. Enharmonic
/// equals return the first.
pub fn absolute_higher_pitch<'a>(first: &'a Pitch, second: &'a Pitch) -> &'a Pitch {
    if notes_to_chromatic(first, second).semitones > 0.0 {
        second
    } else {
        first
    }
}

/// The pitch that sounds lower: music21's `getAbsoluteLowerNote`. Enharmonic
/// equals return the first.
pub fn absolute_lower_pitch<'a>(first: &'a Pitch, second: &'a Pitch) -> &'a Pitch {
    if notes_to_chromatic(first, second).semitones < 0.0 {
        second
    } else {
        first
    }
}

/// The generic interval from one pitch to another, counted by staff
/// position: music21's `notesToGeneric`.
pub fn notes_to_generic(p1: &Pitch, p2: &Pitch) -> Result<GenericInterval> {
    let dnn1 = p1.step().step_to_dnn_offset() + (7 * p1.octave().unwrap_or(4));
    let dnn2 = p2.step().step_to_dnn_offset() + (7 * p2.octave().unwrap_or(4));
    let staff_dist = dnn2 - dnn1;
    GenericInterval::from_int(convert_staff_distance_to_interval(staff_dist))
}

/// The semitone distance from one pitch to another: music21's
/// `notesToChromatic`.
pub fn notes_to_chromatic(p1: &Pitch, p2: &Pitch) -> ChromaticInterval {
    ChromaticInterval::new(p2.ps() - p1.ps())
}

fn specifier_from_generic_chromatic(
    g_int: &GenericInterval,
    c_int: &ChromaticInterval,
) -> Result<Specifier> {
    let note_vals: [IntegerType; 7] = [0, 2, 4, 5, 7, 9, 11];
    let normal_semis = note_vals[(g_int.simple_undirected() - 1) as usize]
        + 12 * g_int.simple_steps_and_octaves().1;

    let c_direction = c_int.direction();

    let these_semis = if g_int.direction() != c_direction
        && g_int.direction() != direction::Direction::Oblique
        && c_direction != direction::Direction::Oblique
    {
        -c_int.undirected()
    } else if g_int.undirected() == 1 {
        c_int.directed()
    } else {
        c_int.undirected()
    };

    let rounding_error = if c_int.undirected() > 0.0 {
        0.0001
    } else {
        -0.0001
    };
    let diff = (these_semis + rounding_error).round() as IntegerType - normal_semis;

    if g_int.is_perfectable() {
        specifier_at(&PERFECTABLE_SPECIFIERS, 4 + diff, "Perfect", diff)
    } else {
        specifier_at(&MAJOR_SPECIFIERS, 5 + diff, "Major", diff)
    }
}

/// The qualities a perfectable interval takes, widest flat to widest sharp:
/// music21's `perfSpecifiers`, with `Perfect` in the middle.
const PERFECTABLE_SPECIFIERS: [Specifier; 9] = [
    Specifier::QuadrupleDiminished,
    Specifier::TripleDiminished,
    Specifier::DoubleDiminished,
    Specifier::Diminished,
    Specifier::Perfect,
    Specifier::Augmented,
    Specifier::DoubleAugmented,
    Specifier::TripleAugmented,
    Specifier::QuadrupleAugmented,
];

/// The same for an interval that is major or minor rather than perfect:
/// music21's `specifiers`.
const MAJOR_SPECIFIERS: [Specifier; 10] = [
    Specifier::QuadrupleDiminished,
    Specifier::TripleDiminished,
    Specifier::DoubleDiminished,
    Specifier::Diminished,
    Specifier::Minor,
    Specifier::Major,
    Specifier::Augmented,
    Specifier::DoubleAugmented,
    Specifier::TripleAugmented,
    Specifier::QuadrupleAugmented,
];

/// The quality at a place in one of those tables.
///
/// music21 indexes the table with a plain Python subscript, so a note flatter
/// than the widest diminished it can spell does not raise — the index goes
/// negative and Python counts back from the end, which answers an augmented
/// interval for a flattened one. It is a strange answer and it is the one
/// music21 gives, and the interval's own cent shift still says how far off
/// the note really is. Only an index off the sharp end raises, as it does
/// upstream.
fn specifier_at(
    table: &[Specifier],
    index: IntegerType,
    from: &str,
    diff: IntegerType,
) -> Result<Specifier> {
    let length = table.len() as IntegerType;
    let wrapped = if index < 0 { index + length } else { index };
    if index >= length || wrapped < 0 {
        return Err(Error::Interval(format!(
            "cannot get a specifier for a note with this many semitones off of {from}: {diff}"
        )));
    }
    Ok(table[wrapped as usize])
}

/// Reads the quality off a generic and a chromatic interval together:
/// music21's `intervalsToDiatonic`, so a third of four semitones is major.
pub fn intervals_to_diatonic(
    g_int: &GenericInterval,
    c_int: &ChromaticInterval,
) -> Result<DiatonicInterval> {
    let specifier = specifier_from_generic_chromatic(g_int, c_int)?;
    Ok(DiatonicInterval::new(specifier, g_int))
}

/// The simplest quality and generic size for a semitone count: music21's
/// `convertSemitoneToSpecifierGeneric`, so `6` is a diminished fifth and
/// `-14` a descending major ninth.
pub fn convert_semitone_to_specifier_generic(count: FloatType) -> (Specifier, IntegerType) {
    let (specifier, generic, _) = convert_semitone_to_specifier_generic_microtone(count);
    (specifier, generic)
}

/// The quality and generic size of a whole number of semitones within one
/// octave, music21's `SEMITONES_TO_SPEC_GENERIC` table.
fn semitones_to_specifier_generic(size: IntegerType) -> (Specifier, IntegerType) {
    match size {
        0 => (Specifier::Perfect, 1),
        1 => (Specifier::Minor, 2),
        2 => (Specifier::Major, 2),
        3 => (Specifier::Minor, 3),
        4 => (Specifier::Major, 3),
        5 => (Specifier::Perfect, 4),
        6 => (Specifier::Diminished, 5),
        7 => (Specifier::Perfect, 5),
        8 => (Specifier::Minor, 6),
        9 => (Specifier::Major, 6),
        10 => (Specifier::Minor, 7),
        _ => (Specifier::Major, 7),
    }
}

/// Like [`convert_semitone_to_specifier_generic`] for a fractional semitone
/// count, returning the leftover in cents as well: music21's
/// `convertSemitoneToSpecifierGenericMicrotone`, so `2.5` is a major second
/// and fifty cents, and `-2.5` a descending minor third and fifty.
pub fn convert_semitone_to_specifier_generic_microtone(
    count: FloatType,
) -> (Specifier, IntegerType, FloatType) {
    let dir_scale = if count < 0.0 { -1 } else { 1 };
    let mut whole = count.floor();
    let mut cents = (count - whole) * 100.0;
    if cents > 50.0 {
        cents -= 100.0;
        whole += 1.0;
    }
    let whole = whole as IntegerType;
    let size = whole.abs() % 12;
    let octave = whole.abs() / 12;
    let (specifier, generic) = semitones_to_specifier_generic(size);
    (specifier, (generic + octave * 7) * dir_scale, cents)
}

/// The step letter and octave of a diatonic note number, counting `C0` as
/// `1`: music21's `convertDiatonicNumberToStep`, so `15` is `C` in octave
/// `2`, `0` is `B` in octave `-1`, and `-19` is `D` in octave `-3`.
pub fn convert_diatonic_number_to_step(dn: IntegerType) -> (char, IntegerType) {
    const STEPS: [char; 7] = ['C', 'D', 'E', 'F', 'G', 'A', 'B'];
    let zero_based = dn - 1;
    let octave = zero_based.div_euclid(7);
    let step = STEPS[zero_based.rem_euclid(7) as usize];
    (step, octave)
}

/// Parses an interval quality from its prefix or spelled-out name: music21's
/// `parseSpecifier`, so `"P"`, `"perfect"` and `"Perfect"` all give
/// [`Specifier::Perfect`].
pub fn parse_specifier(value: &str) -> Result<Specifier> {
    Specifier::from_name(value)
}

impl Interval {
    pub(crate) fn between(start: PitchOrNote, end: PitchOrNote) -> Result<Self> {
        let start_pitch = extract_pitch(start);
        let end_pitch = extract_pitch(end);
        let generic = notes_to_generic(&start_pitch, &end_pitch)?;
        let chromatic = notes_to_chromatic(&start_pitch, &end_pitch);
        let diatonic = intervals_to_diatonic(&generic, &chromatic)?;

        Ok(Self {
            implicit_diatonic: false,
            diatonic,
            chromatic,
            pitch_start: Some(start_pitch),
            pitch_end: Some(end_pitch),
        })
    }

    /// Builds an interval from its two halves as given, without checking
    /// that they agree: music21's `Interval(diatonic=..., chromatic=...)`.
    pub fn from_diatonic_and_chromatic(
        diatonic: DiatonicInterval,
        chromatic: ChromaticInterval,
    ) -> Result<Interval> {
        Ok(Self {
            implicit_diatonic: false,
            diatonic,
            chromatic,
            pitch_start: None,
            pitch_end: None,
        })
    }

    /// Builds an interval from a diatonic interval alone, deriving the
    /// semitone count: music21's `Interval(diatonic=...)`.
    pub fn from_diatonic(diatonic: DiatonicInterval) -> Result<Self> {
        let chromatic = diatonic.get_chromatic()?;
        Self::from_diatonic_and_chromatic(diatonic, chromatic)
    }

    /// Builds an interval from a semitone count alone, spelling it the
    /// simplest way and marking the spelling as implicit: music21's
    /// `Interval(chromatic=...)`.
    pub fn from_chromatic(chromatic: ChromaticInterval) -> Result<Self> {
        let diatonic = chromatic.get_diatonic();
        let mut interval = Self::from_diatonic_and_chromatic(diatonic, chromatic)?;
        interval.implicit_diatonic = true;
        Ok(interval)
    }

    /// The generic half of the interval.
    pub fn generic(&self) -> &GenericInterval {
        &self.diatonic.generic
    }

    /// The diatonic half of the interval: quality plus generic size.
    pub fn diatonic(&self) -> &DiatonicInterval {
        &self.diatonic
    }

    /// The chromatic half of the interval: the semitone count.
    pub fn chromatic(&self) -> &ChromaticInterval {
        &self.chromatic
    }

    /// The quality.
    pub fn specifier(&self) -> Specifier {
        self.diatonic.specifier
    }

    /// Builds an interval from a generic size and a semitone count: music21's
    /// `intervalFromGenericAndChromatic`, so a third of four semitones is a
    /// major third and a fifth of six a diminished fifth. Negative values
    /// give a descending interval.
    pub fn from_generic_and_chromatic(
        generic: IntegerType,
        semitones: IntegerType,
    ) -> Result<Self> {
        let generic = GenericInterval::from_int(generic)?;
        let chromatic = ChromaticInterval::from_int(semitones);
        let diatonic = intervals_to_diatonic(&generic, &chromatic)?;
        Self::from_diatonic_and_chromatic(diatonic, chromatic)
    }

    /// The pitch the interval was measured from, when it was built from two
    /// pitches or notes.
    pub fn pitch_start(&self) -> Option<&Pitch> {
        self.pitch_start.as_ref()
    }

    /// The pitch the interval was measured to, when it was built from two
    /// pitches or notes.
    pub fn pitch_end(&self) -> Option<&Pitch> {
        self.pitch_end.as_ref()
    }

    /// [`Self::pitch_start`] as a note without a duration.
    pub fn note_start(&self) -> Option<Note> {
        self.pitch_start.clone().map(Note::from_pitch)
    }

    /// [`Self::pitch_end`] as a note without a duration.
    pub fn note_end(&self) -> Option<Note> {
        self.pitch_end.clone().map(Note::from_pitch)
    }

    /// Parses an interval name such as `"M3"`, `"P5"`, or `"-m6"`.
    pub fn from_name(name: impl Into<String>) -> Result<Self> {
        let (diatonic, chromatic, inferred) = parse_interval_name(name.into())?;
        Ok(Self {
            implicit_diatonic: inferred,
            diatonic,
            chromatic,
            pitch_start: None,
            pitch_end: None,
        })
    }

    /// Creates an implicit diatonic interval from a chromatic semitone count.
    pub fn from_semitones(semitones: IntegerType) -> Result<Self> {
        let chromatic = ChromaticInterval::from_int(semitones);
        let diatonic = chromatic.get_diatonic();
        Ok(Self {
            implicit_diatonic: true,
            diatonic,
            chromatic,
            pitch_start: None,
            pitch_end: None,
        })
    }

    /// Returns the directed interval from `start` to `end`.
    pub fn between_pitches(start: &Pitch, end: &Pitch) -> Result<Self> {
        Self::between(
            PitchOrNote::Pitch(start.clone()),
            PitchOrNote::Pitch(end.clone()),
        )
    }

    /// Returns the directed interval from `start` to `end`.
    pub fn between_notes(start: &Note, end: &Note) -> Result<Self> {
        Self::between(
            PitchOrNote::Note(start.clone()),
            PitchOrNote::Note(end.clone()),
        )
    }

    /// Returns the directed chromatic size in semitones, fractional for a
    /// microtonal interval.
    pub fn semitones(&self) -> FloatType {
        self.chromatic.semitones
    }

    /// Returns the directed chromatic size rounded to whole semitones.
    pub fn whole_semitones(&self) -> IntegerType {
        self.chromatic.whole_semitones()
    }

    /// Returns the directed interval direction.
    pub fn direction(&self) -> IntervalDirection {
        self.chromatic.direction()
    }

    /// Returns the human-readable interval name, such as `"Major Third"`.
    pub fn name(&self) -> String {
        self.nice_name()
    }

    /// Returns the simple or compound generic interval number.
    pub fn generic_number(&self) -> IntegerType {
        self.generic().simple_directed()
    }

    /// Returns `true` when the interval was inferred from semitones only.
    pub fn is_implicit_diatonic(&self) -> bool {
        self.implicit_diatonic
    }

    /// Returns the complementary interval inversion.
    pub fn inversion(&self) -> Result<Self> {
        let direction = match self.direction() {
            IntervalDirection::Oblique => 1,
            direction => direction.as_int(),
        };
        let simple = self.generic().simple_undirected();
        let inverted_generic = if simple == 1 { 1 } else { 9 - simple };
        let generic = GenericInterval::from_int(inverted_generic * direction)?;
        let diatonic = DiatonicInterval::new(self.diatonic.specifier.inversion(), &generic);
        let chromatic = diatonic.get_chromatic()?;
        Self::from_diatonic_and_chromatic(diatonic, chromatic)
    }

    /// Returns the same interval in the opposite direction.
    pub fn reversed(&self) -> Result<Self> {
        self.reverse()
    }

    /// Returns the Pythagorean tuning ratio for this interval.
    ///
    /// The ratio is expressed as a rational fraction built from pure fifths,
    /// matching the helper music21 uses for enharmonic scoring.
    pub fn pythagorean_ratio(&self) -> Result<FractionType> {
        interval_to_pythagorean_ratio(self)
    }

    /// Transposes a pitch by this interval.
    pub fn transpose_pitch(&self, pitch: &Pitch) -> Result<Pitch> {
        self.transpose_pitch_with_options(pitch, false, Some(4))
    }

    /// Transposes a note by this interval.
    pub fn transpose_note(&self, note: &Note) -> Result<Note> {
        let mut out = note.clone();
        out.pitch = self.transpose_pitch(&note.pitch)?;
        Ok(out)
    }

    /// Returns music21's compact undirected name, such as `"P5"` or `"m3"`.
    pub fn short_name(&self) -> String {
        format!(
            "{}{}",
            self.diatonic.specifier.prefix(),
            self.generic().undirected()
        )
    }

    /// Returns the compact name folded into one octave, so a ninth is `"M2"`.
    pub fn simple_name(&self) -> String {
        format!(
            "{}{}",
            self.diatonic.specifier.prefix(),
            self.generic().simple_undirected()
        )
    }

    /// Returns the compact name folded into one octave, except that octaves
    /// stay `8` rather than becoming unisons.
    pub fn semi_simple_name(&self) -> String {
        format!(
            "{}{}",
            self.diatonic.specifier.prefix(),
            self.generic().semi_simple_undirected()
        )
    }

    /// Returns the compact name with music21's direction sign, such as `"m-6"`.
    pub fn directed_name(&self) -> String {
        format!(
            "{}{}",
            self.diatonic.specifier.prefix(),
            self.generic().directed()
        )
    }

    /// Returns whether this is a second of any quality.
    pub fn is_diatonic_step(&self) -> bool {
        self.generic().undirected() == 2
    }

    /// Returns whether this spans exactly one semitone.
    pub fn is_chromatic_step(&self) -> bool {
        self.chromatic.undirected() == 1.0
    }

    /// Returns whether this is a step diatonically or chromatically.
    pub fn is_step(&self) -> bool {
        self.is_chromatic_step() || self.is_diatonic_step()
    }

    /// Returns whether this is larger than a second. Unisons are neither
    /// steps nor skips.
    pub fn is_skip(&self) -> bool {
        self.generic().undirected() > 2
    }

    /// Returns whether this is a common-practice consonance: a perfect unison
    /// or fifth, or a major or minor third or sixth, in any octave.
    pub fn is_consonant(&self) -> bool {
        matches!(
            (self.diatonic.specifier, self.generic().simple_undirected()),
            (Specifier::Perfect, 1 | 5) | (Specifier::Major | Specifier::Minor, 3 | 6)
        )
    }

    /// Returns the interval that completes this one to an octave, so a major
    /// third becomes a minor sixth and an octave becomes a unison.
    pub fn complement(&self) -> Result<Self> {
        let generic = GenericInterval::from_int(9 - self.generic().semi_simple_undirected())?;
        let diatonic = DiatonicInterval::new(self.diatonic.specifier.inversion(), &generic);
        let chromatic = diatonic.get_chromatic()?;
        Self::from_diatonic_and_chromatic(diatonic, chromatic)
    }

    /// Returns the interval class, the smaller of the semitone count within an
    /// octave and its complement, from `0` to `6`.
    pub fn interval_class(&self) -> IntegerType {
        self.chromatic.interval_class()
    }

    /// Adds intervals end to end, as music21's `interval.add` does.
    ///
    /// Direction matters: a perfect fifth followed by a descending perfect
    /// fourth is a major second.
    pub fn sum<'a>(intervals: impl IntoIterator<Item = &'a Interval>) -> Result<Self> {
        let start = Pitch::from_name("C4")?;
        let mut end = start.clone();
        let mut any = false;
        for interval in intervals {
            end = interval.transpose_pitch(&end)?;
            any = true;
        }
        if !any {
            return Err(Error::Interval(
                "cannot add an empty set of intervals".to_string(),
            ));
        }
        Self::between_pitches(&start, &end)
    }

    /// Subtracts every following interval from the first, as music21's
    /// `interval.subtract` does.
    pub fn difference<'a>(intervals: impl IntoIterator<Item = &'a Interval>) -> Result<Self> {
        let start = Pitch::from_name("C4")?;
        let mut intervals = intervals.into_iter();
        let Some(first) = intervals.next() else {
            return Err(Error::Interval(
                "cannot subtract an empty set of intervals".to_string(),
            ));
        };
        let mut end = first.transpose_pitch(&start)?;
        for interval in intervals {
            end = interval.reversed()?.transpose_pitch(&end)?;
        }
        Self::between_pitches(&start, &end)
    }

    /// Returns the compact name folded into one octave but keeping the
    /// direction sign, such as `"A-6"` for a descending augmented thirteenth.
    pub fn directed_simple_name(&self) -> String {
        format!(
            "{}{}",
            self.diatonic.specifier.prefix(),
            self.generic().simple_directed()
        )
    }

    pub(crate) fn directed_simple_key(&self) -> (Specifier, IntegerType) {
        (self.diatonic.specifier, self.generic().simple_directed())
    }

    pub(crate) fn simple_key(&self) -> (Specifier, IntegerType) {
        (self.diatonic.specifier, self.generic().simple_undirected())
    }

    pub(crate) fn semi_simple_key(&self) -> (Specifier, IntegerType) {
        (
            self.diatonic.specifier,
            self.generic().semi_simple_undirected(),
        )
    }

    pub(crate) fn is_perfect_unison(&self) -> bool {
        self.generic().undirected() == 1 && self.chromatic.semitones == 0.0
    }

    pub(crate) fn nice_name(&self) -> String {
        self.diatonic.nice_name()
    }

    /// The spelled-out name with compound intervals folded to an octave at
    /// most: music21's `semiSimpleNiceName`, so a ninth is `Minor Second`
    /// but an octave stays `Perfect Octave`.
    pub fn semi_simple_nice_name(&self) -> String {
        self.diatonic.semi_simple_nice_name()
    }

    /// The spelled-out name within one octave: music21's `simpleNiceName`,
    /// so both a ninth and a sixteenth are `Major Second` and an octave is a
    /// `Perfect Unison`.
    pub fn simple_nice_name(&self) -> String {
        format!(
            "{} {}",
            self.diatonic.specifier.nice_name(),
            self.generic().simple_nice_name()
        )
    }

    /// The direction music21's `DiatonicInterval` reports, which differs from
    /// the generic direction only on altered unisons: a diminished unison is
    /// always `Descending` and an augmented one always `Ascending`, whatever
    /// sign the interval was written with.
    fn diatonic_direction(&self) -> Direction {
        self.diatonic.direction()
    }

    fn directed(&self, name: String) -> String {
        format!("{} {name}", self.diatonic_direction().name())
    }

    /// The spelled-out name with its direction: music21's `directedNiceName`,
    /// `Descending Major Third`.
    pub fn directed_nice_name(&self) -> String {
        self.directed(self.nice_name())
    }

    /// [`Self::simple_nice_name`] with its direction: music21's
    /// `directedSimpleNiceName`.
    pub fn directed_simple_nice_name(&self) -> String {
        self.directed(self.simple_nice_name())
    }

    /// [`Self::semi_simple_nice_name`] with its direction: music21's
    /// `directedSemiSimpleNiceName`.
    pub fn directed_semi_simple_nice_name(&self) -> String {
        self.directed(self.semi_simple_nice_name())
    }

    /// The specifier spelled out on its own: music21's `specificName`,
    /// `Doubly-Diminished` for `dd5`.
    pub fn specific_name(&self) -> String {
        self.diatonic.specifier.nice_name()
    }

    /// The chromatic size in cents, signed.
    pub fn cents(&self) -> FloatType {
        self.chromatic.cents()
    }

    /// How far the chromatic size runs past the diatonic spelling, in cents:
    /// music21's `_diatonicIntervalCentShift`, `-50.0` for the augmented
    /// unison between `C1` and a `C1` half-sharp.
    pub fn diatonic_interval_cent_shift(&self) -> FloatType {
        let diatonic_cents = self.diatonic.cents().unwrap_or(0.0);
        self.chromatic.cents() - diatonic_cents
    }

    /// Whether the generic interval is a unison, whatever its quality.
    pub fn is_unison(&self) -> bool {
        self.generic().is_unison()
    }

    /// Whether the generic interval takes perfect rather than major and
    /// minor qualities: unisons, fourths, fifths and their compounds.
    pub fn is_perfectable(&self) -> bool {
        self.generic().is_perfectable()
    }

    /// The signed number of staff steps: music21's `generic.staffDistance`,
    /// `0` for a unison, `2` for a third and `-4` for a descending fifth.
    pub fn staff_distance(&self) -> IntegerType {
        self.generic().staff_distance()
    }

    /// The simple generic size from one to seven, with descending intervals
    /// inverted: music21's `generic.mod7`, so a descending third is `6`.
    pub fn mod7(&self) -> IntegerType {
        self.generic().mod7()
    }

    /// The generic size of the inversion within an octave: music21's
    /// `generic.mod7inversion`, `6` for a third and `1` for an octave.
    pub fn mod7_inversion(&self) -> IntegerType {
        self.generic().mod7_inversion()
    }

    /// The semitones reduced to a pitch class, `0` to `11`: music21's
    /// `chromatic.mod12`, so a descending major third is `8`.
    pub fn mod12(&self) -> IntegerType {
        self.chromatic.mod12()
    }

    /// Moves a pitch by the interval the way music21's `transposePitch`
    /// does, with its keyword arguments: `reverse` transposes by the
    /// reversed interval, and `max_accidental` respells any result carrying
    /// more accidentals than that (music21's default is `Some(4)`), `None`
    /// meaning no limit.
    pub fn transpose_pitch_with_options(
        &self,
        p: &Pitch,
        reverse: bool,
        max_accidental: Option<IntegerType>,
    ) -> Result<Pitch> {
        if reverse {
            return self
                .reverse()?
                .transpose_pitch_with_options(p, false, max_accidental);
        }
        if self.implicit_diatonic {
            return self.chromatic.transpose_pitch(p);
        }

        let use_implicit_octave = p.octave().is_none();
        let inherit_accidental_display = self.diatonic.simple_name() == "P1";
        let cents_origin = if p.is_twelve_tone() {
            0.0
        } else {
            p.microtone().map_or(0.0, crate::pitch::Microtone::cents)
        };
        let new_dnn = p.diatonic_note_number() + self.diatonic.generic.staff_distance();
        let (new_step, new_octave) = convert_diatonic_number_to_step(new_dnn);
        let mut pitch2 = crate::pitch::PitchOptions::new()
            .step(new_step)
            .octave(new_octave)
            .build()?;
        let origin_ps = p.ps() - cents_origin / 100.0;
        let mut half_steps_to_fix = self.chromatic.semitones - (pitch2.ps() - origin_ps);
        while half_steps_to_fix >= 12.0 {
            half_steps_to_fix -= 12.0;
            pitch2.octave_setter(Some(pitch2.octave().unwrap_or(4) - 1));
        }
        while half_steps_to_fix <= -12.0 {
            half_steps_to_fix += 12.0;
            pitch2.octave_setter(Some(pitch2.octave().unwrap_or(4) + 1));
        }
        if half_steps_to_fix != 0.0 {
            if max_accidental.is_some_and(|limit| half_steps_to_fix.abs() > limit as FloatType) {
                pitch2.set_ps(pitch2.ps() + half_steps_to_fix);
            } else {
                pitch2.set_accidental_alter(half_steps_to_fix)?;
            }
            match (
                inherit_accidental_display,
                pitch2.has_accidental(),
                p.explicit_accidental(),
            ) {
                (false, true, Some(source)) => {
                    if let Some(target) = pitch2.explicit_accidental_mut() {
                        target.inherit_display(source);
                        target.set_display_status(None);
                    }
                }
                (true, false, Some(source)) => {
                    let mut natural = crate::pitch::Accidental::natural();
                    natural.inherit_display(source);
                    pitch2.set_accidental(Some(natural));
                }
                (true, true, Some(source)) => {
                    if let Some(target) = pitch2.explicit_accidental_mut() {
                        target.inherit_display(source);
                    }
                }
                (true, true, None) => {
                    if let Some(target) = pitch2.explicit_accidental_mut() {
                        target.set_display_status(Some(false));
                    }
                }
                _ => {}
            }
        } else if inherit_accidental_display
            && p.explicit_accidental()
                .is_some_and(|accidental| accidental.name() == "natural")
        {
            pitch2.set_accidental(p.explicit_accidental().cloned());
        }
        if cents_origin != 0.0 {
            let cents = pitch2
                .microtone()
                .map_or(0.0, crate::pitch::Microtone::cents);
            pitch2.set_microtone_cents(cents + cents_origin)?;
        }
        if use_implicit_octave {
            pitch2.octave_setter(None);
        }
        Ok(pitch2)
    }

    /// Transposes a pitch in place by this interval.
    pub fn transpose_pitch_in_place(&self, pitch: &mut Pitch) -> Result<()> {
        *pitch = self.transpose_pitch(pitch)?;
        Ok(())
    }
}

impl fmt::Display for Interval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let shift = self.diatonic_interval_cent_shift();
        if shift == 0.0 {
            write!(f, "{}", self.directed_name())
        } else {
            write!(
                f,
                "{} {}",
                self.directed_name(),
                crate::pitch::Microtone::from_cents(shift, 1)
            )
        }
    }
}

impl FromStr for Interval {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        Self::from_name(value)
    }
}

impl TryFrom<&str> for Interval {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self> {
        Self::from_name(value)
    }
}

impl TryFrom<String> for Interval {
    type Error = Error;

    fn try_from(value: String) -> Result<Self> {
        Self::from_name(value)
    }
}

impl TryFrom<IntegerType> for Interval {
    type Error = Error;

    fn try_from(value: IntegerType) -> Result<Self> {
        Self::from_semitones(value)
    }
}

fn parse_interval_name(mut value: String) -> Result<(DiatonicInterval, ChromaticInterval, bool)> {
    let mut inferred = false;
    let mut dir_scale = 1;

    // Check for '-' and remove them:
    if value.contains('-') {
        value = value.replace('-', "");
        dir_scale = -1;
    }
    // Remove directional words:
    {
        let (without_descending, found_descending) = strip_direction_word(&value, "descending");
        if found_descending {
            value = without_descending;
            dir_scale = -1;
        } else {
            let (without_ascending, found_ascending) = strip_direction_word(&value, "ascending");
            if found_ascending {
                value = without_ascending;
            }
        }
    }
    let value_lower = value.to_lowercase();

    // Handle whole/half abbreviations:
    if value_lower == "w" || value_lower == "whole" || value_lower == "tone" {
        value = "M2".to_string();
        inferred = true;
    } else if value_lower == "h" || value_lower == "half" || value_lower == "semitone" {
        value = "m2".to_string();
        inferred = true;
    }

    // Replace any music ordinal in the string with its index. Almost no name
    // holds one, and the replacement lowercases both sides every time it is
    // called, so the cheap containment test in front of it is what keeps
    // `P5` from allocating three strings per candidate, twenty-three times.
    let mut lowered = value.to_ascii_lowercase();
    for (i, ordinal) in MUSICAL_ORDINAL_STRINGS_LOWER.iter().enumerate() {
        if !lowered.contains(ordinal.as_str()) {
            continue;
        }
        let replacement = i.to_string();
        let (next_value, replaced) =
            replace_music_ordinal(&value, &MUSICAL_ORDINAL_STRINGS[i], &replacement);
        if replaced {
            value = next_value;
            lowered = value.to_ascii_lowercase();
        }
    }

    // Extract number and remaining spec:
    let (found, remain) = get_num_from_str(&value, "0123456789");
    let generic_number: IntegerType = found
        .parse::<IntegerType>()
        .map_err(|_| Error::Interval(format!("cannot read an interval number from {value:?}")))?
        * dir_scale;
    let spec = Specifier::parse(&remain)?;

    let g_interval = GenericInterval::from_int(generic_number)?;
    let d_interval = g_interval.get_diatonic(spec);
    let c_interval = d_interval.get_chromatic()?;
    Ok((d_interval, c_interval, inferred))
}

impl Interval {
    fn reverse(&self) -> Result<Self> {
        if let (Some(start), Some(end)) = (&self.pitch_start, &self.pitch_end) {
            Interval::between(
                PitchOrNote::Pitch(end.clone()),
                PitchOrNote::Pitch(start.clone()),
            )
        } else {
            Interval::from_diatonic_and_chromatic(self.diatonic.reverse(), self.chromatic.reverse())
        }
    }
}

pub(crate) fn interval_to_pythagorean_ratio(interval: &Interval) -> Result<FractionType> {
    let start_pitch = Pitch::from_name("C1")?;

    let end_pitch_wanted = interval.transpose_pitch_with_options(&start_pitch, false, Some(4))?;

    let wanted_name = end_pitch_wanted.name();

    let mut end_pitch_up = start_pitch.clone();
    let mut end_pitch_down = start_pitch.clone();
    let mut found: Option<(Pitch, FractionType)> = None;
    let fifth_up: &Interval = &PERFECT_FIFTH_UP;
    let fifth_down: &Interval = &PERFECT_FIFTH_DOWN;

    for counter in 0..37 {
        if end_pitch_up.name() == wanted_name {
            if counter > 18 {
                return Err(Error::Interval(format!(
                    "pythagorean ratio for {wanted_name} exceeds integer range"
                )));
            }
            found = Some((
                end_pitch_up.clone(),
                FractionPow::<IntegerType>::powi(&FractionType::new(3i32, 2i32), counter),
            ));
            break;
        } else if end_pitch_down.name() == wanted_name {
            if counter > 18 {
                return Err(Error::Interval(format!(
                    "pythagorean ratio for {wanted_name} exceeds integer range"
                )));
            }
            found = Some((
                end_pitch_down.clone(),
                FractionPow::<IntegerType>::powi(&FractionType::new(2i32, 3i32), counter),
            ));
            break;
        } else {
            end_pitch_up = fifth_up.transpose_pitch_with_options(&end_pitch_up, false, Some(4))?;
            end_pitch_down =
                fifth_down.transpose_pitch_with_options(&end_pitch_down, false, Some(4))?;
        }
    }

    let (found_pitch, found_ratio) = match found {
        Some(val) => val,
        None => {
            return Err(Error::Interval(format!(
                "Could not find a pythagorean ratio for {interval:?}"
            )));
        }
    };

    let octaves = (end_pitch_wanted.ps() - found_pitch.ps()) / 12.0;
    let octave_multiplier =
        FractionPow::<IntegerType>::powi(&FractionType::new(2i32, 1i32), octaves as IntegerType);

    Ok(found_ratio * octave_multiplier)
}

#[cfg(test)]
mod tests {

    #[test]
    fn a_note_flatter_than_the_table_wraps_the_way_music21_does() {
        // music21 subscripts its own quality table with a plain index, so a
        // fifth five semitones flat comes back quadruply *augmented* with a
        // nine-hundred-cent shift saying how far off it really is.
        let interval = Interval::between_pitches(
            &Pitch::from_name("A##3").unwrap(),
            &Pitch::from_name("E---4").unwrap(),
        )
        .unwrap();
        assert_eq!(interval.short_name(), "AAAA5");
        assert_eq!(interval.chromatic().semitones(), 2.0);

        // The sharp end of the table is reached but never passed, since the
        // widest accidental a pitch can spell stops short of it.
        let widest = Interval::between_pitches(
            &Pitch::from_name("C4").unwrap(),
            &Pitch::from_name("E####4").unwrap(),
        )
        .unwrap();
        assert_eq!(widest.short_name(), "AAAA3");
    }

    #[test]
    fn generic_and_chromatic_build_the_interval_music21_does() {
        let cases: [(i32, i32, &str, &str); 14] = [
            (3, 4, "M3", "M3"),
            (3, 3, "m3", "m3"),
            (5, 7, "P5", "P5"),
            (5, 6, "d5", "d5"),
            (4, 6, "A4", "A4"),
            (1, 0, "P1", "P1"),
            (1, 1, "A1", "A1"),
            (8, 12, "P8", "P8"),
            (-3, -4, "M3", "M-3"),
            (2, 3, "A2", "A2"),
            (7, 10, "m7", "m7"),
            (-2, -1, "m2", "m-2"),
            (9, 13, "m9", "m9"),
            (3, 2, "d3", "d3"),
        ];
        for (generic, semitones, name, directed) in cases {
            let interval = Interval::from_generic_and_chromatic(generic, semitones).unwrap();
            assert_eq!(interval.short_name(), name, "{generic} {semitones}");
            assert_eq!(interval.directed_name(), directed, "{generic} {semitones}");
            assert_eq!(
                interval.semitones(),
                FloatType::from(semitones),
                "{generic} {semitones}"
            );
            assert!(interval.pitch_start().is_none());
        }
        assert!(Interval::from_generic_and_chromatic(0, 0).is_err());
        assert_eq!(
            [-3, -1, 0, 1, 2, 7].map(staff_distance_to_generic_number),
            [-4, -2, 1, 2, 3, 8]
        );
    }

    #[test]
    fn written_and_sounding_order_match_music21() {
        let cases: [(&str, &str, [&str; 4]); 8] = [
            ("C4", "E4", ["E4", "C4", "E4", "C4"]),
            ("E4", "C4", ["E4", "C4", "E4", "C4"]),
            ("B#3", "C4", ["C4", "B#3", "B#3", "B#3"]),
            ("C4", "B#3", ["C4", "B#3", "C4", "C4"]),
            ("C-4", "B3", ["C-4", "B3", "C-4", "C-4"]),
            ("F#4", "G-4", ["G-4", "F#4", "F#4", "F#4"]),
            ("C4", "C4", ["C4", "C4", "C4", "C4"]),
            ("B3", "C-4", ["C-4", "B3", "B3", "B3"]),
        ];
        for (first, second, expected) in cases {
            let a = Pitch::from_name(first).unwrap();
            let b = Pitch::from_name(second).unwrap();
            let actual = [
                written_higher_pitch(&a, &b),
                written_lower_pitch(&a, &b),
                absolute_higher_pitch(&a, &b),
                absolute_lower_pitch(&a, &b),
            ]
            .map(Pitch::name_with_octave);
            assert_eq!(actual, expected, "{first} {second}");
        }
    }

    #[test]
    fn intervals_remember_the_pitches_they_were_measured_between() {
        let c = Pitch::from_name("C4").unwrap();
        let g = Pitch::from_name("G4").unwrap();
        let fifth = Interval::between_pitches(&c, &g).unwrap();
        assert_eq!(fifth.pitch_start().unwrap().name_with_octave(), "C4");
        assert_eq!(fifth.pitch_end().unwrap().name_with_octave(), "G4");
        assert_eq!(fifth.note_start().unwrap().pitch_name_with_octave(), "C4");
        assert_eq!(fifth.note_end().unwrap().pitch_name_with_octave(), "G4");
        let named = Interval::from_name("P5").unwrap();
        assert!(named.pitch_start().is_none());
        assert!(named.note_end().is_none());
    }

    #[test]
    fn nice_name_variants_match_music21() {
        let cases: [(&str, [&str; 7]); 15] = [
            (
                "P1",
                [
                    "Perfect Unison",
                    "Oblique Perfect Unison",
                    "Perfect Unison",
                    "Perfect Unison",
                    "Oblique Perfect Unison",
                    "Oblique Perfect Unison",
                    "Perfect",
                ],
            ),
            (
                "m2",
                [
                    "Minor Second",
                    "Ascending Minor Second",
                    "Minor Second",
                    "Minor Second",
                    "Ascending Minor Second",
                    "Ascending Minor Second",
                    "Minor",
                ],
            ),
            (
                "P8",
                [
                    "Perfect Octave",
                    "Ascending Perfect Octave",
                    "Perfect Unison",
                    "Perfect Octave",
                    "Ascending Perfect Unison",
                    "Ascending Perfect Octave",
                    "Perfect",
                ],
            ),
            (
                "m9",
                [
                    "Minor Ninth",
                    "Ascending Minor Ninth",
                    "Minor Second",
                    "Minor Second",
                    "Ascending Minor Second",
                    "Ascending Minor Second",
                    "Minor",
                ],
            ),
            (
                "M10",
                [
                    "Major Tenth",
                    "Ascending Major Tenth",
                    "Major Third",
                    "Major Third",
                    "Ascending Major Third",
                    "Ascending Major Third",
                    "Major",
                ],
            ),
            (
                "P12",
                [
                    "Perfect Twelfth",
                    "Ascending Perfect Twelfth",
                    "Perfect Fifth",
                    "Perfect Fifth",
                    "Ascending Perfect Fifth",
                    "Ascending Perfect Fifth",
                    "Perfect",
                ],
            ),
            (
                "-M3",
                [
                    "Major Third",
                    "Descending Major Third",
                    "Major Third",
                    "Major Third",
                    "Descending Major Third",
                    "Descending Major Third",
                    "Major",
                ],
            ),
            (
                "-m9",
                [
                    "Minor Ninth",
                    "Descending Minor Ninth",
                    "Minor Second",
                    "Minor Second",
                    "Descending Minor Second",
                    "Descending Minor Second",
                    "Minor",
                ],
            ),
            (
                "dd5",
                [
                    "Doubly-Diminished Fifth",
                    "Ascending Doubly-Diminished Fifth",
                    "Doubly-Diminished Fifth",
                    "Doubly-Diminished Fifth",
                    "Ascending Doubly-Diminished Fifth",
                    "Ascending Doubly-Diminished Fifth",
                    "Doubly-Diminished",
                ],
            ),
            (
                "AA4",
                [
                    "Doubly-Augmented Fourth",
                    "Ascending Doubly-Augmented Fourth",
                    "Doubly-Augmented Fourth",
                    "Doubly-Augmented Fourth",
                    "Ascending Doubly-Augmented Fourth",
                    "Ascending Doubly-Augmented Fourth",
                    "Doubly-Augmented",
                ],
            ),
            (
                "P15",
                [
                    "Perfect Double-octave",
                    "Ascending Perfect Double-octave",
                    "Perfect Unison",
                    "Perfect Octave",
                    "Ascending Perfect Unison",
                    "Ascending Perfect Octave",
                    "Perfect",
                ],
            ),
            (
                "d1",
                [
                    "Diminished Unison",
                    "Descending Diminished Unison",
                    "Diminished Unison",
                    "Diminished Unison",
                    "Descending Diminished Unison",
                    "Descending Diminished Unison",
                    "Diminished",
                ],
            ),
            (
                "A1",
                [
                    "Augmented Unison",
                    "Ascending Augmented Unison",
                    "Augmented Unison",
                    "Augmented Unison",
                    "Ascending Augmented Unison",
                    "Ascending Augmented Unison",
                    "Augmented",
                ],
            ),
            (
                "-A1",
                [
                    "Augmented Unison",
                    "Ascending Augmented Unison",
                    "Augmented Unison",
                    "Augmented Unison",
                    "Ascending Augmented Unison",
                    "Ascending Augmented Unison",
                    "Augmented",
                ],
            ),
            (
                "P-8",
                [
                    "Perfect Octave",
                    "Descending Perfect Octave",
                    "Perfect Unison",
                    "Perfect Octave",
                    "Descending Perfect Unison",
                    "Descending Perfect Octave",
                    "Perfect",
                ],
            ),
        ];
        for (name, expected) in cases {
            let interval = Interval::from_name(name).unwrap();
            let actual = [
                interval.name(),
                interval.directed_nice_name(),
                interval.simple_nice_name(),
                interval.semi_simple_nice_name(),
                interval.directed_simple_nice_name(),
                interval.directed_semi_simple_nice_name(),
                interval.specific_name(),
            ];
            assert_eq!(actual, expected, "{name}");
        }
    }

    #[test]
    #[allow(clippy::type_complexity)]
    fn generic_and_chromatic_helpers_match_music21() {
        let cases: [(&str, f64, bool, bool, i32, i32, i32, i32); 14] = [
            ("P1", 0.0, true, true, 0, 1, 8, 0),
            ("m2", 100.0, false, false, 1, 2, 7, 1),
            ("P4", 500.0, false, true, 3, 4, 5, 5),
            ("P5", 700.0, false, true, 4, 5, 4, 7),
            ("M7", 1100.0, false, false, 6, 7, 2, 11),
            ("P8", 1200.0, false, true, 7, 1, 1, 0),
            ("m9", 1300.0, false, false, 8, 2, 7, 1),
            ("-M3", -400.0, false, false, -2, 6, 6, 8),
            ("-P5", -700.0, false, true, -4, 4, 4, 5),
            ("-m9", -1300.0, false, false, -8, 7, 7, 11),
            ("P15", 2400.0, false, true, 14, 1, 1, 0),
            ("d1", -100.0, true, true, 0, 1, 8, 11),
            ("-A1", -100.0, true, true, 0, 8, 8, 11),
            ("P-8", -1200.0, false, true, -7, 1, 1, 0),
        ];
        for (name, cents, unison, perfectable, staff, mod7, mod7_inversion, mod12) in cases {
            let interval = Interval::from_name(name).unwrap();
            assert_eq!(interval.cents(), cents, "{name} cents");
            assert_eq!(interval.is_unison(), unison, "{name} unison");
            assert_eq!(interval.is_perfectable(), perfectable, "{name} perfectable");
            assert_eq!(interval.staff_distance(), staff, "{name} staff distance");
            assert_eq!(interval.mod7(), mod7, "{name} mod7");
            assert_eq!(
                interval.mod7_inversion(),
                mod7_inversion,
                "{name} mod7 inversion"
            );
            assert_eq!(interval.mod12(), mod12, "{name} mod12");
        }
    }
    use super::*;

    fn pitch(name: &str) -> Pitch {
        Pitch::from_name(name).expect("valid pitch")
    }

    #[test]
    fn malformed_interval_names_error_instead_of_panicking() {
        // Regression: the generic number was pulled out of the string with
        // `.expect("Failed to parse number")`, so any name with no digits in it
        // panicked out of a Result-returning public API.
        for bad in ["", "X", "perfect", "?!", "MM"] {
            assert!(
                Interval::from_name(bad).is_err(),
                "Interval::from_name({bad:?}) should be an error"
            );
        }
    }

    #[test]
    fn interval_between_microtonal_pitches_keeps_the_cent_shift() {
        let c1 = pitch("C1");
        let mut half_sharp = pitch("C1");
        half_sharp.set_accidental(Some(
            crate::pitch::Accidental::new("half-sharp").expect("half-sharp is an accidental"),
        ));

        let quarter_tone = Interval::between_pitches(&c1, &half_sharp).unwrap();
        assert_eq!(quarter_tone.semitones(), 0.5);
        assert_eq!(quarter_tone.cents(), 50.0);
        assert_eq!(quarter_tone.directed_name(), "A1");
        assert_eq!(quarter_tone.diatonic_interval_cent_shift(), -50.0);
        assert_eq!(quarter_tone.to_string(), "A1 (-50c)");
        assert!(quarter_tone.pythagorean_ratio().is_err());
    }

    #[test]
    fn interval_cents_carry_a_pitch_microtone() {
        let c4 = pitch("C4");
        let mut d4 = pitch("D4");
        d4.set_microtone_cents(30.0).unwrap();

        let interval = Interval::between_pitches(&c4, &d4).unwrap();
        assert_eq!(interval.cents(), 230.0);
        assert_eq!(interval.to_string(), "M2 (+30c)");
    }

    #[test]
    fn interval_from_string_has_expected_chromatic() {
        let interval = Interval::from_name("M3").unwrap();
        assert_eq!(interval.chromatic.semitones, 4.0);
        assert!(!interval.implicit_diatonic);
    }

    #[test]
    fn interval_parser_accepts_direction_words_and_ordinals() {
        let descending = Interval::from_name("Descending Perfect Twelfth").unwrap();
        assert_eq!(descending.semitones(), -19.0);
        assert_eq!(descending.generic_number(), -5);

        let ascending = Interval::from_name("ascending Major Second").unwrap();
        assert_eq!(ascending.semitones(), 2.0);
        assert_eq!(ascending.generic_number(), 2);

        let major_third = Interval::from_name("Major Third").unwrap();
        assert_eq!(major_third.semitones(), 4.0);
        assert_eq!(major_third.generic_number(), 3);
    }

    #[test]
    fn interval_from_int_is_implicit_diatonic() {
        let interval = Interval::from_semitones(1).unwrap();
        assert!(interval.implicit_diatonic);
        assert_eq!(interval.chromatic.semitones, 1.0);
    }

    #[test]
    fn interval_between_pitches() {
        let c4 = pitch("C4");
        let g4 = pitch("G4");
        let interval = Interval::between(PitchOrNote::Pitch(c4), PitchOrNote::Pitch(g4)).unwrap();
        assert_eq!(interval.chromatic.semitones, 7.0);
        assert_eq!(interval.generic().staff_distance(), 4);
    }

    #[test]
    fn interval_transpose_pitch() {
        let c4 = pitch("C4");
        let m3 = Interval::from_name("m3").unwrap();
        let out = m3.transpose_pitch(&c4).unwrap();
        assert_eq!(out.name_with_octave(), "E-4");
    }

    #[test]
    fn interval_transpose_pitch_in_place() {
        let mut c4 = pitch("C4");
        Interval::from_name("M2")
            .unwrap()
            .transpose_pitch_in_place(&mut c4)
            .unwrap();
        assert_eq!(c4.name_with_octave(), "D4");
    }

    #[test]
    fn compact_names_match_music21() {
        let cases = [
            ("P5", "P5", "P5", "P5", "P5"),
            ("M3", "M3", "M3", "M3", "M3"),
            ("m-6", "m6", "m6", "m6", "m-6"),
            ("AA4", "AA4", "AA4", "AA4", "AA4"),
            ("d8", "d8", "d1", "d8", "d8"),
            ("P8", "P8", "P1", "P8", "P8"),
            ("M9", "M9", "M2", "M2", "M9"),
            ("P-5", "P5", "P5", "P5", "P-5"),
            ("P15", "P15", "P1", "P8", "P15"),
        ];
        for (input, short, simple, semi_simple, directed) in cases {
            let interval = Interval::from_name(input).unwrap();
            assert_eq!(interval.short_name(), short, "{input}");
            assert_eq!(interval.simple_name(), simple, "{input}");
            assert_eq!(interval.semi_simple_name(), semi_simple, "{input}");
            assert_eq!(interval.directed_name(), directed, "{input}");
        }
    }

    #[test]
    fn complement_and_interval_class_match_music21() {
        let cases = [
            ("P5", "P4", 5),
            ("M3", "m6", 4),
            ("m-6", "M3", 4),
            ("AA4", "dd5", 5),
            ("d8", "A1", 1),
            ("P8", "P1", 0),
            ("P1", "P8", 0),
            ("A1", "d8", 1),
            ("M9", "m7", 2),
            ("m2", "M7", 1),
            ("P15", "P1", 0),
        ];
        for (input, complement, interval_class) in cases {
            let interval = Interval::from_name(input).unwrap();
            assert_eq!(
                interval.complement().unwrap().short_name(),
                complement,
                "{input}"
            );
            assert_eq!(interval.interval_class(), interval_class, "{input}");
        }
    }

    #[test]
    fn step_skip_and_consonance_match_music21() {
        let cases = [
            ("P5", false, true, true, false, false),
            ("M3", false, true, true, false, false),
            ("AA4", false, true, false, false, false),
            ("d8", false, true, false, false, false),
            ("P8", false, true, true, false, false),
            ("P1", false, false, true, false, false),
            ("A1", true, false, false, false, true),
            ("M9", false, true, false, false, false),
            ("m2", true, false, false, true, true),
        ];
        for (input, step, skip, consonant, diatonic_step, chromatic_step) in cases {
            let interval = Interval::from_name(input).unwrap();
            assert_eq!(interval.is_step(), step, "{input} step");
            assert_eq!(interval.is_skip(), skip, "{input} skip");
            assert_eq!(interval.is_consonant(), consonant, "{input} consonant");
            assert_eq!(
                interval.is_diatonic_step(),
                diatonic_step,
                "{input} diatonic"
            );
            assert_eq!(
                interval.is_chromatic_step(),
                chromatic_step,
                "{input} chromatic"
            );
        }
    }

    #[test]
    fn sum_and_difference_match_music21() {
        fn intervals(names: &[&str]) -> Vec<Interval> {
            names
                .iter()
                .map(|name| Interval::from_name(*name).unwrap())
                .collect()
        }
        assert_eq!(
            Interval::sum(&intervals(&["A2", "P5"]))
                .unwrap()
                .short_name(),
            "A6"
        );
        assert_eq!(
            Interval::sum(&intervals(&["P5", "m2"]))
                .unwrap()
                .short_name(),
            "m6"
        );
        assert_eq!(
            Interval::sum(&intervals(&["W", "W", "H", "W", "W", "W", "H"]))
                .unwrap()
                .short_name(),
            "P8"
        );
        assert_eq!(
            Interval::sum(&intervals(&["P5", "P-4"]))
                .unwrap()
                .directed_name(),
            "M2"
        );
        assert!(Interval::sum(&[]).is_err());

        let cases = [
            (&["P5", "M3"][..], "m3"),
            (&["P4", "d3"][..], "A2"),
            (&["M6", "m2", "m2"][..], "AA4"),
            (&["P4", "M-2"][..], "P5"),
            (&["A2", "A2"][..], "P1"),
            (&["P8", "A1"][..], "d8"),
        ];
        for (names, expected) in cases {
            let difference = Interval::difference(&intervals(names)).unwrap();
            assert_eq!(difference.short_name(), expected, "{names:?}");
        }
        let descending_unison = Interval::difference(&intervals(&["P5", "A5"])).unwrap();
        assert_eq!(descending_unison.directed_name(), "d1");
        assert_eq!(descending_unison.semitones(), -1.0);
        assert!(Interval::difference(&[]).is_err());
    }

    #[test]
    fn interval_pythagorean_ratio() {
        let ratio = Interval::from_name("P5")
            .unwrap()
            .pythagorean_ratio()
            .unwrap();
        assert_eq!(ratio, FractionType::new(3, 2));
    }

    #[test]
    fn interval_inverts_oblique_unison() {
        let unison = Interval::from_name("P1").unwrap();
        let inverted = unison.inversion().unwrap();

        assert_eq!(inverted.semitones(), 0.0);
        assert_eq!(inverted.generic_number(), 1);
    }
    #[test]
    fn specifier_case_matters_only_for_major_versus_minor() {
        // Verified against music21: it accepts either case for every specifier
        // letter, and m/M is the sole pair where case changes the interval.
        for (lower, upper) in [
            ("p5", "P5"),
            ("a2", "A2"),
            ("d5", "D5"),
            ("aa2", "AA2"),
            ("dd5", "DD5"),
            ("aaa2", "AAA2"),
            ("ddd5", "DDD5"),
        ] {
            let a = Interval::from_name(lower).expect("lowercase parses");
            let b = Interval::from_name(upper).expect("uppercase parses");
            assert_eq!(a.semitones(), b.semitones(), "{lower} vs {upper}");
            assert_eq!(a.name(), b.name(), "{lower} vs {upper}");
        }

        // The carve-out: these must stay different.
        let minor = Interval::from_name("m3").expect("m3 parses");
        let major = Interval::from_name("M3").expect("M3 parses");
        assert_eq!(minor.semitones(), 3.0);
        assert_eq!(major.semitones(), 4.0);
    }

    #[test]
    fn an_unknown_specifier_errors_instead_of_panicking() {
        for name in ["Q5", "x3", "5", "zz2"] {
            assert!(
                Interval::from_name(name).is_err(),
                "{name:?} should be rejected, not panic"
            );
        }
    }

    #[test]
    fn a_hyphen_anywhere_makes_an_interval_name_descending() {
        // Verified against music21: it parses every form below identically,
        // so these assertions pin parity rather than a local accident. The
        // prefix form is this crate's convention; "M-2" is where music21's
        // directedName puts the hyphen.
        for name in ["-M2", "M-2"] {
            let interval = Interval::from_name(name).expect("descending name parses");
            assert_eq!(interval.semitones(), -2.0, "{name}");
            assert_eq!(interval.generic_number(), -2, "{name}");
        }

        // Count and position are both irrelevant: hyphens do not cancel, so
        // repeating one leaves the interval descending rather than flipping it
        // back. music21 agrees on both spellings.
        for name in ["--M2", "-M-2"] {
            let interval = Interval::from_name(name).expect("repeated hyphen parses");
            assert_eq!(interval.semitones(), -2.0, "{name}");
        }

        // A specifier other than major is unaffected by where the hyphen sits.
        assert_eq!(
            Interval::from_name("d-5").expect("d-5 parses").semitones(),
            -6.0
        );
    }
}
