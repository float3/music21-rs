#!/usr/bin/env python3
import argparse
import sys
from pathlib import Path

sys.path.append("./music21")
from music21.chord import tables

# TODO: generate both NoneMode and non-NoneMode tables and have a rust feature flag to switch between HashMaps and Vectors
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
    if NoneMode:
        # Generate code for Vec<Vec<Option<TNIStructure>>>
        rust_code = (
            "    pub(crate) static ref FORTE: Vec<Vec<Option<TNIStructure>>> = vec![\n"
        )

        for card in range(len(tables.FORTE)):
            # Add a comment for each cardinality
            if Comments:
                rust_code += f"        // Cardinality {card} {CARDINALITIES[card]}\n"

            if card == 0:
                # For the 0th element, use an empty vector
                rust_code += "        vec![],\n"
                continue

            card_data = tables.FORTE[card]
            rust_code += "        vec![\n"

            for i, entry in enumerate(card_data):
                if entry is None:
                    # Use None for entries that are not present
                    rust_code += "            None,"
                    if Comments:
                        rust_code += f"// Index {i} unused"
                else:
                    pcs, icv, inv_vec, z_relation = entry
                    pcs_vec = f"vec!{list(pcs)}"
                    icv_vec = f"vec!{list(icv)}"
                    inv_vec_vec = f"vec!{list(inv_vec)}"
                    z_rel = "None" if z_relation is None else f"{z_relation}"
                    rust_code += f"\n            Some(({pcs_vec}, {icv_vec}, {inv_vec_vec}, {z_rel})),\n"

            rust_code += "        ],\n"
        rust_code += "    ];\n"

    else:
        # Generate code for Vec<HashMap<TNIStructure>>
        rust_code = "    pub(crate) static ref FORTE: HashMap<u8, HashMap<u8, TNIStructure>> = {"
        rust_code += "\n        let mut outer = HashMap::new();\n"

        for card in range(len(tables.FORTE)):
            if Comments:
                rust_code += f"\n        // Cardinality {card} {CARDINALITIES[card]}\n"

            if card == 0:
                # For the 0th element, skip
                continue

            card_data = tables.FORTE[card]
            rust_code += f"\n        let mut inner_{card} = HashMap::new();\n"

            for i, entry in enumerate(card_data):
                if entry is None:
                    # Skip entries that are not present
                    continue
                else:
                    pcs, icv, inv_vec, z_relation = entry
                    # Construct the TNIStructure key
                    pcs_vec = f"vec!{list(pcs)}"
                    icv_vec = f"vec!{list(icv)}"
                    inv_vec_vec = f"vec!{list(inv_vec)}"
                    z_rel = "None" if z_relation is None else f"{z_relation}"
                    # Define the key (TNIStructure) and value (YourValueType)
                    value = f"({pcs_vec}, {icv_vec}, {inv_vec_vec}, {z_rel})"
                    key = f"{i}"
                    rust_code += f"        inner_{card}.insert({key}, {value});\n"

            rust_code += f"        outer.insert({card},inner_{card});\n"
        rust_code += "        outer\n"
        rust_code += "    };\n"

    return rust_code


def generate_inversion_default_pitch_class():
    rust_code = "\n    pub(crate) static ref INVERSION_DEFAULT_PITCH_CLASSES: HashMap<(u8, u8), Vec<u8>> = {"

    rust_code += "\n        let mut m = HashMap::new();\n"
    for card_forte, pcs in tables.inversionDefaultPitchClasses.items():
        card, forte = card_forte
        rust_code += f"        m.insert(({card}, {forte}), vec!{list(pcs)});\n"
    rust_code += "        m\n"
    rust_code += "    };\n"

    return rust_code


def generate_cardinality_to_chord_members():
    rust_code = "\n    pub(crate) static ref CARDINALITY_TO_CHORD_MEMBERS: CardinalityToChordMembers = {"

    rust_code += "\n        let mut outer = HashMap::new();\n"

    for card in range(len(tables.FORTE)):
        if NoneMode == False and card == 0:
            continue
        if Comments:
            rust_code += f"        // Cardinality {card} {CARDINALITIES[card]}\n"
        if card == 0:
            rust_code += f"        let inner_{card} = HashMap::new();\n"
        else:
            rust_code += f"        let mut inner_{card} = HashMap::new();\n"

        card_data = tables.FORTE[card]
        if card != 0:
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
                    inv_pcs = tables.inversionDefaultPitchClasses.get(
                        (card, forte_idx), []
                    )
                    rust_code += f"        inner_{card}.insert(({forte_idx}, -1), (vec!{list(inv_pcs)}, vec!{list(inv_vec)}, vec!{list(icv)}));\n"

        rust_code += f"        outer.insert({card}, inner_{card});\n"

    rust_code += "        outer\n"
    rust_code += "    };\n"
    return rust_code


