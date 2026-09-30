//! TinyNotation: music21's `tinyNotation`, a line of notes written as
//! letters and numbers.
//!
//! `4/4 c4 d8 e f#2 trip{g8 a b} c'1` is a bar of quarter, two eighths and a
//! half, then a triplet and a whole note an octave up. A capital letter is
//! the octave below middle C and each letter more an octave lower; a small
//! one is the octave from middle C and each `'` an octave higher. A number
//! is the note value, kept until the next one; `r` is a rest; `~` ties a
//! note to the next; `_word` is a lyric.

use crate::defaults::{FloatType, IntegerType};
use crate::duration::{Duration, DurationType, Tuplet, TupletType};
use crate::error::{Error, Result};
use crate::expressions::{Expression, Fermata};
use crate::makenotation::{make_measures, op_frac};
use crate::meter::TimeSignature;
use crate::notation::{Tie, TieType};
use crate::note::Note;
use crate::pitch::{Accidental, PitchOptions};
use crate::rest::Rest;
use crate::stream::{Stream, StreamElement, StreamEvent, StreamKind};

/// Something a bracket or a tie holds open over the tokens after it.
enum State {
    /// `trip{` or `quad{`: the notes inside are so many in the time of so
    /// many.
    Tuplet {
        actual: u32,
        normal: u32,
        tokens: Vec<usize>,
    },
    /// `~`: this note and the next are tied.
    Tie { tokens: Vec<usize> },
}

fn is_modifier_free(character: char) -> bool {
    !character.is_whitespace() && character != '_'
}

/// Every place the modifiers starting at `at` may end, as music21's
/// modifier pattern has it, down to the end of the token.
fn modifiers_reach_end(text: &[char], at: usize) -> bool {
    if at == text.len() {
        return true;
    }
    let closed_by = |close: char| -> bool {
        (at + 1..text.len())
            .filter(|index| text[*index] == close)
            .any(|index| modifiers_reach_end(text, index + 1))
    };
    match text[at] {
        '=' => {
            let mut end = at + 1;
            while end < text.len() && is_modifier_free(text[end]) {
                end += 1;
            }
            (at + 1..=end)
                .rev()
                .any(|stop| modifiers_reach_end(text, stop))
        }
        '*' => closed_by('*'),
        '<' => closed_by('>'),
        '(' => closed_by(')'),
        '[' => closed_by(']'),
        '_' => {
            at + 1 < text.len()
                && !text[at + 1].is_whitespace()
                && text[at + 1] != '='
                && modifiers_reach_end(text, at + 2)
        }
        _ => false,
    }
}

/// Where a duration written at `at` may end: a number and dots, dots and a
/// number, dots alone, or nothing.
fn duration_ends(text: &[char], at: usize) -> Vec<usize> {
    let run = |from: usize, test: fn(&char) -> bool| -> usize {
        from + text[from..].iter().take_while(|c| test(c)).count()
    };
    let digit: fn(&char) -> bool = |c| c.is_ascii_digit();
    let dot: fn(&char) -> bool = |c| *c == '.';
    let mut ends = vec![at];
    let digits = run(at, digit);
    if digits > at {
        for stop in at + 1..=digits {
            for dots in stop..=run(stop, dot) {
                ends.push(dots);
            }
        }
    }
    let dots = run(at, dot);
    if dots > at {
        for stop in at + 1..=dots {
            ends.push(stop);
            for number in stop + 1..=run(stop, digit) {
                ends.push(number);
            }
        }
    }
    ends
}

/// Where an accidental at `at` may end: sharps, flats or an `n`, bare or in
/// parentheses, or nothing.
fn accidental_ends(text: &[char], at: usize) -> Vec<usize> {
    let mut ends = vec![at];
    let bare = |from: usize| -> Vec<usize> {
        match text.get(from) {
            Some('n') => vec![from + 1],
            Some(sign @ ('#' | '-')) => {
                let run = text[from..].iter().take_while(|c| *c == sign).count();
                (from + 1..=from + run).collect()
            }
            _ => Vec::new(),
        }
    };
    ends.extend(bare(at));
    if text.get(at) == Some(&'(') {
        ends.extend(
            bare(at + 1)
                .into_iter()
                .filter(|end| text.get(*end) == Some(&')'))
                .map(|end| end + 1),
        );
    }
    ends
}

