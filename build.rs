#!/usr/bin/env nix-shell
//! ```cargo
//! [features]
//! default = ["python"]
//! python = ["dep:pyo3", "dep:reqwest"]
//!
//! [dependencies]
//! pyo3 = { version = "0.24.2", features = ["auto-initialize"], optional = true }
//! utils = { path = "./utils", default-features = false }
//! reqwest = { version = "0.12.15", features = ["blocking"], optional = true }
//! ```
/*
#!nix-shell -i rust-script -p rustc -p rust-script -p cargo -p rustfmt -p python312 -p python312Packages.virtualenv -p git -p openssl
*/

use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    #[cfg(feature = "python")]
    python::main()?;

    println!("cargo:rerun-if-changed=./build.rs");
    Ok(())
}

#[cfg(feature = "python")]
mod python {
    mod utils {
        include!(concat!(env!("CARGO_MANIFEST_DIR"), "/shared.rs"));
    }

    use pyo3::PyErr;
    use pyo3::PyResult;
    use pyo3::exceptions::PyIOError;
    use pyo3::prelude::*;
    use pyo3::types::PyDict;
    use pyo3::types::PyTuple;
    use std::collections::HashMap;
    use std::error::Error;
    use std::fs;
    use std::path::PathBuf;
    use utils::Tables;
    use utils::get_tables;
    use utils::init_py_with_dummies;
    use utils::run_command;

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

    /// Given a Python iterable over pitch‐class indices, build a `[bool; 12]`
    /// vector.
    fn build_pc_vec(pcs: &Bound<'_, PyAny>) -> PyResult<Vec<bool>> {
        let mut vec = vec![false; 12];
        for obj in pcs.try_iter()? {
            let idx: usize = obj?.extract()?;
            if idx < 12 {
                vec[idx] = true;
            }
        }
        Ok(vec)
    }

    // Helper to convert sign integer to string representation.
    fn sign_str(inv: i32) -> String {
        match inv {
            -1 => "Sign::NegativeOne".to_string(),
            0 => "Sign::Zero".to_string(),
            1 => "Sign::One".to_string(),
            other => other.to_string(),
        }
    }

    fn generate_forte_table(forte_list: &Bound<'_, PyTuple>) -> PyResult<String> {
        let table_lines: Result<Vec<String>, PyErr> = forte_list
            .iter()
            .enumerate()
            .map(|(card, item)| {
                if card == 0 {
                    Ok("vec![],".to_string())
                } else {
                    let card_data: &Bound<'_, PyTuple> = item.downcast()?;
                    let entries: Result<Vec<String>, PyErr> = card_data
                        .iter()
                        .map(|entry| {
                            if entry.is_none() {
                                Ok("None,".to_string())
                            } else {
                                let tup: &Bound<'_, PyTuple> = entry.downcast()?;
                                let pcs = tup.get_item(0)?;
                                let icv = tup.get_item(1)?;
                                let iv = tup.get_item(2)?;
                                let z_relation = tup.get_item(3)?;

                                let pcs_vec_str = format!("{:?}", build_pc_vec(&pcs)?);
                                let icv_vec: Vec<i32> = icv.extract()?;
                                let iv_vec: Vec<i32> = iv.extract()?;
                                let icv_vec_str = format!("{icv_vec:?}");
                                let iv_vec_str = format!("{iv_vec:?}");
                                let z_rel_str = if z_relation.is_none() {
                                    "None".to_string()
                                } else {
                                    z_relation.str()?.to_str()?.to_string()
                                };
                                Ok(format!(
                                    "Some(({pcs_vec_str}, {icv_vec_str}, {iv_vec_str}, {z_rel_str})),"
                                ))
                            }
                        })
                        .collect();
                    let joined = entries?.join(" ");
                    Ok(format!("vec![{joined}],"))
                }
            })
            .collect();
        let table_body = table_lines?.join("\n");

        let rust_code =
            format!("pub(super) static FORTE: Forte = LazyLock::new(|| {{[\n{table_body}\n]}});");
        Ok(rust_code)
    }

    fn generate_inversion_default_pitch_class(tables: &Tables) -> PyResult<String> {
        let inv_default = tables.getattr("inversionDefaultPitchClasses")?;
        let inv_dict: &Bound<'_, PyDict> = inv_default.downcast()?;
        let entries: Result<Vec<String>, PyErr> = inv_dict
            .iter()
            .map(|(key, value)| {
                let key_tuple: &Bound<'_, PyTuple> = key.downcast()?;
                let card: i32 = key_tuple.get_item(0)?.extract()?;
                let forte: i32 = key_tuple.get_item(1)?.extract()?;
                let pcs_list: Vec<usize> = value.extract()?;
                let mut pcs_vec = vec![false; 12];
                for i in pcs_list {
                    if i < 12 {
                        pcs_vec[i] = true;
                    }
                }
                Ok(format!("    m.insert(({card}, {forte}), {pcs_vec:?});"))
            })
            .collect();
        let rust_code = format!(
            "pub(super) static INVERSION_DEFAULT_PITCH_CLASSES: InversionDefaultPitchClasses = LazyLock::new(|| {{\n    let mut m = HashMap::new();\n{}\n    m\n}});",
            entries?.join("\n")
        );
        Ok(rust_code)
    }

