//! Reductions written into a score by its lyrics: music21's
//! `analysis.reduction`.
//!
//! A lyric starting `::` marks its note for a reduction, saying how it is
//! written there: `::/p:e/o:5/nf:no/ta:3/g:Ursatz` takes the E of a chord,
//! puts it in the fifth octave with a hollow notehead and the text `3`
//! above, in the part for the group `Ursatz`. A [`ScoreReduction`] gathers
//! every note so marked into parts of their own, above the score with the
//! marks taken out of its lyrics. A [`PartReduction`] answers when, and how
//! loudly, each part or group of parts plays.
//!
//! ```
//! use music21_rs::analysis::reduction::ScoreReduction;
//! use music21_rs::stream::{Stream, StreamElement, StreamKind};
//! use music21_rs::tinynotation::from_tiny_notation;
//!
//! let mut part = from_tiny_notation("4/4 c4 d e f g1")?;
//! part.set_kind(StreamKind::Part);
//! part.for_each_mut(&mut |_, element| {
//!     if let StreamElement::Note(note) = element
//!         && note.pitch().name() == "E"
//!     {
//!         note.add_lyric("::/o:5/tb:3", None, false)
//!             .expect("a note takes a lyric");
//!     }
//! });
//! let mut score = Stream::with_kind(StreamKind::Score);
//! score.insert(0.0, part);
//! let mut reduction = ScoreReduction::new();
//! reduction.set_score(score);
//! let reduced = reduction.reduce()?;
//! let notes = reduced.parts()[0].notes();
//! assert_eq!(notes.len(), 1);
//! # Ok::<(), music21_rs::Error>(())
//! ```

use crate::{
    defaults::{FloatType, IntegerType},
    duration::Duration,
    error::{Error, Result},
    expressions::TextExpression,
    instrument::Instrument,
    makenotation::op_frac,
    notation::{Lyric, StemDirection},
    note::Note,
    pitch::Pitch,
    rest::Rest,
    stream::{Stream, StreamElement, StreamEvent, StreamKind, TemplateOptions},
};

/// How a lyric marks its note for a reduction: music21's `ReductiveNote`
/// parameters, each as the lyric writes it. The keys are `p` for the pitch
/// taken from a chord, `o` the octave, `nf` the notehead fill, `sd` the stem
/// direction, `g` the group, `v` the voice, `ta` text above and `tb` text
/// below.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ReductiveParameters {
    /// The pitch name taken from a chord: `p`.
    pub pitch: Option<String>,
    /// The octave the note is written in: `o`.
    pub octave: Option<String>,
    /// Whether the notehead is filled: `nf`, `yes` or `no` or a fill
    /// music21 names.
    pub notehead_fill: Option<String>,
    /// Which way the stem points: `sd`, no stem unless said.
    pub stem_direction: Option<String>,
    /// The group whose part the note goes in: `g`.
    pub group: Option<String>,
    /// The voice the note goes in: `v`.
    pub voice: Option<String>,
    /// Text written above the note: `ta`.
    pub text_above: Option<String>,
    /// Text written below the note, as a lyric: `tb`.
    pub text_below: Option<String>,
}

impl ReductiveParameters {
    /// The parameters a lyric says, or nothing for a lyric that does not
    /// start `::`. Each `/`-separated `key:value` after the first sets the
    /// parameter the key names; one naming none is passed over, as is
    /// anything without a `:`. The stem direction is `noStem` unless said.
    ///
    /// # Errors
    ///
    /// A key written in capitals, which music21 recognizes but cannot look
    /// up, or an argument with more than one `:`.
    pub fn parse(spec: &str) -> Result<Option<Self>> {
        let spec = spec.trim();
        if !spec.starts_with("::") {
            return Ok(None);
        }
        let mut parameters = Self {
            stem_direction: Some("noStem".to_string()),
            ..Self::default()
        };
        for argument in spec.split('/').skip(1) {
            if !argument.contains(':') {
                continue;
            }
            let [key, value] = argument.split(':').collect::<Vec<_>>()[..] else {
                return Err(Error::Analysis(format!(
                    "too many values to unpack in {argument:?}"
                )));
            };
            let key = key.trim();
            let value = Some(value.trim().to_string());
            let slot = match key.to_lowercase().as_str() {
                "p" => &mut parameters.pitch,
                "o" => &mut parameters.octave,
                "nf" => &mut parameters.notehead_fill,
                "sd" => &mut parameters.stem_direction,
                "g" => &mut parameters.group,
                "v" => &mut parameters.voice,
                "ta" => &mut parameters.text_above,
                "tb" => &mut parameters.text_below,
                _ => continue,
            };
            if key.chars().any(char::is_uppercase) {
                return Err(Error::Analysis(format!("KeyError: {key:?}")));
            }
            *slot = value;
        }
        Ok(Some(parameters))
    }
}

/// A note marked by a lyric for a reduction, with where it stands: music21's
/// `ReductiveNote`.
#[derive(Clone, Debug)]
pub struct ReductiveNote {
    parameters: ReductiveParameters,
    parsed: bool,
    note: StreamElement,
    measure_index: usize,
    measure_offset: FloatType,
}

impl ReductiveNote {
    /// The note `note`, marked by the lyric `spec`, in the measure of that
    /// index at that offset. A lyric not starting `::` marks nothing, and
    /// leaves the parameters at their defaults.
    ///
    /// # Errors
    ///
    /// A lyric [`ReductiveParameters::parse`] refuses.
    pub fn new(
        spec: &str,
        note: StreamElement,
        measure_index: usize,
        measure_offset: FloatType,
    ) -> Result<Self> {
        let parsed = ReductiveParameters::parse(spec)?;
        Ok(Self {
            parsed: parsed.is_some(),
            parameters: parsed.unwrap_or_else(|| ReductiveParameters {
                stem_direction: Some("noStem".to_string()),
                ..ReductiveParameters::default()
            }),
            note,
            measure_index,
            measure_offset,
        })
    }

