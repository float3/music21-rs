pub(crate) mod chordbase;
pub(crate) mod tables;

use chordbase::{ChordBase, ChordBaseTrait, IntoNotRests};

use crate::{
    base::Music21ObjectTrait,
    defaults::IntegerType,
    duration::Duration,
    exceptions::{Exception, ExceptionResult},
    key::keysignature::KeySignature,
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
    pub fn new<T>(notes: Option<T>) -> ExceptionResult<Self>
    where
        T: IntoNotes + Clone + IntoNotRests,
    {
        let mut chord = Self {
            chordbase: ChordBase::new(notes.clone(), &None)?,
            _notes: notes.as_ref().map_or_else(Vec::new, |notes| {
                notes
                    .clone()
                    .into_notes()
                    .into_iter()
                    .collect::<Vec<Note>>()
            }),
        };
        chord.simplify_enharmonics(true, None)?;
        Ok(chord)
    }

    pub fn pitched_common_name(&self) -> String {
        todo!()
    }

    pub fn common_name(&self) -> String {
        todo!()
    }

    pub(crate) fn pitches(&self) -> Vec<Pitch> {
        self._notes.iter().map(|note| note._pitch.clone()).collect()
    }

    fn simplify_enharmonics(
        &mut self,
        in_place: bool,
        key_context: Option<KeySignature>,
    ) -> ExceptionResult<Option<Self>> {
        if in_place {
            self.simplify_enharmonics_in_place(key_context)?;
            Ok(None)
        } else {
            let mut new_chord = self.clone();
            new_chord.simplify_enharmonics_in_place(key_context)?;
            Ok(Some(new_chord))
        }
    }

    fn simplify_enharmonics_in_place(
        &mut self,
        key_context: Option<KeySignature>,
    ) -> ExceptionResult<()> {
        match crate::pitch::simplify_multiple_enharmonics(self.pitches(), None, key_context) {
            Ok(pitches) => {
                for (i, pitch) in pitches.iter().enumerate() {
                    if let Some(note) = self._notes.get_mut(i) {
                        note._pitch = pitch.clone();
                    }
                }
                Ok(())
            }
            Err(err) => Err(Exception::Chord(format!(
                "simplifying multiple enharmonics failed because of {}",
                err
            ))),
        }
    }
}

pub(crate) trait ChordTrait {}

impl ChordTrait for Chord {}

impl ChordBaseTrait for Chord {}

impl NotRestTrait for Chord {}

impl GeneralNoteTrait for Chord {
    fn duration(&self) -> &Option<Duration> {
        if self.chordbase.duration().is_none() && self._notes.is_empty() {
            if let Some(duration) = self._notes[0].duration() {
                self.set_duration(duration)
            }
        }

        self.chordbase.duration()
    }

    fn set_duration(&self, duration: &Duration) {
        self.chordbase.set_duration(duration);
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

#[cfg(test)]
mod tests {
    use pyo3::{prelude::PyModule, types::PyAnyMethods, Bound, PyAny, PyErr, PyResult, Python};
    use utils::{init_py, prepare};

    use crate::chord::Chord;

    #[test]
    #[ignore]
    fn c_e_g_pitchedcommonname() {
        let chord = Chord::new(Some("C E G"));

        assert!(chord.is_ok());

        assert_eq!(chord.unwrap().pitched_common_name(), "C-major triad");
    }

    #[test]
    #[ignore]
    fn compare_chords_python() {
        let x = "C E G";
        let y = "C C# D D# E F F# G G# A A# B";

        prepare().unwrap();

        Python::with_gil(|py| -> PyResult<()> {
            init_py(py)?;

            let chord: Bound<'_, PyModule> = py.import("music21.chord")?;

            let chord_class = chord.getattr("Chord")?;

            compare_chord(x, &chord_class)?;
            compare_chord(y, &chord_class)?;

            Ok(())
        })
        .unwrap();
    }

    fn compare_chord(x: &str, chord_class: &Bound<'_, PyAny>) -> Result<(), PyErr> {
        let chord_instance = chord_class.call1((x,))?;

        let chord = Chord::new(Some(x)).unwrap();

        let pitched_common_name = chord_instance.getattr("pitchedCommonName")?;
        assert_eq!(
            chord.pitched_common_name(),
            format!("{:?}", pitched_common_name)
        );

        let common_name = chord_instance.getattr("commonName")?;
        assert_eq!(chord.common_name(), format!("{:?}", common_name));
        Ok(())
    }
}
