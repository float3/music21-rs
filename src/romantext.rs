//! RomanText: music21's `romanText`, a harmonic analysis written measure by
//! measure as roman numerals.
//!
//! ```text
//! Composer: J. S. Bach
//! Time Signature: 4/4
//! m1 C: I b3 V6
//! m2 I b2 ii6/5 b3 V7 b4 vi
//! m3 = m1
//! ```
//!
//! A line starting `m` and a number is a measure; in it a key is a note name
//! and a colon, `b` and a number is the beat the next chord stands on, and
//! anything else is a roman numeral. The other lines are a tag, a colon and
//! what it says.

use crate::bar::{Barline, Ending, RepeatDirection};
use crate::chord::Chord;
use crate::chordsymbol::ChordSymbol;
use crate::defaults::{FloatType, IntegerType};
use crate::duration::Duration;
use crate::error::{Error, Result};
use crate::key::{Key, KeySignature, convert_key_string_to_music21_key_string};
use crate::makenotation::{make_accidentals, make_beams, op_frac, sorted_events};
use crate::metadata::{Metadata, MetadataValue};
use crate::meter::TimeSignature;
use crate::notation::{Tie, TieType};
use crate::rest::Rest;
use crate::roman::{Minor67Default, RomanNumeral};
use crate::stream::{Stream, StreamElement, StreamEvent, StreamKind};

fn roman_text_error(message: impl Into<String>) -> Error {
    Error::RomanText(message.into())
}

/// One word of a measure line: music21's `RTAtom` and its kinds.
#[derive(Clone, Debug, PartialEq)]
enum Atom {
    /// `b2.5`: the beat what follows stands on.
    Beat(String),
    /// `G;:` sets the key and its signature.
    Key(String),
    /// `G:` sets the key chords are read in.
    AnalyticKey(String),
    /// `KS-2`: a key signature of so many sharps.
    KeySignature(String),
    RepeatStart,
    RepeatStop,
    NoChord,
    Chord(String),
    /// A phrase mark, a parenthesis or an optional key, none of which says
    /// anything a score holds.
    Other,
}

/// One line: music21's `RTTagged` and `RTMeasure`.
#[derive(Clone, Debug)]
enum Token {
    Tagged {
        tag: String,
        data: String,
    },
    Measure {
        numbers: Vec<IntegerType>,
        letters: Vec<String>,
        variant: bool,
        data: String,
        atoms: Vec<Atom>,
    },
}

impl Token {
    fn is_movement(&self) -> bool {
        matches!(self, Self::Tagged { tag, .. } if tag.eq_ignore_ascii_case("movement"))
    }
}

/// How far a measure tag reaches into a line: `m`, digits, letters `a` to
/// `h`, hyphens, digits and letters again.
fn measure_tag_end(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    if bytes.first() != Some(&b'm') {
        return None;
    }
    let mut end = 1;
    let run = |from: usize, test: fn(&u8) -> bool| -> usize {
        from + bytes[from..].iter().take_while(|byte| test(byte)).count()
    };
    let digits = run(end, u8::is_ascii_digit);
    if digits == end {
        return None;
    }
    end = digits;
    let letter: fn(&u8) -> bool = |byte| (b'a'..=b'h').contains(byte);
    end = run(end, letter);
    end = run(end, |byte| *byte == b'-');
    end = run(end, u8::is_ascii_digit);
    end = run(end, letter);
    Some(end)
}

/// A measure number and its repeat letter, or two of each for a range:
/// music21's `_getMeasureNumberData`.
fn measure_numbers(text: &str) -> Result<(Vec<IntegerType>, Vec<String>)> {
    let pieces: Vec<&str> = if text.contains('-') {
        let pieces: Vec<&str> = text.split('-').collect();
        if pieces.len() != 2 {
            return Err(roman_text_error(format!(
                "cannot read the measure numbers {text:?}"
            )));
        }
        pieces
    } else {
        vec![text]
    };
    let mut numbers = Vec::new();
    let mut letters = Vec::new();
    for piece in pieces {
        let digits: String = piece.chars().filter(char::is_ascii_digit).collect();
        let other: String = piece.chars().filter(|c| !c.is_ascii_digit()).collect();
        numbers.push(digits.parse::<IntegerType>().map_err(|_| {
            roman_text_error(format!("cannot read the measure number in {piece:?}"))
        })?);
        letters.push(other.replace('m', ""));
    }
    Ok((numbers, letters))
}

/// Whether a word starts with a key written as music21's patterns have it:
/// note letters, flats and sharps, then the ending asked for.
fn starts_with_key(word: &str, ending: &str) -> bool {
    let letters = word
        .chars()
        .take_while(|c| matches!(c, 'A'..='G' | 'a'..='g'))
        .count();
    if letters == 0 {
        return false;
    }
    let rest = &word[letters..];
    let signs = rest.chars().take_while(|c| matches!(c, 'b' | '#')).count();
    rest[signs..].starts_with(ending)
}

/// music21's `tokenizeAtoms`.
fn atoms_of(data: &str) -> Vec<Atom> {
    let mut atoms = Vec::new();
    for word in data.split(' ') {
        let word = word.trim();
        let chars: Vec<char> = word.chars().collect();
        if word.is_empty() {
            continue;
        }
        if word == "=" {
            break;
        }
        let no_chord = word.starts_with("NC")
            || word.starts_with("nc")
            || (chars.len() >= 4 && chars[0] == 'N' && chars[2] == 'C');
        let atom = if matches!(word, "||" | "(" | ")") {
            Atom::Other
        } else if chars[0] == 'b' && chars.get(1).is_some_and(|c| matches!(c, '1'..='9' | '.')) {
            Atom::Beat(word.to_string())
        } else if word
            .strip_prefix("?(")
            .or_else(|| word.strip_prefix("?)"))
            .is_some_and(|rest| starts_with_key(rest, ""))
            && (word.starts_with("?)") || starts_with_key(&word[2..], ":"))
        {
            Atom::Other
        } else if starts_with_key(word, ";:") {
            Atom::Key(word.to_string())
        } else if starts_with_key(word, ":") {
            Atom::AnalyticKey(word.to_string())
        } else if word.strip_prefix("KS").is_some_and(|rest| {
            rest.strip_prefix('-')
                .unwrap_or(rest)
                .starts_with(|c: char| ('0'..='7').contains(&c))
        }) {
            Atom::KeySignature(word.to_string())
        } else if word.starts_with("||:") {
            Atom::RepeatStart
        } else if word.starts_with(":||") {
            Atom::RepeatStop
        } else if no_chord {
            Atom::NoChord
        } else {
            Atom::Chord(word.to_string())
        };
        atoms.push(atom);
    }
    atoms
}

