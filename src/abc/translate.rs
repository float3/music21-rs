//! ABC tokens turned into a score: music21's `abcFormat.translate`.
//!
//! The tokens [`super::step`] cuts and processes are read off voice by voice
//! and measure by measure into parts, then a score, or an opus of scores.
//! The order and the oddities are music21's throughout, since the score it
//! makes is what this one is held to.

use super::step::{
    AbcMetadata, AbcNote, Bar, DEFAULT_VERSION, Handler, Merge, Spanning, Token, abc_error,
    at_line, extract_reference_number, has_notes, merge_plan, op_frac,
};
use crate::articulations::{Articulation, ArticulationKind};
use crate::bar::{Barline, Ending, RepeatDirection};
use crate::chord::Chord;
use crate::chordsymbol::ChordSymbol;
use crate::clef::Clef;
use crate::defaults::{FloatType, IntegerType};
use crate::duration::{Duration, Tuplet};
use crate::error::{Error, Position, Result};
use crate::interval::Interval;
use crate::makenotation::split_element;
use crate::metadata::Metadata;
use crate::meter::TimeSignature;
use crate::notation::{Tie, TieStyle, TieType};
use crate::note::Note;
use crate::pitch::Pitch;
use crate::rest::Rest;
use crate::spanner::{Spanner, SpannerKind};
use crate::stream::{Stream, StreamElement, StreamEvent, StreamKind};

/// A bar's worth of tokens with the barlines either side: music21's
/// `ABCHandlerBar`.
#[derive(Clone, Debug, Default)]
struct BarHandler {
    tokens: Vec<Token>,
    left: Option<Bar>,
    right: Option<Bar>,
}

impl BarHandler {
    /// music21's `ABCHandlerBar.__add__`.
    fn joined(&self, other: &BarHandler) -> BarHandler {
        BarHandler {
            tokens: self
                .tokens
                .iter()
                .cloned()
                .chain(other.tokens.iter().cloned())
                .collect(),
            left: other.left.clone().or_else(|| self.left.clone()),
            right: other.right.clone().or_else(|| self.right.clone()),
        }
    }
}

/// The measures of `tokens`, leading metadata merged into the measure
/// after: music21's `splitByMeasure` then `mergeLeadingMetaData`.
fn bar_handlers(tokens: &[Token]) -> Vec<BarHandler> {
    let handler = Handler {
        tokens: tokens.to_vec(),
        ..Handler::new(DEFAULT_VERSION)
    };
    let bar = |at: Option<usize>| match at.map(|at| &tokens[at]) {
        Some(Token::Bar(bar)) => Some(bar.clone()),
        _ => None,
    };
    let split: Vec<BarHandler> = handler
        .split_by_measure()
        .into_iter()
        .map(|split| BarHandler {
            tokens: tokens[split.tokens].to_vec(),
            left: bar(split.left),
            right: bar(split.right),
        })
        .collect();
    let holding: Vec<bool> = split.iter().map(|each| has_notes(&each.tokens)).collect();
    merge_plan(&holding)
        .into_iter()
        .map(|merge| match merge {
            Merge::Whole => split
                .iter()
                .fold(BarHandler::default(), |whole, each| whole.joined(each)),
            Merge::Kept(at) => split[at].clone(),
            Merge::Pair(at) => split[at].joined(&split[at + 1]),
        })
        .collect()
}

/// One element with where it stands and the order it was put in.
#[derive(Clone, Debug)]
struct Item {
    offset: FloatType,
    seq: usize,
    uid: Option<usize>,
    element: StreamElement,
}

/// A measure or a part while it is being filled: what it holds, and where
/// the next thing appended goes.
#[derive(Clone, Debug, Default)]
struct Line {
    items: Vec<Item>,
    end: FloatType,
    seq: usize,
}

impl Line {
    /// music21's `coreAppend`.
    fn append(&mut self, element: impl Into<StreamElement>, uid: Option<usize>) {
        let element = element.into();
        let length = element.quarter_length();
        self.items.push(Item {
            offset: self.end,
            seq: self.seq,
            uid,
            element,
        });
        self.seq += 1;
        self.end = op_frac(self.end + length);
    }

    /// A signature or a clef put at the start in place of the one there,
    /// as music21's `measure.timeSignature = ...` does.
    fn set_at_start(&mut self, element: StreamElement) {
        let same = |held: &StreamElement| {
            matches!(
                (held, &element),
                (
                    StreamElement::TimeSignature(_),
                    StreamElement::TimeSignature(_)
                ) | (StreamElement::Clef(_), StreamElement::Clef(_))
                    | (
                        StreamElement::KeySignature(_) | StreamElement::Key(_),
                        StreamElement::KeySignature(_) | StreamElement::Key(_)
                    )
            )
        };
        self.items.retain(|item| !same(&item.element));
        self.items.push(Item {
            offset: 0.0,
            seq: self.seq,
            uid: None,
            element,
        });
        self.seq += 1;
    }

    fn meter(&self) -> Option<&TimeSignature> {
        self.items.iter().find_map(|item| match &item.element {
            StreamElement::TimeSignature(meter) => Some(meter),
            _ => None,
        })
    }

    /// The elements in the order music21 sorts a stream into.
    fn sorted(&self) -> Vec<&Item> {
        let mut sorted: Vec<&Item> = self.items.iter().collect();
        sorted.sort_by(|left, right| {
            let grace = |item: &Item| item.element.duration().is_some_and(Duration::is_grace);
            left.offset
                .total_cmp(&right.offset)
                .then(class_sort_order(&left.element).cmp(&class_sort_order(&right.element)))
                .then(grace(right).cmp(&grace(left)))
                .then(left.seq.cmp(&right.seq))
        });
        sorted
    }
}

/// music21's `classSortOrder`.
pub(super) fn class_sort_order(element: &StreamElement) -> i32 {
    match element {
        StreamElement::TextExpression(_) => -30,
        StreamElement::Instrument(_) => -25,
        StreamElement::Stream(_) => -20,
        StreamElement::Clef(_) => 0,
        StreamElement::MetronomeMark(_) | StreamElement::TempoText(_) => 1,
        StreamElement::KeySignature(_) | StreamElement::Key(_) => 2,
        StreamElement::TimeSignature(_) => 4,
        StreamElement::Dynamic(_) => 10,
        StreamElement::ChordSymbol(_) => 19,
        _ => 20,
    }
}

#[derive(Clone, Debug, Default)]
struct MeasureIr {
    line: Line,
    number: IntegerType,
    left: Option<Barline>,
    right: Option<Barline>,
    padding_left: FloatType,
}

