//! The crate's General MIDI percussion map against music21's:
//! `midi.percussion.PercussionMapper`.
//!
//! Every key from 0 to 127 is asked of both sides, and the answers must
//! agree down to how the instrument is played: the class, the modifier and
//! the drum that modifier plays, or a refusal where music21 raises its
//! `MIDIPercussionException`. Then every instrument class music21 has is
//! asked for its key, which is the map read the other way; a class the
//! crate does not carry must be one music21 gives no key for.
//!
//! music21 is music21 here, with nothing of the crate installed over it.

use music21_rs::instrument::{Instrument, for_percussion_key};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

/// music21's side: the map both ways, and the classes to ask about.
const MUSIC21: &str = r#"
from music21 import instrument
from music21.midi import percussion

mapper = percussion.PercussionMapper()

def for_key(key):
    try:
        played = mapper.midiPitchToInstrument(key)
    except percussion.MIDIPercussionException:
        return None
    # `Whistle` and the other classes music21 derives straight from
    # `Percussion` have no modifier at all.
    return (
        type(played).__name__,
        getattr(played, 'modifier', None),
        getattr(played, 'percMapPitch', None),
    )

def key_of(class_name):
    try:
        played = getattr(instrument, class_name)()
        return mapper.midiInstrumentToPitch(played).midi
    except percussion.MIDIPercussionException:
        return None

def classes():
    out = []
    for name in dir(instrument):
        value = getattr(instrument, name)
        if isinstance(value, type) and issubclass(value, instrument.Instrument):
            out.append(name)
    return out
"#;

#[test]
fn the_crate_maps_percussion_keys_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, keys, classes) = Python::attach(|py| -> PyResult<(Vec<String>, usize, usize)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let _ = py.import("music21")?;
        let music21 = PyModule::from_code(
            py,
            &std::ffi::CString::new(MUSIC21).expect("no nul in the helper"),
            c"percussion_parity_music21.py",
            c"percussion_parity_music21",
        )?;

        let mut failures = Vec::new();
        let mut keys = 0;
        for key in 0..=127u8 {
            let theirs: Option<(String, Option<String>, Option<u8>)> =
                music21.getattr("for_key")?.call1((key,))?.extract()?;
            let ours = for_percussion_key(key).map(|played| {
                (
                    played.kind().to_string(),
                    played.modifier().map(str::to_string),
                    played.percussion_pitch(),
                )
            });
            match (theirs, ours) {
                (Some(theirs), Ok(ours)) => {
                    if theirs != ours {
                        failures.push(format!("key {key}: {ours:?} where music21 has {theirs:?}"));
                    } else {
                        keys += 1;
                    }
                }
                (None, Err(_)) => keys += 1,
                (Some(theirs), Err(error)) => failures.push(format!(
                    "key {key}: the crate refused it ({error}) where music21 has {theirs:?}"
                )),
                (None, Ok(ours)) => failures.push(format!(
                    "key {key}: the crate answers {ours:?} where music21 raises"
                )),
            }
        }

        // The other direction: every class music21 has, asked for its key.
        let named: Vec<String> = music21.getattr("classes")?.call0()?.extract()?;
        let mut classes = 0;
        for class in &named {
            let theirs: Option<u8> = music21.getattr("key_of")?.call1((class,))?.extract()?;
            match Instrument::of_kind(class) {
                Ok(played) => {
                    if played.percussion_pitch() != theirs {
                        failures.push(format!(
                            "{class}: the crate plays key {:?} where music21 plays {theirs:?}",
                            played.percussion_pitch()
                        ));
                    } else {
                        classes += 1;
                    }
                }
                // A class the crate does not carry must be one music21
                // gives no key for.
                Err(_) => match theirs {
                    None => classes += 1,
                    Some(key) => failures.push(format!(
                        "{class}: the crate has no such instrument, and music21 plays key {key}"
                    )),
                },
            }
        }
        Ok((failures, keys, classes))
    })
    .expect("the comparison runs");

    println!("{keys} keys and {classes} classes agree");
    assert!(
        failures.is_empty(),
        "the two maps differ:\n  {}",
        failures.join("\n  ")
    );
}