fn tagged(line: &str) -> Token {
    match line.split_once(':') {
        Some((tag, data)) => Token::Tagged {
            tag: tag.trim().to_string(),
            data: data.trim().to_string(),
        },
        None => Token::Tagged {
            tag: String::new(),
            data: line.to_string(),
        },
    }
}

/// music21's `RTHandler.tokenize`: the header's lines, then the body's.
fn tokenize(text: &str) -> Result<Vec<Token>> {
    let lines: Vec<&str> = text.split('\n').collect();
    let body = lines
        .iter()
        .position(|line| measure_tag_end(line.trim()).is_some())
        .ok_or_else(|| {
            roman_text_error("Cannot find the first measure definition in this file.")
        })?;
    let mut tokens = Vec::new();
    for line in &lines[..body] {
        let line = line.trim();
        if !line.is_empty() {
            tokens.push(tagged(line));
        }
    }
    for line in &lines[body..] {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some(end) = measure_tag_end(line) else {
            tokens.push(tagged(line));
            continue;
        };
        let (numbers, letters) = measure_numbers(line[..end].trim())?;
        let raw = line[end..].trim();
        // `var1` and `varA` mark a variant reading, which is passed over.
        let variant = raw.strip_prefix("var").is_some_and(|rest| {
            rest.starts_with(|c: char| c.is_ascii_digit() || c.is_ascii_uppercase())
        });
        tokens.push(Token::Measure {
            numbers,
            letters,
            variant,
            data: raw.to_string(),
            atoms: atoms_of(raw),
        });
    }
    Ok(tokens)
}

/// The key a word names, and the words it puts in front of the next chord:
/// music21's `_getKeyAndPrefix`.
fn key_and_prefix(word: &str) -> Result<(Key, String)> {
    let name = convert_key_string_to_music21_key_string(word.trim_end_matches([';', ':']));
    let key = Key::from_tonic(&name)?;
    // The lyric is music21's `tonicPitchNameWithCase`, which spells a flat
    // `-`: the text a score carries is music21's, not a name of the crate's.
    let mut letters = key.tonic().name();
    let step = letters.remove(0);
    let mut tonic = format!("{step}{}", letters.replace('b', "-"));
    if key.mode() == "minor" {
        tonic = tonic.to_lowercase();
    }
    Ok((key, format!("{tonic}: ")))
}

/// music21's `getBeatFloatOrFrac`: `b2.5` is beat two and a half, `b1.66`
/// beat one and two thirds, and `b1.66.5` half a third further on.
fn beat_of(word: &str) -> Result<FloatType> {
    let text = word.replace('b', "");
    let parts: Vec<&str> = text.split('.').collect();
    if parts.len() > 4 {
        return Err(roman_text_error(format!(
            "cannot handle specification: {word}"
        )));
    }
    let read = |text: &str| -> Result<FloatType> {
        text.parse::<FloatType>()
            .map_err(|_| roman_text_error(format!("cannot read the beat {word:?}")))
    };
    let main = read(parts[0])?;
    // A third or a sixth written to two places is the fraction itself.
    let mut fraction = 0.0;
    let mut denominator = 1.0;
    if parts.len() > 1 {
        fraction = read(&format!(".{}", parts[1]))?;
        for (numerator, over) in [(1.0, 3.0), (2.0, 3.0), (1.0, 6.0), (5.0, 6.0)] {
            if (fraction - numerator / over).abs() <= 1e-2 {
                fraction = numerator / over;
            }
        }
        denominator = crate::duration::limited_fraction(fraction, 1_000_000)
            .map_or(1.0, |(_, denominator)| denominator as FloatType);
    }
    let further = if parts.len() > 2 {
        op_frac(1.0 / (denominator / read(&format!(".{}", parts[2]))?))
    } else {
        0.0
    };
    Ok(op_frac(main + fraction + further))
}

/// What a sounding element is.
// Small and short-lived: boxing the larger variant would buy nothing.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug)]
enum Sound {
    Numeral {
        figure: String,
        key: Key,
        sixth: Minor67Default,
        seventh: Minor67Default,
        follows_key_change: bool,
        /// The key of the chord this one pivots to.
        pivot: Option<Key>,
    },
    Rest,
}

/// A chord or a rest of the analysis, kept apart from the measures so that
/// a later line can lengthen or tie one in an earlier measure.
#[derive(Clone, Debug)]
struct Sounding {
    sound: Sound,
    quarter_length: FloatType,
    /// The verses, a line each; none where there are no lyrics at all.
    lyric: Option<String>,
    tie: Option<TieType>,
}

impl Sounding {
    fn is_numeral(&self) -> bool {
        matches!(self.sound, Sound::Numeral { .. })
    }

    fn element(&self) -> Result<StreamElement> {
        let duration = Duration::new(self.quarter_length)?;
        match &self.sound {
            Sound::Rest => {
                let mut rest = Rest::new(duration);
                if let Some(lyric) = &self.lyric {
                    rest.set_lyric(Some(lyric));
                }
                rest.set_tie(self.tie.map(Tie::new));
                Ok(rest.into())
            }
            Sound::Numeral {
                figure,
                key,
                sixth,
                seventh,
                ..
            } => {
                let numeral =
                    RomanNumeral::with_minor_defaults(figure, key.clone(), *sixth, *seventh)?;
                let mut chord: Chord = numeral
                    .to_chord()?
                    .with_duration(duration)
                    .with_numeral(numeral);
                if let (Some(lyric), Some(singer)) = (&self.lyric, chord.notes_mut().first_mut()) {
                    singer.set_lyric(Some(lyric))?;
                }
                chord.set_tie(self.tie.map(Tie::new));
                Ok(chord.into())
            }
        }
    }
}

/// What a measure holds at an offset.
// Small and short-lived: boxing the larger variant would buy nothing.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug)]
enum Held {
    /// A chord or rest, by its place among the soundings.
    Sounding(usize),
    Element(StreamElement),
}