/// An ending while its measures are being met.
#[derive(Clone, Debug)]
struct Bracket {
    number: u32,
    measures: Vec<usize>,
    complete: bool,
}

/// What reading one voice keeps between its measures.
struct PartReader {
    spanners: Vec<Spanning>,
    next_uid: usize,
    /// The notes a line of words is sung to, by measure and place.
    lyric_targets: Vec<(Option<usize>, usize)>,
    post_transposition: IntegerType,
}

impl PartReader {
    fn uid(&mut self) -> usize {
        self.next_uid += 1;
        self.next_uid
    }
}

/// music21's `addABCLyrics`, as places: each syllable of a `w:` line and
/// which of `targets` notes since the line before it is sung to. `*` and
/// `-` skip a note; syllables past the last note are dropped.
///
/// ```
/// use music21::abc::translate::lyric_syllables;
///
/// let sung = lyric_syllables("hel-lo * world", 3);
/// assert_eq!(sung, [(0, "hel-".to_string()), (1, "-lo".to_string())]);
/// ```
pub fn lyric_syllables(data: &str, targets: usize) -> Vec<(usize, String)> {
    let mut sung = Vec::new();
    let mut at = 0usize;
    for word in data.split_whitespace() {
        if word == "*" || word == "-" {
            at += 1;
            continue;
        }
        let syllables: Vec<&str> = word.split('-').collect();
        for (index, syllable) in syllables.iter().enumerate() {
            if syllable.is_empty() {
                at += 1;
                continue;
            }
            if at >= targets {
                return sung;
            }
            let text = format!(
                "{}{syllable}{}",
                if index > 0 { "-" } else { "" },
                if index + 1 < syllables.len() { "-" } else { "" }
            );
            sung.push((at, text));
            at += 1;
        }
    }
    sung
}

/// The words of a `w:` line put on the notes since the line before.
fn add_lyrics(data: &str, targets: &mut [&mut StreamElement]) -> Result<()> {
    for (at, text) in lyric_syllables(data, targets.len()) {
        match &mut targets[at] {
            StreamElement::Note(note) => note.add_lyric(&text, None, false)?,
            StreamElement::Chord(chord) => chord.add_lyric(&text, None, false)?,
            _ => {}
        }
    }
    Ok(())
}

/// An element and the number spanners know it by.
type Placed = (StreamElement, Option<usize>);

/// music21's `parseABCNote`: the note, chord or rest a token stands for,
/// after the chord symbol written over it.
fn note_elements(
    token: &AbcNote,
    reader: &mut PartReader,
) -> Result<(Option<ChordSymbol>, Option<Placed>)> {
    let mut symbol = None;
    if let Some(first) = token.chord_symbols.first() {
        let mut name: String = first.replace('"', "").trim().to_string();
        name = name.replace(['(', ')'], "");
        // A flat written `b` after a note letter is music21's `-`.
        let chars: Vec<char> = name.chars().collect();
        let mut cleaned = String::new();
        for (index, character) in chars.iter().enumerate() {
            let after_letter = index > 0 && matches!(chars[index - 1], 'A'..='G' | 'a'..='g');
            cleaned.push(if *character == 'b' && after_letter {
                '-'
            } else {
                *character
            });
        }
        if matches!(cleaned.as_str(), "NC" | "N.C." | "No Chord" | "None") {
            symbol = Some(ChordSymbol::no_chord(Some(cleaned)));
        } else if cleaned.starts_with('>') {
            return Ok((None, None));
        } else if let Ok(read) = ChordSymbol::parse_music21(cleaned) {
            symbol = Some(read);
        }
    }

    let with_tuplet = |duration: &mut Duration, tuplet: Option<Tuplet>| {
        if let Some(mut tuplet) = tuplet {
            if tuplet.duration_normal().is_none()
                && let Some((kind, dots)) = duration.type_and_dots()
            {
                tuplet.set_duration_type(kind, dots);
            }
            duration.append_tuplet(tuplet);
        }
    };

    if token.is_chord {
        if token.sub_tokens.is_empty() {
            return Ok((symbol, None));
        }
        let mut members = Vec::new();
        for sub in &token.sub_tokens {
            let Some(name) = &sub.pitch_name else {
                continue;
            };
            let mut pitch = Pitch::from_name(name)?;
            if let Some(accidental) = pitch.written_accidental_mut() {
                accidental.set_display_status(sub.accidental_display);
            }
            members.push(Note::from_pitch(pitch));
        }
        let mut duration = Duration::new(token.quarter_length)?;
        with_tuplet(&mut duration, token.tuplet);
        let chord = Chord::new(members)?.with_duration(duration);
        let uid = reader.uid();
        return Ok((symbol, Some((chord.into(), Some(uid)))));
    }

    let mut duration = Duration::new(token.quarter_length)?;
    with_tuplet(&mut duration, token.tuplet);
    if token.is_rest {
        let uid = reader.uid();
        for spanner in &token.spanners {
            reader.spanners[*spanner].uids.push(uid);
        }
        if token.in_grace {
            let mut stray = Rest::new(duration.clone());
            stray.set_tie(tie_of(token.tie));
            for spanner in &token.spanners {
                reader.spanners[*spanner]
                    .strays
                    .push((uid, stray.clone().into()));
            }
        }
        let duration = if token.in_grace {
            grace_of(&duration)
        } else {
            duration
        };
        // A grace rest is a copy made after the spanners took the first.
        let placed = if token.in_grace { reader.uid() } else { uid };
        let mut rest = Rest::new(duration);
        rest.set_tie(tie_of(token.tie));
        *rest.articulations_mut() = articulations_of(&token.articulations);
        return Ok((symbol, Some((rest.into(), Some(placed)))));
    }
    let name = token
        .pitch_name
        .as_deref()
        .ok_or_else(|| abc_error("a note with no pitch"))?;
    let mut pitch = Pitch::from_name(name)?;
    if let Some(accidental) = pitch.written_accidental_mut() {
        accidental.set_display_status(token.accidental_display);
    }
    let mut note = Note::from_pitch(pitch);
    note.set_tie(tie_of(token.tie));
    let uid = reader.uid();
    for spanner in &token.spanners {
        if !reader.spanners[*spanner].uids.contains(&uid) {
            reader.spanners[*spanner].uids.push(uid);
        }
    }
    // A grace note is a copy made after the spanners took the note itself,
    // which no stream then holds.
    let placed = if token.in_grace {
        let mut stray = note.clone();
        stray.set_duration(duration.clone());
        for spanner in &token.spanners {
            reader.spanners[*spanner]
                .strays
                .push((uid, stray.clone().into()));
        }
        duration = grace_of(&duration);
        reader.uid()
    } else {
        uid
    };
    note.set_duration(duration);
    *note.articulations_mut() = articulations_of(&token.articulations);
    Ok((symbol, Some((note.into(), Some(placed)))))
}

