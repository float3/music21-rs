use std::collections::HashMap;
use std::{fs, path::PathBuf};

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyTuple};

#[allow(unused)]
const CARDINALITIES: [&str; 13] = [
    "none",
    "monad",
    "diad",
    "trichord",
    "tetrachord",
    "pentachord",
    "hexachord",
    "septachord",
    "octachord",
    "nonachord",
    "decachord",
    "undecachord",
    "duodecachord",
];

fn generate_forte_table(tables: &Tables) -> PyResult<String> {
    let mut rust_code = String::from("pub(super) static FORTE: Forte = LazyLock::new(|| {[\n");

    let forte = tables.getattr("FORTE")?;
    let forte_list: &Bound<'_, PyTuple> = forte.downcast_exact()?;

    for (card, item) in forte_list.iter().enumerate() {
        if card == 0 {
            rust_code.push_str("vec![],");
            continue;
        }
        let card_data: &Bound<'_, PyTuple> = item.downcast()?;
        rust_code.push_str("vec![");
        for entry in card_data.iter() {
            if entry.is_none() {
                rust_code.push_str("None,");
            } else {
                // entry is expected to be a tuple (pcs, icv, iv, z_relation)
                let tup: &Bound<'_, PyTuple> = entry.downcast()?;
                let pcs = tup.get_item(0)?;
                let icv = tup.get_item(1)?;
                let iv = tup.get_item(2)?;
                let z_relation = tup.get_item(3)?;

                // Process pcs: create a [bool; 12] vector and set indices to true.
                let mut pcs_vec = vec![false; 12];
                for obj in pcs.try_iter()? {
                    let idx: usize = obj?.extract()?;
                    if idx < 12 {
                        pcs_vec[idx] = true;
                    }
                }
                let pcs_vec_str = format!("{:?}", pcs_vec);

                // Process icv and iv – assume they are lists of numbers.
                let icv_vec: Vec<i32> = icv.extract()?;
                let iv_vec: Vec<i32> = iv.extract()?;
                let icv_vec_str = format!("{:?}", icv_vec);
                let iv_vec_str = format!("{:?}", iv_vec);

                // For z_relation, use "None" if missing.
                let z_rel_str = if z_relation.is_none() {
                    "None".to_string()
                } else {
                    // Use the Python string representation.
                    z_relation.str()?.to_str()?.to_string()
                };

                rust_code.push_str(&format!(
                    "Some(({}, {}, {}, {})),",
                    pcs_vec_str, icv_vec_str, iv_vec_str, z_rel_str
                ));
            }
        }
        rust_code.push_str("],");
        rust_code.push('\n');
    }
    rust_code.push_str("]\n});");
    Ok(rust_code)
}

fn generate_inversion_default_pitch_class(tables: &Tables) -> PyResult<String> {
    let mut rust_code = String::from("pub(super) static INVERSION_DEFAULT_PITCH_CLASSES: InversionDefaultPitchClasses = LazyLock::new(|| {\n");
    rust_code.push_str("    let mut m = HashMap::new();\n");

    let inv_default = tables.getattr("inversionDefaultPitchClasses")?;
    let inv_dict: &Bound<'_, PyDict> = inv_default.downcast()?;
    for (key, value) in inv_dict {
        // key is a tuple (card, forte)
        let key_tuple: &Bound<'_, PyTuple> = key.downcast()?;
        let card: i32 = key_tuple.get_item(0)?.extract()?;
        let forte: i32 = key_tuple.get_item(1)?.extract()?;
        // value is a list of pitch-class indices
        let pcs_list: Vec<usize> = value.extract()?;
        let mut pcs_vec = vec![false; 12];
        for i in pcs_list {
            if i < 12 {
                pcs_vec[i] = true;
            }
        }
        let pcs_vec_str = format!("{:?}", pcs_vec);
        rust_code.push_str(&format!(
            "    m.insert(({}, {}), {});\n",
            card, forte, pcs_vec_str
        ));
    }
    rust_code.push_str("    m\n});");
    Ok(rust_code)
}