#[derive(Clone, Debug, Default)]
struct Measure {
    number: IntegerType,
    suffix: Option<String>,
    items: Vec<(FloatType, Held)>,
    left: Option<Barline>,
    right: Option<Barline>,
    /// What a pickup leaves out before its first chord, and the last
    /// measure after its last.
    padding_left: FloatType,
    padding_right: FloatType,
}

/// music21's `PartTranslator`.
struct Translator {
    metadata: Metadata,
    measures: Vec<Measure>,
    soundings: Vec<Sounding>,
    meter: TimeSignature,
    meter_at_last_chord: TimeSignature,
    meter_set: bool,
    last_measure_number: IntegerType,
    last_letters: Vec<String>,
    previous: Option<usize>,
    signature: Option<KeySignature>,
    signature_placed: bool,
    found_signature: bool,
    key: Key,
    prefix: String,
    sixth: Minor67Default,
    seventh: Minor67Default,
    /// The measures under each ending, by the number music21 counts them
    /// under, in the order they were met.
    endings: Vec<(u32, Vec<(IntegerType, usize)>)>,
    // What is true while one measure is read.
    previous_in_measure: Option<usize>,
    pivot_possible: bool,
    key_change: bool,
    offset: FloatType,
}

impl Translator {
    fn new() -> Result<Self> {
        Ok(Self {
            metadata: Metadata::new(),
            measures: Vec::new(),
            soundings: Vec::new(),
            meter: TimeSignature::new(4, 4)?,
            meter_at_last_chord: TimeSignature::new(4, 4)?,
            meter_set: false,
            last_measure_number: 0,
            last_letters: Vec::new(),
            previous: None,
            signature: None,
            signature_placed: true,
            found_signature: false,
            key: Key::from_tonic("C")?,
            prefix: String::new(),
            sixth: Minor67Default::Cautionary,
            seventh: Minor67Default::Cautionary,
            endings: Vec::new(),
            previous_in_measure: None,
            pivot_possible: false,
            key_change: false,
            offset: 0.0,
        })
    }

    fn add(&mut self, sounding: Sounding) -> usize {
        self.soundings.push(sounding);
        self.soundings.len() - 1
    }

    /// Ties the last chord on to a copy of it, which is handed back.
    fn continued(&mut self, quarter_length: FloatType) -> Option<usize> {
        let previous = self.previous?;
        let mut copy = self.soundings[previous].clone();
        copy.lyric = Some(String::new());
        copy.quarter_length = quarter_length;
        copy.tie = Some(TieType::Stop);
        let held = &mut self.soundings[previous];
        held.tie = Some(if held.tie.is_none() {
            TieType::Start
        } else {
            TieType::Continue
        });
        let made = self.add(copy);
        self.previous = Some(made);
        Some(made)
    }

    /// music21's `appendMeasureToRepeatEndingsDict`.
    fn note_ending(
        &mut self,
        letters: &[String],
        measure: &mut Measure,
        place: usize,
    ) -> Result<()> {
        if letters.is_empty() {
            return Ok(());
        }
        measure.suffix = Some(letters[0].clone()).filter(|letter| !letter.is_empty());
        for letter in letters {
            if letter.is_empty() {
                continue;
            }
            let number = match letter.as_str() {
                "a" => 1,
                "b" => 2,
                "c" => 3,
                "d" => 4,
                "e" => 5,
                "f" => 6,
                "g" => 7,
                "h" => 8,
                _ => {
                    return Err(roman_text_error(format!(
                        "Improper repeat letter: {letter}"
                    )));
                }
            };
            let entry = (measure.number, place);
            match self.endings.iter_mut().find(|(held, _)| *held == number) {
                Some((_, measures)) => measures.push(entry),
                None => self.endings.push((number, vec![entry])),
            }
        }
        Ok(())
    }

    /// music21's `fillMeasureFromPreviousRn`.
    fn fill_from_previous(&mut self, measure: &mut Measure) {
        let length = self.meter_at_last_chord.bar_quarter_length();
        if let Some(made) = self.continued(length) {
            let at = measure
                .items
                .iter()
                .map(|(offset, held)| match held {
                    Held::Sounding(index) => offset + self.soundings[*index].quarter_length,
                    Held::Element(_) => *offset,
                })
                .fold(0.0, FloatType::max);
            measure.items.push((at, Held::Sounding(made)));
        }
    }

    fn measure_length(&self, measure: &Measure) -> FloatType {
        measure
            .items
            .iter()
            .map(|(offset, held)| match held {
                Held::Sounding(index) => offset + self.soundings[*index].quarter_length,
                Held::Element(_) => *offset,
            })
            .fold(0.0, FloatType::max)
    }

    /// A copy of a measure already read, its chords read again in the key
    /// in force: music21's `_copySingleMeasure`.
    fn copied(&mut self, source: usize, number: IntegerType) -> Measure {
        let mut measure = self.measures[source].clone();
        measure.number = number;
        for (_, held) in &mut measure.items {
            let Held::Sounding(index) = held else {
                continue;
            };
            let mut copy = self.soundings[*index].clone();
            if let Sound::Numeral {
                figure,
                key,
                sixth,
                seventh,
                follows_key_change,
                pivot,
            } = &mut copy.sound
            {
                if *follows_key_change {
                    self.key = key.clone();
                } else if let Some(pivot) = pivot {
                    self.key = pivot.clone();
                } else if key.tonic().name() != self.key.tonic().name()
                    || key.mode() != self.key.mode()
                {
                    // Read in another key the chord is built afresh, and
                    // the ties its notes carried are gone.
                    *key = self.key.clone();
                    copy.tie = None;
                }
                // A chord of another chord is read afresh, with nothing of
                // what the first reading was told.
                if figure.contains('/')
                    && RomanNumeral::new(figure.as_str(), key.clone())
                        .is_ok_and(|numeral| numeral.secondary().is_some())
                {
                    *key = self.key.clone();
                    *sixth = Minor67Default::default();
                    *seventh = Minor67Default::default();
                    *follows_key_change = false;
                    *pivot = None;
                    copy.tie = None;
                }
            }
            self.soundings.push(copy);
            *index = self.soundings.len() - 1;
        }
        measure
    }