/// The tie a token asks for, on a note or a rest alike.
fn tie_of(tie: Option<TieType>) -> Option<Tie> {
    match tie? {
        tie_type @ (TieType::Start | TieType::Continue) => {
            let mut tie = Tie::new(tie_type);
            tie.set_style(TieStyle::Normal);
            Some(tie)
        }
        tie_type => Some(Tie::new(tie_type)),
    }
}

/// The marks a token carries, last written first: music21 pops them off
/// the end of its list.
fn articulations_of(marks: &[String]) -> Vec<Articulation> {
    marks
        .iter()
        .rev()
        .filter_map(|mark| {
            let class = match mark.as_str() {
                "staccato" => "Staccato",
                "upbow" => "UpBow",
                "downbow" => "DownBow",
                "accent" => "Accent",
                "strongaccent" => "StrongAccent",
                "tenuto" => "Tenuto",
                _ => return None,
            };
            ArticulationKind::from_class_name(class).map(Articulation::of_kind)
        })
        .collect()
}

/// A length as a grace note's: the written value, lasting nothing and in no
/// tuplet, whatever bracket the note was written inside.
fn grace_of(duration: &Duration) -> Duration {
    duration.grace_duration()
}

/// An ending a part opens and never closes: music21 leaves its
/// `RepeatBracket` in the spanner bundle, for the part after.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenEnding {
    /// The ending's number.
    pub number: u32,
    /// The measures it spans so far, by their place among the part's.
    pub measures: Vec<usize>,
}

/// A voice read by [`part_of`]: the part, the numbers its leaves carry,
/// the spanners it started and the endings it left open.
struct VoiceRead {
    stream: Stream,
    uids: Vec<Option<usize>>,
    spanners: Vec<Spanning>,
    open_endings: Vec<OpenEnding>,
}

