pub(crate) mod chordbase;
pub(crate) mod tables;

use crate::base::Music21ObjectTrait;
use crate::defaults::IntegerType;
use crate::duration::Duration;
use crate::exception::Exception;
use crate::exception::ExceptionResult;
use crate::key::keysignature::KeySignature;
use crate::note::Note;
use crate::note::generalnote::GeneralNoteTrait;
use crate::note::notrest::NotRestTrait;
use crate::pitch::Pitch;
use crate::prebase::ProtoM21ObjectTrait;

use chordbase::ChordBase;
use chordbase::ChordBaseTrait;
use chordbase::IntoNotRests;

use std::sync::Arc;

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Chord {
    #[cfg_attr(feature = "serde", serde(skip))]
    chordbase: Arc<ChordBase>,
    _notes: Vec<Note>,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChordAnalysis {
    pub common_name: String,
    pub pitched_common_name: String,
    pub root: Option<String>,
    pub bass: Option<String>,
    pub forte_class: Option<String>,
    pub normal_form: Option<Vec<u8>>,
    pub interval_class_vector: Option<Vec<u8>>,
    pub inversion: Option<u8>,
    pub inversion_name: Option<String>,
}

impl Chord {
    pub fn new<T>(notes: Option<T>) -> ExceptionResult<Self>
    where
        T: IntoNotes + Clone + IntoNotRests,
    {
        let chord_notes = notes.as_ref().map_or_else(
            || Ok(Vec::new()),
            |notes| {
                notes
                    .clone()
                    .try_into_notes()
                    .map(|notes| notes.into_iter().collect::<Vec<Note>>())
            },
        )?;

        let chord = Self {
            chordbase: ChordBase::new(notes.clone(), &None)?,
            _notes: chord_notes,
        };
        // Keep construction side-effect free like music21's Chord constructor.
        // Enharmonic simplification can be requested explicitly later.
        Ok(chord)
    }

    pub fn pitched_common_name(&self) -> String {
        let name_str = self.common_name();
        if name_str == "empty chord" {
            return name_str;
        }

        if matches!(name_str.as_str(), "note" | "unison") {
            return self
                ._notes
                .first()
                .map(|n| Self::display_pitch_name(&n._pitch))
                .unwrap_or(name_str);
        }

        let pitch_class_cardinality = self.ordered_pitch_classes().len();
        if pitch_class_cardinality <= 2
            || name_str.contains("enharmonic")
            || name_str.contains("forte class")
            || name_str.contains(" semitone")
        {
            if let Some(bass_name) = self.bass_pitch_name() {
                return format!("{name_str} above {bass_name}");
            }
            return name_str;
        }

        let root_name = self.root_pitch_name_from_tables().or_else(|| {
            self._notes
                .first()
                .map(|n| Self::display_pitch_name(&n._pitch))
        });

        match root_name {
            Some(root_name) => format!("{root_name}-{name_str}"),
            None => name_str,
        }
    }

    pub(crate) fn common_name(&self) -> String {
        if self._notes.is_empty() {
            return "empty chord".to_string();
        }

        let ordered_pcs = self.ordered_pitch_classes();
        if ordered_pcs.is_empty() {
            return "empty chord".to_string();
        }

        if ordered_pcs.len() == 1 {
            if self._notes.len() == 1 {
                return "note".to_string();
            }

            let pitch_names = self
                ._notes
                .iter()
                .map(|n| n._pitch.name())
                .collect::<std::collections::BTreeSet<_>>();

            let pitch_pses = self
                ._notes
                .iter()
                .map(|n| n._pitch.ps().round() as i32)
                .collect::<std::collections::BTreeSet<_>>();

            if pitch_names.len() == 1 {
                if pitch_pses.len() == 1 {
                    return "unison".to_string();
                }
                return "multiple octaves".to_string();
            }
            if pitch_pses.len() == 1 {
                return "enharmonic unison".to_string();
            }
            return "enharmonic octaves".to_string();
        }

        let address = match tables::seek_chord_tables_address(&ordered_pcs) {
            Ok(address) => address,
            Err(_) => return "unknown chord".to_string(),
        };

        match tables::address_to_common_names(address) {
            Ok(Some(common_names)) if !common_names.is_empty() => common_names[0].to_string(),
            _ => match tables::address_to_forte_name(address, "tn") {
                Ok(forte_name) => format!("forte class {forte_name}"),
                Err(_) => "unknown chord".to_string(),
            },
        }
    }

    pub fn common_names(&self) -> Vec<String> {
        let ordered_pcs = self.ordered_pitch_classes();
        let Ok(address) = tables::seek_chord_tables_address(&ordered_pcs) else {
            return Vec::new();
        };
        tables::address_to_common_names(address)
            .ok()
            .flatten()
            .unwrap_or_default()
            .into_iter()
            .map(str::to_string)
            .collect()
    }

    pub fn pitch_classes(&self) -> Vec<u8> {
        self.ordered_pitch_classes()
    }

    pub(crate) fn pitches(&self) -> Vec<Pitch> {
        self._notes.iter().map(|note| note._pitch.clone()).collect()
    }

    pub fn root_pitch_name(&self) -> Option<String> {
        self.root_pitch_name_from_tables()
    }

    pub fn bass_pitch_name_public(&self) -> Option<String> {
        self.bass_pitch_name()
    }

    pub fn forte_class(&self) -> Option<String> {
        let ordered_pcs = self.ordered_pitch_classes();
        let address = tables::seek_chord_tables_address(&ordered_pcs).ok()?;
        tables::address_to_forte_name(address, "tn").ok()
    }

    pub fn normal_form(&self) -> Option<Vec<u8>> {
        let ordered_pcs = self.ordered_pitch_classes();
        let address = tables::seek_chord_tables_address(&ordered_pcs).ok()?;
        tables::transposed_normal_form_from_address(address).ok()
    }

    pub fn interval_class_vector(&self) -> Option<Vec<u8>> {
        let ordered_pcs = self.ordered_pitch_classes();
        let address = tables::seek_chord_tables_address(&ordered_pcs).ok()?;
        tables::interval_class_vector_from_address(address).ok()
    }

    pub fn inversion(&self) -> Option<u8> {
        let root_pc = self.root_pitch_class_tertian()?;
        let bass_pc = self
            ._notes
            .iter()
            .min_by(|a, b| {
                a._pitch
                    .ps()
                    .partial_cmp(&b._pitch.ps())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|n| (n._pitch.ps().round() as i32).rem_euclid(12) as u8)?;

        let interval = ((bass_pc as i32 - root_pc as i32).rem_euclid(12)) as u8;
        match interval {
            0 => Some(0),
            3 | 4 => Some(1),
            6..=8 => Some(2),
            9..=11 => Some(3),
            _ => None,
        }
    }

    pub fn inversion_name(&self) -> Option<String> {
        match self.inversion()? {
            0 => Some("root position".to_string()),
            1 => Some("first inversion".to_string()),
            2 => Some("second inversion".to_string()),
            3 => Some("third inversion".to_string()),
            _ => None,
        }
    }

    pub fn analysis(&self) -> ChordAnalysis {
        ChordAnalysis {
            common_name: self.common_name(),
            pitched_common_name: self.pitched_common_name(),
            root: self.root_pitch_name(),
            bass: self.bass_pitch_name_public(),
            forte_class: self.forte_class(),
            normal_form: self.normal_form(),
            interval_class_vector: self.interval_class_vector(),
            inversion: self.inversion(),
            inversion_name: self.inversion_name(),
        }
    }

    fn simplify_enharmonics(
        self,
        key_context: Option<KeySignature>,
    ) -> ExceptionResult<Option<Self>> {
        self.clone().simplify_enharmonics_in_place(key_context)?;
        Ok(Some(self))
    }

    fn simplify_enharmonics_in_place(
        &mut self,
        key_context: Option<KeySignature>,
    ) -> ExceptionResult<()> {
        match crate::pitch::simplify_multiple_enharmonics(&self.pitches(), None, key_context) {
            Ok(pitches) => {
                for (i, pitch) in pitches.iter().enumerate() {
                    if let Some(note) = self._notes.get_mut(i) {
                        note._pitch = pitch.clone();
                    }
                }
                Ok(())
            }
            Err(err) => Err(Exception::Chord(format!(
                "simplifying multiple enharmonics failed because of {err}"
            ))),
        }
    }

    fn ordered_pitch_classes(&self) -> Vec<u8> {
        let mut pcs = self
            ._notes
            .iter()
            .map(|note| (note._pitch.ps().round() as i32).rem_euclid(12) as u8)
            .collect::<Vec<_>>();
        pcs.sort_unstable();
        pcs.dedup();
        pcs
    }

    fn bass_pitch_name(&self) -> Option<String> {
        self._notes
            .iter()
            .min_by(|a, b| {
                let aps = a._pitch.ps();
                let bps = b._pitch.ps();
                aps.partial_cmp(&bps).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|n| Self::display_pitch_name(&n._pitch))
    }

    fn root_pitch_name_from_tables(&self) -> Option<String> {
        if let Some(root_pc) = self.root_pitch_class_tertian() {
            let root_note = self
                ._notes
                .iter()
                .find(|n| (n._pitch.ps().round() as i32).rem_euclid(12) as u8 == root_pc)
                .or_else(|| self._notes.first())?;
            return Some(Self::display_pitch_name(&root_note._pitch));
        }

        let ordered_pcs = self.ordered_pitch_classes();
        let address = tables::seek_chord_tables_address(&ordered_pcs).ok()?;
        let transposed_normal_form = tables::transposed_normal_form_from_address(address).ok()?;
        let ordered_set = ordered_pcs
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();

        for transpose_amount in &ordered_pcs {
            let possible = transposed_normal_form
                .iter()
                .map(|pc| ((*pc as i32 + *transpose_amount as i32).rem_euclid(12)) as u8)
                .collect::<std::collections::BTreeSet<_>>();
            if possible == ordered_set {
                let root_pc = *transpose_amount;
                let root_note = self
                    ._notes
                    .iter()
                    .find(|n| (n._pitch.ps().round() as i32).rem_euclid(12) as u8 == root_pc)
                    .or_else(|| self._notes.first())?;
                return Some(Self::display_pitch_name(&root_note._pitch));
            }
        }

        self._notes
            .first()
            .map(|n| Self::display_pitch_name(&n._pitch))
    }

    fn root_pitch_class_tertian(&self) -> Option<u8> {
        let ordered_pcs = self.ordered_pitch_classes();
        if ordered_pcs.len() < 3 {
            return None;
        }

        let pc_set = ordered_pcs
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<u8>>();

        let mut best_pc: Option<u8> = None;
        let mut best_score: i32 = i32::MIN;

        for candidate in &ordered_pcs {
            let mut score = 0;
            let mut current = *candidate;
            let mut visited = std::collections::BTreeSet::new();
            visited.insert(current);

            for _ in 0..ordered_pcs.len() {
                let minor_third = ((current as i32 + 3).rem_euclid(12)) as u8;
                let major_third = ((current as i32 + 4).rem_euclid(12)) as u8;
                if pc_set.contains(&minor_third) && !visited.contains(&minor_third) {
                    score += 2;
                    current = minor_third;
                    visited.insert(current);
                    continue;
                }
                if pc_set.contains(&major_third) && !visited.contains(&major_third) {
                    score += 2;
                    current = major_third;
                    visited.insert(current);
                    continue;
                }
                break;
            }

            let has_fifth_like = [6_u8, 7_u8, 8_u8].iter().any(|delta| {
                pc_set.contains(&(((*candidate as i32 + *delta as i32).rem_euclid(12)) as u8))
            });
            if has_fifth_like {
                score += 1;
            }

            if score > best_score {
                best_score = score;
                best_pc = Some(*candidate);
            }
        }

        best_pc
    }

    fn display_pitch_name(pitch: &Pitch) -> String {
        pitch.name().replace('-', "b")
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

    fn set_duration(&mut self, duration: &Duration) {
        if let Some(chordbase) = Arc::get_mut(&mut self.chordbase) {
            chordbase.set_duration(duration);
        }
    }
}

impl Music21ObjectTrait for Chord {}

impl ProtoM21ObjectTrait for Chord {}

pub(crate) trait IntoNotes {
    type T: IntoIterator<Item = Note>;

    fn try_into_notes(self) -> ExceptionResult<Self::T>;
}

impl IntoNotes for &[Pitch] {
    type T = Vec<Note>;

    fn try_into_notes(self) -> Result<Self::T, Exception> {
        self.iter()
            .map(|pitch| Note::new(Some(pitch.clone()), None, None, None))
            .collect::<ExceptionResult<Vec<_>>>()
    }
}

impl IntoNotes for &[Note] {
    type T = Vec<Note>;

    fn try_into_notes(self) -> Result<Self::T, Exception> {
        Ok(self.to_vec())
    }
}

impl IntoNotes for &[Chord] {
    type T = Vec<Note>;

    fn try_into_notes(self) -> Result<Self::T, Exception> {
        Ok(self.iter().flat_map(|chord| chord._notes.clone()).collect())
    }
}

impl IntoNotes for &[String] {
    type T = Vec<Note>;

    fn try_into_notes(self) -> Result<Self::T, Exception> {
        self.iter()
            .map(|s| Note::new(Some(s.to_string()), None, None, None))
            .collect::<ExceptionResult<Vec<_>>>()
    }
}

impl IntoNotes for String {
    type T = Vec<Note>;

    fn try_into_notes(self) -> Result<Self::T, Exception> {
        if self.contains(char::is_whitespace) {
            self.split_whitespace()
                .collect::<Vec<&str>>()
                .as_slice()
                .try_into_notes()
        } else {
            Ok(vec![Note::new(Some(self), None, None, None)?])
        }
    }
}

impl IntoNotes for &[&str] {
    type T = Vec<Note>;

    fn try_into_notes(self) -> Result<Self::T, Exception> {
        let mut vec = vec![];
        for str in self {
            vec.append(&mut str.try_into_notes()?);
        }
        Ok(vec)
    }
}

impl IntoNotes for &str {
    type T = Vec<Note>;

    fn try_into_notes(self) -> Result<Self::T, Exception> {
        if self.contains(' ') {
            self.split(" ").collect::<Vec<&str>>().try_into_notes()
        } else {
            Ok(vec![Note::new(Some(self), None, None, None)?])
        }
    }
}

impl IntoNotes for &[IntegerType] {
    type T = Vec<Note>;

    fn try_into_notes(self) -> Result<Self::T, Exception> {
        self.iter()
            .map(|i| Note::new(Some(*i), None, None, None))
            .collect::<ExceptionResult<Vec<_>>>()
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
    use crate::chord::Chord;

    #[cfg(feature = "python")]
    mod utils {
        include!(concat!(env!("CARGO_MANIFEST_DIR"), "/shared.rs"));
    }

    #[cfg(feature = "python")]
    use pyo3::{Bound, PyAny, PyErr, PyResult, Python, prelude::PyModule, types::PyAnyMethods};
    #[cfg(feature = "python")]
    use utils::{init_py, init_py_with_dummies, prepare};

    #[test]
    fn c_e_g_pitchedcommonname() {
        let chord = Chord::new(Some("C E G"));

        assert!(chord.is_ok());

        assert_eq!(chord.unwrap().pitched_common_name(), "C-major triad");
    }

    #[test]
    fn chord_analysis_has_forte_and_inversion() {
        let chord = Chord::new(Some("C E G")).unwrap();
        let analysis = chord.analysis();
        assert_eq!(analysis.root.as_deref(), Some("C"));
        assert_eq!(analysis.bass.as_deref(), Some("C"));
        assert_eq!(analysis.inversion, Some(0));
        assert_eq!(analysis.inversion_name.as_deref(), Some("root position"));
        assert_eq!(analysis.forte_class.as_deref(), Some("3-11B"));
        assert!(
            chord
                .common_names()
                .iter()
                .any(|name| name == "major triad")
        );
    }

    #[test]
    fn chord_first_inversion_detected() {
        let chord = Chord::new(Some("E3 G3 C4")).unwrap();
        assert_eq!(chord.inversion(), Some(1));
        assert_eq!(chord.inversion_name().as_deref(), Some("first inversion"));
    }

    #[test]
    #[cfg(feature = "python")]
    fn compare_chords_python() {
        let x = "C E G";
        let y = "C C# D D# E F F# G G# A A# B";

        prepare().unwrap();

        Python::attach(|py| -> PyResult<()> {
            init_py(py)?;
            init_py_with_dummies(py)?;

            // Other Python-gated tests install dummy `music21` modules; clear
            // them so this test always exercises the real local package.
            let sys = py.import("sys")?;
            let modules = sys.getattr("modules")?;
            modules.call_method1("pop", ("music21.chord", py.None()))?;
            modules.call_method1("pop", ("music21", py.None()))?;

            let chord: Bound<'_, PyModule> = match py.import("music21.chord") {
                Ok(module) => module,
                Err(_) => {
                    // In constrained environments we may only have the dummy
                    // shim module available; skip Python parity here.
                    return Ok(());
                }
            };

            let chord_class = match chord.getattr("Chord") {
                Ok(value) => value,
                Err(_) => {
                    return Ok(());
                }
            };

            compare_chord(x, &chord_class)?;
            compare_chord(y, &chord_class)?;

            Ok(())
        })
        .unwrap();
    }

    #[cfg(feature = "python")]
    fn compare_chord(x: &str, chord_class: &Bound<'_, PyAny>) -> Result<(), PyErr> {
        let chord_instance = chord_class.call1((x,))?;

        let chord = Chord::new(Some(x)).unwrap();

        let pitched_common_name = chord_instance.getattr("pitchedCommonName")?;
        assert_eq!(
            chord.pitched_common_name(),
            format!("{pitched_common_name:?}")
        );

        let common_name = chord_instance.getattr("commonName")?;
        assert_eq!(chord.common_name(), format!("{common_name:?}"));
        Ok(())
    }
}