    /// Whether the lyric marked the note at all: music21's `isParsed`.
    pub fn is_parsed(&self) -> bool {
        self.parsed
    }

    /// What the lyric says.
    pub fn parameters(&self) -> &ReductiveParameters {
        &self.parameters
    }

    /// The note marked.
    pub fn note(&self) -> &StreamElement {
        &self.note
    }

    /// Which measure of its part the note is in, counting from nought.
    pub fn measure_index(&self) -> usize {
        self.measure_index
    }

    /// Where in its measure, or its voice, the note starts.
    pub fn measure_offset(&self) -> FloatType {
        self.measure_offset
    }

    /// The note as the reduction writes it, and the text above it if any:
    /// music21's `getNoteAndTextExpression`.
    ///
    /// A chord gives the last of its notes named as the `p` parameter says,
    /// C where none is said. The note keeps its pitch and length but loses
    /// its lyrics, tie, expressions, articulations and dots; its accidental,
    /// if written, is shown. The octave, stem direction and notehead fill
    /// are then set as said, and the text below added as a lyric.
    ///
    /// # Errors
    ///
    /// A chord with no note of the name, an unpitched note or percussion
    /// chord, or a parameter music21 cannot read: an octave not a number, a
    /// stem direction or notehead fill it does not name.
    pub fn note_and_text_expression(&self) -> Result<(Note, Option<TextExpression>)> {
        let components: Vec<Note> = match &self.note {
            StreamElement::Note(_) => Vec::new(),
            StreamElement::Chord(chord) => chord
                .notes()
                .iter()
                .map(|note| {
                    let mut note = note.clone();
                    if let Some(duration) = chord.duration() {
                        note.set_duration(duration.clone());
                    }
                    note
                })
                .collect(),
            StreamElement::ChordSymbol(symbol) => symbol
                .pitches()?
                .into_iter()
                .map(Note::from_pitch)
                .collect(),
            other => {
                return Err(Error::Analysis(format!(
                    "{other:?} has no attribute 'pitch'"
                )));
            }
        };
        let mut note = if let StreamElement::Note(note) = &self.note {
            note.clone()
        } else {
            let wanted = match &self.parameters.pitch {
                Some(name) => Pitch::from_name(name.as_str())?,
                None => Pitch::default(),
            }
            .name()
            .to_lowercase();
            components
                .into_iter()
                .rfind(|note| note.pitch().name().to_lowercase() == wanted)
                .ok_or_else(|| {
                    Error::Analysis(format!(
                        "Could not find pitch, {:?} in the note",
                        self.parameters.pitch
                    ))
                })?
        };
        note.lyrics_mut().clear();
        note.set_tie(None);
        note.expressions_mut().clear();
        note.articulations_mut().clear();
        let mut duration = note.duration().cloned().unwrap_or_default();
        duration.set_dots(0)?;
        note.set_duration(duration);
        let mut pitch = note.pitch().clone();
        if let Some(accidental) = pitch.written_accidental_mut() {
            accidental.set_display_status(Some(true));
        }
        if let Some(octave) = self.parameters.octave.as_deref().filter(|o| !o.is_empty()) {
            let octave: IntegerType = octave.parse().map_err(|_| {
                Error::Analysis(format!(
                    "invalid literal for int() with base 10: {octave:?}"
                ))
            })?;
            pitch.set_octave(Some(octave));
        }
        note.set_pitch(pitch);
        if let Some(stem) = &self.parameters.stem_direction {
            let direction = StemDirection::from_name(stem)
                .map_err(|_| Error::Analysis(format!("not a valid stem direction name: {stem}")))?;
            note.set_stem_direction(direction);
        }
        if let Some(fill) = self
            .parameters
            .notehead_fill
            .as_deref()
            .filter(|f| !f.is_empty())
        {
            let fill = match fill {
                "none" | "default" => None,
                "yes" | "filled" => Some(true),
                "no" | "notfilled" => Some(false),
                other => {
                    return Err(Error::Analysis(format!(
                        "not a valid notehead fill value: {other:?}"
                    )));
                }
            };
            note.set_notehead_fill(fill);
        }
        if let Some(text) = &self.parameters.text_below {
            note.add_lyric(text, None, false)?;
        }
        let text = self.parameters.text_above.clone().map(TextExpression::new);
        Ok((note, text))
    }
}

/// A score with its marked notes drawn out into parts of their own:
/// music21's `ScoreReduction`.
///
/// Each group the marks name has a part, made from the first part's
/// template, with the marked notes in the voices the marks name. The parts
/// stand above the chord reduction's parts, if one is given, and the score's,
/// whose marks are taken out of their lyrics.
#[derive(Clone, Debug, Default)]
pub struct ScoreReduction {
    score: Option<Stream>,
    chord_reduction: Option<Stream>,
}

impl ScoreReduction {
    /// A reduction of nothing yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// The score reduced.
    pub fn score(&self) -> Option<&Stream> {
        self.score.as_ref()
    }

    /// Sets the score reduced. A stream holding no parts is taken as the
    /// one part of a score.
    pub fn set_score(&mut self, score: Stream) {
        self.score = Some(as_score(score));
    }

    /// A score of chords whose marked notes go in the reduction too, and
    /// whose parts are written below it: music21's `chordReduction`.
    pub fn chord_reduction(&self) -> Option<&Stream> {
        self.chord_reduction.as_ref()
    }

    /// Sets the chord reduction. A stream holding no parts is taken as the
    /// one part of a score.
    pub fn set_chord_reduction(&mut self, chords: Stream) {
        self.chord_reduction = Some(as_score(chords));
    }