/// music21's `abcToStreamPart`: one voice as a part, with the spanners it
/// holds, each naming its notes by the numbers the part's leaves carry.
fn part_of(tokens: &[Token], spanners: &[Spanning]) -> Result<VoiceRead> {
    let handler = Handler {
        tokens: tokens.to_vec(),
        ..Handler::new(DEFAULT_VERSION)
    };
    let merged: Vec<BarHandler> = if handler.defines_measures() {
        bar_handlers(tokens)
    } else {
        vec![BarHandler {
            tokens: tokens.to_vec(),
            ..BarHandler::default()
        }]
    };
    let use_measures = merged.len() > 1;

    let mut reader = PartReader {
        spanners: spanners.to_vec(),
        next_uid: 0,
        lyric_targets: Vec::new(),
        post_transposition: 0,
    };
    // Which of the handler's spanners this voice starts, in order, each
    // with where the part had got to: the measure it opened in, or the
    // length of the part's own line.
    let mut started: Vec<(usize, Option<usize>, FloatType)> = Vec::new();
    let mut part = Line::default();
    let mut measures: Vec<MeasureIr> = Vec::new();
    let mut brackets: Vec<Bracket> = Vec::new();
    let mut bar_count = 0;
    let mut measure_number = 1;

    for handler in &merged {
        let in_measure = use_measures && has_notes(&handler.tokens);
        let mut measure = MeasureIr::default();
        if in_measure {
            let index = measures.len();
            if let Some(left) = &handler.left {
                measure.left = left.barline();
                if let Some(number) = left.repeat_bracket() {
                    match brackets.iter_mut().find(|bracket| !bracket.complete) {
                        None => brackets.push(Bracket {
                            number,
                            measures: vec![index],
                            complete: number == 2,
                        }),
                        Some(open) => {
                            if !open.measures.contains(&index) {
                                open.measures.push(index);
                            }
                            open.complete = true;
                        }
                    }
                }
            }
            if let Some(right) = &handler.right {
                // A repeat closing a measure ends the passage.
                measure.right = right.barline().map(|barline| {
                    if barline.repeat_direction() == Some(RepeatDirection::Start) {
                        Barline::repeat(RepeatDirection::End, None)
                    } else {
                        barline
                    }
                });
                if right.is_repeat()
                    && let Some(open) = brackets.iter_mut().find(|bracket| !bracket.complete)
                {
                    if !open.measures.contains(&index) {
                        open.measures.push(index);
                    }
                    open.complete = true;
                }
            }
            bar_count += 1;
        }

        // music21's `parseTokens`.
        reader.post_transposition = 0;
        let place = in_measure.then_some(measures.len());
        {
            let line = if in_measure {
                &mut measure.line
            } else {
                &mut part
            };
            for token in &handler.tokens {
                // What goes wrong says its line, and its measure counted
                // from one as the tune writes them, a pickup the first.
                let refused = |error: Error| {
                    let error = at_line(error, token.line());
                    if in_measure {
                        return error.placed(|| Position::measure(bar_count.to_string()));
                    }
                    error
                };
                match token {
                    Token::Metadata(line_token) => {
                        let (tag, data) = (&line_token.tag, &line_token.data);
                        if tag == "w" {
                            let wanted = std::mem::take(&mut reader.lyric_targets);
                            let mut targets: Vec<&mut StreamElement> = Vec::new();
                            // Notes of this line only; those of measures
                            // already closed are reached below.
                            let mut here: Vec<usize> = wanted
                                .iter()
                                .filter(|(held, _)| *held == place)
                                .map(|(_, index)| *index)
                                .collect();
                            here.sort_unstable();
                            let outside = wanted.iter().any(|(held, _)| *held != place);
                            if !outside {
                                let mut rest: &mut [Item] = &mut line.items;
                                let mut passed = 0;
                                for index in here {
                                    let (_, tail) = rest.split_at_mut(index - passed);
                                    let (item, tail) = tail.split_first_mut().ok_or_else(|| {
                                        refused(abc_error("a lyric's note is missing"))
                                    })?;
                                    targets.push(&mut item.element);
                                    rest = tail;
                                    passed = index + 1;
                                }
                                add_lyrics(data, &mut targets).map_err(refused)?;
                            } else {
                                add_lyrics_across(data, &wanted, &mut measures, line, place)
                                    .map_err(refused)?;
                            }
                        }
                        match tag.as_str() {
                            "M" => {
                                if let Some(meter) = line_token.meter().map_err(refused)? {
                                    if use_measures {
                                        line.set_at_start(meter.into());
                                    } else {
                                        line.append(meter, None);
                                    }
                                }
                            }
                            "K" => {
                                let key = line_token.key().map_err(refused)?;
                                if use_measures {
                                    line.set_at_start(key);
                                } else {
                                    line.append(key, None);
                                }
                                if let Some((clef, transposition)) =
                                    line_token.clef().map_err(refused)?
                                {
                                    if use_measures {
                                        line.set_at_start(clef.into());
                                    } else {
                                        line.append(clef, None);
                                    }
                                    reader.post_transposition = transposition;
                                }
                            }
                            "Q" => line.append(line_token.tempo().map_err(refused)?, None),
                            _ => {}
                        }
                    }
                    Token::Note(note) => {
                        let (symbol, made) = note_elements(note, &mut reader).map_err(refused)?;
                        if let Some(symbol) = symbol {
                            line.append(symbol, None);
                        }
                        if let Some((element, uid)) = made {
                            let sung = !matches!(element, StreamElement::Rest(_)) && !note.in_grace;
                            line.append(element, uid);
                            if sung {
                                reader.lyric_targets.push((place, line.items.len() - 1));
                            }
                        }
                    }
                    Token::SpannerStart(_, Some(index)) => {
                        started.push((*index, place, line.end));
                    }
                    _ => {}
                }
            }
        }

        if in_measure {
            if bar_count == 1
                && let Some(meter) = measure.line.meter()
            {
                // A short first measure is a pickup, numbered nought.
                let bar = meter.bar_quarter_length();
                let highest = measure_stream(&measure.line).end_offset();
                if highest < bar {
                    measure.padding_left = bar - highest;
                }
                measure.number = 0;
            } else {
                measure.number = measure_number;
                measure_number += 1;
            }
            // Lyric targets name a measure by its place among them.
            measures.push(measure);
        }
    }

    // Where each measure stood as music21 appended it, before `reBar`
    // cut any in two: where a spanner opening in it was appended.
    let mut appended_at: Vec<FloatType> = Vec::new();
    let mut reached = 0.0;
    for measure in &measures {
        appended_at.push(reached);
        reached = op_frac(reached + measure.line.end);
    }

    re_bar(&mut measures, &mut brackets);

    // Endings, from the brackets over the measures.
    let mut endings: Vec<Option<Ending>> = vec![None; measures.len()];
    // An ending never closed is left out of the measures, as music21 leaves
    // it, and handed back.
    let open_endings = brackets
        .iter()
        .filter(|bracket| !bracket.complete)
        .map(|bracket| OpenEnding {
            number: bracket.number,
            measures: bracket.measures.clone(),
        })
        .collect();
    for bracket in brackets.iter().filter(|bracket| bracket.complete) {
        for (place, index) in bracket.measures.iter().enumerate() {
            if let Some(slot) = endings.get_mut(*index)
                && slot.is_none()
            {
                *slot = Some(Ending::new(
                    vec![bracket.number],
                    place == 0,
                    place + 1 == bracket.measures.len(),
                ));
            }
        }
    }

    // A part with no clef takes the one that fits its notes best.
    let has_clef = measures
        .iter()
        .flat_map(|measure| &measure.line.items)
        .chain(&part.items)
        .any(|item| matches!(item.element, StreamElement::Clef(_)));
    if !has_clef {
        let mut pitches: Vec<Pitch> = Vec::new();
        for item in measures
            .iter()
            .flat_map(|measure| measure.line.sorted())
            .chain(part.sorted())
        {
            match &item.element {
                StreamElement::Note(note) => pitches.push(note.pitch().clone()),
                StreamElement::Chord(chord) => pitches.extend(chord.pitches()),
                StreamElement::ChordSymbol(symbol) => {
                    pitches.extend(symbol.pitches().unwrap_or_default());
                }
                _ => {}
            }
        }
        let clef = Clef::best_for(&pitches, false);
        match measures.first_mut() {
            Some(first) if use_measures => first.line.set_at_start(clef.into()),
            _ => {
                part.items.push(Item {
                    offset: 0.0,
                    seq: part.seq,
                    uid: None,
                    element: clef.into(),
                });
                part.seq += 1;
            }
        }
    }

    if reader.post_transposition != 0 {
        // music21 transposes by a count of semitones, which spells each
        // note afresh from where it lands and decides nothing about how its
        // accidental is shown.
        let interval = Interval::from_semitones(reader.post_transposition)?;
        for item in measures
            .iter_mut()
            .flat_map(|measure| &mut measure.line.items)
            .chain(&mut part.items)
        {
            // A chord symbol moves with the notes, as music21 moves any
            // element that can be transposed.
            let moves = item.element.is_note_or_chord()
                || matches!(item.element, StreamElement::ChordSymbol(_));
            if moves {
                item.element = item.element.transpose(&interval)?;
            }
        }
    }

    // Put together: the measures one after another, or the part's own line.
    let mut uids: Vec<Option<usize>> = Vec::new();
    let mut events = Vec::new();
    let mut offset = 0.0;
    let has_meter = part.meter().is_some()
        || measures
            .iter()
            .any(|measure| measure.line.meter().is_some());
    for (index, measure) in measures.into_iter().enumerate() {
        let mut inner = Vec::new();
        for item in measure.line.sorted() {
            uids.push(item.uid);
            inner.push(StreamEvent::new(item.offset, item.element.clone()));
        }
        let mut stream = Stream::from_events(inner);
        stream.set_kind(StreamKind::Measure);
        stream.set_number(measure.number);
        stream.set_left_barline(measure.left);
        stream.set_right_barline(measure.right);
        stream.set_ending(endings[index].clone());
        stream.set_padding_left(measure.padding_left);
        let length = measure.line.end;
        events.push(StreamEvent::new(offset, stream));
        offset = op_frac(offset + length);
    }
    for item in part.sorted() {
        uids.push(item.uid);
        events.push(StreamEvent::new(item.offset, item.element.clone()));
    }
    let mut stream = Stream::from_events(events);
    stream.set_kind(StreamKind::Part);

    if use_measures && has_meter {
        // music21 warns and carries on where a measure cannot be beamed.
        let _ = crate::makenotation::make_beams(&mut stream);
    }

    // A measure's spanner stands where the measure did when music21
    // appended the spanner to the part.
    let held: Vec<Spanning> = started
        .into_iter()
        .map(|(index, measure, end)| {
            let mut held = reader.spanners[index].clone();
            held.offset = match measure {
                Some(measure) => appended_at.get(measure).copied().unwrap_or(reached),
                None => end,
            };
            held
        })
        .collect();
    Ok(VoiceRead {
        stream,
        uids,
        spanners: held,
        open_endings,
    })
}

