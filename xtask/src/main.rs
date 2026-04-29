//! Development task runner for regenerating committed chord-table artifacts.

mod shared {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/../shared.rs"));
}

use proc_macro2::{Literal, TokenStream};
#[cfg(feature = "python")]
use pyo3::exceptions::PyValueError;
#[cfg(feature = "python")]
use pyo3::prelude::*;
#[cfg(feature = "python")]
use pyo3::types::{PyDict, PyTuple};
use quote::quote;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use shared::run_command;
#[cfg(feature = "python")]
use shared::{Tables, get_tables, init_py_with_dummies, prepare};

const CARDINALITIES: usize = 13;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ChordTablesData {
    forte: Vec<ForteEntryData>,
    inversion_default_pitch_classes: Vec<InversionDefaultPitchClassesData>,
    forte_number_with_inversion_to_index: Vec<ForteNumberWithInversionToIndexData>,
    tn_index_to_chord_info: Vec<TnIndexToChordInfoData>,
    maximum_index_number_without_inversion_equivalence: Vec<u8>,
    maximum_index_number_with_inversion_equivalence: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ForteEntryData {
    cardinality: u8,
    index: u8,
    pitch_classes: Vec<u8>,
    interval_class_vector: [u8; 6],
    invariance_vector: [u8; 8],
    z_relation: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct InversionDefaultPitchClassesData {
    cardinality: u8,
    index: u8,
    pitch_classes: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ForteNumberWithInversionToIndexData {
    cardinality: u8,
    index: u8,
    inversion: i8,
    tn_index: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct TnIndexToChordInfoData {
    cardinality: u8,
    index: u8,
    inversion: i8,
    #[serde(skip_serializing_if = "Option::is_none")]
    names: Option<Vec<String>>,
}

#[derive(Clone, Debug)]
struct GeneratedChordMember {
    index: u8,
    inversion: i8,
    pitch_classes: Vec<u8>,
    invariance_vector: [u8; 8],
    interval_class_vector: [u8; 6],
}

fn main() -> Result<(), Box<dyn Error>> {
    let workspace_root = workspace_root()?;
    std::env::set_current_dir(&workspace_root)?;

    match std::env::args().nth(1).as_deref() {
        Some("regenerate-tables") => regenerate_tables(&workspace_root),
        Some("emit-tables") => emit_tables(&workspace_root),
        Some("-h") | Some("--help") | None => {
            print_help();
            Ok(())
        }
        Some(command) => Err(format!("unknown xtask command {command:?}").into()),
    }
}

fn print_help() {
    eprintln!("Usage:");
    eprintln!("  cargo run -p xtask -- regenerate-tables");
    eprintln!("  cargo run -p xtask -- emit-tables");
    eprintln!("  cargo run -p xtask --no-default-features -- emit-tables");
}

fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "xtask manifest dir has no parent".into())
}

fn table_data_path(workspace_root: &Path) -> PathBuf {
    workspace_root.join("data/chord_tables.toml")
}

fn generated_path(workspace_root: &Path) -> PathBuf {
    workspace_root.join("src/chord/tables/generated.rs")
}

#[cfg(feature = "python")]
fn regenerate_tables(workspace_root: &Path) -> Result<(), Box<dyn Error>> {
    prepare()?;

    let data_path = table_data_path(workspace_root);
    let mut data = Python::attach(|py| -> PyResult<ChordTablesData> {
        init_py_with_dummies(py)?;
        let tables = get_tables(py)?;
        extract_chord_tables(py, &tables)
    })?;

    normalize_table_data(&mut data);
    write_table_data(&data_path, &data)?;
    emit_generated_rust(workspace_root, &data)?;
    Ok(())
}

#[cfg(not(feature = "python"))]
fn regenerate_tables(_: &Path) -> Result<(), Box<dyn Error>> {
    Err(
        "regenerate-tables requires xtask's python feature; run `cargo run -p xtask --features python -- regenerate-tables`"
            .into(),
    )
}

fn emit_tables(workspace_root: &Path) -> Result<(), Box<dyn Error>> {
    let data = read_table_data(&table_data_path(workspace_root))?;
    emit_generated_rust(workspace_root, &data)
}

#[cfg(feature = "python")]
fn write_table_data(path: &Path, data: &ChordTablesData) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, toml::to_string_pretty(data)?)?;
    println!("Chord table TOML written to {}", path.display());
    Ok(())
}