    /// The reduction: music21's `reduce`.
    ///
    /// The marked notes are gathered, the chord reduction's first and then
    /// the score's, part by part and measure by measure, each lyric of a
    /// note once. Each group named has a part: the first part's template
    /// without its voices, its id and an instrument's part name the group.
    /// Where only one group is named, or none, every note goes in its part.
    /// A measure given a note loses its rests for a voice for each voice
    /// named, and each note is added into the voice its mark names, or the
    /// first, as [`Stream::insert_into_note_or_chord`] adds it, with its text
    /// above in the measure. Each voice given notes has its gaps filled with
    /// rests; voices left empty go, and one left alone is flattened into its
    /// measure; and every rest of the part is hidden.
    ///
    /// # Errors
    ///
    /// Nothing to reduce; a lyric with no text, or one marking a note
    /// [`ReductiveNote`] cannot write; a mark in a measure the first part
    /// has not got; or two notes added where music21 refuses them.
    pub fn reduce(&self) -> Result<Stream> {
        if self.score.is_none() && self.chord_reduction.is_none() {
            return Err(Error::Analysis("no score defined to reduce".to_string()));
        }
        let mut chords = self.chord_reduction.clone();
        let mut score = self.score.clone();
        let mut notes: Vec<(usize, String, ReductiveNote)> = Vec::new();
        let mut serial = 0;
        for source in [&mut chords, &mut score].into_iter().flatten() {
            extract(source, &mut notes, &mut serial)?;
        }
        let mut groups: Vec<Option<String>> = Vec::new();
        let mut voices: Vec<Option<String>> = Vec::new();
        for (_, _, note) in &notes {
            for (names, name) in [
                (&mut groups, &note.parameters.group),
                (&mut voices, &note.parameters.voice),
            ] {
                if !names.contains(name) {
                    names.push(name.clone());
                }
                if names.len() == 2 && names.contains(&None) {
                    names.retain(Option::is_some);
                }
            }
        }
        let one_group = groups.len() == 1;
        let one_voice = voices.len() == 1;

        let source = match (&score, &chords) {
            (Some(score), _) if !score.events().is_empty() => score,
            (_, Some(chords)) => chords,
            _ => return Err(Error::Analysis("no part to make a template of".to_string())),
        };
        let first = source
            .parts()
            .into_iter()
            .next()
            .ok_or_else(|| Error::Analysis("no part to make a template of".to_string()))?;
        let template = first.template(&TemplateOptions {
            retain_voices: false,
            ..TemplateOptions::default()
        });

        let mut reduced = Stream::with_kind(StreamKind::Score);
        let mut parts: Vec<Stream> = Vec::new();
        for group in &groups {
            let mut part = template.clone();
            part.set_id(group.clone());
            let mut instrument = Instrument::new();
            instrument.set_part_name(group.clone());
            part.insert_sorted(vec![StreamEvent::new(0.0, instrument)]);
            for (_, _, note) in &notes {
                if one_group || note.parameters.group == *group {
                    place(&mut part, note, &voices, one_voice)?;
                }
            }
            for event in part.events_mut() {
                if let StreamElement::Stream(measure) = event.element_mut()
                    && measure.kind() == StreamKind::Measure
                {
                    finish(measure)?;
                }
            }
            parts.push(part);
        }
        for source in [chords, score].into_iter().flatten() {
            parts.extend(source.parts().into_iter().cloned());
        }
        reduced.insert_sorted(
            parts
                .into_iter()
                .map(|part| StreamEvent::new(0.0, part))
                .collect(),
        );
        Ok(reduced)
    }
}

/// A stream holding parts as it is, anything else as the one part of a
/// score.
fn as_score(stream: Stream) -> Stream {
    if stream.has_part_like_streams() {
        return stream;
    }
    let mut score = Stream::with_kind(StreamKind::Score);
    score.insert(0.0, stream);
    score
}

/// Whether music21 counts an element among a stream's `notes`.
fn is_note(element: &StreamElement) -> bool {
    matches!(
        element,
        StreamElement::Note(_)
            | StreamElement::Chord(_)
            | StreamElement::ChordSymbol(_)
            | StreamElement::Unpitched(_)
            | StreamElement::PercussionChord(_)
    )
}

/// An element's lyrics, where it has any.
fn lyrics_mut(element: &mut StreamElement) -> Option<&mut Vec<Lyric>> {
    match element {
        StreamElement::Note(note) => Some(note.lyrics_mut()),
        StreamElement::Chord(chord) => chord.notes_mut().first_mut().map(Note::lyrics_mut),
        StreamElement::ChordSymbol(symbol) => Some(symbol.lyrics_mut()),
        StreamElement::Unpitched(unpitched) => Some(unpitched.written_mut().lyrics_mut()),
        StreamElement::PercussionChord(chord) => chord
            .written_mut()
            .notes_mut()
            .first_mut()
            .map(Note::lyrics_mut),
        _ => None,
    }
}

/// Gathers the notes a score's lyrics mark, each lyric of a note once, and
/// blanks the lyrics that marked them: music21's `_extractReductionEvents`.
/// Each note is told apart by `serial`, counted across every score
/// gathered from.
fn extract(
    score: &mut Stream,
    notes: &mut Vec<(usize, String, ReductiveNote)>,
    serial: &mut usize,
) -> Result<()> {
    for event in score.events_mut() {
        let StreamElement::Stream(part) = event.element_mut() else {
            continue;
        };
        if !matches!(part.kind(), StreamKind::Part | StreamKind::PartStaff) {
            continue;
        }
        let mut index = 0;
        for event in part.events_mut() {
            let StreamElement::Stream(measure) = event.element_mut() else {
                continue;
            };
            if measure.kind() != StreamKind::Measure {
                continue;
            }
            extract_measure(measure, index, 0, None, notes, serial)?;
            index += 1;
        }
    }
    Ok(())
}