fn generate_cardinality_to_chord_members(py: Python, tables: &Tables) -> PyResult<String> {
    let mut rust_code = String::from("pub(super) static CARDINALITY_TO_CHORD_MEMBERS_GENERATED: CardinalityToChordMembersGenerated = LazyLock::new(|| {\n");
    let mut inner_vars = Vec::new();
    let forte = tables.getattr("FORTE")?;
    let forte_list: &Bound<'_, PyTuple> = forte.downcast_exact()?;

    for (card, item) in forte_list.iter().enumerate() {
        if card == 0 {
            rust_code.push_str(&format!("    let inner_{} = HashMap::new();\n", card));
        } else {
            rust_code.push_str(&format!("    let mut inner_{} = HashMap::new();\n", card));
        }
        inner_vars.push(format!("inner_{}", card));
        if card != 0 {
            let card_data: &Bound<'_, PyTuple> = item.downcast()?;
            // Iterate over forte indices starting at 1.
            for forte_idx in 1..card_data.len() {
                let entry = card_data.get_item(forte_idx)?;
                if entry.is_none() {
                    continue;
                }
                let tup: &Bound<'_, PyTuple> = entry.downcast()?;
                let pcs = tup.get_item(0)?;
                let icv = tup.get_item(1)?;
                let inv_vec = tup.get_item(2)?;
                let _z_rel = tup.get_item(3)?;

                let inv_vec_list: Vec<i32> = inv_vec.extract()?;
                let has_distinct = if inv_vec_list.len() > 1 {
                    inv_vec_list[1] == 0
                } else {
                    false
                };

                let mut pcs_vec = vec![false; 12];
                for obj in pcs.try_iter()? {
                    let idx: usize = obj?.extract()?;
                    if idx < 12 {
                        pcs_vec[idx] = true;
                    }
                }
                let pcs_vec_str = format!("{:?}", pcs_vec);
                let icv_vec: Vec<i32> = icv.extract()?;
                let inv_vec_str = format!("{:?}", inv_vec_list);
                let icv_vec_str = format!("{:?}", icv_vec);

                let sign_str = if has_distinct {
                    "Sign::One"
                } else {
                    "Sign::Zero"
                };
                rust_code.push_str(&format!(
                    "    inner_{}.insert(({}, {}), ({}, {}, {}));\n",
                    card, forte_idx, sign_str, pcs_vec_str, inv_vec_str, icv_vec_str
                ));

                if has_distinct {
                    // Insert inverted entry.
                    let inversion_default = tables.getattr("inversionDefaultPitchClasses")?;
                    let card_py = card.into_pyobject(py)?;
                    let forte_idx_py = forte_idx.into_pyobject(py)?;
                    let key = PyTuple::new(py, &[card_py, forte_idx_py])?;

                    let inv_pcs = inversion_default
                        .get_item(key)
                        .unwrap_or(py.None().bind(py).clone());
                    let mut inv_pcs_vec = vec![false; 12];
                    if !inv_pcs.is_none() {
                        let inv_pcs_list: Vec<usize> = inv_pcs.extract()?;
                        for i in inv_pcs_list {
                            if i < 12 {
                                inv_pcs_vec[i] = true;
                            }
                        }
                    }
                    let inv_pcs_vec_str = format!("{:?}", inv_pcs_vec);
                    rust_code.push_str(&format!(
                        "    inner_{}.insert(({}, Sign::NegativeOne), ({}, {}, {}));\n",
                        card, forte_idx, inv_pcs_vec_str, inv_vec_str, icv_vec_str
                    ));
                }
            }
        }
    }
    rust_code.push_str("    [\n");
    for var in inner_vars {
        rust_code.push_str(&format!("        {},\n", var));
    }
    rust_code.push_str("    ]\n});\n");
    Ok(rust_code)
}

