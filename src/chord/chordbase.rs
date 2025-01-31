use super::{IntegerType, Pitch};
use crate::{
    base::Music21ObjectTrait,
    duration::Duration,
    note::{
        generalnote::GeneralNoteTrait,
        notrest::{NotRest, NotRestTrait},
    },
    prebase::ProtoM21ObjectTrait,
};
use std::collections::HashMap;

#[derive(Clone)]
pub(crate) struct ChordBase {
    notrest: NotRest,
    _notes: Vec<NotRest>,
    duration: Option<Duration>,
    overrides: HashMap<String, String>,
}

pub enum NoteInput {
    Str(String),
    Pitch(Pitch),
    ChordBase(ChordBase),
    NotRest(NotRest),
    Integer(IntegerType),
}

impl ChordBase {
    pub fn new(mut notes_input: Option<Vec<NoteInput>>, duration: Option<Duration>) -> Self {
        if let Some(ref mut vec) = notes_input {
            if vec.len() == 1 {
                if let NoteInput::Str(s) = &vec[0] {
                    if s.contains(' ') {
                        let tokens = s
                            .split_whitespace()
                            .map(|t| NoteInput::Str(t.to_string()))
                            .collect();
                        *vec = tokens;
                    }
                }
            }
        }
        let mut chord = ChordBase {
            notrest: NotRest::new(),
            _notes: Vec::new(),
            duration,
            overrides: HashMap::new(),
        };
        // Call the add_core_or_init routine to process the notes.
        if let Some(inputs) = notes_input {
            // If no duration was passed in via the parameters, pass None so that add_core_or_init
            // will try to use the chord’s duration.
            let use_duration = chord.duration.clone();
            chord.add_core_or_init(inputs, use_duration);
        }
        chord
    }

    pub fn add_core_or_init(
        &mut self,
        notes: Vec<NoteInput>,
        mut use_duration: Option<f64>,
    ) -> Option<f64> {
        // quick_duration is true if we still have a duration to use.
        let mut quick_duration = use_duration.is_some();

        for n in notes {
            match n {
                NoteInput::Pitch(p) => {
                    // Create a new “note” string. (In a full implementation,
                    // you might create a Note struct here.)
                    let note_str = if let Some(dur) = use_duration {
                        format!("Note({}, duration={})", p, dur)
                    } else {
                        format!("Note({})", p)
                    };
                    self._notes.push(note_str);
                }
                NoteInput::ChordBase(mut chord) => {
                    // For a nested chord, add all of its notes.
                    // (A deep-copy would be performed in a full implementation.)
                    for note in chord._notes.iter() {
                        self._notes.push(note.clone());
                    }
                    // If we still have a duration to use, update our chord’s duration
                    // from the nested chord and then “consume” the duration.
                    if quick_duration {
                        self.duration = chord.duration;
                        use_duration = None;
                        quick_duration = false;
                    }
                }
                NoteInput::NotRest(nr) => {
                    // Treat NotRest similar to a note.
                    self._notes.push(format!("NotRest({})", nr));
                    if quick_duration {
                        // In a full implementation the NotRest might have its own duration.
                        // Here we simply consume the duration.
                        use_duration = None;
                        quick_duration = false;
                    }
                }
                NoteInput::Str(s) => {
                    let note_str = if let Some(dur) = use_duration {
                        format!("Note({}, duration={})", s, dur)
                    } else {
                        format!("Note({})", s)
                    };
                    self._notes.push(note_str);
                }
                NoteInput::Int(i) => {
                    let note_str = if let Some(dur) = use_duration {
                        format!("Note({} duration={})", i, dur)
                    } else {
                        format!("Note({})", i)
                    };
                    self._notes.push(note_str);
                }
            }
        }
        use_duration
    }

    /// Return the number of “notes” stored in the chord.
    pub fn len(&self) -> usize {
        self._notes.len()
    }

    /// Return an iterator over the notes (as strings).
    pub fn iter(&self) -> std::slice::Iter<String> {
        self._notes.iter()
    }

    pub(crate) fn new<T>(notes: Option<T>) -> Self
    where
        T: IntoNotRests,
    {
        Self {
            notrest: NotRest::new(),
            _notes: notes.as_ref().map_or_else(Vec::new, |notes| {
                notes.into_not_rests().into_iter().collect()
            }),
            duration: todo!(),
        }
    }
}

pub(crate) trait ChordBaseTrait {}

impl ChordBaseTrait for ChordBase {}

impl NotRestTrait for ChordBase {}

impl GeneralNoteTrait for ChordBase {}

impl Music21ObjectTrait for ChordBase {}

impl ProtoM21ObjectTrait for ChordBase {}

pub trait IntoNotRests {
    type T: IntoIterator<Item = NotRest>;

    fn into_not_rests(&self) -> Self::T;
}

impl IntoNotRests for String {
    type T = Vec<NotRest>;
    fn into_not_rests(&self) -> Self::T {
        if self.contains(char::is_whitespace) {
            // Delegate splitting to the &[&str] implementation.
            self.split_whitespace()
                .collect::<Vec<&str>>()
                .as_slice()
                .into_not_rests()
        } else {
            // Treat the entire string as one note.
            vec![NotRest::new(self.as_str())]
        }
    }
}

impl IntoNotRests for &[String] {
    type T = Vec<NotRest>;
    fn into_not_rests(&self) -> Self::T {
        self.iter()
            .map(|s| {
                // We assume that if a string is provided within a sequence, it represents a single note.
                NotRest::new(s.as_str())
            })
            .collect()
    }
}

impl IntoNotRests for &str {
    type T = Vec<NotRest>;
    fn into_not_rests(&self) -> Self::T {
        if self.contains(char::is_whitespace) {
            // Split and then delegate to the &[&str] implementation.
            self.split_whitespace()
                .collect::<Vec<&str>>()
                .as_slice()
                .into_not_rests()
        } else {
            vec![NotRest::new(*self)]
        }
    }
}

impl IntoNotRests for &[&str] {
    type T = Vec<NotRest>;
    fn into_not_rests(&self) -> Self::T {
        self.iter().map(|s| NotRest::new(*s)).collect()
    }
}

impl IntoNotRests for &[Pitch] {
    type T = Vec<NotRest>;
    fn into_not_rests(&self) -> Self::T {
        self.iter().map(|p| NotRest::new(*p)).collect()
    }
}

impl IntoNotRests for &[ChordBase] {
    type T = Vec<NotRest>;

    fn into_not_rests(&self) -> Self::T {
        self.iter()
            .flat_map(|chord_base| chord_base._notes.clone())
            .collect()
    }
}

impl IntoNotRests for &[NotRest] {
    type T = Vec<NotRest>;

    fn into_not_rests(&self) -> Self::T {
        self.to_vec()
    }
}

impl IntoNotRests for &[IntegerType] {
    type T = Vec<NotRest>;
    fn into_not_rests(&self) -> Self::T {
        self.iter().map(|i| NotRest::new(*i)).collect()
    }
}

// pub trait IntoNotRest {
//     fn into_not_rest(&self) -> NotRest;
// }

// impl<T> IntoNotRests for T
// where
//     T: IntoNotRest,
// {
//     type T = Vec<NotRest>;

//     fn into_not_rests(&self) -> Self::T {
//         vec![self.into_not_rest()]
//     }
// }