/// [`extract`] within a measure, `depth` streams down. A note directly in
/// the measure, or directly in one of its voices, stands at its offset
/// there; one deeper is read as at the start, as music21 reads it.
fn extract_measure(
    stream: &mut Stream,
    index: usize,
    depth: usize,
    in_voice: Option<bool>,
    notes: &mut Vec<(usize, String, ReductiveNote)>,
    serial: &mut usize,
) -> Result<()> {
    for event in stream.events_mut() {
        let offset = event.offset();
        let element = event.element_mut();
        if let StreamElement::Stream(inner) = element {
            let voice = depth == 0 && inner.kind() == StreamKind::Voice;
            extract_measure(inner, index, depth + 1, Some(voice), notes, serial)?;
            continue;
        }
        if !is_note(element) {
            continue;
        }
        let note_serial = *serial;
        *serial += 1;
        let Some(lyrics) = lyrics_mut(element).map(|lyrics| lyrics.clone()) else {
            continue;
        };
        if lyrics.is_empty() {
            continue;
        }
        let measure_offset = match (depth, in_voice) {
            (0, _) | (1, Some(true)) => offset,
            _ => 0.0,
        };
        let mut marked = Vec::new();
        for (position, lyric) in lyrics.iter().enumerate() {
            let text = lyric
                .explicit_text()
                .ok_or_else(|| Error::Analysis("a lyric with no text".to_string()))?;
            let note = ReductiveNote::new(&text, element.clone(), index, measure_offset)?;
            if note.is_parsed() {
                match notes
                    .iter_mut()
                    .find(|(serial, known, _)| *serial == note_serial && *known == text)
                {
                    Some(entry) => entry.2 = note,
                    None => notes.push((note_serial, text, note)),
                }
                marked.push(position);
            }
        }
        if let Some(lyrics) = lyrics_mut(element) {
            for position in marked {
                lyrics[position] = Lyric::unsung();
            }
        }
    }
    Ok(())
}

/// Puts a marked note in the reduction's part: into the voice its mark
/// names, or the measure's first, its text above in the measure.
fn place(
    part: &mut Stream,
    note: &ReductiveNote,
    voices: &[Option<String>],
    one_voice: bool,
) -> Result<()> {
    let measure = part
        .events_mut()
        .iter_mut()
        .filter_map(|event| match event.element_mut() {
            StreamElement::Stream(measure) if measure.kind() == StreamKind::Measure => {
                Some(measure)
            }
            _ => None,
        })
        .nth(note.measure_index)
        .ok_or_else(|| Error::Analysis("list index out of range".to_string()))?;
    if measure.voices().is_empty() {
        measure.retain_leaves(&mut |_, element| !matches!(element, StreamElement::Rest(_)));
        for id in voices {
            let mut voice = Stream::with_kind(StreamKind::Voice);
            voice.set_id(id.clone());
            measure.insert_sorted(vec![StreamEvent::new(0.0, voice)]);
        }
    }
    let wanted = note
        .parameters
        .voice
        .as_deref()
        .filter(|_| !one_voice)
        .map(str::to_lowercase);
    let (written, text) = note.note_and_text_expression()?;
    let mut voices_here =
        measure
            .events_mut()
            .iter_mut()
            .filter_map(|event| match event.element_mut() {
                StreamElement::Stream(voice) if voice.kind() == StreamKind::Voice => Some(voice),
                _ => None,
            });
    let voice = match &wanted {
        Some(wanted) => {
            let mut chosen = None;
            let mut first = None;
            for voice in voices_here {
                let named = voice.id().is_some_and(|id| id.to_lowercase() == *wanted);
                if named {
                    chosen = Some(voice);
                    break;
                }
                if first.is_none() {
                    first = Some(voice);
                }
            }
            chosen.or(first)
        }
        None => voices_here.next(),
    };
    if let Some(voice) = voice {
        voice.insert_into_note_or_chord(
            note.measure_offset,
            StreamElement::Note(written),
            false,
        )?;
    }
    if let Some(text) = text {
        measure.insert_sorted(vec![StreamEvent::new(note.measure_offset, text)]);
    }
    Ok(())
}

/// A reduction's measure finished: each voice given notes has its gaps
/// filled with rests, as music21's `makeRests` fills them; voices left empty
/// go, and one left alone is flattened; and every rest is hidden.
fn finish(measure: &mut Stream) -> Result<()> {
    for event in measure.events_mut() {
        if let StreamElement::Stream(voice) = event.element_mut()
            && voice.kind() == StreamKind::Voice
            && voice.events().iter().any(|event| is_note(event.element()))
        {
            fill_gaps(voice)?;
        }
    }
    measure.flatten_unnecessary_voices(false);
    measure.for_each_mut(&mut |_, element| {
        if let StreamElement::Rest(rest) = element {
            rest.set_hidden(true);
        }
    });
    Ok(())
}

/// Rests from the start of a voice to where it first sounds and in every
/// gap after: music21's `makeRests` with `fillGaps` on a voice.
fn fill_gaps(voice: &mut Stream) -> Result<()> {
    let mut gaps = Vec::new();
    let mut reached: FloatType = 0.0;
    for event in voice.events() {
        if event.offset() > reached {
            let length = op_frac(event.offset() - reached);
            gaps.push(StreamEvent::new(reached, Rest::new(Duration::new(length)?)));
        }
        reached = op_frac(reached.max(event.offset() + event.element().quarter_length()));
    }
    voice.insert_sorted(gaps);
    Ok(())
}