/// Words sung to notes of measures already closed as well as this one.
fn add_lyrics_across(
    data: &str,
    wanted: &[(Option<usize>, usize)],
    measures: &mut [MeasureIr],
    line: &mut Line,
    place: Option<usize>,
) -> Result<()> {
    // One mutable borrow per note: each is reached in turn by where it is.
    let mut elements: Vec<StreamElement> = Vec::new();
    for (held, index) in wanted {
        let item = if *held == place {
            line.items.get(*index)
        } else {
            held.and_then(|measure| measures.get(measure))
                .and_then(|measure| measure.line.items.get(*index))
        };
        elements.push(
            item.map(|item| item.element.clone())
                .ok_or_else(|| abc_error("a lyric's note is missing"))?,
        );
    }
    {
        let mut targets: Vec<&mut StreamElement> = elements.iter_mut().collect();
        add_lyrics(data, &mut targets)?;
    }
    for ((held, index), element) in wanted.iter().zip(elements) {
        let slot = if *held == place {
            line.items.get_mut(*index)
        } else {
            held.and_then(|measure| measures.get_mut(measure))
                .and_then(|measure| measure.line.items.get_mut(*index))
        };
        if let Some(slot) = slot {
            slot.element = element;
        }
    }
    Ok(())
}

/// A line as the measure it will be, for the questions asked of a stream.
fn measure_stream(line: &Line) -> Stream {
    let mut stream = Stream::from_events(
        line.sorted()
            .into_iter()
            .map(|item| StreamEvent::new(item.offset, item.element.clone())),
    );
    stream.set_kind(StreamKind::Measure);
    stream
}

/// music21's `reBar`: a measure holding more than its bar is cut in two, the
/// second taking the meter that fits it where it is not a whole bar, and
/// every later measure is numbered one further on.
///
/// An overflow no meter fits is kept as a measure stating none. music21
/// gives up where it stands on a measure with no meter in force, or one it
/// cannot cut, keeping what it had done.
fn re_bar(measures: &mut Vec<MeasureIr>, brackets: &mut [Bracket]) {
    let mut meter: Option<TimeSignature> = None;
    let mut shift = 0;
    // Where each measure read stands once the cut ones are in.
    let mut places: Vec<usize> = (0..measures.len()).collect();
    let mut index = 0;
    let mut original = 0;
    let count = measures.len();
    'measures: while original < count {
        if let Some(own) = measures[index].line.meter() {
            meter = Some(own.clone());
        }
        let Some(current) = meter.clone() else {
            break;
        };
        places[original] = index;
        let bar = op_frac(current.bar_quarter_length());
        let end = measures[index].line.end;
        measures[index].number += shift;
        if end > bar + 1e-9 {
            let mut second = Line::default();
            let mut kept: Vec<Item> = Vec::new();
            let mut moved: Vec<Item> = Vec::new();
            let items = std::mem::take(&mut measures[index].line.items);
            let mut broken = false;
            for item in items {
                let length = item.element.quarter_length();
                if (item.offset - bar).abs() <= 1e-9 && length == 0.0 {
                    // Something with no length standing on the barline ends
                    // where the new measure starts, and stays behind.
                    kept.push(item);
                } else if item.offset >= bar - 1e-9 {
                    moved.push(item);
                } else if item.offset + length > bar + 1e-9 {
                    match split_element(&item.element, op_frac(bar - item.offset)) {
                        Ok((first, remain)) => {
                            second.items.push(Item {
                                offset: 0.0,
                                seq: second.seq,
                                uid: None,
                                element: remain,
                            });
                            second.seq += 1;
                            kept.push(Item {
                                element: first,
                                ..item
                            });
                        }
                        Err(_) => {
                            broken = true;
                            kept.push(item);
                        }
                    }
                } else {
                    kept.push(item);
                }
            }
            measures[index].line.items = kept;
            measures[index].line.end = bar;
            if broken {
                break 'measures;
            }
            for item in moved {
                second.items.push(Item {
                    offset: op_frac(item.offset - bar),
                    seq: second.seq,
                    ..item
                });
                second.seq += 1;
            }
            second.end = op_frac(end - bar);
            if (bar - second.end).abs() > 1e-9
                && let Ok(best) = crate::meter::best_time_signature(&measure_stream(&second))
            {
                second.set_at_start(best.into());
                if original + 1 < count && measures[index + 1].line.meter().is_none() {
                    // music21 puts the very meter object back, so it stands
                    // in two measures at once.
                    let mut again = current.clone();
                    again.set_restated(true);
                    measures[index + 1].line.set_at_start(again.into());
                }
            }
            let number = measures[index].number + 1;
            shift += 1;
            // The barline closing the measure closes the new one.
            let right = measures[index].right.take();
            measures.insert(
                index + 1,
                MeasureIr {
                    line: second,
                    number,
                    left: None,
                    right,
                    padding_left: 0.0,
                },
            );
            index += 1;
        }
        index += 1;
        original += 1;
    }
    for (passed, place) in places.iter_mut().skip(original).enumerate() {
        *place = index + passed;
    }
    for bracket in brackets {
        for measure in &mut bracket.measures {
            *measure = places[*measure];
        }
    }
}

/// Each voice's tokens with the header in front: music21's `splitByVoice`,
/// then the header added to each voice, as `abcToStreamScore` does.
fn voice_tokens(handler: &Handler) -> Vec<Vec<Token>> {
    let voices = handler.split_by_voice();
    if voices.len() == 1 {
        return vec![handler.tokens.clone()];
    }
    let header = &handler.tokens[voices[0].clone()];
    voices[1..]
        .iter()
        .map(|voice| {
            header
                .iter()
                .chain(&handler.tokens[voice.clone()])
                .cloned()
                .collect()
        })
        .collect()
}