    fn last_numeral(&self, measure: &Measure) -> Option<usize> {
        let mut items: Vec<&(FloatType, Held)> = measure.items.iter().collect();
        items.sort_by(|left, right| left.0.total_cmp(&right.0));
        items.into_iter().rev().find_map(|(_, held)| match held {
            Held::Sounding(index) if self.soundings[*index].is_numeral() => Some(*index),
            _ => None,
        })
    }

    /// music21's `translateMeasureLineToken`.
    fn measure(
        &mut self,
        numbers: &[IntegerType],
        letters: &[String],
        variant: bool,
        data: &str,
        atoms: &[Atom],
    ) -> Result<()> {
        if variant {
            return Ok(());
        }
        let is_copy = data.starts_with('=');
        if numbers[0] > self.last_measure_number + 1 && self.previous.is_some() {
            // Measures the file leaves out hold the last chord still.
            for number in self.last_measure_number + 1..numbers[0] {
                let mut fill = Measure {
                    number,
                    ..Measure::default()
                };
                self.fill_from_previous(&mut fill);
                let letters = self.last_letters.clone();
                let place = self.measures.len();
                self.note_ending(&letters, &mut fill, place)?;
                fill.number = number;
                self.measures.push(fill);
            }
            self.last_measure_number = numbers[0] - 1;
            self.last_letters = letters.to_vec();
        }

        if numbers.len() == 1 && is_copy {
            let (targets, _) = measure_numbers(data.replace('=', "").trim())?;
            if targets.len() > 1 {
                return Err(roman_text_error(
                    "a single measure cannot define a copy operation for multiple measures",
                ));
            }
            let source = self
                .measures
                .iter()
                .position(|measure| measure.number == targets[0])
                .ok_or_else(|| {
                    roman_text_error(format!(
                        "Could not find measure {} to copy from",
                        targets[0]
                    ))
                })?;
            let measure = self.copied(source, numbers[0]);
            if let Some(last) = self.last_numeral(&measure) {
                self.previous = Some(last);
            }
            self.last_measure_number = measure.number;
            self.last_letters = letters.to_vec();
            self.measures.push(measure);
        } else if numbers.len() > 1 {
            let (targets, _) = measure_numbers(data.replace('=', "").trim())?;
            if targets.len() == 1 {
                return Err(roman_text_error(
                    "a multiple measure range cannot copy a single measure",
                ));
            }
            let (start, end) = (targets[0], targets[1]);
            if numbers[1] - numbers[0] != end - start {
                return Err(roman_text_error(
                    "both the source and destination sections need to have the same number of \
                     measures",
                ));
            }
            if numbers[0] < end {
                return Err(roman_text_error(
                    "the source section cannot overlap with the destination section",
                ));
            }
            let mut made: Vec<Measure> = Vec::new();
            for source in 0..self.measures.len() {
                let number = self.measures[source].number;
                if (start..=end).contains(&number) {
                    made.push(self.copied(source, numbers[0] + number - start));
                }
                if number == end {
                    break;
                }
            }
            let Some(last) = made.last() else {
                return Err(roman_text_error(format!(
                    "Could not find measures {start}-{end} to copy from"
                )));
            };
            if let Some(last) = self.last_numeral(last) {
                self.previous = Some(last);
            }
            self.last_measure_number = last.number;
            self.last_letters = letters.to_vec();
            self.measures.extend(made);
        } else {
            let mut measure = self.single(numbers[0], letters, atoms)?;
            if self.measure_length(&measure) == 0.0 {
                self.fill_from_previous(&mut measure);
            }
            self.measures.push(measure);
        }
        Ok(())
    }

    /// music21's `translateSingleMeasure`.
    fn single(
        &mut self,
        number: IntegerType,
        letters: &[String],
        atoms: &[Atom],
    ) -> Result<Measure> {
        let mut measure = Measure {
            number,
            ..Measure::default()
        };
        let place = self.measures.len();
        self.note_ending(letters, &mut measure, place)?;
        self.last_measure_number = number;
        self.last_letters = letters.to_vec();
        if !self.meter_set {
            measure
                .items
                .push((0.0, Held::Element(self.meter.clone().into())));
            self.meter_set = true;
        }
        if !self.signature_placed
            && let Some(signature) = &self.signature
        {
            measure
                .items
                .push((0.0, Held::Element(signature.clone().into())));
            self.signature_placed = true;
        }
        self.offset = 0.0;
        self.previous_in_measure = None;
        self.pivot_possible = false;
        self.key_change = false;
        for (index, atom) in atoms.iter().enumerate() {
            self.atom(atom, &mut measure, index + 1 == atoms.len())?;
        }
        // The last chord read, in whatever measure it stands, lasts what is
        // left of this bar.
        if let Some(previous) = self.previous {
            self.soundings[previous].quarter_length = self.meter.bar_quarter_length() - self.offset;
        }
        Ok(measure)
    }

    /// Gives the chord before this one in the measure the length up to
    /// here.
    fn end_previous(&mut self, measure: &Measure) -> Result<()> {
        let Some(previous) = self.previous_in_measure else {
            return Ok(());
        };
        let start = measure
            .items
            .iter()
            .find_map(|(offset, held)| match held {
                Held::Sounding(index) if *index == previous => Some(*offset),
                _ => None,
            })
            .unwrap_or(0.0);
        let length = self.offset - start;
        if length <= 0.0 {
            return Err(roman_text_error("too many notes in this measure"));
        }
        self.soundings[previous].quarter_length = length;
        Ok(())
    }

