#!/usr/bin/env python3
import sys

sys.path.append("./music21")
from music21.chord import tables

CARDINALITIES = [
    "None",
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
]


def rustify_value(value):
    """Convert Python values to Rust equivalents with proper type handling"""
    if value is None:
        return "None"
    if isinstance(value, (list, tuple)):
        return f"vec!{list(value)}"
    if isinstance(value, dict):
        return "HashMap::from_iter(" + str([(k, v) for k, v in value.items()]) + ")"
    return str(value)


def generate_forte_table():
    rust_code = f"""
    pub(crate) static ref FORTE: Vec<Vec<Option<TNIStructure>>> = vec![
        // Index 0 is unused (Cardinality 0 {CARDINALITIES[0]})
        vec![],
    """

    for card in range(1, len(tables.FORTE)):
        card_data = tables.FORTE[card]
        if card == 1:
            rust_code += f"    // Cardinality {card} {CARDINALITIES[card]}\n"
        else:
            rust_code += f"        // Cardinality {card} {CARDINALITIES[card]}\n"
        rust_code += "        vec![\n"
        rust_code += "            None, // Index 0 unused\n"

        for i in range(1, len(card_data)):
            entry = card_data[i]
            if entry is None:
                rust_code += "            None,\n"
            else:
                pcs, icv, inv_vec, z_relation = entry
                pcs_vec = f"vec!{list(pcs)}"
                icv_vec = f"vec!{list(icv)}"
                inv_vec_vec = f"vec!{list(inv_vec)}"
                z_rel = "None" if z_relation is None else f"{z_relation}"
                rust_code += f"            Some(({pcs_vec}, {icv_vec}, {inv_vec_vec}, {z_rel})),\n"

        rust_code += "        ],\n"

    rust_code += "    ];\n"

    return rust_code


def generate_inversion_default_pitch_class():
    rust_code = "\n    pub(crate) static ref INVERSION_DEFAULT_PITCH_CLASSES: HashMap<(u8, u8), Vec<u8>> = {"

    rust_code += "        let mut m = HashMap::new();\n"
    for card_forte, pcs in tables.inversionDefaultPitchClasses.items():
        card, forte = card_forte
        rust_code += f"        m.insert(({card}, {forte}), vec!{list(pcs)});\n"
    rust_code += "        m\n"
    rust_code += "    };\n"

    return rust_code


def generate_cardinality_to_chord_members_rust():
    rust_code = "\n    pub(crate) static ref CARDINALITY_TO_CHORD_MEMBERS: HashMap<u8, HashMap<(u8, i8), (Vec<u8>, Vec<u8>, Vec<u8>)>> = {"

    rust_code += "\n        let mut outer = HashMap::new();\n"

    rust_code += f"        // Cardinality 0 {CARDINALITIES[0]}\n"

    rust_code += "        let inner_0 = HashMap::new();\n"
    rust_code += "        outer.insert(0, inner_0);\n"

    for card in range(1, len(tables.FORTE)):
        rust_code += f"        // Cardinality {card} {CARDINALITIES[card]}\n"
        rust_code += f"        let mut inner_{card} = HashMap::new();\n"

        card_data = tables.FORTE[card]
        for forte_idx in range(1, len(card_data)):
            entry = card_data[forte_idx]
            if entry is None:
                continue

            pcs, icv, inv_vec, z_rel = entry
            has_distinct = inv_vec[1] == 0

            # Original entry
            key = (forte_idx, 1 if has_distinct else 0)
            rust_code += f"        inner_{card}.insert({key}, (vec!{list(pcs)}, vec!{list(inv_vec)}, vec!{list(icv)}));\n"

            if has_distinct:
                # Inverted entry
                inv_pcs = tables.inversionDefaultPitchClasses.get((card, forte_idx), [])
                rust_code += f"        inner_{card}.insert(({forte_idx}, -1), (vec!{list(inv_pcs)}, vec!{list(inv_vec)}, vec!{list(icv)}));\n"

        rust_code += f"        outer.insert({card}, inner_{card});\n"

    rust_code += "        outer\n"
    rust_code += "    };\n"
    return rust_code


def generate_maximum_index_number_without_inversion_equivalence():
    rust_code = "\n    pub(crate) static ref MAXIMUM_INDEX_NUMBER_WITHOUT_INVERSION_EQUIVALENCE: HashMap<u8, u8> = {"
    rust_code += "        let mut m = HashMap::new();\n"
    for idx in range(0, len(tables.maximumIndexNumberWithoutInversionEquivalence)):
        rust_code += f"        m.insert({idx}, {tables.maximumIndexNumberWithoutInversionEquivalence[idx]});\n"
    rust_code += "        m\n"
    rust_code += "    };\n"
    return rust_code


def generate_maximum_index_number_with_inversion_equivalence():
    rust_code = "\n    pub(crate) static ref MAXIMUM_INDEX_NUMBER_WITH_INVERSION_EQUIVALENCE: HashMap<u8, u8> = {"
    rust_code += "\n        let mut m = HashMap::new();\n"
    for idx in range(0, len(tables.maximumIndexNumberWithInversionEquivalence)):
        rust_code += f"        m.insert({idx}, {tables.maximumIndexNumberWithInversionEquivalence[idx]});\n"
    rust_code += "        m\n"
    rust_code += "    };\n"
    return rust_code


def generate_forte_number_with_inversion_to_tn_index():
    rust_code = "\n    pub(crate) static ref FORTE_NUMBER_WITH_INVERSION_TO_INDEX: HashMap<(u8, u8, i8), u8> = {"
    rust_code += "\n        let mut m = HashMap::new();\n"
    for key, i in tables.forteNumberWithInversionToTnIndex.items():
        card, idx, inv = key
        rust_code += f"        m.insert(({card}, {idx}, {inv}), {i});\n"
    rust_code += "        m\n"
    rust_code += "    };\n"
    return rust_code


def generate_tn_index_to_chord_info():
    rust_code = "\n    pub(crate) static ref TN_INDEX_TO_CHORD_INFO: HashMap<(u8, u8, i8), Option<Vec<&'static str>>> = {"

    rust_code += "\n        let mut m = HashMap::new();\n"
    for key, info in tables.tnIndexToChordInfo.items():
        card, idx, inv = key
        names = info.get("name", [])
        if names:
            names_str = ", ".join(f'"{n}"' for n in names)
            rust_code += (
                f"        m.insert(({card}, {idx}, {inv}), Some(vec![{names_str}]));\n"
            )
        else:
            rust_code += f"        m.insert(({card}, {idx}, {inv}), None);\n"
    rust_code += "        m\n"
    rust_code += "    };\n"
    return rust_code


def generate_rust_tables():
    rust_code = "lazy_static! {"
    rust_code += generate_forte_table()
    rust_code += generate_inversion_default_pitch_class()
    rust_code += generate_cardinality_to_chord_members_rust()
    rust_code += generate_maximum_index_number_without_inversion_equivalence()
    rust_code += generate_maximum_index_number_with_inversion_equivalence()
    rust_code += generate_forte_number_with_inversion_to_tn_index()
    rust_code += generate_tn_index_to_chord_info()
    rust_code += "}\n"
    return rust_code


if __name__ == "__main__":
    rust = generate_rust_tables()
    with open("src/chord/tables.rs", "r+") as f:
        content = f.read()
        start = content.find("// BEGIN_GENERATED_CODE") + len(
            "// BEGIN_GENERATED_CODE\n"
        )
        end = content.find("// END_GENERATED_CODE")
        new_content = content[:start] + rust + content[end:]
        f.seek(0)
        f.write(new_content)
        f.truncate()
