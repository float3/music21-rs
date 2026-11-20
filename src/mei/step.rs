//! The steps of music21's `mei.base` that decide what an attribute's value
//! stands for, one value at a time.
//!
//! music21 reads MEI element by element, and its callers call those steps
//! one by one: `_accidentalFromAttr`, `_qlDurationFromAttr`,
//! `_sharpsFromAttr`, `_getOctaveShift` and the rest. Each table and rule
//! here is one of them; [`from_mei`](super::from_mei) reads a whole
//! document through them, and the wheel's `music21.mei.base` hands its
//! callers each one.

use crate::defaults::{FloatType, IntegerType};
use crate::error::{Error, Result};
use crate::notation::{Syllabic, TieType};

/// music21's `_ACCID_ATTR_DICT`: a written `accid` and the accidental it
/// names.
pub const WRITTEN_ACCIDENTALS: [(&str, &str); 17] = [
    ("s", "#"),
    ("f", "-"),
    ("ss", "##"),
    ("x", "##"),
    ("ff", "--"),
    ("xs", "###"),
    ("ts", "###"),
    ("tf", "---"),
    ("n", "n"),
    ("nf", "-"),
    ("ns", "#"),
    ("su", "#~"),
    ("sd", "~"),
    ("fu", "`"),
    ("fd", "-`"),
    ("nu", "~"),
    ("nd", "`"),
];

/// music21's `_ACCID_GES_ATTR_DICT`: a sounding `accid.ges` and the
/// accidental it names.
pub const GESTURAL_ACCIDENTALS: [(&str, &str); 9] = [
    ("s", "#"),
    ("f", "-"),
    ("ss", "##"),
    ("ff", "--"),
    ("n", "n"),
    ("su", "#~"),
    ("sd", "~"),
    ("fu", "`"),
    ("fd", "-`"),
];

/// music21's `_DUR_ATTR_DICT`: a `dur` and its quarter length.
pub const DURATIONS: [(&str, FloatType); 14] = [
    ("long", 16.0),
    ("breve", 8.0),
    ("1", 4.0),
    ("2", 2.0),
    ("4", 1.0),
    ("8", 0.5),
    ("16", 0.25),
    ("32", 0.125),
    ("64", 0.0625),
    ("128", 0.03125),
    ("256", 0.015625),
    ("512", 0.0078125),
    ("1024", 0.00390625),
    ("2048", 0.001953125),
];

/// The quarter length music21 gives an element with no `dur`, small enough
/// to be told apart: an `<mRest>` is given its measure's length later.
pub const NO_DURATION: FloatType = 0.00390625;

/// The class music21 makes for a mark it has none of its own for.
pub const PLAIN_ARTICULATION: &str = "Articulation";

/// music21's `_ARTIC_ATTR_DICT`: an `artic` and the name of music21's
/// class for it.
pub const ARTICULATIONS: [(&str, &str); 30] = [
    ("acc", "Accent"),
    ("stacc", "Staccato"),
    ("ten", "Tenuto"),
    ("stacciss", "Staccatissimo"),
    ("marc", "StrongAccent"),
    ("spicc", "Spiccato"),
    ("doit", "Doit"),
    ("plop", "Plop"),
    ("fall", "Falloff"),
    ("dnbow", "DownBow"),
    ("upbow", "UpBow"),
    ("harm", "Harmonic"),
    ("snap", "SnapPizzicato"),
    ("stop", "Stopped"),
    ("open", "OpenString"),
    ("dbltongue", "DoubleTongue"),
    ("toe", "OrganToe"),
    ("trpltongue", "TripleTongue"),
    ("heel", "OrganHeel"),
    ("tap", PLAIN_ARTICULATION),
    ("lhpizz", PLAIN_ARTICULATION),
    ("dot", PLAIN_ARTICULATION),
    ("stroke", PLAIN_ARTICULATION),
    ("rip", PLAIN_ARTICULATION),
    ("bend", PLAIN_ARTICULATION),
    ("flip", PLAIN_ARTICULATION),
    ("smear", PLAIN_ARTICULATION),
    ("fingernail", PLAIN_ARTICULATION),
    ("damp", PLAIN_ARTICULATION),
    ("dampall", PLAIN_ARTICULATION),
];