/// A group of a score's parts drawn as one: music21's `partGroups` entry.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PartGroup {
    /// The group's name, which is also its id in what is drawn.
    pub name: String,
    /// The colour the group is drawn in.
    pub color: String,
    /// What a part's id must hold, ignoring case, for the part to be in the
    /// group: the name where nothing is given. music21 also reads each as a
    /// regular expression matched at the start of the id, which for a name
    /// without pattern characters says nothing more; patterns are not read.
    pub matches: Option<Vec<String>>,
}

/// When and how loudly each part, or group of parts, of a score plays:
/// music21's `PartReduction`, whose `getGraphHorizontalBarWeightedData`
/// [`PartReduction::weighted_spans`] answers.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PartReduction {
    /// The groups drawn, or each part on its own where there are none.
    pub part_groups: Option<Vec<PartGroup>>,
    /// Whether a part plays the whole of each measure it has a note in,
    /// rather than each run of notes: music21's `fillByMeasure`. A score
    /// with a part holding no measures is read by its runs of notes.
    pub fill_by_measure: bool,
    /// Whether each stretch is cut where a dynamic changes, each piece as
    /// loud as its dynamic: music21's `segmentByTarget`.
    pub segment_by_target: bool,
    /// Whether the weights are scaled so the loudest is one: music21's
    /// `normalize`.
    pub normalize: bool,
    /// Whether each part's loudest is one, rather than the score's:
    /// music21's `normalizeByPart`.
    pub normalize_by_part: bool,
}

impl Default for PartReduction {
    fn default() -> Self {
        Self {
            part_groups: None,
            fill_by_measure: true,
            segment_by_target: true,
            normalize: true,
            normalize_by_part: false,
        }
    }
}

/// A stretch of a part's playing, and how loud: one bar of music21's
/// weighted horizontal bar graph.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WeightedSpan {
    /// Where the stretch starts, in quarter lengths from the score's start.
    pub start: FloatType,
    /// How long it lasts.
    pub span: FloatType,
    /// How loud it is.
    pub weight: FloatType,
    /// The colour it is drawn in.
    pub color: String,
}

/// A part, or a group of parts, and the stretches it plays.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PartActivity {
    /// The group's name, or the part's id.
    pub id: Option<String>,
    /// The stretches it plays, in order.
    pub spans: Vec<WeightedSpan>,
}

/// A stretch being weighed: music21's `ds` dictionaries.
#[derive(Clone, Debug)]
struct Stretch {
    start: FloatType,
    span: FloatType,
    weight: Option<FloatType>,
    color: String,
}

/// An element of a group's flattened parts, with which part and leaf it is.
struct Flat {
    offset: FloatType,
    element: StreamElement,
    part: usize,
    leaf: usize,
}

/// music21 divides the summed loudness by the length of the class name it
/// looks for, `'Dynamic'`, rather than by how many dynamics there are.
const DYNAMIC_NAME_LENGTH: FloatType = 7.0;

/// The weight a stretch takes when nothing has said one: music21's
/// `minValue`.
const MIN_WEIGHT: FloatType = 0.01;