/// Where a note's name may end.
fn name_ends(text: &[char]) -> Vec<usize> {
    let Some(first) = text.first() else {
        return Vec::new();
    };
    if ('A'..='G').contains(first) {
        let run = text.iter().take_while(|c| *c == first).count();
        return (1..=run)
            .flat_map(|stop| accidental_ends(text, stop))
            .collect();
    }
    if ('a'..='g').contains(first) {
        let ticks = |from: usize| text[from..].iter().take_while(|c| **c == '\'').count();
        let mut ends = Vec::new();
        for after in accidental_ends(text, 1) {
            for stop in after..=after + ticks(after) {
                ends.push(stop);
            }
        }
        for stop in 1..=1 + ticks(1) {
            ends.extend(accidental_ends(text, stop));
        }
        return ends;
    }
    Vec::new()
}

/// Whether the text is a note as music21's token pattern reads one.
fn is_note_token(text: &[char]) -> bool {
    name_ends(text).into_iter().any(|name| {
        duration_ends(text, name)
            .into_iter()
            .any(|end| modifiers_reach_end(text, end))
    })
}

/// Whether the text after an `r` is what a rest may carry.
fn is_rest_body(text: &[char]) -> bool {
    duration_ends(text, 0)
        .into_iter()
        .any(|end| modifiers_reach_end(text, end))
}

fn is_meter_token(text: &str) -> bool {
    text.split_once('/').is_some_and(|(top, bottom)| {
        !top.is_empty()
            && !bottom.is_empty()
            && top.chars().all(|c| c.is_ascii_digit())
            && bottom.chars().all(|c| c.is_ascii_digit())
    })
}

/// The first run of characters passing a test, as a search for `(x+)`
/// finds it.
fn first_run(text: &str, test: impl Fn(char) -> bool) -> Option<&str> {
    let start = text.find(&test)?;
    let rest = &text[start..];
    let end = rest.find(|c| !test(c)).unwrap_or(rest.len());
    Some(&rest[..end])
}

fn without(text: &str, test: impl Fn(char) -> bool) -> String {
    text.chars().filter(|c| !test(*c)).collect()
}

/// The reader's state: music21's `Converter`.
struct Reader {
    elements: Vec<StreamElement>,
    meter: Option<TimeSignature>,
    last_length: FloatType,
    states: Vec<State>,
}

fn with_duration(element: &mut StreamElement, change: impl FnOnce(&mut Duration)) {
    match element {
        StreamElement::Note(note) => {
            let mut duration = note.duration().cloned().unwrap_or_else(Duration::quarter);
            change(&mut duration);
            note.set_duration(duration);
        }
        StreamElement::Rest(rest) => {
            let mut duration = rest.duration().clone();
            change(&mut duration);
            rest.set_duration(duration);
        }
        _ => {}
    }
}

/// The written value a duration is, a quarter where it is no one value.
fn written_type(duration: &Duration) -> DurationType {
    match duration.components().as_slice() {
        [(kind, _)] => *kind,
        _ => DurationType::Quarter,
    }
}

impl Reader {
    /// music21's `applyDuration`: the length a token says, or the last one
    /// said. The second answer is whether a whole-bar fermata was asked for.
    fn duration_of(&mut self, text: &str) -> Result<Option<(Duration, bool)>> {
        let mut duration = Duration::quarter();
        let mut fermata = false;
        let mut found = false;
        if let Some(digits) = first_run(text, |c| c.is_ascii_digit()) {
            found = true;
            let number: u64 = digits.parse().map_err(|_| {
                Error::Duration(format!("Cannot parse token with duration {digits}"))
            })?;
            if number == 0 {
                if let Some(meter) = &self.meter {
                    duration = meter.bar_duration();
                    fermata = true;
                }
            } else {
                let kind = (number <= 2048 && number.is_power_of_two())
                    .then(|| {
                        DurationType::ALL
                            .into_iter()
                            .find(|kind| kind.quarter_length() == 4.0 / number as FloatType)
                    })
                    .flatten();
                // A number that is no note value leaves the token unread.
                let Some(kind) = kind else {
                    return Ok(None);
                };
                duration = Duration::from_type(kind);
            }
        }
        if let Some(dots) = first_run(text, |c| c == '.') {
            let kind = written_type(&duration);
            duration = Duration::from_type_with_dots(kind, dots.len() as u32);
        }
        if !found {
            duration = Duration::new(self.last_length)?;
        }
        self.last_length = duration.quarter_length();
        Ok(Some((duration, fermata)))
    }