fn generate_maximum_index_number_without_inversion_equivalence(
    tables: &Tables,
) -> PyResult<String> {
    let mut rust_code = String::from("pub(super) static MAXIMUM_INDEX_NUMBER_WITHOUT_INVERSION_EQUIVALENCE: MaximumIndexNumberWithoutInversionEquivalence = LazyLock::new(|| vec![");
    let arr = tables.getattr("maximumIndexNumberWithoutInversionEquivalence")?;
    let arr_list: &Bound<'_, PyDict> = arr.downcast()?;
    let hashmap = arr_list.extract::<HashMap<usize, i32>>()?;
    for num in 0..hashmap.len() {
        rust_code.push_str(&format!("{}, ", hashmap.get(&num).unwrap()));
    }
    if rust_code.ends_with(", ") {
        rust_code.truncate(rust_code.len() - 2);
    }
    rust_code.push_str("]);");
    Ok(rust_code)
}

fn generate_maximum_index_number_with_inversion_equivalence(tables: &Tables) -> PyResult<String> {
    let mut rust_code = String::from("pub(super) static MAXIMUM_INDEX_NUMBER_WITH_INVERSION_EQUIVALENCE: MaximumIndexNumberWithInversionEquivalence = LazyLock::new(|| vec![");
    let arr = tables.getattr("maximumIndexNumberWithInversionEquivalence")?;
    let arr_list: &Bound<'_, PyDict> = arr.downcast()?;
    let hashmap = arr_list.extract::<HashMap<usize, i32>>()?;
    for num in 0..hashmap.len() {
        rust_code.push_str(&format!("{}, ", hashmap.get(&num).unwrap()));
    }
    if rust_code.ends_with(", ") {
        rust_code.truncate(rust_code.len() - 2);
    }
    rust_code.push_str("]);");
    Ok(rust_code)
}

fn generate_forte_number_with_inversion_to_tn_index(tables: &Tables) -> PyResult<String> {
    let mut rust_code = String::from("pub(super) static FORTE_NUMBER_WITH_INVERSION_TO_INDEX: ForteNumberWithInversionToIndex = LazyLock::new(|| {\n");
    rust_code.push_str("    let mut m = HashMap::new();\n");
    let dict = tables.getattr("forteNumberWithInversionToTnIndex")?;
    let dict_py: &Bound<'_, PyDict> = dict.downcast()?;
    for (key, value) in dict_py {
        let key_tuple: &Bound<'_, PyTuple> = key.downcast()?;
        let card: i32 = key_tuple.get_item(0)?.extract()?;
        let idx: i32 = key_tuple.get_item(1)?.extract()?;
        let inv: i32 = key_tuple.get_item(2)?.extract()?;
        let inv_str = match inv {
            -1 => "Sign::NegativeOne".to_string(),
            0 => "Sign::Zero".to_string(),
            1 => "Sign::One".to_string(),
            _ => format!("{}", inv),
        };
        let i: i32 = value.extract()?;
        rust_code.push_str(&format!(
            "    m.insert(({}, {}, {}), {});\n",
            card, idx, inv_str, i
        ));
    }
    rust_code.push_str("    m\n});");
    Ok(rust_code)
}