/// The spanners each part started, put on `stream`, whose leaves carry
/// `uids` (a part's place and the note's uid) in order.
fn place_spanners(
    stream: &mut Stream,
    uids: &[Option<(usize, usize)>],
    held: Vec<(usize, Spanning)>,
) {
    for (part, spanner) in held {
        let positions: Vec<Option<usize>> = spanner
            .uids
            .iter()
            .map(|uid| uids.iter().position(|held| *held == Some((part, *uid))))
            .collect();
        let mut made = match spanner.kind {
            SpannerKind::Crescendo | SpannerKind::Diminuendo => {
                let template = Spanner::wedge(spanner.kind, Vec::new());
                let mut wedge = Spanner::with_unplaced(spanner.kind, positions);
                wedge.set_placement(template.placement());
                wedge.set_spread(template.spread());
                wedge
            }
            kind => Spanner::with_unplaced(kind, positions),
        };
        made.set_offset(spanner.offset);
        for (place, uid) in spanner.uids.iter().enumerate() {
            if made.spanned().get(place).is_some_and(Option::is_some) {
                continue;
            }
            if let Some((_, stray)) = spanner.strays.iter().find(|(held, _)| held == uid) {
                made.set_unplaced_element(place, stray.clone());
            }
        }
        stream.add_spanner(made);
    }
}

/// A part read by [`part_stream`], and the endings it left open.
#[derive(Clone, Debug)]
pub struct ReadPart {
    /// The part, holding its spanners and closed endings.
    pub stream: Stream,
    /// The endings it opened and never closed.
    pub open_endings: Vec<OpenEnding>,
}

/// music21's `abcToStreamPart`: the tokens of one voice, header included,
/// as a part holding the spanners it starts, and the endings it leaves
/// open. `spanners` are those [`Handler::process`] handed back.
pub fn part_stream(tokens: &[Token], spanners: &[Spanning]) -> Result<ReadPart> {
    let read = part_of(tokens, spanners)?;
    let mut part = read.stream;
    let uids: Vec<Option<(usize, usize)>> = read
        .uids
        .into_iter()
        .map(|uid| uid.map(|uid| (0, uid)))
        .collect();
    let held = read
        .spanners
        .into_iter()
        .map(|spanner| (0, spanner))
        .collect();
    place_spanners(&mut part, &uids, held);
    Ok(ReadPart {
        stream: part,
        open_endings: read.open_endings,
    })
}

/// The metadata of a tune's header: music21's `abcToStreamScore` reading
/// titles, the composer, the place and the number.
fn tune_metadata(handler: &Handler) -> Result<Metadata> {
    let mut metadata = Metadata::new();
    let mut titles = 0;
    for token in &handler.tokens {
        let Some((tag, data)) = token.metadata() else {
            continue;
        };
        match tag {
            "T" => {
                let name = if titles == 0 {
                    "title"
                } else {
                    "alternativeTitle"
                };
                metadata.add_text(name, data);
                titles += 1;
            }
            "C" => metadata.add_contributor("composer", data),
            "O" => metadata.add_text("localeOfComposition", data),
            "X" => {
                let number = data.trim().parse::<IntegerType>().map_err(|_| {
                    let refused = abc_error(format!("the reference number {data:?} is no number"));
                    at_line(refused, token.line())
                })?;
                metadata.add_text("number", number.to_string());
            }
            _ => {}
        }
    }
    Ok(metadata)
}

/// The ABC version an `I:abc-version 2.1` line of the handler's names, the
/// last where there are several: what music21's `abcToStreamScore` sets
/// on the handler.
pub fn named_version(handler: &Handler) -> Option<(u32, u32, u32)> {
    handler
        .tokens
        .iter()
        .filter_map(|token| match token {
            Token::Metadata(line) => version_named(line),
            _ => None,
        })
        .next_back()
}

/// `I:abc-version 2.1`'s version: music21's `isVersion` and the match it
/// reads.
fn version_named(line: &AbcMetadata) -> Option<(u32, u32, u32)> {
    if line.tag != "I" || !line.data.starts_with("abc-version ") {
        return None;
    }
    let named = line.data.replace("abc-version", "");
    let mut numbers = named
        .trim()
        .split(|c: char| !c.is_ascii_digit())
        .filter(|piece| !piece.is_empty())
        .map(|piece| piece.parse::<u32>().ok());
    let major = numbers.next()??;
    let minor = numbers.next()??;
    let patch = numbers.next().flatten().unwrap_or(0);
    Some((major, minor, patch))
}

/// music21's `abcToStreamScore`: a processed tune as a score of a part per
/// voice, with its metadata. `spanners` are those [`Handler::process`]
/// handed back.
pub fn score_of(handler: &Handler, spanners: &[Spanning]) -> Result<Stream> {
    let metadata = tune_metadata(handler)?;
    let mut events = Vec::new();
    let mut uids: Vec<Option<(usize, usize)>> = Vec::new();
    let mut held: Vec<(usize, Spanning)> = Vec::new();
    for (index, tokens) in voice_tokens(handler).iter().enumerate() {
        let read = part_of(tokens, spanners)?;
        let (part, part_uids, part_spanners) = (read.stream, read.uids, read.spanners);
        uids.extend(part_uids.into_iter().map(|uid| uid.map(|uid| (index, uid))));
        held.extend(part_spanners.into_iter().map(|spanner| (index, spanner)));
        events.push(StreamEvent::new(0.0, part));
    }
    let mut score = Stream::from_events(events);
    score.set_kind(StreamKind::Score);
    score.set_metadata(Some(metadata));
    place_spanners(&mut score, &uids, held);
    Ok(score)
}

/// music21's `abcToStreamOpus`: an opus of the handler's tunes in the order
/// of their numbers, or of the one numbered `number` where there is one.
/// A handler of one tune is an opus of one score.
pub fn opus_of(
    handler: &Handler,
    spanners: &[Spanning],
    number: Option<IntegerType>,
) -> Result<Stream> {
    let mut scores = Vec::new();
    if handler.defines_reference_numbers() {
        let mut tunes = handler.split_by_reference_number()?;
        if let Some(wanted) = number
            && let Some(at) = tunes.iter().position(|(held, _)| *held == Some(wanted))
        {
            tunes = vec![tunes.swap_remove(at)];
        }
        tunes.sort_by_key(|(held, _)| *held);
        for (_, indices) in tunes {
            let tune = Handler {
                tokens: indices
                    .iter()
                    .map(|at| handler.tokens[*at].clone())
                    .collect(),
                ..Handler::new(handler.version)
            };
            scores.push(score_of(&tune, spanners)?);
        }
    } else {
        scores.push(score_of(handler, spanners)?);
    }
    let mut events = Vec::new();
    let mut offset = 0.0;
    for score in scores {
        let length = score.end_offset();
        events.push(StreamEvent::new(offset, score));
        offset += length;
    }
    let mut opus = Stream::from_events(events);
    opus.set_kind(StreamKind::Opus);
    Ok(opus)
}