    /// music21's `NoteToken.parse`.
    fn note(&mut self, token: &str) -> Result<Option<StreamElement>> {
        let mut text = token.to_string();
        let mut step = 'C';
        let mut octave: IntegerType = 4;
        let low = |c: char| ('A'..='G').contains(&c);
        if let Some(run) = first_run(&text, low) {
            step = run.chars().next().unwrap_or('C');
            octave = 4 - run.chars().count() as IntegerType;
            text = without(&text, low);
        }
        let high = |c: char| ('a'..='g').contains(&c);
        if let Some(start) = text.find(high) {
            let letter = text[start..].chars().next().unwrap_or('c');
            let ticks = text[start + 1..].chars().take_while(|c| *c == '\'').count();
            step = letter.to_ascii_uppercase();
            octave = 4 + ticks as IntegerType;
            // Every small letter goes, with the marks after it.
            let mut kept = String::new();
            let mut after_letter = false;
            for character in text.chars() {
                if high(character) {
                    after_letter = true;
                } else if character == '\'' && after_letter {
                } else {
                    after_letter = false;
                    kept.push(character);
                }
            }
            text = kept;
        }
        // An accidental in parentheses is editorial, and is not the note's.
        let mut editorial = false;
        let signs = |c: char| matches!(c, '#' | '-' | 'n');
        let chars: Vec<char> = text.chars().collect();
        let found = (0..chars.len()).find_map(|open| {
            if chars[open] != '(' {
                return None;
            }
            let run = chars[open + 1..].iter().take_while(|c| signs(**c)).count();
            (run > 0 && chars.get(open + 1 + run) == Some(&')')).then_some((open, run))
        });
        if let Some((open, run)) = found {
            editorial = true;
            text = chars[open + 1..open + 1 + run]
                .iter()
                .chain(&chars[open + run + 2..])
                .collect();
        }
        let mut alter: Option<IntegerType> = None;
        for (sign, direction) in [('#', 1), ('-', -1), ('n', 0)] {
            let is_sign = |c: char| c == sign;
            if let Some(run) = first_run(&text, is_sign) {
                let count = if sign == 'n' { 1 } else { run.len() };
                alter = Some(direction * count as IntegerType);
                text = without(&text, is_sign);
            }
        }
        let mut pitch = PitchOptions::new().step(step).octave(octave).build()?;
        if let Some(alter) = alter
            && !editorial
        {
            pitch.set_written_accidental(Some(Accidental::new(alter)?));
        }
        let Some((duration, fermata)) = self.duration_of(&text)? else {
            return Ok(None);
        };
        let mut note = Note::from_pitch(pitch).with_duration(duration);
        if fermata {
            note.expressions_mut()
                .push(Expression::Fermata(Fermata::new()));
        }
        Ok(Some(note.into()))
    }

    fn rest(&mut self, body: &str) -> Result<Option<StreamElement>> {
        let Some((duration, fermata)) = self.duration_of(body)? else {
            return Ok(None);
        };
        let mut rest = Rest::new(duration);
        if fermata {
            rest.expressions_mut()
                .push(Expression::Fermata(Fermata::new()));
        }
        Ok(Some(rest.into()))
    }

    /// What closing a state does: music21's `State.end`.
    fn end(&mut self, state: State) -> Result<()> {
        match state {
            State::Tie { tokens } => {
                let opened = |tie: Option<&Tie>| match tie.cloned() {
                    None => Tie::new(TieType::Start),
                    Some(mut tie) => {
                        tie.set_tie_type(TieType::Continue);
                        tie
                    }
                };
                match tokens.first().map(|index| &mut self.elements[*index]) {
                    Some(StreamElement::Note(first)) => {
                        let tie = opened(first.tie());
                        first.set_tie(Some(tie));
                    }
                    Some(StreamElement::Rest(first)) => {
                        let tie = opened(first.tie());
                        first.set_tie(Some(tie));
                    }
                    Some(_) => {
                        return Err(Error::Meter("a time signature cannot be tied".to_string()));
                    }
                    None => {}
                }
                match tokens.get(1).map(|index| &mut self.elements[*index]) {
                    Some(StreamElement::Note(second)) => {
                        second.set_tie(Some(Tie::new(TieType::Stop)));
                    }
                    Some(StreamElement::Rest(second)) => {
                        second.set_tie(Some(Tie::new(TieType::Stop)));
                    }
                    _ => {}
                }
            }
            State::Tuplet { tokens, .. } => {
                let mark = |element: &mut StreamElement, kind: TupletType| {
                    with_duration(element, |duration| {
                        let mut tuplets = duration.tuplets();
                        if let Some(first) = tuplets.first_mut() {
                            first.set_tuplet_type(Some(kind));
                        }
                        duration.set_tuplets(tuplets);
                    });
                };
                if let (Some(first), Some(last)) = (tokens.first(), tokens.last()) {
                    mark(&mut self.elements[*first], TupletType::Start);
                    mark(&mut self.elements[*last], TupletType::Stop);
                }
            }
        }
        Ok(())
    }