fn read_table_data(path: &Path) -> Result<ChordTablesData, Box<dyn Error>> {
    Ok(toml::from_str(&fs::read_to_string(path)?)?)
}

fn emit_generated_rust(
    workspace_root: &Path,
    data: &ChordTablesData,
) -> Result<(), Box<dyn Error>> {
    let path = generated_path(workspace_root);
    fs::write(&path, render_generated_rust(data))?;
    println!("Rust chord tables regenerated at {}", path.display());

    let path = path
        .to_str()
        .ok_or_else(|| "generated table path is not valid UTF-8".to_string())?;
    run_command(&["rustfmt", path], "rustfmt")?;
    Ok(())
}

#[cfg(feature = "python")]
fn extract_chord_tables(_: Python<'_>, tables: &Tables<'_>) -> PyResult<ChordTablesData> {
    let forte = extract_forte(tables)?;
    let inversion_default_pitch_classes = extract_inversion_defaults(tables)?;
    let forte_number_with_inversion_to_index = extract_forte_number_with_inversion(tables)?;
    let tn_index_to_chord_info = extract_tn_index_to_chord_info(tables)?;
    let maximum_index_number_without_inversion_equivalence =
        extract_maximum_index_number(tables, "maximumIndexNumberWithoutInversionEquivalence")?;
    let maximum_index_number_with_inversion_equivalence =
        extract_maximum_index_number(tables, "maximumIndexNumberWithInversionEquivalence")?;

    Ok(ChordTablesData {
        forte,
        inversion_default_pitch_classes,
        forte_number_with_inversion_to_index,
        tn_index_to_chord_info,
        maximum_index_number_without_inversion_equivalence,
        maximum_index_number_with_inversion_equivalence,
    })
}

#[cfg(feature = "python")]
fn extract_forte(tables: &Tables<'_>) -> PyResult<Vec<ForteEntryData>> {
    let forte = tables.getattr("FORTE")?;
    let forte: &Bound<'_, PyTuple> = forte.cast_exact()?;
    let mut entries = Vec::new();

    for (cardinality, item) in forte.iter().enumerate() {
        if cardinality == 0 {
            continue;
        }
        let card_data: &Bound<'_, PyTuple> = item.cast()?;
        for (index, entry) in card_data.iter().enumerate().skip(1) {
            if entry.is_none() {
                continue;
            }

            let tuple: &Bound<'_, PyTuple> = entry.cast()?;
            let pitch_classes = extract_pitch_classes(&tuple.get_item(0)?)?;
            let interval_class_vector = fixed_u8_array::<6>(
                tuple.get_item(1)?.extract::<Vec<u8>>()?,
                "interval_class_vector",
            )?;
            let invariance_vector = fixed_u8_array::<8>(
                tuple.get_item(2)?.extract::<Vec<u8>>()?,
                "invariance_vector",
            )?;
            let z_relation = extract_z_relation(&tuple.get_item(3)?)?;

            entries.push(ForteEntryData {
                cardinality: cardinality as u8,
                index: index as u8,
                pitch_classes,
                interval_class_vector,
                invariance_vector,
                z_relation,
            });
        }
    }

    Ok(entries)
}

#[cfg(feature = "python")]
fn extract_inversion_defaults(
    tables: &Tables<'_>,
) -> PyResult<Vec<InversionDefaultPitchClassesData>> {
    let inv_default = tables.getattr("inversionDefaultPitchClasses")?;
    let inv_dict: &Bound<'_, PyDict> = inv_default.cast()?;
    let mut entries = Vec::new();

    for (key, value) in inv_dict.iter() {
        let key_tuple: &Bound<'_, PyTuple> = key.cast()?;
        entries.push(InversionDefaultPitchClassesData {
            cardinality: key_tuple.get_item(0)?.extract()?,
            index: key_tuple.get_item(1)?.extract()?,
            pitch_classes: normalize_pitch_classes(value.extract()?),
        });
    }

    Ok(entries)
}

