pub(crate) mod chordbase;
pub(crate) mod tables;

use chordbase::{ChordBase, ChordBaseTrait, IntoNotRests};

use crate::{
    base::Music21ObjectTrait,
    defaults::IntegerType,
    duration::Duration,
    key::KeySignature,
    note::{generalnote::GeneralNoteTrait, notrest::NotRestTrait, Note},
    pitch::Pitch,
    prebase::ProtoM21ObjectTrait,
};

#[derive(Clone, Debug)]
pub struct Chord {
    chordbase: ChordBase,
    _notes: Vec<Note>,
}

impl Chord {
    pub fn new<T>(notes: Option<T>) -> Self
    where
        T: IntoNotes + Clone + IntoNotRests,
    {
        let mut chord = Self {
            chordbase: ChordBase::new(notes.clone(), &None),
            _notes: notes.as_ref().map_or_else(Vec::new, |notes| {
                notes
                    .clone()
                    .into_notes()
                    .into_iter()
                    .collect::<Vec<Note>>()
            }),
        };
        chord.simplify_enharmonics(true, None);
        chord
    }

    pub fn pitched_common_name(&self) -> String {
        todo!()
    }

    pub(crate) fn pitches(&self) -> Vec<Pitch> {
        self._notes.iter().map(|note| note._pitch.clone()).collect()
    }

    fn simplify_enharmonics(
        &mut self,
        in_place: bool,
        key_context: Option<KeySignature>,
    ) -> Option<Self> {
        if in_place {
            self.simplify_enharmonics_in_place(key_context);
            None
        } else {
            let mut new_chord = self.clone();
            new_chord.simplify_enharmonics_in_place(key_context);
            Some(new_chord)
        }
    }

    fn simplify_enharmonics_in_place(&mut self, key_context: Option<KeySignature>) {
        let pitches = crate::pitch::simplify_multiple_enharmonics(self.pitches());
        match pitches {
            Some(pitches) => {
                for (i, pitch) in pitches.iter().enumerate() {
                    if let Some(note) = self._notes.get_mut(i) {
                        note._pitch = pitch.clone();
                    }
                }
            }
            None => {
                eprintln!("simplify_multiple_enharmonics failed")
            }
        }
    }
}

pub(crate) trait ChordTrait {}

impl ChordTrait for Chord {}

impl ChordBaseTrait for Chord {}

impl NotRestTrait for Chord {}

impl GeneralNoteTrait for Chord {
    fn duration(&self) -> &Option<Duration> {
        self.chordbase.duration()
    }
}

impl Music21ObjectTrait for Chord {}

impl ProtoM21ObjectTrait for Chord {}

pub(crate) trait IntoNotes {
    type T: IntoIterator<Item = Note>;

    fn into_notes(self) -> Self::T;
}

impl IntoNotes for &[Pitch] {
    type T = Vec<Note>;

    fn into_notes(self) -> Self::T {
        todo!()
    }
}

impl IntoNotes for &[Note] {
    type T = Vec<Note>;

    fn into_notes(self) -> Self::T {
        todo!()
    }
}

impl IntoNotes for &[Chord] {
    type T = Vec<Note>;

    fn into_notes(self) -> Self::T {
        self.iter().flat_map(|chord| chord._notes.clone()).collect()
    }
}

impl IntoNotes for &[String] {
    type T = Vec<Note>;

    fn into_notes(self) -> Self::T {
        todo!()
    }
}

impl IntoNotes for String {
    type T = Vec<Note>;

    fn into_notes(self) -> Self::T {
        todo!()
    }
}

impl IntoNotes for &str {
    type T = Vec<Note>;

    fn into_notes(self) -> Self::T {
        todo!()
    }
}

impl IntoNotes for &[IntegerType] {
    type T = Vec<Note>;

    fn into_notes(self) -> Self::T {
        todo!()
    }
}

// pub(crate) trait IntoNote {
//     fn into_note(&self) -> Note;
// }

// impl<T> IntoNotes for T
// where
//     T: IntoNote,
// {
//     type T = Vec<Note>;

//     fn into_notes(self) -> Self::T {
//         vec![self.into_note()]
//     }
// }

#[test]
#[ignore]
fn c_e_g_pitchedcommonname() {
    let chord = Chord::new(Some("C E G"));
    assert_eq!(chord.pitched_common_name(), "C-major triad");
}
