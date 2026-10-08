//! Notes as tuples of what they are -- pitch, length, offset -- to compare
//! two streams by: music21's `alpha.analysis.hasher`.

use crate::{
    defaults::FloatType,
    error::Result,
    interval::Interval,
    pitch::Pitch,
    stream::{Stream, StreamElement, StreamKind},
};

/// One part of a note's hash.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum HashValue {
    /// A whole number: a MIDI pitch, an octave, an interval class.
    Integer(i64),
    /// A length or an offset.
    Float(FloatType),
    /// A name: a pitch's, or a chord's normal order or prime form.
    Text(String),
    /// Nothing, as music21 hashes what it cannot say.
    None,
}

/// Which element a hash was made from: its place among the elements of the
/// stream music21's `recurse` walks, and for a note of a chord hashed as
/// notes, which note.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HashReference {
    /// The element's place in [`Stream::recurse`].
    pub element: usize,
    /// The note of a chord, where a chord is hashed note by note.
    pub component: Option<usize>,
}

/// A note's hash: the values the hasher says, named, and what it was made
/// from where asked: music21's `NoteHash` and `NoteHashWithReference`.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NoteHash {
    /// What each value is: `Pitch`, `Duration`, `Offset` and the rest.
    pub keys: Vec<String>,
    /// The values, one a key.
    pub values: Vec<HashValue>,
    /// What the hash was made from, where the hasher includes it.
    pub reference: Option<HashReference>,
}

/// What a hasher hashes of each note, rest and chord: music21's `Hasher`.
///
/// By default a note is its MIDI pitch, its length and its offset, each
/// rounded to a 32nd of a quarter, and a chord is hashed note by note.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hasher {
    /// Whether each hash says what it was made from.
    pub include_reference: bool,
    /// Whether a note's pitch is hashed.
    pub hash_pitch: bool,
    /// Whether the pitch is its MIDI number rather than its name.
    pub hash_midi: bool,
    /// Whether a named pitch leaves out its octave: music21's
    /// `hashNoteNameOctave`.
    pub hash_note_name_octave: bool,
    /// Whether the octave is hashed.
    pub hash_octave: bool,
    /// Whether the length is hashed.
    pub hash_duration: bool,
    /// Whether lengths and offsets are rounded to `granularity`.
    pub round_duration_and_offset: bool,
    /// Whether the offset in the element's own measure or voice is hashed.
    pub hash_offset: bool,
    /// How many parts of a quarter lengths and offsets are rounded to.
    pub granularity: u32,
    /// Whether the interval class from the note before in the same part is
    /// hashed; a part's first note has none.
    pub hash_interval_from_last_note: bool,
    /// Whether whether a note is altered is hashed: music21 hashes nothing
    /// for it yet, so the value is always nothing.
    pub hash_is_accidental: bool,
    /// Whether ties are hashed: music21 does not use it.
    pub hash_is_tied: bool,
    /// Whether a chord is hashed note by note.
    pub hash_chords_as_notes: bool,
    /// Whether a chord is hashed as one.
    pub hash_chords_as_chords: bool,
    /// Whether a chord's normal order is hashed, where chords are hashed
    /// as chords.
    pub hash_normal_order_string: bool,
    /// Whether a chord's prime form is hashed, where chords are hashed as
    /// chords.
    pub hash_prime_form_string: bool,
}

impl Default for Hasher {
    fn default() -> Self {
        Self {
            include_reference: false,
            hash_pitch: true,
            hash_midi: true,
            hash_note_name_octave: false,
            hash_octave: false,
            hash_duration: true,
            round_duration_and_offset: true,
            hash_offset: true,
            granularity: 32,
            hash_interval_from_last_note: false,
            hash_is_accidental: false,
            hash_is_tied: false,
            hash_chords_as_notes: true,
            hash_chords_as_chords: false,
            hash_normal_order_string: false,
            hash_prime_form_string: false,
        }
    }
}