#[cfg(feature = "python")]
fn extract_forte_number_with_inversion(
    tables: &Tables<'_>,
) -> PyResult<Vec<ForteNumberWithInversionToIndexData>> {
    let dict = tables.getattr("forteNumberWithInversionToTnIndex")?;
    let dict: &Bound<'_, PyDict> = dict.cast()?;
    let mut entries = Vec::new();

    for (key, value) in dict.iter() {
        let key_tuple: &Bound<'_, PyTuple> = key.cast()?;
        entries.push(ForteNumberWithInversionToIndexData {
            cardinality: key_tuple.get_item(0)?.extract()?,
            index: key_tuple.get_item(1)?.extract()?,
            inversion: key_tuple.get_item(2)?.extract()?,
            tn_index: value.extract()?,
        });
    }

    Ok(entries)
}

#[cfg(feature = "python")]
fn extract_tn_index_to_chord_info(tables: &Tables<'_>) -> PyResult<Vec<TnIndexToChordInfoData>> {
    let dict = tables.getattr("tnIndexToChordInfo")?;
    let dict: &Bound<'_, PyDict> = dict.cast()?;
    let mut entries = Vec::new();

    for (key, value) in dict.iter() {
        let key_tuple: &Bound<'_, PyTuple> = key.cast()?;
        let value_dict: &Bound<'_, PyDict> = value.cast()?;
        let names = value_dict
            .get_item("name")?
            .map(|names| names.extract::<Vec<String>>())
            .transpose()?
            .filter(|names| !names.is_empty());

        entries.push(TnIndexToChordInfoData {
            cardinality: key_tuple.get_item(0)?.extract()?,
            index: key_tuple.get_item(1)?.extract()?,
            inversion: key_tuple.get_item(2)?.extract()?,
            names,
        });
    }

    Ok(entries)
}

#[cfg(feature = "python")]
fn extract_maximum_index_number(tables: &Tables<'_>, attr_name: &str) -> PyResult<Vec<u8>> {
    let dict = tables.getattr(attr_name)?;
    let dict: &Bound<'_, PyDict> = dict.cast()?;
    let hashmap: HashMap<usize, u8> = dict.extract()?;
    Ok((0..hashmap.len())
        .map(|index| hashmap.get(&index).copied().unwrap_or_default())
        .collect())
}

#[cfg(feature = "python")]
fn extract_pitch_classes(pcs: &Bound<'_, PyAny>) -> PyResult<Vec<u8>> {
    let mut values = Vec::new();
    for obj in pcs.try_iter()? {
        values.push(obj?.extract()?);
    }
    Ok(normalize_pitch_classes(values))
}

#[cfg(feature = "python")]
fn normalize_pitch_classes(mut values: Vec<u8>) -> Vec<u8> {
    for value in &mut values {
        *value %= 12;
    }
    values.sort_unstable();
    values.dedup();
    values
}

#[cfg(feature = "python")]
fn extract_z_relation(value: &Bound<'_, PyAny>) -> PyResult<u8> {
    if value.is_none() {
        return Ok(0);
    }
    value.extract::<u8>().or_else(|_| {
        let text: String = value.str()?.extract()?;
        text.parse::<u8>()
            .map_err(|err| PyErr::new::<PyValueError, _>(format!("invalid z-relation: {err}")))
    })
}

#[cfg(feature = "python")]
fn fixed_u8_array<const N: usize>(values: Vec<u8>, name: &str) -> PyResult<[u8; N]> {
    values.try_into().map_err(|values: Vec<u8>| {
        PyErr::new::<PyValueError, _>(format!("{name} expected {N} values, got {}", values.len()))
    })
}

