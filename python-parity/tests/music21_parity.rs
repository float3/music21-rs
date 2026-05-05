use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use music21_rs::Chord;
use serde::Deserialize;
use utils::{
    get_tables, init_py, init_py_with_dummies, prepare,
    pyo3::{
        Bound, PyAny, PyErr, PyResult, Python, prelude::PyModule, types::PyAnyMethods,
        types::PyDict, types::PyDictMethods, types::PyTuple, types::PyTupleMethods,
    },
};

const CARDINALITIES: usize = 13;

#[derive(Debug, Deserialize)]
struct ChordTablesData {
    forte: Vec<ForteEntry>,
    inversion_default_pitch_classes: Vec<InversionDefaultPitchClasses>,
}

#[derive(Debug, Deserialize)]
struct ForteEntry {
    cardinality: u8,
    index: u8,
    pitch_classes: Vec<u8>,
    interval_class_vector: [u8; 6],
    invariance_vector: [u8; 8],
    z_relation: u8,
}

#[derive(Debug, Deserialize)]
struct InversionDefaultPitchClasses {
    cardinality: u8,
    index: u8,
    pitch_classes: Vec<u8>,
}

#[derive(Debug)]
struct ChordMember {
    index: u8,
    inversion: i8,
    pitch_classes: Vec<u8>,
    invariance_vector: [u8; 8],
    interval_class_vector: [u8; 6],
}

type TniTuple = (Vec<u8>, Vec<u8>, Vec<u8>, u8);
type ChordMemberTuple = (Vec<u8>, Vec<u8>, Vec<u8>);

#[test]
fn python_music21_parity() {
    let root = repo_root();
    prepare_music21(&root);
    let data = read_chord_tables_data(&root);

    Python::attach(|py| -> PyResult<()> {
        init_py(py)?;
        init_py_with_dummies(py)?;

        compare_cardinality_to_chord_members(py, &data)?;
        compare_forte_table(py, &data)?;
        compare_named_chords(py)?;
        compare_all_pitch_class_subsets(py)?;

        Ok(())
    })
    .unwrap();
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("python-parity lives inside the repository")
        .to_path_buf()
}

fn prepare_music21(root: &Path) {
    static PREPARED: OnceLock<()> = OnceLock::new();
    PREPARED.get_or_init(|| {
        std::env::set_current_dir(root).expect("set repository root as test cwd");
        prepare().expect("prepare music21 reference checkout");
    });
}

fn read_chord_tables_data(root: &Path) -> ChordTablesData {
    let text = fs::read_to_string(root.join("data/chord_tables.toml"))
        .expect("read committed chord table data");
    toml::from_str(&text).expect("parse committed chord table data")
}

fn import_music21_chord_without_package_init(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let sys = py.import("sys")?;
    let modules = sys.getattr("modules")?;
    modules.call_method1("pop", ("music21.chord", py.None()))?;
    modules.call_method1("pop", ("music21", py.None()))?;

    let music21_src = repo_root().join("music21/music21");
    let types = py.import("types")?;
    let music21_pkg = types.getattr("ModuleType")?.call1(("music21",))?;
    music21_pkg.setattr("__path__", vec![music21_src.display().to_string()])?;
    modules.call_method1("__setitem__", ("music21", music21_pkg))?;

    py.import("music21.chord")
}

fn compare_named_chords(py: Python<'_>) -> Result<(), PyErr> {
    let chord = import_music21_chord_without_package_init(py)?;
    let chord_class = chord.getattr("Chord")?;

    for chord_input in ["C E G", "C C# D D# E F F# G G# A A# B"] {
        compare_chord(chord_input, &chord_class)?;
    }

    Ok(())
}

fn compare_all_pitch_class_subsets(py: Python<'_>) -> Result<(), PyErr> {
    let chord = import_music21_chord_without_package_init(py)?;
    let chord_class = chord.getattr("Chord")?;

    for mask in 0_u16..(1_u16 << 12) {
        let pcs = (0..12)
            .filter(|pc| mask & (1 << pc) != 0)
            .collect::<Vec<_>>();
        let chord_instance = chord_class.call1((pcs.clone(),))?;

        let python_common_name: String = chord_instance.getattr("commonName")?.extract()?;
        let python_pitched_common_name: String =
            chord_instance.getattr("pitchedCommonName")?.extract()?;

        let rust_chord = Chord::new(pcs.as_slice()).unwrap();
        assert_eq!(
            rust_chord.common_name(),
            python_common_name,
            "commonName mismatch for mask {mask:012b} pcs {pcs:?}"
        );
        assert_eq!(
            rust_chord.pitched_common_name(),
            python_pitched_common_name,
            "pitchedCommonName mismatch for mask {mask:012b} pcs {pcs:?}"
        );
    }

    Ok(())
}

fn compare_chord(chord_input: &str, chord_class: &Bound<'_, PyAny>) -> Result<(), PyErr> {
    let chord_instance = chord_class.call1((chord_input,))?;
    let python_common_name: String = chord_instance.getattr("commonName")?.extract()?;
    let python_pitched_common_name: String =
        chord_instance.getattr("pitchedCommonName")?.extract()?;

    let chord = Chord::new(chord_input).unwrap();
    assert_eq!(chord.common_name(), python_common_name);
    assert_eq!(chord.pitched_common_name(), python_pitched_common_name);
    Ok(())
}