/// The `artic` values that are two marks, which music21 makes apart from
/// its table.
pub const COMPOUND_ARTICULATIONS: [(&str, [&str; 2]); 2] = [
    ("marc-stacc", ["StrongAccent", "Staccato"]),
    ("ten-stacc", ["Tenuto", "Staccato"]),
];

/// music21's `_BAR_ATTR_DICT`: a `right` or `rend` and the barline style
/// it names. Repeats, which start with `rpt`, are made apart.
pub const BARLINES: [(&str, &str); 6] = [
    ("dashed", "dashed"),
    ("dotted", "dotted"),
    ("dbl", "double"),
    ("end", "final"),
    ("invis", "none"),
    ("single", "regular"),
];

/// What a repeat's `right` starts with.
pub const REPEAT_PREFIX: &str = "rpt";

/// music21's `_MEI_UNITDUR_TO_TREMOLO_MARKS`: a `<bTrem>`'s `unitdur` and
/// the marks its tremolo is drawn with.
pub const TREMOLO_MARKS: [(&str, IntegerType); 8] = [
    ("8", 1),
    ("16", 2),
    ("32", 3),
    ("64", 4),
    ("128", 5),
    ("256", 6),
    ("512", 7),
    ("1024", 8),
];

/// The octaves a clef's `dis` moves it by.
const DISPLACEMENTS: [(&str, IntegerType); 3] = [("8", 1), ("15", 2), ("22", 3)];

/// The `dis.place` that moves a clef down.
const BELOW: &str = "below";

/// The steps of an octave, and the semitones.
const STEPS_PER_OCTAVE: i64 = 7;
const SEMITONES_PER_OCTAVE: i64 = 12;

/// The semitones one octave of a transposition may stray from its steps
/// before music21 decides the steps left an octave out.
const STRAY_PER_OCTAVE: i64 = 5;

/// A `<syl>`'s `wordpos` and the place in its word it names.
const WORD_POSITIONS: [(&str, Syllabic); 3] = [
    ("i", Syllabic::Begin),
    ("m", Syllabic::Middle),
    ("t", Syllabic::End),
];

/// A `<syl>`'s `con` and what joins it to the next syllable.
const CONNECTORS: [(&str, &str); 4] = [("s", " "), ("d", "-"), ("t", "~"), ("u", "_")];

/// What joins syllables that name no `con`.
const DEFAULT_CONNECTOR: &str = "-";

/// An MEI document music21 refuses, with its message.
pub(crate) fn mei_error(message: impl Into<String>) -> Error {
    Error::mei(message.into())
}

/// music21's `_UNEXPECTED_ATTR_VALUE`: what music21 says of an attribute
/// value it has no meaning for.
pub fn unexpected(name: &str, value: &str) -> String {
    format!("Unexpected value for \"{name}\" attribute: {value}")
}

/// music21's `_attrTranslator`: what `attribute` stands for in `table`,
/// refused by the name of the attribute it was the value of.
///
/// ```
/// use music21::mei::step::{translate, WRITTEN_ACCIDENTALS};
///
/// assert_eq!(translate(&WRITTEN_ACCIDENTALS, "s", "accid").unwrap(), "#");
/// assert!(translate(&WRITTEN_ACCIDENTALS, "q", "accid").is_err());
/// ```
pub fn translate<T: Copy>(table: &[(&str, T)], attribute: &str, name: &str) -> Result<T> {
    table
        .iter()
        .find(|(key, _)| *key == attribute)
        .map(|(_, value)| *value)
        .ok_or_else(|| mei_error(unexpected(name, attribute)))
}

/// music21's `_accidentalFromAttr`: the accidental a written `accid`
/// names, none where there is no `accid`.
pub fn written_accidental(attribute: Option<&str>) -> Result<Option<&'static str>> {
    attribute
        .map(|attribute| translate(&WRITTEN_ACCIDENTALS, attribute, "accid"))
        .transpose()
}

/// music21's `_accidGesFromAttr`: the accidental an `accid.ges` names.
pub fn gestural_accidental(attribute: &str) -> Result<&'static str> {
    translate(&GESTURAL_ACCIDENTALS, attribute, "accid.ges")
}