#[cfg(feature = "python")]
fn normalize_table_data(data: &mut ChordTablesData) {
    data.forte
        .sort_by_key(|entry| (entry.cardinality, entry.index));
    data.inversion_default_pitch_classes
        .sort_by_key(|entry| (entry.cardinality, entry.index));
    data.forte_number_with_inversion_to_index
        .sort_by_key(|entry| (entry.cardinality, entry.index, entry.inversion));
    data.tn_index_to_chord_info
        .sort_by_key(|entry| (entry.cardinality, entry.index, entry.inversion));
}

fn render_generated_rust(data: &ChordTablesData) -> String {
    let forte = forte_tokens(data);
    let cardinality_to_chord_members = chord_member_tokens(data);
    let forte_number_with_inversion_to_index = forte_number_with_inversion_tokens(data);
    let tn_index_to_chord_info = tn_index_to_chord_info_tokens(data);
    let max_without = u8_slice_tokens(&data.maximum_index_number_without_inversion_equivalence);
    let max_with = u8_slice_tokens(&data.maximum_index_number_with_inversion_equivalence);

    let tokens = quote! {
        use super::{
            CardinalityToChordMembers, Forte, ForteNumberWithInversionToIndex,
            MaximumIndexNumberWithInversionEquivalence,
            MaximumIndexNumberWithoutInversionEquivalence, Pcivicv, Sign, TnIndexToChordInfo,
            TNIStructure, U8SB,
        };

        pub(super) static FORTE: Forte = #forte;

        pub(super) static CARDINALITY_TO_CHORD_MEMBERS: CardinalityToChordMembers =
            #cardinality_to_chord_members;

        pub(super) static FORTE_NUMBER_WITH_INVERSION_TO_INDEX:
            ForteNumberWithInversionToIndex = #forte_number_with_inversion_to_index;

        pub(super) static TN_INDEX_TO_CHORD_INFO: TnIndexToChordInfo =
            #tn_index_to_chord_info;

        pub(super) static MAXIMUM_INDEX_NUMBER_WITHOUT_INVERSION_EQUIVALENCE:
            MaximumIndexNumberWithoutInversionEquivalence = #max_without;

        pub(super) static MAXIMUM_INDEX_NUMBER_WITH_INVERSION_EQUIVALENCE:
            MaximumIndexNumberWithInversionEquivalence = #max_with;
    };

    let mut rust = String::from(
        "/*\nThis file is autogenerated from data/chord_tables.toml.\nRegenerate it with `cargo run -p xtask -- regenerate-tables`.\n*/\n\n",
    );
    rust.push_str(&tokens.to_string());
    rust.push('\n');
    rust
}