/// music21's `parseABCNote`, for one token alone: the chord symbol written
/// over it and the note, chord or rest it stands for, either of which can
/// be missing.
pub fn note_of(token: &AbcNote) -> Result<(Option<ChordSymbol>, Option<StreamElement>)> {
    let wanted = token.spanners.iter().max().map_or(0, |most| most + 1);
    let mut reader = PartReader {
        spanners: vec![Spanning::new(SpannerKind::Slur); wanted],
        next_uid: 0,
        lyric_targets: Vec::new(),
        post_transposition: 0,
    };
    let (symbol, made) = note_elements(token, &mut reader)?;
    Ok((symbol, made.map(|(element, _)| element)))
}

/// A part cut by [`re_bar_part`], and where each of its leaves came from.
#[derive(Clone, Debug)]
pub struct ReBarred {
    /// The part, its measures cut.
    pub stream: Stream,
    /// For each of the part's leaves in order, the leaf of the part read it
    /// was, where it was one: music21 cuts measures in place, so a note
    /// kept, or the first half of one cut, is the object it was.
    pub origins: Vec<Option<usize>>,
}

/// How many leaves an element is: a stream's, or one.
fn leaf_count(element: &StreamElement) -> usize {
    match element.as_stream() {
        Some(stream) => stream.leaves().len(),
        None => 1,
    }
}

/// The origins of an element's leaves: `start` onwards for one read,
/// nothing for one made.
fn origins_of(element: &StreamElement, start: Option<usize>) -> Vec<Option<usize>> {
    let count = leaf_count(element);
    match start {
        Some(start) => (start..start + count).map(Some).collect(),
        None => vec![None; count],
    }
}

/// music21's `reBar` over a part: each measure holding more than its bar
/// cut in two, as in reading a tune. The part's other elements are kept;
/// its spanners are left to the caller, which knows its notes by
/// [`ReBarred::origins`].
///
/// # Errors
///
/// A part whose first measure states no meter.
pub fn re_bar_part(part: &Stream) -> Result<ReBarred> {
    let mut measures: Vec<MeasureIr> = Vec::new();
    let mut endings: Vec<Ending> = Vec::new();
    let mut brackets: Vec<Bracket> = Vec::new();
    // Each top element with the origins of its leaves.
    let mut tops: Vec<(StreamEvent, Vec<Option<usize>>)> = Vec::new();
    let mut start: Option<FloatType> = None;
    let mut read = 0usize;
    for event in part.events() {
        let Some(measure) = event
            .element()
            .as_stream()
            .filter(|stream| stream.kind() == StreamKind::Measure)
        else {
            tops.push((event.clone(), origins_of(event.element(), Some(read))));
            read += leaf_count(event.element());
            continue;
        };
        start.get_or_insert(event.offset());
        if let Some(ending) = measure.ending() {
            brackets.push(Bracket {
                number: 0,
                measures: vec![measures.len()],
                complete: true,
            });
            endings.push(ending.clone());
        }
        measures.push(measure_ir(measure, &mut read));
    }
    if measures
        .first()
        .is_some_and(|first| first.line.meter().is_none())
    {
        return Err(abc_error("No time signature found in this Part"));
    }

    re_bar(&mut measures, &mut brackets);

    let mut placed_endings: Vec<Option<Ending>> = vec![None; measures.len()];
    for (bracket, ending) in brackets.iter().zip(endings) {
        if let Some(slot) = bracket
            .measures
            .first()
            .and_then(|at| placed_endings.get_mut(*at))
        {
            *slot = Some(ending);
        }
    }
    let mut offset = start.unwrap_or(0.0);
    for (measure, ending) in measures.into_iter().zip(placed_endings) {
        let length = measure.line.end;
        let origins: Vec<Option<usize>> = measure
            .line
            .sorted()
            .into_iter()
            .flat_map(|item| origins_of(&item.element, item.uid))
            .collect();
        let mut stream = measure_stream(&measure.line);
        stream.set_number(measure.number);
        stream.set_left_barline(measure.left);
        stream.set_right_barline(measure.right);
        stream.set_ending(ending);
        stream.set_padding_left(measure.padding_left);
        tops.push((StreamEvent::new(offset, stream), origins));
        offset = op_frac(offset + length);
    }

    // In the order the part keeps them, which sorts by offset alone.
    tops.sort_by(|left, right| left.0.offset().total_cmp(&right.0.offset()));
    let origins = tops.iter().flat_map(|(_, held)| held.clone()).collect();
    let mut made = Stream::from_events(tops.into_iter().map(|(event, _)| event));
    made.set_kind(part.kind());
    Ok(ReBarred {
        stream: made,
        origins,
    })
}

/// A measure as the lines `re_bar` cuts, each item knowing the first leaf
/// it was among the part's, counted on from `read`.
fn measure_ir(measure: &Stream, read: &mut usize) -> MeasureIr {
    let mut line = Line::default();
    for event in measure.events() {
        line.items.push(Item {
            offset: event.offset(),
            seq: line.seq,
            uid: Some(*read),
            element: event.element().clone(),
        });
        *read += leaf_count(event.element());
        line.seq += 1;
        line.end = line.end.max(op_frac(event.end_offset()));
    }
    MeasureIr {
        line,
        number: measure.number(),
        left: measure.left_barline().cloned(),
        right: measure.right_barline().cloned(),
        padding_left: measure.padding_left(),
    }
}

/// Reads processed tokens from text: music21's `ABCFile.readstr`.
fn handler_of(text: &str) -> Result<(Handler, Vec<Spanning>)> {
    let mut handler = Handler::new(DEFAULT_VERSION);
    handler.read_version(&text.chars().take(100).collect::<String>());
    handler.tokenize(text)?;
    let spanners = handler.process()?;
    Ok((handler, spanners))
}

/// Reads ABC text into a score: music21's `converter.parse` for the format.
///
/// A file holding one tune comes back as a score, with a part for each of
/// its voices. A file holding several tunes, each under its own `X:`
/// reference number, comes back as an opus of scores in the order of their
/// numbers.
///
/// ```
/// use music21::abc::from_abc;
///
/// let score = from_abc("X:1\nT:Scale\nM:4/4\nL:1/4\nK:G\nG A B c | d e f g |\n")?;
/// let part = score.parts()[0];
/// assert_eq!(part.measures().len(), 2);
/// // The key's F sharp is read into the note.
/// assert_eq!(part.measures()[1].pitches()[2].name_with_octave(), "F#5");
/// # Ok::<(), music21::Error>(())
/// ```
pub fn from_abc(text: &str) -> Result<Stream> {
    let (handler, spanners) = handler_of(text)?;
    if handler.defines_reference_numbers() {
        return opus_of(&handler, &spanners, None);
    }
    score_of(&handler, &spanners)
}