/// music21's `_qlDurationFromAttr`: the quarter length of a `dur`,
/// [`NO_DURATION`] where there is none.
pub fn quarter_length(attribute: Option<&str>) -> Result<FloatType> {
    match attribute {
        None => Ok(NO_DURATION),
        Some(attribute) => translate(&DURATIONS, attribute, "dur"),
    }
}

/// music21's `_articulationFromAttr`: the names of music21's classes for
/// the marks one `artic` value names.
pub fn articulation_classes(attribute: &str) -> Result<Vec<&'static str>> {
    if let Some((_, pair)) = COMPOUND_ARTICULATIONS
        .iter()
        .find(|(name, _)| *name == attribute)
    {
        return Ok(pair.to_vec());
    }
    Ok(vec![translate(&ARTICULATIONS, attribute, "artic")?])
}

/// music21's `_sharpsFromAttr`: the sharps a `key.sig` counts, flats
/// negative.
///
/// ```
/// use music21::mei::step::sharps_of;
///
/// assert_eq!(sharps_of("3s").unwrap(), 3);
/// assert_eq!(sharps_of("3f").unwrap(), -3);
/// assert_eq!(sharps_of("0").unwrap(), 0);
/// ```
pub fn sharps_of(signature: &str) -> Result<IntegerType> {
    if signature.starts_with('0') {
        return Ok(0);
    }
    let count: IntegerType = signature
        .get(..1)
        .unwrap_or_default()
        .parse()
        .map_err(|_| mei_error(format!("the key signature {signature:?} has no number")))?;
    if signature.ends_with('s') {
        return Ok(count);
    }
    Ok(-count)
}

/// music21's `_getOctaveShift`: the octaves a clef's `dis` and `dis.place`
/// move it by, down negative.
pub fn octave_shift(displaced: Option<&str>, place: Option<&str>) -> Result<IntegerType> {
    let octaves = match displaced {
        None => 0,
        Some(displaced) => translate(&DISPLACEMENTS, displaced, "dis")?,
    };
    if place == Some(BELOW) {
        return Ok(-octaves);
    }
    Ok(octaves)
}

/// music21's `_transpositionFromAttrs` before it makes the interval: the
/// generic interval and the semitones of a `trans.diat` and `trans.semi`.
///
/// MEI counts steps from a unison of 0, music21 from 1; a transposition
/// past an octave written with its steps inside one has the octaves put
/// back.
pub fn transposition_steps(diatonic: i64, semitones: i64) -> Result<(IntegerType, IntegerType)> {
    let mut diatonic = diatonic;
    let octaves = semitones.abs() / SEMITONES_PER_OCTAVE;
    if (semitones - diatonic).abs() > STRAY_PER_OCTAVE * (octaves + 1) {
        diatonic += STEPS_PER_OCTAVE * octaves * semitones.signum();
    }
    diatonic += diatonic.signum();
    let too_far = || mei_error(format!("a transposition of {diatonic} steps is too far"));
    let generic = IntegerType::try_from(diatonic).map_err(|_| too_far())?;
    let chromatic = IntegerType::try_from(semitones).map_err(|_| too_far())?;
    Ok((generic, chromatic))
}

/// music21's `_tieFromAttr`: the part of a tie a `tie` names.
pub fn tie_type(attribute: &str) -> TieType {
    let has = |mark: char| attribute.contains(mark);
    if has('m') || (has('t') && has('i')) {
        return TieType::Continue;
    }
    if has('i') {
        return TieType::Start;
    }
    TieType::Stop
}

/// music21's `sylFromElement` before it makes the lyric: a syllable's text
/// with what joins it to its neighbours, and its place in its word, none
/// where it names no `wordpos`.
pub fn syllable(
    text: &str,
    word_position: Option<&str>,
    connector: Option<&str>,
) -> Result<(String, Option<Syllabic>)> {
    let Some(word_position) = word_position else {
        return Ok((text.to_owned(), None));
    };
    let joiner = match connector {
        None => DEFAULT_CONNECTOR,
        Some(connector) => translate(&CONNECTORS, connector, "con")?,
    };
    let syllabic = translate(&WORD_POSITIONS, word_position, "wordpos")?;
    let joined = match syllabic {
        Syllabic::Begin => format!("{text}{joiner}"),
        Syllabic::Middle => format!("{joiner}{text}{joiner}"),
        _ => format!("{joiner}{text}"),
    };
    Ok((joined, Some(syllabic)))
}