def generate_maximum_index_number_without_inversion_equivalence():
    rust_code = "\n    pub(crate) static MAXIMUM_INDEX_NUMBER_WITHOUT_INVERSION_EQUIVALENCE: Vec<u8> = vec!["
    for idx in range(0, len(tables.maximumIndexNumberWithoutInversionEquivalence)):
        rust_code += f"{tables.maximumIndexNumberWithoutInversionEquivalence[idx]}, "
    rust_code = rust_code.rstrip(", ")  # Remove the trailing comma and space
    rust_code += "];\n"
    return rust_code


def generate_maximum_index_number_with_inversion_equivalence():
    rust_code = "\n\n    pub(crate) static MAXIMUM_INDEX_NUMBER_WITH_INVERSION_EQUIVALENCE: Vec<u8> = vec!["
    for idx in range(0, len(tables.maximumIndexNumberWithInversionEquivalence)):
        rust_code += f"{tables.maximumIndexNumberWithInversionEquivalence[idx]}, "
    rust_code = rust_code.rstrip(", ")  # Remove the trailing comma and space
    rust_code += "];\n"
    return rust_code


def generate_forte_number_with_inversion_to_tn_index():
    rust_code = "\n\n    pub(crate) static ref FORTE_NUMBER_WITH_INVERSION_TO_INDEX: HashMap<(u8, u8, i8), u8> = {"
    rust_code += "\n        let mut m = HashMap::new();\n"
    for key, i in tables.forteNumberWithInversionToTnIndex.items():
        card, idx, inv = key
        rust_code += f"        m.insert(({card}, {idx}, {inv}), {i});\n"
    rust_code += "        m\n"
    rust_code += "    };\n"
    return rust_code


def generate_tn_index_to_chord_info():
    if NoneMode:
        rust_code = "\n    pub(crate) static ref TN_INDEX_TO_CHORD_INFO: HashMap<(u8, u8, i8), Option<Vec<&'static str>>> = {"
    else:
        rust_code = "\n    pub(crate) static ref TN_INDEX_TO_CHORD_INFO: HashMap<(u8, u8, i8), Vec<&'static str>> = {"

    rust_code += "\n        let mut m = HashMap::new();\n"
    for key, info in tables.tnIndexToChordInfo.items():
        card, idx, inv = key
        names = info.get("name", [])
        if names:
            names_str = ", ".join(f'"{n}"' for n in names)
            if NoneMode:
                rust_code += f"        m.insert(({card}, {idx}, {inv}), Some(vec![{names_str}]));\n"
            else:
                rust_code += (
                    f"        m.insert(({card}, {idx}, {inv}), vec![{names_str}]);\n"
                )
        elif NoneMode:
            rust_code += f"        m.insert(({card}, {idx}, {inv}), None);\n"
    rust_code += "        m\n"
    rust_code += "    };\n"
    return rust_code


def generate_rust_tables():
    rust_code = "lazy_static! {\n"
    rust_code += generate_forte_table()
    rust_code += generate_inversion_default_pitch_class()
    # rust_code += generate_cardinality_to_chord_members()
    rust_code += generate_forte_number_with_inversion_to_tn_index()
    rust_code += generate_tn_index_to_chord_info()
    rust_code += "}\n"
    rust_code += generate_maximum_index_number_without_inversion_equivalence()
    rust_code += generate_maximum_index_number_with_inversion_equivalence()
    return rust_code


NoneMode: bool = False
Comments: bool = False


def main():
    parser = argparse.ArgumentParser(description="Generate Rust chord tables.")
    parser.add_argument(
        "--NoneMode",
        "-n",
        action="store_true",
        help="Enable NoneMode functionality",
    )
    parser.add_argument(
        "--output",
        "-o",
        type=str,
        default="src/chord/tables.rs",
        help="Output file path",
    )
    parser.add_argument(
        "--Comments",
        "-c",
        action="store_true",
        help="Enable comments in the generated code",
    )
    args = parser.parse_args()

    global NoneMode
    global Comments
    NoneMode = args.NoneMode
    Comments = args.Comments

    rust = generate_rust_tables()

    try:
        file_path = Path(args.output)
        if not file_path.exists():
            raise FileNotFoundError(f"File {file_path} does not exist.")

        with file_path.open("r+") as f:
            content = f.read()
            start = content.find("// BEGIN_GENERATED_CODE") + len(
                "// BEGIN_GENERATED_CODE\n"
            )
            end = content.find("// END_GENERATED_CODE")

            if start == -1 or end == -1:
                raise ValueError(
                    "Missing markers for generated code in the target file."
                )

            new_content = content[:start] + rust + content[end:]
            f.seek(0)
            f.write(new_content)
            f.truncate()

        print("Rust tables generated successfully.")

    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