    /// music21's `translateSingleMeasureAtom`.
    fn atom(&mut self, atom: &Atom, measure: &mut Measure, is_last: bool) -> Result<()> {
        let at_start = if measure.number <= 1 {
            0.0
        } else {
            self.offset
        };
        match atom {
            Atom::Key(word) => {
                self.set_key(word)?;
                measure.items.push((
                    at_start,
                    Held::Element(StreamElement::Key(self.key.clone())),
                ));
                self.found_signature = true;
            }
            Atom::AnalyticKey(word) if !self.found_signature => {
                self.set_key(word)?;
                measure.items.push((
                    at_start,
                    Held::Element(StreamElement::Key(self.key.clone())),
                ));
                self.found_signature = true;
            }
            Atom::AnalyticKey(word) => self.set_key(word)?,
            Atom::KeySignature(word) => {
                let sharps = word[2..].parse::<IntegerType>().map_err(|_| {
                    roman_text_error(format!("cannot get a key signature from {word}"))
                })?;
                measure
                    .items
                    .push((at_start, Held::Element(KeySignature::new(sharps).into())));
                self.found_signature = true;
            }
            Atom::Beat(word) => {
                let offset = self.meter.offset_from_beat(beat_of(word)?).unwrap_or(0.0);
                // A measure that opens after its first beat holds the last
                // chord until then.
                if self.previous_in_measure.is_none()
                    && offset > 0.0
                    && let Some(made) = self.continued(offset)
                {
                    self.previous_in_measure = Some(made);
                    measure.items.push((0.0, Held::Sounding(made)));
                }
                self.pivot_possible = false;
                self.offset = offset;
            }
            Atom::NoChord => {
                self.meter_at_last_chord = self.meter.clone();
                measure.items.push((
                    self.offset,
                    Held::Element(ChordSymbol::no_chord(None).into()),
                ));
                if !self.pivot_possible {
                    self.end_previous(measure)?;
                    self.prefix.clear();
                    let made = self.add(Sounding {
                        sound: Sound::Rest,
                        quarter_length: 1.0,
                        lyric: None,
                        tie: None,
                    });
                    measure.items.push((self.offset, Held::Sounding(made)));
                    self.previous_in_measure = Some(made);
                    self.previous = Some(made);
                }
            }
            Atom::Chord(figure) => {
                self.meter_at_last_chord = self.meter.clone();
                // Read now, so that a figure that cannot be read is refused
                // where it stands.
                let _ = RomanNumeral::with_minor_defaults(
                    figure.as_str(),
                    self.key.clone(),
                    self.sixth,
                    self.seventh,
                )?;
                let follows_key_change = std::mem::take(&mut self.key_change);
                if self.pivot_possible {
                    // Two chords with no beat between are one chord read in
                    // two keys.
                    if let Some(previous) = self.previous_in_measure {
                        let prefix = std::mem::take(&mut self.prefix);
                        let held = &mut self.soundings[previous];
                        held.lyric = Some(format!(
                            "{}//{prefix}{figure}",
                            held.lyric.clone().unwrap_or_default()
                        ));
                        if let Sound::Numeral { pivot, .. } = &mut held.sound {
                            *pivot = Some(self.key.clone());
                        }
                    }
                    self.pivot_possible = false;
                } else {
                    self.end_previous(measure)?;
                    let prefix = std::mem::take(&mut self.prefix);
                    let made = self.add(Sounding {
                        sound: Sound::Numeral {
                            figure: figure.clone(),
                            key: self.key.clone(),
                            sixth: self.sixth,
                            seventh: self.seventh,
                            follows_key_change,
                            pivot: None,
                        },
                        quarter_length: 1.0,
                        lyric: Some(format!("{prefix}{figure}")),
                        tie: None,
                    });
                    measure.items.push((self.offset, Held::Sounding(made)));
                    self.previous_in_measure = Some(made);
                    self.previous = Some(made);
                    self.pivot_possible = true;
                }
            }
            Atom::RepeatStart | Atom::RepeatStop => {
                let stop = *atom == Atom::RepeatStop;
                if (stop && is_last)
                    || (stop
                        && self.offset != 0.0
                        && self.meter.bar_quarter_length() == self.offset)
                {
                    measure.right = Some(Barline::repeat(RepeatDirection::End, None));
                } else if self.offset == 0.0 && !stop {
                    measure.left = Some(Barline::repeat(RepeatDirection::Start, None));
                }
            }
            Atom::Other => {}
        }
        Ok(())
    }

    fn set_key(&mut self, word: &str) -> Result<()> {
        let (key, prefix) = key_and_prefix(word)?;
        self.key = key;
        self.prefix.push_str(&prefix);
        self.key_change = true;
        Ok(())
    }