    /// music21's `parseOne`.
    fn token(&mut self, token: &str) -> Result<()> {
        let mut text = token.to_string();

        // Brackets opening: a word and a brace, taken out one at a time.
        loop {
            let chars: Vec<char> = text.chars().collect();
            let Some(brace) = (1..chars.len()).find(|index| {
                chars[*index] == '{'
                    && (chars[index - 1].is_alphanumeric() || chars[index - 1] == '_')
            }) else {
                break;
            };
            let start = (0..brace)
                .rev()
                .take_while(|index| chars[*index].is_alphanumeric() || chars[*index] == '_')
                .last()
                .unwrap_or(brace);
            let word: String = chars[start..brace].iter().collect();
            text = chars[..start].iter().chain(&chars[brace + 1..]).collect();
            let ratio = match word.as_str() {
                "trip" => Some((3, 2)),
                "quad" => Some((4, 3)),
                // A bracket of no known kind is dropped.
                _ => None,
            };
            if let Some((actual, normal)) = ratio {
                self.states.push(State::Tuplet {
                    actual,
                    normal,
                    tokens: Vec::new(),
                });
            }
        }
        if text.contains('~') {
            text = text.replace('~', "");
            self.states.push(State::Tie { tokens: Vec::new() });
        }

        let closing = text.matches('}').count();
        text = text.replace('}', "");

        // `=name` names the element.
        let mut id: Option<String> = None;
        if text.contains('=') {
            let mut kept = String::new();
            let mut rest = &text[..];
            while let Some(at) = rest.find('=') {
                kept.push_str(&rest[..at]);
                let after = &rest[at + 1..];
                let end = after
                    .find(|c: char| !c.is_ascii_alphanumeric())
                    .unwrap_or(after.len());
                id.get_or_insert_with(|| after[..end].to_string());
                rest = &after[end..];
            }
            kept.push_str(rest);
            text = kept;
        }
        let mut lyric: Option<String> = None;
        if let Some(start) = text.find('_') {
            lyric = Some(text[start + 1..].to_string());
            text.truncate(start);
        }

        let chars: Vec<char> = text.chars().collect();
        let mut made: Option<StreamElement> = None;
        if is_meter_token(&text) {
            let meter = TimeSignature::from_ratio_string(&text)?;
            self.meter = Some(meter.clone());
            made = Some(meter.into());
        } else if chars.first() == Some(&'r') && is_rest_body(&chars[1..]) {
            made = self.rest(&text[1..])?;
        }
        if made.is_none() && is_note_token(&chars) {
            made = self.note(&text)?;
        }

        if let Some(mut element) = made {
            if let (Some(id), StreamElement::Note(note)) = (&id, &mut element) {
                note.set_id(Some(id.clone()));
            }
            if let Some(lyric) = &lyric {
                match &mut element {
                    StreamElement::Note(note) => note.set_lyric(Some(lyric))?,
                    StreamElement::Rest(rest) => rest.set_lyric(Some(lyric)),
                    _ => {}
                }
            }
            // A meter passes through the open states like a note: it takes
            // the place of one in a tie or at the end of a bracket.
            let index = self.elements.len();
            self.elements.push(element);
            {
                let mut place = 0;
                while place < self.states.len() {
                    let expired = match &mut self.states[place] {
                        State::Tuplet {
                            actual,
                            normal,
                            tokens,
                        } => {
                            tokens.push(index);
                            let (actual, normal) = (*actual, *normal);
                            let element = &mut self.elements[index];
                            let values = element
                                .duration()
                                .map_or(1, |duration| duration.components().len());
                            if values != 1 && !matches!(element, StreamElement::TimeSignature(_)) {
                                return Err(Error::Duration("Unknown type: complex".to_string()));
                            }
                            with_duration(element, |duration| {
                                let kind = written_type(duration);
                                duration.append_tuplet(Tuplet::new(actual, normal, kind, 0));
                            });
                            false
                        }
                        State::Tie { tokens } => {
                            tokens.push(index);
                            tokens.len() == 2
                        }
                    };
                    if expired {
                        let state = self.states.remove(place);
                        self.end(state)?;
                    } else {
                        place += 1;
                    }
                }
            }
        }

        for _ in 0..closing.min(self.states.len()) {
            if let Some(state) = self.states.pop() {
                self.end(state)?;
            }
        }
        Ok(())
    }
}