/// Reads one tune out of ABC text holding several, by its `X:` reference
/// number: music21's `converter.parse(..., number=n)`.
pub fn from_abc_number(text: &str, number: IntegerType) -> Result<Stream> {
    let (before, tune) = extract_reference_number(text, number)?;
    let read = handler_of(&tune).and_then(|(handler, spanners)| score_of(&handler, &spanners));
    read.map_err(|error| error.lines_down(before))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bar::BarlineType;
    use crate::error::FormatErrorKind;

    /// A slur over a grace note holds the note music21 made before it
    /// copied it as a grace, which no stream holds: the spanner keeps it.
    #[test]
    fn a_slur_keeps_the_grace_note_no_stream_holds() {
        let score = from_abc("X:1\nM:4/4\nL:1/8\nK:C\n(g2{a}g) c4 |\n").unwrap();
        let slur = &score.spanners()[0];
        assert_eq!(slur.len(), 3);
        assert_eq!(slur.spanned()[1], None);
        let stray: Vec<(usize, String)> = slur
            .unplaced_elements()
            .iter()
            .map(|(place, element)| (*place, element.pitches()[0].name_with_octave()))
            .collect();
        assert_eq!(stray, [(1, "A5".to_string())]);
    }

    #[test]
    fn a_refusal_says_its_line_and_measure() {
        let error = from_abc("X:1\nK:C\nc d ~.").unwrap_err();
        assert_eq!(error.to_string(), "ABC error: a decoration ends the text");
        assert_eq!(error.format_error().unwrap().kind, FormatErrorKind::Invalid);
        assert_eq!(error.position(), Some(&Position::line(3).with_column(5)));

        let error = from_abc("X:1\nL:1/4\nK:C\n(0c d e\n").unwrap_err();
        assert_eq!(error.position(), Some(&Position::line(4)));

        let tune = "X:2\nM:4/4\nL:1/4\nK:C\nc d e f |\ng a Q:1/4=x\nb c |\nc d e f |\n";
        let error = from_abc(tune).unwrap_err();
        assert_eq!(
            error.to_string(),
            "ABC error: cannot read the tempo \"1/4=x\""
        );
        let mut wanted = Position::line(6);
        wanted.measure = Some("2".to_string());
        assert_eq!(error.position(), Some(&wanted));

        let opus = format!("X:1\nK:C\nc d e f |\n{tune}");
        let error = from_abc_number(&opus, 2).unwrap_err();
        wanted.line = Some(9);
        assert_eq!(error.position(), Some(&wanted));
    }

    #[test]
    fn a_tune_is_a_score_of_measures() {
        let score =
            from_abc("X:1\nT:Tune\nC:Anon\nM:3/4\nL:1/4\nK:D\nD E F | G A B | c d2 |]\n").unwrap();
        assert_eq!(score.kind(), StreamKind::Score);
        let metadata = score.metadata().unwrap();
        assert_eq!(metadata.first_text("title"), Some("Tune"));
        let part = score.parts()[0];
        let measures = part.measures();
        assert_eq!(measures.len(), 3);
        let names: Vec<String> = measures[0]
            .pitches()
            .iter()
            .map(|pitch| pitch.name_with_octave())
            .collect();
        assert_eq!(names, ["D4", "E4", "F#4"]);
        assert_eq!(
            measures[2].right_barline().unwrap().bar_type(),
            BarlineType::Final
        );
    }

    #[test]
    fn several_tunes_are_an_opus_and_one_can_be_asked_for() {
        let text = "X:1\nT:One\nM:2/4\nL:1/4\nK:C\nC D | E F |]\n\nX:2\nT:Two\nM:2/4\nL:1/4\nK:C\nG A | B c |]\n";
        let opus = from_abc(text).unwrap();
        assert_eq!(opus.kind(), StreamKind::Opus);
        assert_eq!(opus.events().len(), 2);
        let second = from_abc_number(text, 2).unwrap();
        assert_eq!(second.metadata().unwrap().first_text("title"), Some("Two"));
        assert!(from_abc_number(text, 3).is_err());
    }

    #[test]
    fn a_measure_holding_more_than_its_bar_is_cut_in_two() {
        // Six quarters between barlines in 4/4: a whole bar, then a 2/4 bar
        // holding the tied remainder of the long note, and the meter put
        // back on the measure after.
        let score = from_abc(
            "X:1
M:4/4
L:1/4
K:C
CDEF|GAB4|CDEF|
",
        )
        .unwrap();
        let part = score.events()[0].element().as_stream().unwrap();
        let measures = part.measures();
        let numbers: Vec<_> = measures.iter().map(|measure| measure.number()).collect();
        assert_eq!(numbers, [0, 1, 2, 3]);
        let meter = |measure: &Stream| {
            measure
                .events()
                .iter()
                .find_map(|event| match event.element() {
                    StreamElement::TimeSignature(meter) => Some(meter.ratio_string()),
                    _ => None,
                })
        };
        assert_eq!(meter(measures[1]), None);
        assert_eq!(meter(measures[2]).as_deref(), Some("2/4"));
        assert_eq!(meter(measures[3]).as_deref(), Some("4/4"));
        let ties: Vec<_> = measures[1..3]
            .iter()
            .flat_map(|measure| measure.events())
            .filter_map(|event| match event.element() {
                StreamElement::Note(note) if note.pitch().name() == "B" => Some((
                    event.element().quarter_length(),
                    note.tie().map(Tie::tie_type),
                )),
                _ => None,
            })
            .collect();
        assert_eq!(
            ties,
            [(2.0, Some(TieType::Start)), (2.0, Some(TieType::Stop))]
        );
    }

    #[test]
    fn an_ending_never_closed_is_handed_back_open() {
        let (handler, spanners) =
            handler_of("M:2/4\nL:1/4\nK:C\nc d | e f |[1 g a | b c |").unwrap();
        let read = part_stream(&handler.tokens, &spanners).unwrap();
        assert_eq!(
            read.open_endings,
            [OpenEnding {
                number: 1,
                measures: vec![2]
            }]
        );
    }

    #[test]
    fn a_rebarred_part_knows_where_each_leaf_came_from() {
        let score = from_abc("X:1\nM:2/4\nL:1/4\nK:C\nc d | e f g a | b c |\n").unwrap();
        let part = score.parts()[0].clone();
        let cut = re_bar_part(&part).unwrap();
        assert_eq!(cut.stream.leaves().len(), cut.origins.len());
        // Nothing overflows, as the reader cut already: every leaf is one read.
        assert!(cut.origins.iter().all(Option::is_some));
    }
}