    /// music21's `translateOneLineToken`.
    fn token(&mut self, token: &Token) -> Result<()> {
        let (tag, data) = match token {
            Token::Measure {
                numbers,
                letters,
                variant,
                data,
                atoms,
            } => return self.measure(numbers, letters, *variant, data, atoms),
            Token::Tagged { tag, data } => (tag.to_lowercase(), data),
        };
        let text = |metadata: &mut Metadata, name: &str| {
            metadata.add(name, MetadataValue::new(data.clone()));
        };
        match tag.as_str() {
            "title" => text(&mut self.metadata, "title"),
            "work" | "madrigal" | "piece" => text(&mut self.metadata, "alternativeTitle"),
            "composer" => self.metadata.add_contributor("composer", data.clone()),
            "movement" => text(&mut self.metadata, "movementNumber"),
            "analyst" => self.metadata.add_contributor("analyst", data.clone()),
            "proofreader" | "proof reader" => {
                self.metadata.add_contributor("proofreader", data.clone());
            }
            "timesignature" | "time signature" => {
                let meter = match data.as_str() {
                    "C" | "c" => Some(TimeSignature::common()),
                    "C|" | "Cut" | "cut" => Some(TimeSignature::cut()),
                    other => TimeSignature::from_ratio_string(other).ok(),
                };
                // A meter that cannot be read leaves the one in force.
                if let Some(meter) = meter {
                    self.meter = meter;
                    self.meter_set = false;
                }
            }
            "keysignature" | "key signature" => {
                let sharps = match data.as_str() {
                    "" => 0,
                    "Bb" => -1,
                    other => other.parse::<IntegerType>().map_err(|_| {
                        roman_text_error(format!("Cannot parse key signature: {other:?}"))
                    })?,
                };
                self.signature = Some(KeySignature::new(sharps));
                self.signature_placed = false;
                self.found_signature = true;
            }
            "sixthminor" | "sixth minor" | "seventhminor" | "seventh minor" => {
                let sixth = tag.starts_with("sixth");
                let reading = match data.to_lowercase().as_str() {
                    "flat" => Minor67Default::Flat,
                    "sharp" => Minor67Default::Sharp,
                    "quality" => Minor67Default::Quality,
                    "courtesy" | "cautionary" => Minor67Default::Cautionary,
                    "harmonic" if sixth => Minor67Default::Flat,
                    "harmonic" => Minor67Default::Sharp,
                    other => {
                        return Err(roman_text_error(format!(
                            "Cannot parse setting vi or vii parsing: {other:?}"
                        )));
                    }
                };
                if sixth {
                    self.sixth = reading;
                } else {
                    self.seventh = reading;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// music21's `fixPickupMeasure`: a measure numbered nought that opens
    /// after its first beat starts where its first chord does, and the last
    /// measure is cut short by as much.
    fn fix_pickup(&mut self) {
        let Some(first) = self.measures.iter().position(|measure| measure.number == 0) else {
            return;
        };
        let is_chord = |held: &Held, soundings: &[Sounding]| match held {
            Held::Sounding(index) => soundings[*index].is_numeral(),
            Held::Element(element) => matches!(element, StreamElement::ChordSymbol(_)),
        };
        let mut chords: Vec<FloatType> = self.measures[first]
            .items
            .iter()
            .filter(|(_, held)| is_chord(held, &self.soundings))
            .map(|(offset, _)| *offset)
            .collect();
        chords.sort_by(FloatType::total_cmp);
        let Some(padding) = chords.first().copied().filter(|offset| *offset != 0.0) else {
            return;
        };
        for (offset, _) in &mut self.measures[first].items {
            if *offset >= padding {
                *offset -= padding;
            }
        }
        self.measures[first].padding_left = padding;
        let last = self.measures.len() - 1;
        if last == first {
            return;
        }
        let mut sorted: Vec<&(FloatType, Held)> = self.measures[last].items.iter().collect();
        sorted.sort_by(|left, right| left.0.total_cmp(&right.0));
        let closing = sorted.into_iter().rev().find_map(|(_, held)| match held {
            Held::Sounding(index) if self.soundings[*index].is_numeral() => Some(Some(*index)),
            Held::Element(StreamElement::ChordSymbol(_)) => Some(None),
            _ => None,
        });
        if let Some(Some(index)) = closing
            && self.soundings[index].quarter_length > padding
        {
            let length = self.measure_length(&self.measures[last]);
            self.soundings[index].quarter_length -= length - padding;
            self.measures[last].padding_right = length - padding;
        }
    }

    /// The part, once every line is read: music21's `translateTokens`.
    fn part(mut self) -> Result<(Stream, Metadata)> {
        self.fix_pickup();

        // music21's `_consolidateRepeatEndings`: each run of measures in a
        // row under one ending is a bracket.
        let mut brackets: Vec<(Vec<usize>, u32)> = Vec::new();
        for (number, measures) in &self.endings {
            let mut run: Vec<usize> = Vec::new();
            let mut last: Option<IntegerType> = None;
            for (measure_number, place) in measures {
                if last.is_some_and(|last| *measure_number > last + 1) {
                    brackets.push((std::mem::take(&mut run), *number));
                }
                run.push(*place);
                last = Some(*measure_number);
            }
            if !run.is_empty() {
                brackets.push((run, *number));
            }
        }
        let mut endings: Vec<Option<Ending>> = vec![None; self.measures.len()];
        for (places, number) in &brackets {
            for (index, place) in places.iter().enumerate() {
                if let Some(slot) = endings.get_mut(*place) {
                    *slot = Some(Ending::new(
                        vec![*number],
                        index == 0,
                        index + 1 == places.len(),
                    ));
                }
            }
            if *number == 1
                && let Some(measure) = places
                    .last()
                    .and_then(|place| self.measures.get_mut(*place))
                && measure.right.is_none()
            {
                measure.right = Some(Barline::repeat(RepeatDirection::End, None));
            }
        }

        let mut events = Vec::new();
        let mut offset = 0.0;
        for (measure, ending) in self.measures.iter().zip(endings) {
            let mut held = Vec::new();
            for (at, item) in &measure.items {
                let element = match item {
                    Held::Sounding(index) => self.soundings[*index].element()?,
                    Held::Element(element) => element.clone(),
                };
                held.push(StreamEvent::new(*at, element));
            }
            let mut stream =
                Stream::with_kind(StreamKind::Measure).with_events(sorted_events(held));
            stream.set_number(measure.number);
            stream.set_number_suffix(measure.suffix.clone());
            stream.set_padding_left(measure.padding_left);
            stream.set_padding_right(measure.padding_right);
            stream.set_left_barline(measure.left.clone());
            stream.set_right_barline(measure.right.clone());
            stream.set_ending(ending);
            let length = stream.end_offset();
            events.push(StreamEvent::new(offset, stream));
            offset = op_frac(offset + length);
        }
        let mut part = Stream::with_kind(StreamKind::Part).with_events(events);
        // music21 goes on where a measure cannot be beamed.
        let _ = make_beams(&mut part);
        make_accidentals(&mut part);
        Ok((part, self.metadata))
    }
}

fn score_of(tokens: &[Token]) -> Result<Stream> {
    let mut translator = Translator::new()?;
    for token in tokens {
        translator.token(token)?;
    }
    let (part, metadata) = translator.part()?;
    let mut score =
        Stream::with_kind(StreamKind::Score).with_events(vec![StreamEvent::new(0.0, part)]);
    score.set_metadata(Some(metadata));
    Ok(score)
}

/// Reads a RomanText analysis into a score: music21's
/// `converter.parse` of one.
///
/// The score holds one part of measures, and in each the chords the
/// numerals stand for, in the key they were written in, each carrying its
/// figure as its lyric -- the key too where it has just changed, as
/// `G: V7`. A chord lasts until the next, or to the end of its bar; one
/// held into a measure that opens after its first beat is tied there. `NC`
/// is a rest under a no-chord symbol. A measure written `m5 = m1` is a copy
/// of another, its chords read in the key now in force.
///
/// A file of several `Movement:` lines comes back as an opus of scores, the
/// lines before the first heading each of them.
///
/// ```
/// use music21_rs::romantext::from_roman_text;
///
/// let score = from_roman_text("Time Signature: 3/4\nm1 G: I b3 V6\nm2 I\n")?;
/// let part = score.parts()[0];
/// assert_eq!(part.measures().len(), 2);
/// let names: Vec<String> = part.measures()[0]
///     .pitches()
///     .iter()
///     .map(|pitch| pitch.name())
///     .collect();
/// assert_eq!(names, ["G", "B", "D", "F#", "A", "D"]);
/// # Ok::<(), music21_rs::Error>(())
/// ```
pub fn from_roman_text(text: &str) -> Result<Stream> {
    let tokens = tokenize(text)?;
    let movements = tokens.iter().filter(|token| token.is_movement()).count();
    if movements < 2 {
        return score_of(&tokens);
    }
    // Each movement takes the lines before the first heading as its own.
    let mut pieces: Vec<Vec<Token>> = Vec::new();
    let mut current: Vec<Token> = Vec::new();
    for token in tokens {
        if token.is_movement() {
            pieces.push(std::mem::take(&mut current));
        }
        current.push(token);
    }
    if !current.is_empty() {
        pieces.push(current);
    }
    let head = pieces.remove(0);
    let mut events = Vec::new();
    let mut offset = 0.0;
    for piece in pieces {
        let tokens: Vec<Token> = head.iter().cloned().chain(piece).collect();
        let score = score_of(&tokens)?;
        let length = score.end_offset();
        events.push(StreamEvent::new(offset, score));
        offset = op_frac(offset + length);
    }
    Ok(Stream::with_kind(StreamKind::Opus).with_events(events))
}

/// Writes a score's roman numerals as a RomanText analysis, as music21's
/// `romanText.writeRoman.RnWriter` writes one, a line after each line.
///
/// The header names the composer, the title (with the movement's number and
/// name), the analyst and the proofreader, as the score's metadata gives
/// them. Then measure by measure: a `Time Signature:` line where a meter
/// stands, and a line of the measure's numerals -- each chord standing for
/// one ([`Chord::numeral`]) at its beat, `b2.5`, beat one left unsaid, the
/// key written before a numeral where it is not the key of the one before,
/// and a chord tied from the one before passed over -- with `||:` and
/// `:||` where a repeat starts and ends. A score is written from its first
/// part, an opus score by score.
///
/// ```
/// use music21_rs::romantext::{from_roman_text, to_roman_text};
///
/// let score = from_roman_text("Time Signature: 3/4\nm1 G: I b3 V6\nm2 I\n")?;
/// let written = to_roman_text(&score)?;
/// assert!(written.ends_with("Time Signature: 3/4\nm1 G: I b3 V6\nm2 I\n"));
/// # Ok::<(), music21_rs::Error>(())
/// ```
///
/// # Errors
///
/// A numeral standing where a meter has no beat.
pub fn to_roman_text(stream: &Stream) -> Result<String> {
    let mut lines = Vec::new();
    write_lines(stream, &mut lines)?;
    Ok(lines.iter().map(|line| format!("{line}\n")).collect())
}

/// `RnWriter`'s `combinedList` for one stream.
fn write_lines(stream: &Stream, lines: &mut Vec<String>) -> Result<()> {
    if stream.kind() == StreamKind::Opus {
        for event in stream.events() {
            if let StreamElement::Stream(score) = event.element() {
                write_lines(score, lines)?;
                lines.push("\n".to_string());
            }
        }
        return Ok(());
    }
    let wrapped;
    let container: &Stream = match stream.kind() {
        StreamKind::Score => stream.parts().first().copied().unwrap_or(stream),
        StreamKind::Part | StreamKind::PartStaff => stream,
        StreamKind::Measure => {
            let mut part = Stream::with_kind(StreamKind::Part);
            part.insert(0.0, stream.clone());
            wrapped = part;
            &wrapped
        }
        _ => {
            // music21 appends everything the stream holds to one measure.
            let mut measure = Stream::with_kind(StreamKind::Measure);
            for event in stream.events() {
                measure.push(event.element().clone());
            }
            let mut part = Stream::with_kind(StreamKind::Part);
            part.insert(0.0, measure);
            wrapped = part;
            &wrapped
        }
    };

    let mut composer = "Composer unknown".to_string();
    let mut title = "Title unknown".to_string();
    let mut analyst = String::new();
    let mut proofreader = String::new();
    if let Some(metadata) = stream.metadata() {
        if let Some(prepared) = written_title(metadata) {
            title = prepared;
        }
        if let Some(name) = contributor(metadata, "composer") {
            composer = name;
        }
        if let Some(name) = contributor(metadata, "analyst") {
            analyst = name;
        }
        if let Some(name) = contributor(metadata, "proofreader") {
            proofreader = name;
        }
    }
    lines.push(format!("Composer: {composer}"));
    lines.push(format!("Title: {title}"));
    lines.push(format!("Analyst: {analyst}"));
    lines.push(format!("Proofreader: {proofreader}"));
    lines.push(String::new());

    // A meter standing in the part itself is in force from its start; with
    // none anywhere, music21 puts a 4/4 there.
    let mut meter = container
        .events()
        .iter()
        .find_map(|event| match event.element() {
            StreamElement::TimeSignature(meter) => Some(meter.clone()),
            _ => None,
        });
    let any_meter = container
        .leaves()
        .iter()
        .any(|(_, element)| matches!(element, StreamElement::TimeSignature(_)));
    if !any_meter {
        meter = Some(TimeSignature::new(4, 4)?);
    }

    let mut key_string = String::new();
    for measure in container.measures() {
        let meters: Vec<(FloatType, &TimeSignature)> = measure
            .events()
            .iter()
            .filter_map(|event| match event.element() {
                StreamElement::TimeSignature(meter) => Some((event.offset(), meter)),
                _ => None,
            })
            .collect();
        if let Some((_, first)) = meters.first() {
            lines.push(format!("Time Signature: {}", first.ratio_string()));
            if meters.len() > 1 {
                let rest: Vec<String> = meters[1..]
                    .iter()
                    .map(|(_, meter)| format!("'{}'", meter.ratio_string()))
                    .collect();
                lines.push(format!(
                    "Note: further time signature change(s) unprocessed: [{}]",
                    rest.join(", ")
                ));
            }
        }
        let number = format!(
            "{}{}",
            measure.number(),
            measure.number_suffix().unwrap_or("")
        );
        let mut line = String::new();
        if measure
            .left_barline()
            .is_some_and(|barline| barline.repeat_direction() == Some(RepeatDirection::Start))
        {
            push_chord_string(&mut line, &number, 1.0, "||:");
        }
        let mut last_beat = None;
        for event in measure.events() {
            let StreamElement::Chord(chord) = event.element() else {
                continue;
            };
            let Some(numeral) = chord.numeral() else {
                continue;
            };
            let at = event.offset();
            // The meter in force: the last standing at or before the
            // numeral in this measure, else the one carried in.
            let in_force = meters
                .iter()
                .rev()
                .find(|(offset, _)| *offset <= at)
                .map(|(_, meter)| (*meter).clone())
                .or_else(|| meter.clone())
                .ok_or_else(|| roman_text_error("a roman numeral with no meter in force"))?;
            let beat = in_force.beat_proportion(op_frac(at + measure.padding_left()))?;
            last_beat = Some(beat);
            if chord
                .tie()
                .is_some_and(|tie| tie.tie_type() != TieType::Start)
            {
                continue;
            }
            let key = numeral.key().tonic_pitch_name_with_case().replace('-', "b");
            let written = if key == key_string {
                numeral.figure().to_string()
            } else {
                key_string = key.clone();
                format!("{key}: {}", numeral.figure())
            };
            push_chord_string(&mut line, &number, beat, &written);
        }
        if measure
            .right_barline()
            .is_some_and(|barline| barline.repeat_direction() == Some(RepeatDirection::End))
        {
            push_chord_string(&mut line, &number, last_beat.unwrap_or(1.0), ":||");
        }
        if !line.is_empty() {
            lines.push(line);
        }
        if let Some((_, last)) = meters.last() {
            meter = Some((*last).clone());
        }
    }
    Ok(())
}

/// `rnString`: a chord string added to a measure's line, its beat said
/// where it is not the first.
fn push_chord_string(line: &mut String, number: &str, beat: FloatType, chord: &str) {
    if line.is_empty() {
        line.push('m');
        line.push_str(number);
    }
    let beat = int_beat(beat);
    if beat == "1" {
        line.push_str(&format!(" {chord}"));
    } else {
        line.push_str(&format!(" b{beat} {chord}"));
    }
}

/// `intBeat`: a beat as a whole number where it is one, else rounded to two
/// places, written as Python writes a float.
fn int_beat(beat: FloatType) -> String {
    if beat.fract() == 0.0 {
        return format!("{}", beat as i64);
    }
    let rounded = (beat * 100.0).round() / 100.0;
    let written = format!("{rounded}");
    if written.contains('.') {
        written
    } else {
        format!("{written}.0")
    }
}

/// `prepTitle`: the best title, the movement's number and its name where
/// that is not the title.
fn written_title(metadata: &Metadata) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(title) = ["title", "popularTitle", "alternativeTitle", "movementName"]
        .iter()
        .find_map(|name| metadata.first_text(name))
    {
        parts.push(title.to_string());
    }
    if let Some(number) = metadata.first_text("movementNumber") {
        parts.push(format!("- No.{number}:"));
    }
    if let Some(name) = metadata.first_text("movementName")
        && metadata.first_text("title") != Some(name)
    {
        parts.push(name.to_string());
    }
    (!parts.is_empty()).then(|| parts.join(" "))
}

/// The first contributor in a role, filed under the role or among the
/// other contributors.
fn contributor(metadata: &Metadata, role: &str) -> Option<String> {
    metadata
        .first_text(role)
        .or_else(|| {
            metadata
                .get("otherContributor")
                .iter()
                .find(|value| value.role() == Some(role))
                .map(MetadataValue::text)
        })
        .filter(|name| !name.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lengths(measure: &Stream) -> Vec<(FloatType, FloatType)> {
        measure
            .events()
            .iter()
            .filter(|event| event.element().is_note_or_chord())
            .map(|event| (event.offset(), event.element().quarter_length()))
            .collect()
    }

    #[test]
    fn words_of_a_measure_are_told_apart() {
        assert_eq!(
            atoms_of("C: I b2.5 V6 || Bb;: KS-2 NC ||: :|| ?(d: ii"),
            [
                Atom::AnalyticKey("C:".to_string()),
                Atom::Chord("I".to_string()),
                Atom::Beat("b2.5".to_string()),
                Atom::Chord("V6".to_string()),
                Atom::Other,
                Atom::Key("Bb;:".to_string()),
                Atom::KeySignature("KS-2".to_string()),
                Atom::NoChord,
                Atom::RepeatStart,
                Atom::RepeatStop,
                Atom::Other,
                Atom::Chord("ii".to_string()),
            ]
        );
    }

    #[test]
    fn a_beat_may_be_a_third_written_to_two_places() {
        assert_eq!(beat_of("b2").unwrap(), 2.0);
        assert_eq!(beat_of("b2.5").unwrap(), 2.5);
        assert!((beat_of("b1.66").unwrap() - 5.0 / 3.0).abs() < 1e-9);
        assert!((beat_of("b1.66.5").unwrap() - 11.0 / 6.0).abs() < 1e-9);
    }

    #[test]
    fn a_chord_lasts_until_the_next_or_the_barline() {
        let score = from_roman_text("Time Signature: 4/4\nm1 C: I b3 V\nm2 I\n").unwrap();
        let part = score.parts()[0];
        let measures = part.measures();
        assert_eq!(lengths(measures[0]), [(0.0, 2.0), (2.0, 2.0)]);
        assert_eq!(lengths(measures[1]), [(0.0, 4.0)]);
    }

    #[test]
    fn a_measure_opening_late_holds_the_last_chord_tied() {
        let score = from_roman_text("m1 C: I\nm2 b3 V\n").unwrap();
        let part = score.parts()[0];
        let second = part.measures()[1];
        assert_eq!(lengths(second), [(0.0, 2.0), (2.0, 2.0)]);
        let StreamElement::Chord(held) = second.events()[0].element() else {
            panic!("the measure opens with the chord held over");
        };
        assert_eq!(held.tie().map(Tie::tie_type), Some(TieType::Stop));
    }

    #[test]
    fn a_measure_may_copy_another() {
        let score = from_roman_text("m1 C: I b3 V\nm2 IV\nm3 = m1\n").unwrap();
        let part = score.parts()[0];
        let measures = part.measures();
        assert_eq!(measures.len(), 3);
        assert_eq!(measures[2].number(), 3);
        assert_eq!(measures[2].pitches(), measures[0].pitches());
    }

    #[test]
    fn text_with_no_measure_is_refused() {
        assert!(from_roman_text("Composer: nobody\n").is_err());
    }
}