impl PartReduction {
    /// Each group's stretches of playing and their weights: music21's
    /// `process` and `getGraphHorizontalBarWeightedData`.
    ///
    /// Each stretch is a measure with a note in it, from its start for its
    /// bar's length, or a run of notes. Its weight is the loudness of the
    /// dynamics starting in it, summed and divided by seven as music21
    /// divides it; cut by dynamic, each piece from a dynamic to the next,
    /// with the stretch before the first dynamic left out where that
    /// dynamic starts later, as music21 leaves it. A stretch with no
    /// dynamic takes the weight of the last that had one, and the first,
    /// with none before it, a hundredth. music21 lengthens each dynamic it
    /// cuts by to reach the next as it goes, and a later stretch, or group,
    /// reads those lengths; so does this. Groups, or parts, of one id share
    /// their stretches, as music21 keeps them by id: each is cut, weighed
    /// and normalized again, and each answers the same stretches.
    ///
    /// # Errors
    ///
    /// A stream that is not a score, a group's parts with fewer measures
    /// than its first, or notes whose runs cannot be found.
    pub fn weighted_spans(&self, score: &Stream) -> Result<Vec<PartActivity>> {
        if score.kind() != StreamKind::Score {
            return Err(Error::Analysis("provided Stream must be Score".to_string()));
        }
        let parts = score.parts();
        let fill_by_measure =
            self.fill_by_measure && parts.iter().all(|part| !part.measures().is_empty());
        // Each group's id, colour and parts, by index.
        let mut bundles: Vec<(Option<String>, String, Vec<usize>)> = Vec::new();
        match &self.part_groups {
            Some(groups) => {
                for group in groups {
                    let names = group
                        .matches
                        .clone()
                        .unwrap_or_else(|| vec![group.name.clone()]);
                    let held: Vec<usize> = parts
                        .iter()
                        .enumerate()
                        .filter(|(_, part)| {
                            let id = part.id().unwrap_or_default().to_lowercase();
                            names.iter().any(|name| id.contains(&name.to_lowercase()))
                        })
                        .map(|(index, _)| index)
                        .collect();
                    if !held.is_empty() {
                        bundles.push((Some(group.name.clone()), group.color.clone(), held));
                    }
                }
            }
            None => {
                for (index, part) in parts.iter().enumerate() {
                    bundles.push((
                        part.id().map(str::to_string),
                        "#666666".to_string(),
                        vec![index],
                    ));
                }
            }
        }
        // The length of every leaf of every part, as music21 lengthens the
        // dynamics.
        let mut lengths: Vec<Vec<FloatType>> = parts
            .iter()
            .map(|part| {
                part.leaves()
                    .iter()
                    .map(|(_, element)| element.quarter_length())
                    .collect()
            })
            .collect();

        // music21 keeps each group's stretches under its id, so groups of
        // one id share them: each works on, and reads back, the same list.
        // A part with no id has one of its own there.
        let keys: Vec<Result<String, usize>> = bundles
            .iter()
            .enumerate()
            .map(|(index, (id, _, _))| id.clone().ok_or(index))
            .collect();
        let mut kept: Vec<(Result<String, usize>, Vec<Stretch>)> = Vec::new();
        let slot = |kept: &[(Result<String, usize>, Vec<Stretch>)], key: &Result<String, usize>| {
            kept.iter().position(|(known, _)| known == key)
        };
        let flats: Vec<Vec<Flat>> = bundles
            .iter()
            .map(|(_, _, held)| flatten_parts(&parts, held))
            .collect();
        for (((_, color, held), key), flat) in bundles.iter().zip(&keys).zip(&flats) {
            let mut stretches = if fill_by_measure {
                measure_stretches(&parts, held)?
            } else {
                run_stretches(flat)?
            };
            for stretch in &mut stretches {
                stretch.color = color.clone();
            }
            match slot(&kept, key) {
                Some(index) => kept[index].1 = stretches,
                None => kept.push((key.clone(), stretches)),
            }
        }
        for (key, flat) in keys.iter().zip(&flats) {
            let index = slot(&kept, key).expect("every group has stretches");
            if self.segment_by_target {
                kept[index].1 = split_by_dynamics(&kept[index].1, flat, &mut lengths);
            } else {
                for stretch in &mut kept[index].1 {
                    let end = op_frac(stretch.start + stretch.span);
                    let loudness: Vec<FloatType> = flat
                        .iter()
                        .filter(|item| {
                            in_range(
                                item.offset,
                                lengths[item.part][item.leaf],
                                stretch.start,
                                end,
                                false,
                            )
                        })
                        .filter_map(|item| match &item.element {
                            StreamElement::Dynamic(dynamic) => Some(dynamic.volume_scalar()),
                            _ => None,
                        })
                        .collect();
                    stretch.weight = (!loudness.is_empty())
                        .then(|| loudness.iter().sum::<FloatType>() / DYNAMIC_NAME_LENGTH);
                }
            }
        }
        for key in &keys {
            let index = slot(&kept, key).expect("every group has stretches");
            extend_weights(&mut kept[index].1);
        }
        if self.normalize {
            let maxima: Vec<FloatType> = kept
                .iter()
                .map(|(_, stretches)| {
                    stretches
                        .iter()
                        .filter_map(|stretch| stretch.weight)
                        .fold(0.0, |max, weight| if weight > max { weight } else { max })
                })
                .collect();
            let overall = maxima.iter().copied().fold(0.0, FloatType::max);
            for key in &keys {
                let index = slot(&kept, key).expect("every group has stretches");
                let best = if self.normalize_by_part {
                    maxima[index]
                } else {
                    overall
                };
                for stretch in &mut kept[index].1 {
                    stretch.weight = Some(if best != 0.0 {
                        stretch.weight.unwrap_or_default() / best
                    } else {
                        1.0
                    });
                }
            }
        }
        Ok(bundles
            .into_iter()
            .zip(&keys)
            .map(|((id, _, _), key)| {
                let index = slot(&kept, key).expect("every group has stretches");
                PartActivity {
                    id,
                    spans: kept[index]
                        .1
                        .iter()
                        .map(|stretch| WeightedSpan {
                            start: stretch.start,
                            span: stretch.span,
                            weight: stretch.weight.unwrap_or_default(),
                            color: stretch.color.clone(),
                        })
                        .collect(),
                }
            })
            .collect())
    }
}

/// A group's parts flattened together, as music21 flattens a stream holding
/// them: by offset, class and grace, and otherwise in the order met.
fn flatten_parts(parts: &[&Stream], held: &[usize]) -> Vec<Flat> {
    let mut flat: Vec<Flat> = Vec::new();
    for &part in held {
        for (leaf, (offset, element)) in parts[part].leaves().into_iter().enumerate() {
            flat.push(Flat {
                offset,
                element: element.clone(),
                part,
                leaf,
            });
        }
    }
    let grace = |element: &StreamElement| element.duration().is_some_and(Duration::is_grace);
    flat.sort_by(|left, right| {
        left.offset
            .partial_cmp(&right.offset)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                left.element
                    .class_sort_order()
                    .cmp(&right.element.class_sort_order())
            })
            .then_with(|| grace(&right.element).cmp(&grace(&left.element)))
    });
    flat
}

/// The meter a measure opens with: music21's `timeSignature`.
fn meter_in(measure: &Stream) -> Option<crate::meter::TimeSignature> {
    measure
        .events()
        .iter()
        .find_map(|event| match event.element() {
            StreamElement::TimeSignature(meter) if event.offset() == 0.0 => Some(meter.clone()),
            _ => None,
        })
}