/// What is being hashed: a note or rest standing alone, a note of a chord,
/// or a chord as one.
enum Hashed<'a> {
    Alone(&'a StreamElement),
    InChord(&'a crate::note::Note, &'a crate::chord::Chord),
    Chord(&'a crate::chord::Chord),
}

impl Hasher {
    /// The names of the values each hash holds, in order: music21's
    /// `tupleList`.
    pub fn tuple_list(&self) -> Vec<&'static str> {
        let mut keys = Vec::new();
        if self.hash_pitch {
            keys.push("Pitch");
            if self.hash_is_accidental {
                keys.push("IsAccidental");
            }
        }
        if self.hash_octave {
            keys.push("Octave");
        }
        if !self.hash_chords_as_notes && self.hash_chords_as_chords {
            if self.hash_normal_order_string {
                keys.push("NormalOrderString");
            }
            if self.hash_prime_form_string {
                keys.push("PrimeFormString");
            }
        }
        if self.hash_duration {
            keys.push("Duration");
        }
        if self.hash_offset {
            keys.push("Offset");
        }
        if self.hash_interval_from_last_note {
            keys.push("IntervalFromLastNote");
        }
        keys
    }

    /// A length or offset rounded to the hasher's granularity, a half to
    /// the even, as Python rounds.
    fn rounded(&self, value: FloatType) -> FloatType {
        let granularity = FloatType::from(self.granularity);
        (value * granularity).round_ties_even() / granularity
    }

    /// Every note, rest and chord of a stream, hashed in the order music21's
    /// `recurse` walks them: music21's `hashStream`. A chord is hashed note
    /// by note, as one, or not at all as the hasher says. Offsets are each
    /// element's own in its measure or voice.
    ///
    /// music21 also rounds each element's own length and offset in the
    /// stream as it hashes them; the stream here is left as it is.
    ///
    /// # Errors
    ///
    /// An interval from the note before that cannot be measured, a chord
    /// symbol whose notes cannot be worked out, or octaves asked of chords
    /// hashed as chords, which music21 cannot hash.
    pub fn hash_stream(&self, stream: &Stream) -> Result<Vec<NoteHash>> {
        let keys = self.tuple_list();
        let walked = recurse_with_local_offsets(stream);
        let mut hashes = Vec::new();
        let previous_notes = if self.hash_interval_from_last_note {
            notes_before(stream)
        } else {
            vec![None; walked.len()]
        };
        for (index, (offset, element)) in walked.iter().enumerate() {
            match element {
                StreamElement::Note(_) => {
                    let values = self.values(
                        &keys,
                        Hashed::Alone(element),
                        *offset,
                        previous_notes[index].as_ref(),
                    )?;
                    hashes.push(self.hash(&keys, values, index, None));
                }
                StreamElement::Rest(_) => {
                    let values = self.values(
                        &keys,
                        Hashed::Alone(element),
                        *offset,
                        previous_notes[index].as_ref(),
                    )?;
                    hashes.push(self.hash(&keys, values, index, None));
                }
                StreamElement::ChordSymbol(symbol) => {
                    // music21 hashes a chord symbol as the chord it stands
                    // for, its notes each a quarter.
                    let mut chord = crate::chord::Chord::new(symbol.pitches()?.as_slice())?;
                    chord.set_duration(symbol.duration().clone());
                    self.hash_chord(&keys, &chord, *offset, index, &mut hashes)?;
                }
                StreamElement::Chord(chord) => {
                    self.hash_chord(&keys, chord, *offset, index, &mut hashes)?;
                }
                _ => {}
            }
        }
        Ok(hashes)
    }

    fn hash_chord(
        &self,
        keys: &[&'static str],
        chord: &crate::chord::Chord,
        offset: FloatType,
        index: usize,
        hashes: &mut Vec<NoteHash>,
    ) -> Result<()> {
        if self.hash_chords_as_notes {
            for (component, note) in chord.notes().iter().enumerate() {
                let values = self.values(keys, Hashed::InChord(note, chord), offset, None)?;
                hashes.push(self.hash(keys, values, index, Some(component)));
            }
        } else if self.hash_chords_as_chords {
            let values = self.values(keys, Hashed::Chord(chord), offset, None)?;
            hashes.push(self.hash(keys, values, index, None));
        }
        Ok(())
    }

    fn hash(
        &self,
        keys: &[&'static str],
        values: Vec<HashValue>,
        element: usize,
        component: Option<usize>,
    ) -> NoteHash {
        NoteHash {
            keys: keys.iter().map(|key| (*key).to_string()).collect(),
            values,
            reference: self
                .include_reference
                .then_some(HashReference { element, component }),
        }
    }

    fn values(
        &self,
        keys: &[&'static str],
        hashed: Hashed<'_>,
        offset: FloatType,
        previous_note: Option<&Pitch>,
    ) -> Result<Vec<HashValue>> {
        let as_chord =
            matches!(hashed, Hashed::InChord(..) | Hashed::Chord(_)) && self.hash_chords_as_chords;
        let pitch: Option<&Pitch> = match &hashed {
            Hashed::Alone(StreamElement::Note(note)) => Some(note.pitch()),
            Hashed::InChord(note, _) => Some(note.pitch()),
            _ => None,
        };
        let is_rest = matches!(hashed, Hashed::Alone(StreamElement::Rest(_)));
        let chord = match &hashed {
            Hashed::InChord(_, chord) | Hashed::Chord(chord) => Some(*chord),
            _ => None,
        };
        let length = match &hashed {
            Hashed::Alone(element) => element.quarter_length(),
            Hashed::InChord(_, chord) | Hashed::Chord(chord) => chord
                .duration()
                .map_or(1.0, crate::duration::Duration::quarter_length),
        };
        let mut values = Vec::new();
        for key in keys {
            values.push(match *key {
                "Pitch" => {
                    if as_chord {
                        if self.hash_midi {
                            HashValue::Integer(1)
                        } else {
                            HashValue::Text("z".to_string())
                        }
                    } else if is_rest {
                        if self.hash_midi {
                            HashValue::Integer(0)
                        } else {
                            HashValue::Text("r".to_string())
                        }
                    } else if let Some(pitch) = pitch {
                        let name = pitch_str(pitch);
                        if self.hash_midi {
                            HashValue::Integer(i64::from(pitch.midi()))
                        } else if self.hash_note_name_octave {
                            let mut name = name;
                            name.pop();
                            HashValue::Text(name)
                        } else {
                            HashValue::Text(name)
                        }
                    } else {
                        HashValue::None
                    }
                }
                "IsAccidental" => HashValue::None,
                "Octave" => {
                    if matches!(hashed, Hashed::Chord(_)) {
                        // music21 hands its octave hash nothing for a chord
                        // hashed as one, and asks nothing for its octave.
                        return Err(crate::error::Error::Analysis(
                            "'NoneType' object has no attribute 'octave'".to_string(),
                        ));
                    }
                    if is_rest {
                        HashValue::Integer(-1)
                    } else {
                        pitch
                            .and_then(Pitch::octave)
                            .map_or(HashValue::None, |octave| {
                                HashValue::Integer(i64::from(octave))
                            })
                    }
                }
                "NormalOrderString" => match chord {
                    Some(chord) => HashValue::Text(chord.normal_order_string()),
                    None => HashValue::Text("<>".to_string()),
                },
                "PrimeFormString" => match chord {
                    Some(chord) => HashValue::Text(chord.prime_form_string()),
                    None => HashValue::Text("<>".to_string()),
                },
                "Duration" => HashValue::Float(if self.round_duration_and_offset {
                    self.rounded(length)
                } else {
                    length
                }),
                "Offset" => HashValue::Float(if self.round_duration_and_offset {
                    self.rounded(offset)
                } else {
                    offset
                }),
                "IntervalFromLastNote" => match (&hashed, previous_note) {
                    (Hashed::Alone(StreamElement::Note(note)), Some(previous)) => {
                        let interval = Interval::between_pitches(previous, note.pitch())?;
                        HashValue::Integer(i64::from(interval.interval_class()))
                    }
                    _ => HashValue::None,
                },
                _ => HashValue::None,
            });
        }
        Ok(values)
    }
}

/// A pitch as music21's `str` writes it: its letter, its accidental in
/// music21's spelling -- a flat as `-`, a natural as nothing -- and its
/// octave.
fn pitch_str(pitch: &Pitch) -> String {
    let modifier = match pitch
        .written_accidental()
        .map(|accidental| accidental.name())
    {
        None | Some("natural") => String::new(),
        Some("sharp") => "#".to_string(),
        Some("double-sharp") => "##".to_string(),
        Some("triple-sharp") => "###".to_string(),
        Some("quadruple-sharp") => "####".to_string(),
        Some("flat") => "-".to_string(),
        Some("double-flat") => "--".to_string(),
        Some("triple-flat") => "---".to_string(),
        Some("quadruple-flat") => "----".to_string(),
        Some("half-sharp") => "~".to_string(),
        Some("one-and-a-half-sharp") => "#~".to_string(),
        Some("half-flat") => "`".to_string(),
        Some("one-and-a-half-flat") => "-`".to_string(),
        Some(_) => pitch.accidental().modifier().to_string(),
    };
    let octave = pitch
        .octave()
        .map(|octave| octave.to_string())
        .unwrap_or_default();
    format!("{}{modifier}{octave}", pitch.step().as_char())
}

/// Where an element of a stream's walk stands: the stream holding it, the
/// part it is in, and how a flattened part sorts it.
struct Placed {
    container: Option<usize>,
    part: Option<usize>,
    offset: FloatType,
    sort_order: i32,
    grace: bool,
    pitch: Option<Pitch>,
    leaf: bool,
}

/// The note before each note of a stream's walk, as music21's
/// `previous('Note')` finds it: the last note before it in its own measure
/// or voice, and failing that the last before it in its part, flattened and
/// sorted by offset, class and grace notes first. A part's first note has
/// none.
fn notes_before(stream: &Stream) -> Vec<Option<Pitch>> {
    fn walk(
        stream: &Stream,
        container: Option<usize>,
        part: Option<usize>,
        base: FloatType,
        out: &mut Vec<Placed>,
    ) {
        for event in stream.events() {
            let element = event.element();
            let offset = crate::makenotation::op_frac(base + event.offset());
            let index = out.len();
            out.push(Placed {
                container,
                part,
                offset,
                sort_order: element.class_sort_order(),
                grace: element
                    .duration()
                    .is_some_and(crate::duration::Duration::is_grace),
                pitch: match element {
                    StreamElement::Note(note) => Some(note.pitch().clone()),
                    _ => None,
                },
                leaf: !matches!(element, StreamElement::Stream(_)),
            });
            if let StreamElement::Stream(inner) = element {
                let inner_part = if matches!(inner.kind(), StreamKind::Part | StreamKind::PartStaff)
                {
                    Some(index)
                } else {
                    part
                };
                walk(inner, Some(index), inner_part, offset, out);
            }
        }
    }
    let mut placed = Vec::new();
    walk(stream, None, None, 0.0, &mut placed);
    // Each part's leaves in flattened order.
    let mut flattened: std::collections::BTreeMap<Option<usize>, Vec<usize>> =
        std::collections::BTreeMap::new();
    for (index, item) in placed.iter().enumerate() {
        if item.leaf {
            flattened.entry(item.part).or_default().push(index);
        }
    }
    let mut place_in_part = vec![0; placed.len()];
    for leaves in flattened.values_mut() {
        leaves.sort_by(|&a, &b| {
            placed[a]
                .offset
                .total_cmp(&placed[b].offset)
                .then(placed[a].sort_order.cmp(&placed[b].sort_order))
                .then(placed[b].grace.cmp(&placed[a].grace))
        });
        for (place, &index) in leaves.iter().enumerate() {
            place_in_part[index] = place;
        }
    }
    // The last note met in each measure or voice, walking forward.
    let mut last_in: std::collections::HashMap<Option<usize>, usize> =
        std::collections::HashMap::new();
    let mut out = vec![None; placed.len()];
    for index in 0..placed.len() {
        if placed[index].pitch.is_none() {
            continue;
        }
        let container = placed[index].container;
        out[index] = match last_in.get(&container) {
            Some(&before) => placed[before].pitch.clone(),
            None => flattened[&placed[index].part][..place_in_part[index]]
                .iter()
                .rev()
                .find_map(|&before| placed[before].pitch.clone()),
        };
        last_in.insert(container, index);
    }
    out
}

/// Every element of a stream in the order music21's `recurse` walks it,
/// each with its offset in the stream holding it.
fn recurse_with_local_offsets(stream: &Stream) -> Vec<(FloatType, StreamElement)> {
    fn walk(stream: &Stream, out: &mut Vec<(FloatType, StreamElement)>) {
        for event in stream.events() {
            out.push((event.offset(), event.element().clone()));
            if let StreamElement::Stream(inner) = event.element() {
                walk(inner, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(stream, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{chord::Chord, duration::Duration, note::Note, rest::Rest};

    #[test]
    fn notes_chords_and_rests_are_hashed_as_music21_hashes_them() -> Result<()> {
        // music21's testBasicHash.
        let mut stream = Stream::new();
        stream.push(Note::from_name("C4")?.with_duration(Duration::new(2.0)?));
        stream.push(Note::from_name("F#4")?);
        stream.push(Note::from_name("B-2")?);
        stream.push(Chord::new("C4 G4 E-5")?.with_duration(Duration::new(2.0)?));
        stream.push(Rest::new(Duration::new(1.5)?));
        let hashes: Vec<Vec<HashValue>> = Hasher::default()
            .hash_stream(&stream)?
            .into_iter()
            .map(|hash| hash.values)
            .collect();
        let expected = [
            (60, 2.0, 0.0),
            (66, 1.0, 2.0),
            (46, 1.0, 3.0),
            (60, 2.0, 4.0),
            (67, 2.0, 4.0),
            (75, 2.0, 4.0),
            (0, 1.5, 6.0),
        ];
        let expected: Vec<Vec<HashValue>> = expected
            .iter()
            .map(|(pitch, length, offset)| {
                vec![
                    HashValue::Integer(*pitch),
                    HashValue::Float(*length),
                    HashValue::Float(*offset),
                ]
            })
            .collect();
        assert_eq!(hashes, expected);
        Ok(())
    }
}