/// Reads a line of TinyNotation into a part of measures: music21's
/// `converter.parse('tinyNotation: ...')`.
///
/// The text may start with `tinyNotation:`. A token that is not a meter, a
/// note or a rest is passed over, as music21 passes over it. The part is cut
/// into measures by [`make_measures`]: a line with no meter is in `4/4`,
/// and a note running past a barline is left whole in the measure it starts
/// in.
///
/// ```
/// use music21_rs::tinynotation::from_tiny_notation;
///
/// let part = from_tiny_notation("tinyNotation: 3/4 E4 r f# g=lastG trip{b-8 a g} c'2.")?;
/// let measures = part.measures();
/// assert_eq!(measures.len(), 3);
/// let names: Vec<String> = measures[1]
///     .pitches()
///     .iter()
///     .map(|pitch| pitch.name_with_octave())
///     .collect();
/// // The last note starts in this measure and runs on past its barline.
/// assert_eq!(names, ["G4", "Bb4", "A4", "G4", "C5"]);
/// # Ok::<(), music21_rs::Error>(())
/// ```
pub fn from_tiny_notation(text: &str) -> Result<Stream> {
    let text = text.trim_start();
    let body = match text.split_once(':') {
        Some((head, body)) if head.eq_ignore_ascii_case("tinynotation") => body,
        _ => text,
    };
    let mut reader = Reader {
        elements: Vec::new(),
        meter: None,
        last_length: 1.0,
        states: Vec::new(),
    };
    for token in body.split_whitespace() {
        reader.token(token)?;
    }
    let mut events = Vec::new();
    let mut offset = 0.0;
    for element in reader.elements {
        let length = element.quarter_length();
        events.push(StreamEvent::new(offset, element));
        offset = op_frac(offset + length);
    }
    let mut part = Stream::from_events(events);
    part.set_kind(StreamKind::Part);
    make_measures(&part)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notes(part: &Stream) -> Vec<(String, FloatType)> {
        part.notes()
            .into_iter()
            .filter_map(|(_, element)| match element {
                StreamElement::Note(note) => Some((
                    note.pitch().name_with_octave(),
                    note.duration().map_or(0.0, Duration::quarter_length),
                )),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn letters_and_marks_say_the_octave() {
        // music21: CC is two octaves below middle C's octave, c'' two above.
        let part = from_tiny_notation("CC4 C c c' c''").unwrap();
        let names: Vec<String> = notes(&part).into_iter().map(|(name, _)| name).collect();
        assert_eq!(names, ["C2", "C3", "C4", "C5", "C6"]);
    }

    #[test]
    fn a_length_lasts_until_the_next_one() {
        let part = from_tiny_notation("4/4 c4 d8 e f#2 g-4. an16").unwrap();
        assert_eq!(
            notes(&part),
            [
                ("C4".to_string(), 1.0),
                ("D4".to_string(), 0.5),
                ("E4".to_string(), 0.5),
                ("F#4".to_string(), 2.0),
                ("Gb4".to_string(), 1.5),
                ("A4".to_string(), 0.25),
            ]
        );
    }

    #[test]
    fn a_triplet_is_bracketed_from_its_first_note_to_its_last() {
        let part = from_tiny_notation("2/4 trip{c8 d e} f4").unwrap();
        let measure = part.measures()[0];
        let tuplets: Vec<Option<TupletType>> = measure
            .events()
            .iter()
            .filter_map(|event| match event.element() {
                StreamElement::Note(note) => note
                    .duration()
                    .map(|duration| duration.tuplets().first().and_then(Tuplet::tuplet_type)),
                _ => None,
            })
            .collect();
        assert_eq!(
            tuplets,
            [Some(TupletType::Start), None, Some(TupletType::Stop), None]
        );
        assert!((measure.end_offset() - 2.0).abs() < 1e-9);
    }

    #[test]
    fn a_tie_joins_a_note_to_the_next() {
        let part = from_tiny_notation("c2~ c~ c4 d").unwrap();
        let ties: Vec<Option<TieType>> = part
            .notes()
            .into_iter()
            .filter_map(|(_, element)| match element {
                StreamElement::Note(note) => Some(note.tie().map(Tie::tie_type)),
                _ => None,
            })
            .collect();
        assert_eq!(
            ties,
            [
                Some(TieType::Start),
                Some(TieType::Continue),
                Some(TieType::Stop),
                None
            ]
        );
    }

    #[test]
    fn what_is_no_token_is_passed_over() {
        let part = from_tiny_notation("c4 hello d r4 3").unwrap();
        assert_eq!(notes(&part).len(), 2);
    }
}