fn forte_tokens(data: &ChordTablesData) -> TokenStream {
    let mut buckets = vec![Vec::<Option<ForteEntryData>>::new(); CARDINALITIES];
    for entry in data.forte.iter().cloned() {
        let bucket = &mut buckets[entry.cardinality as usize];
        if bucket.len() <= entry.index as usize {
            bucket.resize(entry.index as usize + 1, None);
        }
        let index = entry.index as usize;
        bucket[index] = Some(entry);
    }

    let bucket_tokens = buckets.iter().map(|bucket| {
        let entries = bucket.iter().map(|entry| match entry {
            Some(entry) => {
                let tni = tni_tokens(entry);
                quote!(Some(#tni))
            }
            None => quote!(None),
        });
        quote!(&[#(#entries),*] as &[Option<TNIStructure>])
    });

    quote!([#(#bucket_tokens),*])
}

fn chord_member_tokens(data: &ChordTablesData) -> TokenStream {
    let buckets = build_chord_members(data);
    let bucket_tokens = buckets.iter().map(|bucket| {
        let entries = bucket.iter().map(|entry| {
            let index = entry.index;
            let inversion = sign_tokens(entry.inversion);
            let pitch_classes = pitch_classes_tokens(&entry.pitch_classes);
            let invariance_vector = fixed_u8_array_tokens(&entry.invariance_vector);
            let interval_class_vector = fixed_u8_array_tokens(&entry.interval_class_vector);

            quote! {
                (
                    (#index, #inversion),
                    (#pitch_classes, #invariance_vector, #interval_class_vector),
                )
            }
        });
        quote!(&[#(#entries),*] as &[(U8SB, Pcivicv)])
    });

    quote!([#(#bucket_tokens),*])
}

fn build_chord_members(data: &ChordTablesData) -> Vec<Vec<GeneratedChordMember>> {
    let mut defaults = HashMap::new();
    for entry in &data.inversion_default_pitch_classes {
        defaults.insert(
            (entry.cardinality, entry.index),
            entry.pitch_classes.clone(),
        );
    }

    let mut buckets = vec![Vec::new(); CARDINALITIES];
    for entry in &data.forte {
        let has_distinct_inversion = entry.invariance_vector[1] == 0;
        let inversion = if has_distinct_inversion { 1 } else { 0 };
        buckets[entry.cardinality as usize].push(GeneratedChordMember {
            index: entry.index,
            inversion,
            pitch_classes: entry.pitch_classes.clone(),
            invariance_vector: entry.invariance_vector,
            interval_class_vector: entry.interval_class_vector,
        });

        if has_distinct_inversion
            && let Some(pitch_classes) = defaults.get(&(entry.cardinality, entry.index))
        {
            buckets[entry.cardinality as usize].push(GeneratedChordMember {
                index: entry.index,
                inversion: -1,
                pitch_classes: pitch_classes.clone(),
                invariance_vector: entry.invariance_vector,
                interval_class_vector: entry.interval_class_vector,
            });
        }
    }

    for bucket in &mut buckets {
        bucket.sort_by_key(|entry| (entry.index, entry.inversion));
    }

    buckets
}

fn forte_number_with_inversion_tokens(data: &ChordTablesData) -> TokenStream {
    let entries = data
        .forte_number_with_inversion_to_index
        .iter()
        .map(|entry| {
            let cardinality = entry.cardinality;
            let index = entry.index;
            let inversion = sign_tokens(entry.inversion);
            let tn_index = entry.tn_index;
            quote!(((#cardinality, #index, #inversion), #tn_index))
        });

    quote!(&[#(#entries),*])
}

fn tn_index_to_chord_info_tokens(data: &ChordTablesData) -> TokenStream {
    let entries = data.tn_index_to_chord_info.iter().map(|entry| {
        let cardinality = entry.cardinality;
        let index = entry.index;
        let inversion = sign_tokens(entry.inversion);
        let names = match &entry.names {
            Some(names) => {
                let names = names.iter().map(|name| Literal::string(name));
                quote!(Some(&[#(#names),*] as &[&str]))
            }
            None => quote!(None),
        };

        quote!(((#cardinality, #index, #inversion), #names))
    });

    quote!(&[#(#entries),*])
}

fn tni_tokens(entry: &ForteEntryData) -> TokenStream {
    let pitch_classes = pitch_classes_tokens(&entry.pitch_classes);
    let interval_class_vector = fixed_u8_array_tokens(&entry.interval_class_vector);
    let invariance_vector = fixed_u8_array_tokens(&entry.invariance_vector);
    let z_relation = entry.z_relation;

    quote! {
        (
            #pitch_classes,
            #interval_class_vector,
            #invariance_vector,
            #z_relation,
        )
    }
}

fn pitch_classes_tokens(values: &[u8]) -> TokenStream {
    let mut pitch_classes = [false; 12];
    for value in values {
        pitch_classes[*value as usize % 12] = true;
    }
    let values = pitch_classes.iter();
    quote!([#(#values),*])
}

fn fixed_u8_array_tokens<const N: usize>(values: &[u8; N]) -> TokenStream {
    let values = values.iter();
    quote!([#(#values),*])
}

fn u8_slice_tokens(values: &[u8]) -> TokenStream {
    quote!([#(#values),*])
}

fn sign_tokens(value: i8) -> TokenStream {
    match value {
        -1 => quote!(Sign::NegativeOne),
        0 => quote!(Sign::Zero),
        1 => quote!(Sign::One),
        other => panic!("unsupported inversion sign {other}"),
    }
}