/// The tremolo marks a `<bTrem>`'s `unitdur` names, none for a value
/// music21 has none for.
pub fn tremolo_marks(unit: &str) -> Option<IntegerType> {
    translate(&TREMOLO_MARKS, unit, "unitdur").ok()
}

/// A random version-4 UUID, as music21 names a slur written apart from its
/// notes with `uuid4()`; `count` keeps two in one read apart.
pub fn fresh_uuid(count: usize) -> String {
    use std::hash::{BuildHasher, Hasher};

    // std seeds each `RandomState` afresh, which is all the randomness
    // a name needs.
    let random = std::collections::hash_map::RandomState::new();
    let mut halves = [0_u64; 2];
    for (half, made) in halves.iter_mut().enumerate() {
        let mut hasher = random.build_hasher();
        hasher.write_usize(count);
        hasher.write_usize(half);
        *made = hasher.finish();
    }
    let high = (halves[0] & !0xF000) | 0x4000;
    let low = (halves[1] & 0x3FFF_FFFF_FFFF_FFFF) | 0x8000_0000_0000_0000;
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        high >> 32,
        (high >> 16) & 0xFFFF,
        high & 0xFFFF,
        low >> 48,
        low & 0xFFFF_FFFF_FFFF
    )
}

/// music21's `removeOctothorpe`: an `xml:id` reference without its `#`.
///
/// ```
/// use music21::mei::step::without_hash;
///
/// assert_eq!(without_hash("#e46c"), "e46c");
/// assert_eq!(without_hash("e46c"), "e46c");
/// ```
pub fn without_hash(id: &str) -> &str {
    id.strip_prefix('#').unwrap_or(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_transposition_past_an_octave_has_its_octaves_put_back() {
        assert_eq!(transposition_steps(-2, -3).unwrap(), (-3, -3));
        assert_eq!(transposition_steps(4, 7).unwrap(), (5, 7));
        assert_eq!(transposition_steps(11, 19).unwrap(), (12, 19));
        assert_eq!(transposition_steps(0, 12).unwrap(), (8, 12));
        assert_eq!(transposition_steps(-4, -19).unwrap(), (-12, -19));
        assert_eq!(transposition_steps(1, 26).unwrap(), (16, 26));
    }

    #[test]
    fn a_clef_moves_by_its_octaves() {
        assert_eq!(octave_shift(Some("15"), Some("above")).unwrap(), 2);
        assert_eq!(octave_shift(Some("22"), Some("below")).unwrap(), -3);
        assert_eq!(octave_shift(Some("8"), None).unwrap(), 1);
        assert_eq!(octave_shift(None, None).unwrap(), 0);
        assert!(octave_shift(Some("9"), None).is_err());
    }

    #[test]
    fn a_tie_names_its_part() {
        assert_eq!(tie_type("i"), TieType::Start);
        assert_eq!(tie_type("ti"), TieType::Continue);
        assert_eq!(tie_type("m"), TieType::Continue);
        assert_eq!(tie_type("t"), TieType::Stop);
    }

    #[test]
    fn a_syllable_is_joined_by_its_place() {
        let (text, place) = syllable("Chri", Some("i"), Some("s")).unwrap();
        assert_eq!((text.as_str(), place), ("Chri ", Some(Syllabic::Begin)));
        let (text, place) = syllable("sto", Some("m"), Some("t")).unwrap();
        assert_eq!((text.as_str(), place), ("~sto~", Some(Syllabic::Middle)));
        let (text, place) = syllable("pher", Some("t"), None).unwrap();
        assert_eq!((text.as_str(), place), ("-pher", Some(Syllabic::End)));
        assert_eq!(syllable("shoe", None, None).unwrap(), ("shoe".into(), None));
    }

    #[test]
    fn an_unknown_value_names_its_attribute() {
        let refused = translate(&DURATIONS, "9", "dur").unwrap_err();
        assert!(refused.to_string().contains(&unexpected("dur", "9")));
        assert_eq!(
            articulation_classes("marc-stacc").unwrap(),
            ["StrongAccent", "Staccato"]
        );
        assert_eq!(tremolo_marks("16"), Some(2));
        assert_eq!(tremolo_marks("4"), None);
    }
}
