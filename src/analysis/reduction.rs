//! Reductions written into a score by its lyrics: music21's
//! `analysis.reduction`.
//!
//! A lyric starting `::` marks its note for a reduction, saying how it is
//! written there: `::/p:e/o:5/nf:no/ta:3/g:Ursatz` takes the E of a chord,
//! puts it in the fifth octave with a hollow notehead and the text `3`
//! above, in the part for the group `Ursatz`. A [`ScoreReduction`] gathers
//! every note so marked into parts of their own, above the score with the
//! marks taken out of its lyrics.
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