fn compare_cardinality_to_chord_members(py: Python<'_>, data: &ChordTablesData) -> PyResult<()> {
    let expected = build_chord_members(data);
    let tables = get_tables(py)?;
    let cardinality_to_chord_members = tables.getattr("cardinalityToChordMembers")?;
    let cardinality_to_chord_members: &Bound<'_, PyDict> =
        cardinality_to_chord_members.cast_exact()?;

    for outer_key in cardinality_to_chord_members.keys() {
        let cardinality: u8 = outer_key.extract()?;
        let inner_dict = cardinality_to_chord_members
            .get_item(&outer_key)?
            .expect("music21 table key disappeared");
        let inner_dict: &Bound<'_, PyDict> = inner_dict.cast_exact()?;
        let expected_bucket = expected
            .get(&cardinality)
            .unwrap_or_else(|| panic!("missing expected cardinality bucket {cardinality}"));

        assert_eq!(
            expected_bucket.len(),
            inner_dict.len(),
            "cardinality {cardinality} member count differs"
        );

        for inner_key in inner_dict.keys() {
            let (index, inversion): (u8, i8) = inner_key.extract()?;
            let python_member: ChordMemberTuple = inner_dict
                .get_item(&inner_key)?
                .expect("music21 member key disappeared")
                .extract()?;
            let rust_member = expected_bucket.get(&(index, inversion)).unwrap_or_else(|| {
                panic!(
                    "missing Rust member for cardinality {cardinality}, index {index}, inversion {inversion}"
                )
            });

            assert_eq!(
                rust_member, &python_member,
                "cardinality {cardinality}, index {index}, inversion {inversion}"
            );
        }
    }

    Ok(())
}

fn compare_forte_table(py: Python<'_>, data: &ChordTablesData) -> PyResult<()> {
    let expected = build_forte_table(data);
    let tables = get_tables(py)?;
    let forte = tables.getattr("FORTE")?;
    let forte: &Bound<'_, PyTuple> = forte.cast_exact()?;

    for (cardinality, expected_entry) in expected.iter().enumerate() {
        let python_entry = forte.get_item(cardinality)?;
        if python_entry.is_none() {
            assert!(
                expected_entry.is_empty(),
                "music21 has no FORTE entry for cardinality {cardinality}, but Rust does"
            );
            continue;
        }

        let python_entry: Vec<Option<TniTuple>> = python_entry.extract()?;
        assert_eq!(
            expected_entry, &python_entry,
            "FORTE mismatch for cardinality {cardinality}"
        );
    }

    Ok(())
}

fn build_chord_members(
    data: &ChordTablesData,
) -> BTreeMap<u8, BTreeMap<(u8, i8), ChordMemberTuple>> {
    let mut defaults = BTreeMap::new();
    for entry in &data.inversion_default_pitch_classes {
        defaults.insert(
            (entry.cardinality, entry.index),
            entry.pitch_classes.clone(),
        );
    }

    let mut buckets = BTreeMap::<u8, BTreeMap<(u8, i8), ChordMemberTuple>>::new();
    for entry in &data.forte {
        let has_distinct_inversion = entry.invariance_vector[1] == 0;
        let inversion = if has_distinct_inversion { 1 } else { 0 };
        insert_chord_member(&mut buckets, entry, inversion, entry.pitch_classes.clone());

        if has_distinct_inversion
            && let Some(pitch_classes) = defaults.get(&(entry.cardinality, entry.index))
        {
            insert_chord_member(&mut buckets, entry, -1, pitch_classes.clone());
        }
    }

    buckets
}

fn insert_chord_member(
    buckets: &mut BTreeMap<u8, BTreeMap<(u8, i8), ChordMemberTuple>>,
    entry: &ForteEntry,
    inversion: i8,
    pitch_classes: Vec<u8>,
) {
    let member = ChordMember {
        index: entry.index,
        inversion,
        pitch_classes,
        invariance_vector: entry.invariance_vector,
        interval_class_vector: entry.interval_class_vector,
    };
    buckets
        .entry(entry.cardinality)
        .or_default()
        .insert((member.index, member.inversion), member.into_tuple());
}

impl ChordMember {
    fn into_tuple(self) -> ChordMemberTuple {
        (
            self.pitch_classes,
            self.invariance_vector.to_vec(),
            self.interval_class_vector.to_vec(),
        )
    }
}

fn build_forte_table(data: &ChordTablesData) -> Vec<Vec<Option<TniTuple>>> {
    let mut buckets = vec![Vec::new(); CARDINALITIES];
    for entry in &data.forte {
        let bucket = &mut buckets[entry.cardinality as usize];
        if bucket.len() <= entry.index as usize {
            bucket.resize(entry.index as usize + 1, None);
        }
        let index = entry.index as usize;
        bucket[index] = Some((
            entry.pitch_classes.clone(),
            entry.interval_class_vector.to_vec(),
            entry.invariance_vector.to_vec(),
            entry.z_relation,
        ));
    }
    buckets
}