    fn generate_cardinality_to_chord_members(
        py: Python,
        tables: &Tables,
        forte_list: &Bound<'_, PyTuple>,
    ) -> PyResult<String> {
        let mut inner_vars = Vec::new();
        let mut lines = Vec::new();

        for (card, item) in forte_list.iter().enumerate() {
            let var_name = format!("inner_{card}");
            inner_vars.push(var_name.clone());
            if card == 0 {
                lines.push(format!("    let {var_name} = HashMap::new();"));
            } else {
                lines.push(format!("    let mut {var_name} = HashMap::new();"));
                let card_data: &Bound<'_, PyTuple> = item.downcast()?;
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
                    let has_distinct = inv_vec_list.get(1).is_some_and(|&v| v == 0);
                    let pcs_vec_str = format!("{:?}", build_pc_vec(&pcs)?);
                    let icv_vec: Vec<i32> = icv.extract()?;
                    let inv_vec_str = format!("{inv_vec_list:?}");
                    let icv_vec_str = format!("{icv_vec:?}");
                    let sign = if has_distinct {
                        sign_str(1)
                    } else {
                        sign_str(0)
                    };
                    lines.push(format!(
                        "    {var_name}.insert(({forte_idx}, {sign}), ({pcs_vec_str}, {inv_vec_str}, {icv_vec_str}));"
                    ));
                    if has_distinct {
                        let inversion_default = tables.getattr("inversionDefaultPitchClasses")?;
                        let card_py = card.into_pyobject(py)?;
                        let forte_idx_py = forte_idx.into_pyobject(py)?;
                        let key = PyTuple::new(py, &[card_py, forte_idx_py])?;

                        let inv_pcs = inversion_default
                            .get_item(key)
                            .unwrap_or_else(|_| py.None().bind(py).clone());
                        let inv_pcs_vec = if inv_pcs.is_none() {
                            vec![false; 12]
                        } else {
                            let inv_pcs_list: Vec<usize> = inv_pcs.extract()?;
                            let mut vec = vec![false; 12];
                            for i in inv_pcs_list {
                                if i < 12 {
                                    vec[i] = true;
                                }
                            }
                            vec
                        };
                        let inv_pcs_vec_str = format!("{inv_pcs_vec:?}");
                        lines.push(format!(
                            "    {}.insert(({}, {}), ({}, {}, {}));",
                            var_name,
                            forte_idx,
                            sign_str(-1),
                            inv_pcs_vec_str,
                            inv_vec_str,
                            icv_vec_str
                        ));
                    }
                }
            }
        }
        let inner_vars_str = inner_vars
            .into_iter()
            .map(|v| format!("        {v},"))
            .collect::<Vec<_>>()
            .join("\n");
        lines.push("    [".to_string());
        lines.push(inner_vars_str);
        lines.push("    ]".to_string());
        let rust_code = format!(
            "pub(super) static CARDINALITY_TO_CHORD_MEMBERS_GENERATED: CardinalityToChordMembers = LazyLock::new(|| {{\n{}\n}});\n",
            lines.join("\n")
        );
        Ok(rust_code)
    }

    fn generate_maximum_index_number(
        tables: &Tables,
        attr_name: &str,
        static_name: &str,
        type_name: &str,
    ) -> PyResult<String> {
        let arr = tables.getattr(attr_name)?;
        let arr_dict: &Bound<'_, PyDict> = arr.downcast()?;
        let hashmap: HashMap<usize, i32> = arr_dict.extract()?;
        let values: Vec<String> = (0..hashmap.len())
            .map(|num| hashmap.get(&num).unwrap().to_string())
            .collect();
        let rust_code = format!(
            "pub(super) static {}: {} = LazyLock::new(|| vec![{}]);",
            static_name,
            type_name,
            values.join(", ")
        );
        Ok(rust_code)
    }

    fn generate_maximum_index_number_without_inversion_equivalence(
        tables: &Tables,
    ) -> PyResult<String> {
        generate_maximum_index_number(
            tables,
            "maximumIndexNumberWithoutInversionEquivalence",
            "MAXIMUM_INDEX_NUMBER_WITHOUT_INVERSION_EQUIVALENCE",
            "MaximumIndexNumberWithoutInversionEquivalence",
        )
    }

    fn generate_maximum_index_number_with_inversion_equivalence(
        tables: &Tables,
    ) -> PyResult<String> {
        generate_maximum_index_number(
            tables,
            "maximumIndexNumberWithInversionEquivalence",
            "MAXIMUM_INDEX_NUMBER_WITH_INVERSION_EQUIVALENCE",
            "MaximumIndexNumberWithInversionEquivalence",
        )
    }

    fn generate_forte_number_with_inversion_to_tn_index(tables: &Tables) -> PyResult<String> {
        let dict = tables.getattr("forteNumberWithInversionToTnIndex")?;
        let dict_py: &Bound<'_, PyDict> = dict.downcast()?;
        let mut lines = Vec::new();
        for (key, value) in dict_py {
            let key_tuple: &Bound<'_, PyTuple> = key.downcast()?;
            let card: i32 = key_tuple.get_item(0)?.extract()?;
            let idx: i32 = key_tuple.get_item(1)?.extract()?;
            let inv: i32 = key_tuple.get_item(2)?.extract()?;
            let inv_str = sign_str(inv);
            let i: i32 = value.extract()?;
            lines.push(format!("    m.insert(({card}, {idx}, {inv_str}), {i});"));
        }
        let rust_code = format!(
            "pub(super) static FORTE_NUMBER_WITH_INVERSION_TO_INDEX: ForteNumberWithInversionToIndex = LazyLock::new(|| {{\n    let mut m = HashMap::new();\n{}\n    m\n}});",
            lines.join("\n")
        );
        Ok(rust_code)
    }

    fn generate_tn_index_to_chord_info(tables: &Tables) -> PyResult<String> {
        let dict = tables.getattr("tnIndexToChordInfo")?;
        let dict_py: &Bound<'_, PyDict> = dict.downcast()?;
        let mut lines = Vec::new();
        for (key, value) in dict_py {
            let key_tuple: &Bound<'_, PyTuple> = key.downcast()?;
            let card: i32 = key_tuple.get_item(0)?.extract()?;
            let idx: i32 = key_tuple.get_item(1)?.extract()?;
            let inv: i32 = key_tuple.get_item(2)?.extract()?;
            let inv_str = sign_str(inv);
            let value_dict: &Bound<'_, PyDict> = value.downcast()?;
            if let Some(names) = value_dict.get_item("name")? {
                let names_list: Vec<String> = names.extract()?;
                if !names_list.is_empty() {
                    let names_str = names_list
                        .into_iter()
                        .map(|s| format!("\"{s}\""))
                        .collect::<Vec<_>>()
                        .join(", ");
                    lines.push(format!(
                        "    m.insert(({card}, {idx}, {inv_str}), Some(vec![{names_str}]));"
                    ));
                } else {
                    lines.push(format!("    m.insert(({card}, {idx}, {inv_str}), None);"));
                }
            } else {
                lines.push(format!("    m.insert(({card}, {idx}, {inv_str}), None);"));
            }
        }
        let rust_code = format!(
            "pub(super) static TN_INDEX_TO_CHORD_INFO: TnIndexToChordInfo = LazyLock::new(|| {{\n    let mut m = HashMap::new();\n{}\n    m\n}});",
            lines.join("\n")
        );
        Ok(rust_code)
    }

    fn generate_rust_tables(py: Python, tables: &Tables, imports: &str) -> PyResult<String> {
        let forte = tables.getattr("FORTE")?;
        let forte_list: &Bound<'_, PyTuple> = forte.downcast_exact()?;
        let parts = [
            generate_forte_table(forte_list)?,
            generate_inversion_default_pitch_class(tables)?,
            generate_cardinality_to_chord_members(py, tables, forte_list)?,
            generate_forte_number_with_inversion_to_tn_index(tables)?,
            generate_tn_index_to_chord_info(tables)?,
            generate_maximum_index_number_without_inversion_equivalence(tables)?,
            generate_maximum_index_number_with_inversion_equivalence(tables)?,
        ];
        let rust_code = format!(
            "/*\nThis file is autogenerated from the tables in the original music21 library\ncheck the build script for details\n*/\n{}\n\n{}",
            imports,
            parts.join("\n\n")
        );
        Ok(rust_code)
    }

    pub(super) fn main() -> Result<(), Box<dyn Error>> {
        let rust_path = "./src/chord/tables/generated.rs";

        Python::with_gil(|py| -> PyResult<()> {
            init_py_with_dummies(py)?;

            let tables = get_tables(py)?;

            let imports = r#"
use super::{
    CardinalityToChordMembers, Forte, ForteNumberWithInversionToIndex,
    InversionDefaultPitchClasses, MaximumIndexNumberWithInversionEquivalence,
    MaximumIndexNumberWithoutInversionEquivalence, Sign, TnIndexToChordInfo,
};
use std::{collections::HashMap, sync::LazyLock};
"#;

            let rust = generate_rust_tables(py, &tables, imports)?;

            let output_path = PathBuf::from(rust_path);
            fs::write(&output_path, rust)
                .map_err(|e| PyErr::new::<PyIOError, _>(format!("{e}")))?;

            println!("Rust tables generated successfully.");
            Ok(())
        })?;

        run_command(&["rustfmt", rust_path], "rustfmt")?;

        println!("cargo:rerun-if-changed=./music21/music21/chord/tables.py");
        println!("cargo:rerun-if-changed=./build.rs");
        println!("cargo:rerun-if-changed=./utils/src/lib.rs");
        Ok(())
    }
}
