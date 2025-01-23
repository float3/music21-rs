#!/usr/bin/env python3
import re
import sys
from textwrap import dedent


def parse_tni_structures(py_code):
    """Parse TNI structures from Python code and return a dict of variables"""
    # Improved regex pattern with better tuple handling and whitespace tolerance
    tni_pattern = re.compile(
        r"t(\d+)\s*=\s*"  # Match variable name (t1, t2, etc.)
        r"\(\(\s*([\d,\s]+)\s*\)\s*,\s*"  # Pitch classes tuple
        r"\(\s*([\d,\s]+)\s*\)\s*,\s*"  # Interval class vector
        r"\(\s*([\d,\s]+)\s*\)\s*,\s*"  # Inversion vector
        r"(\d+)\s*\)\s*#\s*([\w-]+)"  # Z-relation and Forte ID
    )

    tni_entries = {}

    for line in py_code.split("\n"):
        line = line.strip()
        if not line.startswith("t"):
            continue

        match = tni_pattern.search(line)
        if not match:
            continue

        var_name, pcs, icv, inv_vec, z_rel, forte_id = match.groups()

        # Clean and convert values
        var_name = f"t{var_name}"
        pcs = [int(x.strip()) for x in pcs.split(",") if x.strip()]
        icv = [int(x.strip()) for x in icv.split(",") if x.strip()]
        inv_vec = [int(x.strip()) for x in inv_vec.split(",") if x.strip()]
        z_rel = int(z_rel)

        tni_entries[var_name] = {
            "pcs": pcs,
            "icv": icv,
            "inv_vec": inv_vec,
            "z_rel": z_rel,
            "forte_id": forte_id,
        }

    return tni_entries


def parse_cardinality_groups(py_code):
    """Parse cardinality groups (monad, diad, etc.) from Python code"""
    # Remove ^ anchor and add word boundary matching
    group_pattern = re.compile(
        r"\b(monad|diad|trichord|tetrachord|pentachord|hexachord|"
        r"septachord|octachord|nonachord|decachord|undecachord|dodecachord)"
        r"\s*=\s*\((.*?)\)",
        re.DOTALL,
    )

    groups = {}
    for match in group_pattern.finditer(py_code):
        name, body = match.groups()
        entries = []
        for part in re.split(r",\s*", body.replace("\n", "").strip()):
            part = part.strip()
            if part == "None":
                entries.append(None)
            elif re.match(r"t\d+", part):
                entries.append(part)
        groups[name] = entries
    return groups


def generate_rust_code(
    tni_entries, cardinality_groups, inversion_defaults, chord_info, forte_mappings
):
    """Generate Rust code from parsed data structures"""
    rust_code = dedent(
        """\
    use lazy_static::lazy_static;
    use std::collections::HashMap;
    
    type TNIStructure = (Vec<u8>, Vec<u8>, Vec<u8>, u8);
    
    lazy_static! {
        pub(crate) static ref FORTE: Vec<Vec<Option<TNIStructure>>> = vec![
            vec![None], // 0-placeholder
    """
    )

    # Generate FORTE structure
    cardinality_order = [
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
        "dodecachord",
    ]

    for group_name in cardinality_order:
        entries = cardinality_groups[group_name]
        rust_code += f"        // Cardinality {len(entries)-1}\n"
        rust_code += "        vec![\n"
        for entry in entries:
            if entry is None:
                rust_code += "            None,\n"
            else:
                tni = tni_entries[entry]
                rust_code += (
                    f"            Some((vec!{tni['pcs']}, "
                    f"vec!{tni['icv']}, "
                    f"vec!{tni['inv_vec']}, "
                    f"{tni['z_rel']})), // {tni['forte_id']}\n"
                )
        rust_code += "        ],\n"

    rust_code += dedent(
        """\
        ];
    
        pub(crate) static ref INVERSION_DEFAULT_PC: HashMap<(u8, u8), Vec<u8>> = {
            let mut m = HashMap::new();
    """
    )

    # Generate inversion defaults
    for (card, forte_class), pcs in inversion_defaults.items():
        rust_code += f"        m.insert(({card}, {forte_class}), vec!{pcs});\n"

    rust_code += dedent(
        """\
            m
        };
    
        pub(crate) static ref TN_INDEX_TO_CHORD_INFO: HashMap<(u8, u8, i8), HashMap<String, Vec<String>>> = {
            let mut m = HashMap::new();
    """
    )

    # Generate chord info
    for key, info in chord_info.items():
        card, index, inv = key
        names = info.get("name", [])
        rust_code += f"        m.insert(({card}, {index}, {inv}), {{\n"
        rust_code += "            let mut entry = HashMap::new();\n"
        rust_code += f'            entry.insert("name".to_string(), vec!['
        rust_code += ", ".join(
            [f'"{name}"'.replace("(", "").replace(")", "") for name in names]
        )
        rust_code += "]);\n"
        rust_code += "            entry\n        });\n"

    rust_code += dedent(
        """\
            m
        };
    
        pub(crate) static ref FORTE_NUMBER_TO_TN_INDEX: HashMap<(u8, u8, i8), u16> = {
            let mut m = HashMap::new();
    """
    )

    # Generate forte mappings
    for key, value in forte_mappings.items():
        card, index, inv = key
        rust_code += f"        m.insert(({card}, {index}, {inv}), {value});\n"

    rust_code += dedent(
        """\
            m
        };
    }
    """
    )

    return rust_code


# Example usage (you'll need to pass the actual Python code string)
# read contents from first arg file
with open(sys.argv[1], "r") as file:
    py_code = file.read()

# Parse data from Python code
tni_entries = parse_tni_structures(py_code)
cardinality_groups = parse_cardinality_groups(py_code)
# You'll need to add similar parsers for inversion_defaults, chord_info, and forte_mappings
# Generate Rust code
rust_code = generate_rust_code(tni_entries, cardinality_groups, {}, {}, {})

with open(sys.argv[2], "w") as file:
    file.write(rust_code)