/// A stretch for each measure of the group's first part in which any of
/// its parts has a note directly, as long as the measure's bar: music21's
/// `fillByMeasure`.
fn measure_stretches(parts: &[&Stream], held: &[usize]) -> Result<Vec<Stretch>> {
    let measured: Vec<Vec<(FloatType, &Stream)>> = held
        .iter()
        .map(|&part| {
            parts[part]
                .events()
                .iter()
                .filter_map(|event| match event.element() {
                    StreamElement::Stream(measure) if measure.kind() == StreamKind::Measure => {
                        Some((op_frac(event.offset()), &**measure))
                    }
                    _ => None,
                })
                .collect()
        })
        .collect();
    let first = parts[held[0]];
    let loose: Vec<(FloatType, crate::meter::TimeSignature)> = first
        .events()
        .iter()
        .filter_map(|event| match event.element() {
            StreamElement::TimeSignature(meter) => Some((event.offset(), meter.clone())),
            _ => None,
        })
        .collect();
    let mut meter: Option<crate::meter::TimeSignature> = None;
    let mut stretches = Vec::new();
    for (index, (offset, measure)) in measured[0].iter().enumerate() {
        if let Some(own) = meter_in(measure) {
            meter = Some(own);
        }
        let mut active = false;
        for part in &measured {
            let (_, other) = part
                .get(index)
                .ok_or_else(|| Error::Analysis("index out of range".to_string()))?;
            if other.events().iter().any(|event| is_note(event.element())) {
                active = true;
                break;
            }
        }
        if !active {
            continue;
        }
        // music21's `barDuration`: the measure's meter or the one standing
        // before it, else the meter that fits what it holds, else what it
        // holds.
        let bar = match &meter {
            Some(meter) => meter.bar_quarter_length(),
            None => match loose.iter().rev().find(|(at, _)| at < offset) {
                Some((_, meter)) => meter.bar_quarter_length(),
                None => crate::meter::best_time_signature(measure)
                    .map_or_else(|_| measure.end_offset(), |meter| meter.bar_quarter_length()),
            },
        };
        stretches.push(Stretch {
            start: op_frac(*offset),
            span: op_frac(op_frac(*offset + bar) - *offset),
            weight: None,
            color: String::new(),
        });
    }
    Ok(stretches)
}

/// A stretch for each run of consecutive notes of the flattened parts,
/// from the first note's start to the last's end: music21's stretches when
/// not filling by measure.
fn run_stretches(flat: &[Flat]) -> Result<Vec<Stretch>> {
    let stream = Stream::from_events(
        flat.iter()
            .map(|item| StreamEvent::new(item.offset, item.element.clone())),
    );
    let found = stream.find_consecutive_notes(&crate::stream::ConsecutiveOptions::default())?;
    let leaves = stream.leaves();
    let place = |index: usize| {
        let (offset, element) = leaves[index];
        (offset, element.quarter_length())
    };
    let mut stretches = Vec::new();
    let mut start: Option<FloatType> = None;
    let mut last: Option<usize> = None;
    let count = found.len();
    for (position, entry) in found.into_iter().enumerate() {
        match entry {
            None => {
                let Some(begun) = start else {
                    continue;
                };
                let (offset, length) = place(last.expect("a run has a last note"));
                let end = op_frac(offset + length);
                stretches.push(Stretch {
                    start: begun,
                    span: op_frac(end - begun),
                    weight: None,
                    color: String::new(),
                });
                start = None;
            }
            Some(index) if position + 1 >= count => {
                let (offset, length) = place(index);
                let begun = start.unwrap_or(offset);
                let end = op_frac(offset + length);
                stretches.push(Stretch {
                    start: begun,
                    span: op_frac(end - begun),
                    weight: None,
                    color: String::new(),
                });
                start = None;
            }
            Some(index) => {
                if start.is_none() {
                    start = Some(place(index).0);
                }
                last = Some(index);
            }
        }
    }
    Ok(stretches)
}

/// Whether music21's `getElementsByOffset` finds an element at `offset`
/// lasting `length` between `start` and `end`, the element starting in
/// the span and the end itself counted only with `include_end`.
fn in_range(
    offset: FloatType,
    length: FloatType,
    start: FloatType,
    end: FloatType,
    include_end: bool,
) -> bool {
    if offset > end {
        return false;
    }
    if op_frac(offset + length) < start {
        return false;
    }
    if end <= start && length == 0.0 {
        return true;
    }
    if offset < start {
        return false;
    }
    include_end || offset != end
}

/// Each stretch cut at its dynamics, each piece as loud as its dynamic,
/// the dynamics lengthened to reach the next as music21 lengthens them.
fn split_by_dynamics(
    stretches: &[Stretch],
    flat: &[Flat],
    lengths: &mut [Vec<FloatType>],
) -> Vec<Stretch> {
    let mut cut = Vec::new();
    for stretch in stretches {
        let end = op_frac(stretch.start + stretch.span);
        let dynamics: Vec<&Flat> = flat
            .iter()
            .filter(|item| matches!(item.element, StreamElement::Dynamic(_)))
            .filter(|item| {
                in_range(
                    item.offset,
                    lengths[item.part][item.leaf],
                    stretch.start,
                    end,
                    true,
                )
            })
            .collect();
        if dynamics.is_empty() {
            cut.push(stretch.clone());
            continue;
        }
        // music21's `extendDuration`: each dynamic lasts to the next, the
        // last to where the latest of them ends.
        let total = dynamics
            .iter()
            .map(|item| op_frac(item.offset + lengths[item.part][item.leaf]))
            .fold(0.0, FloatType::max);
        for (index, item) in dynamics.iter().enumerate() {
            let length = match dynamics.get(index + 1) {
                Some(next) => op_frac(next.offset - item.offset),
                None => op_frac(total - item.offset),
            };
            lengths[item.part][item.leaf] = length;
        }
        for (index, item) in dynamics.iter().enumerate() {
            let StreamElement::Dynamic(dynamic) = &item.element else {
                continue;
            };
            let mut span = lengths[item.part][item.leaf];
            if op_frac(item.offset + span) > end {
                span = op_frac(end - item.offset);
            }
            if span <= 0.001 {
                span = op_frac(end - item.offset);
            }
            let weight = Some(dynamic.volume_scalar() / DYNAMIC_NAME_LENGTH);
            if index == 0 && stretch.start == item.offset {
                cut.push(Stretch {
                    start: stretch.start,
                    span,
                    weight,
                    color: stretch.color.clone(),
                });
            } else {
                cut.push(Stretch {
                    start: item.offset,
                    span,
                    weight,
                    color: stretch.color.clone(),
                });
            }
        }
    }
    cut
}