fn generate_tn_index_to_chord_info(tables: &Tables) -> PyResult<String> {
    let mut rust_code = String::from(
        "pub(super) static TN_INDEX_TO_CHORD_INFO: TnIndexToChordInfo = LazyLock::new(|| {\n",
    );
    rust_code.push_str("    let mut m = HashMap::new();\n");
    let dict = tables.getattr("tnIndexToChordInfo")?;
    let dict_py: &Bound<'_, PyDict> = dict.downcast()?;
    for (key, value) in dict_py {
        let key_tuple: &Bound<'_, PyTuple> = key.downcast()?;
        let card: i32 = key_tuple.get_item(0)?.extract()?;
        let idx: i32 = key_tuple.get_item(1)?.extract()?;
        let inv: i32 = key_tuple.get_item(2)?.extract()?;
        let inv_str = match inv {
            -1 => "Sign::NegativeOne".to_string(),
            0 => "Sign::Zero".to_string(),
            1 => "Sign::One".to_string(),
            _ => format!("{}", inv),
        };
        // value is expected to be a dict with a "name" key.
        let value_dict: &Bound<'_, PyDict> = value.downcast()?;
        if let Some(names) = value_dict.get_item("name")? {
            let names_list: Vec<String> = names.extract()?;
            if !names_list.is_empty() {
                let names_str = names_list
                    .into_iter()
                    .map(|s| format!("\"{}\"", s))
                    .collect::<Vec<_>>()
                    .join(", ");
                rust_code.push_str(&format!(
                    "    m.insert(({}, {}, {}), Some(vec![{}]));\n",
                    card, idx, inv_str, names_str
                ));
            } else {
                rust_code.push_str(&format!(
                    "    m.insert(({}, {}, {}), None);\n",
                    card, idx, inv_str
                ));
            }
        } else {
            rust_code.push_str(&format!(
                "    m.insert(({}, {}, {}), None);\n",
                card, idx, inv_str
            ));
        }
    }
    rust_code.push_str("    m\n});");
    Ok(rust_code)
}

fn generate_rust_tables(py: Python, tables: &Tables, imports: &str) -> PyResult<String> {
    let mut rust_code = String::new();

    rust_code.push_str("/*\nThis file is autogenerated from the tables in the original music21 library\ncheck the build script for details\n*/\n");
    rust_code.push_str(imports);
    rust_code.push('\n');
    rust_code.push_str(&generate_forte_table(tables)?);
    rust_code.push_str("\n\n");
    rust_code.push_str(&generate_inversion_default_pitch_class(tables)?);
    rust_code.push_str("\n\n");
    rust_code.push_str(&generate_cardinality_to_chord_members(py, tables)?);
    rust_code.push_str("\n\n");
    rust_code.push_str(&generate_forte_number_with_inversion_to_tn_index(tables)?);
    rust_code.push_str("\n\n");
    rust_code.push_str(&generate_tn_index_to_chord_info(tables)?);
    rust_code.push_str("\n\n");
    rust_code.push_str(&generate_maximum_index_number_without_inversion_equivalence(tables)?);
    rust_code.push_str("\n\n");
    rust_code.push_str(&generate_maximum_index_number_with_inversion_equivalence(
        tables,
    )?);
    rust_code.push_str("\n\n");

    Ok(rust_code)
}

type Tables<'py> = Bound<'py, PyModule>;

fn main() -> PyResult<()> {
    Python::with_gil(|py| -> PyResult<()> {
        let sys = py.import("sys")?;
        let path = sys.getattr("path")?;
        path.call_method1("append", ("./venv/lib/python3.12/site-packages",))?;
        path.call_method1("append", ("./music21",))?;

        let tables: Tables = py.import("music21.chord.tables")?;

        let imports = r#"
use super::{
    CardinalityToChordMembersGenerated, Forte, ForteNumberWithInversionToIndex,
    InversionDefaultPitchClasses, MaximumIndexNumberWithInversionEquivalence,
    MaximumIndexNumberWithoutInversionEquivalence, Sign, TnIndexToChordInfo,
};
use std::{collections::HashMap, sync::LazyLock};
"#;

        let rust = generate_rust_tables(py, &tables, imports)?;

        let output_path = PathBuf::from("./src/chord/tables/generated.rs");
        if !output_path.exists() {
            eprintln!("Error: File {} does not exist.", output_path.display());
            std::process::exit(1);
        }
        fs::write(&output_path, rust)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyIOError, _>(format!("{}", e)))?;

        println!("Rust tables generated successfully.");
        Ok(())
    })?;

    let output = std::process::Command::new("rustfmt")
        .arg("src/chord/tables/generated.rs")
        .output()
        .expect("Failed to execute rustfmt");

    if !output.status.success() {
        panic!();
    };
    Ok(())
}