/// Gives each stretch with no weight the last weight before it, and the
/// first, or any with none before, the least: music21's `_extendSpans`.
fn extend_weights(stretches: &mut [Stretch]) {
    let mut last: Option<FloatType> = None;
    for (index, stretch) in stretches.iter_mut().enumerate() {
        if index == 0 {
            match stretch.weight {
                None => stretch.weight = Some(MIN_WEIGHT),
                Some(weight) => last = Some(weight),
            }
            continue;
        }
        match stretch.weight {
            Some(weight) if weight != 0.0 => last = Some(weight),
            weight => {
                if let Some(previous) = last.filter(|previous| *previous != 0.0) {
                    stretch.weight = Some(previous);
                } else if weight.is_none() && last.is_none() {
                    stretch.weight = Some(MIN_WEIGHT);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lyric_says_how_its_note_is_reduced() -> Result<()> {
        let parameters =
            ReductiveParameters::parse("::/o:4/nf:no/g:Ursatz/ta:3 3 200")?.expect("a mark");
        assert_eq!(parameters.octave.as_deref(), Some("4"));
        assert_eq!(parameters.notehead_fill.as_deref(), Some("no"));
        assert_eq!(parameters.group.as_deref(), Some("Ursatz"));
        assert_eq!(parameters.text_above.as_deref(), Some("3 3 200"));
        assert_eq!(parameters.stem_direction.as_deref(), Some("noStem"));
        assert_eq!(ReductiveParameters::parse("test")?, None);
        assert!(ReductiveParameters::parse("::/P:c").is_err());
        assert!(ReductiveParameters::parse("::/ta:a:b").is_err());
        Ok(())
    }

    #[test]
    fn a_chord_gives_the_note_named() -> Result<()> {
        // music21's own test: the E of a C major chord, in the sixth octave.
        let chord = crate::chord::Chord::new("C4 E4 G4")?;
        let note = ReductiveNote::new(
            "::/p:e/o:6/sd:up/nf:no/ta:hi/tb:lo",
            StreamElement::Chord(chord.clone()),
            0,
            0.0,
        )?;
        let (written, text) = note.note_and_text_expression()?;
        assert_eq!(written.pitch().name_with_octave(), "E6");
        assert_eq!(written.stem_direction(), StemDirection::Up);
        assert_eq!(written.notehead_fill(), Some(false));
        assert_eq!(
            text.map(|text| text.content().to_string()),
            Some("hi".to_string())
        );
        // With no pitch said, a chord gives its C.
        let note = ReductiveNote::new("::", StreamElement::Chord(chord), 0, 0.0)?;
        assert_eq!(
            note.note_and_text_expression()?
                .0
                .pitch()
                .name_with_octave(),
            "C4"
        );
        Ok(())
    }

    #[test]
    fn parts_are_weighed_by_their_dynamics() -> Result<()> {
        // music21's testPartReductionC: two parts of the same notes, their
        // dynamics in different places, weighed without normalizing.
        let mut score = Stream::with_kind(StreamKind::Score);
        let placed = [
            [(0.0, "p"), (2.0, "fff"), (6.0, "ppp")],
            [(0.0, "p"), (1.0, "fff"), (2.0, "ppp")],
        ];
        for (id, dynamics) in placed.iter().enumerate() {
            let mut part = Stream::with_kind(StreamKind::Part);
            part.set_id(Some(id.to_string()));
            for length in [1.0, 2.0, 1.0, 4.0] {
                part.push(Note::from_name("C4")?.with_duration(Duration::new(length)?));
            }
            for (offset, mark) in dynamics {
                part.insert(*offset, crate::dynamics::Dynamic::new(*mark));
            }
            score.insert(0.0, part);
        }
        let reduction = PartReduction {
            normalize: false,
            ..PartReduction::default()
        };
        let spans: Vec<Vec<(FloatType, FloatType, FloatType)>> = reduction
            .weighted_spans(&score)?
            .into_iter()
            .map(|activity| {
                activity
                    .spans
                    .into_iter()
                    .map(|span| (span.start, span.span, (span.weight * 1e9).round() / 1e9))
                    .collect()
            })
            .collect();
        assert_eq!(
            spans,
            [
                vec![
                    (0.0, 2.0, 0.05),
                    (2.0, 4.0, 0.128571429),
                    (6.0, 2.0, 0.021428571)
                ],
                vec![
                    (0.0, 1.0, 0.05),
                    (1.0, 1.0, 0.128571429),
                    (2.0, 6.0, 0.021428571)
                ],
            ]
        );
        Ok(())
    }

    #[test]
    fn a_chord_reduction_alone_is_reduced_above_itself() -> Result<()> {
        let mut chord = crate::chord::Chord::new("G3 B3 D4")?;
        chord.add_lyric("::/p:g/tb:V", None, false)?;
        let mut measure = Stream::with_kind(StreamKind::Measure);
        measure.insert(0.0, chord);
        let mut part = Stream::with_kind(StreamKind::Part);
        part.insert(0.0, measure);
        let mut reduction = ScoreReduction::new();
        reduction.set_chord_reduction(part);
        let reduced = reduction.reduce()?;
        let parts = reduced.parts();
        assert_eq!(parts.len(), 2);
        let notes = parts[0].notes();
        assert_eq!(notes.len(), 1);
        let StreamElement::Note(note) = &notes[0].1 else {
            panic!("a note");
        };
        assert_eq!(note.pitch().name_with_octave(), "G3");
        assert_eq!(note.lyrics()[0].text(), "V");
        // The mark is taken out of the chord's lyrics.
        let StreamElement::Chord(chord) = &parts[1].notes()[0].1 else {
            panic!("a chord");
        };
        assert_eq!(chord.lyrics()[0].text(), "");
        Ok(())
    }
}
