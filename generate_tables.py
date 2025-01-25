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
    # Generate code for Vec<Vec<Option<TNIStructure>>>
    rust_code = "pub(super) static FORTE: LazyLock<Vec<Vec<Option<TNIStructure>>>> = LazyLock::new(|| {vec!["

    for card in range(len(tables.FORTE)):
        # Add a comment for each cardinality
        if Comments:
            rust_code += f"\n// Cardinality {card} {CARDINALITIES[card]}\n"

        if card == 0:
            # For the 0th element, use an empty vector
            rust_code += "vec![],"
            continue

        card_data = tables.FORTE[card]
        rust_code += "vec!["

        for i, entry in enumerate(card_data):
            if entry is None:
                # Use None for entries that are not present
                rust_code += "None,"
                if Comments:
                    rust_code += f"\n// Index {i} unused\n"
            else:
                entry: tables.TNIStructure
                pcs, icv, iv, z_relation = entry
                pcs_vec = [False] * 12
                for i in pcs:
                    pcs_vec[i] = True
                pcs_vec = f"{list(pcs_vec)}".replace("True", "true").replace(
                    "False", "false"
                )
                icv_vec = f"{list(icv)}"
                iv_vec = f"{list(iv)}"
                z_rel = "None" if z_relation is None else f"{z_relation}"
                rust_code += f"Some(({pcs_vec}, {icv_vec}, {iv_vec}, {z_rel})),"
        rust_code += "],"
    rust_code += "]});"

    return rust_code


def generate_inversion_default_pitch_class():
    rust_code = "pub(super) static INVERSION_DEFAULT_PITCH_CLASSES: LazyLock<HashMap<(u8, u8), PitchClasses>> = LazyLock::new(|| {"

    rust_code += "let mut m = HashMap::new();"
    for card_forte, pcs in tables.inversionDefaultPitchClasses.items():
        card, forte = card_forte
        pcs_vec = [False] * 12
        for i in pcs:
            pcs_vec[i] = True
        pcs_vec = f"{list(pcs_vec)}".replace("True", "true").replace("False", "false")
        rust_code += f"m.insert(({card}, {forte}), {pcs_vec});"
    rust_code += "m"
    rust_code += "});"

    return rust_code


def generate_cardinality_to_chord_members():
    rust_code = "    pub(super) static CARDINALITY_TO_CHORD_MEMBERS_GENERATED: LazyLock<Vec<HashMap<U8SB, Pcivicv>>> = LazyLock::new(|| {\n"
    inner_vars = []

    for card in range(len(tables.FORTE)):
        # Generate let statement for this card
        if card == 0:
            rust_code += f"        let inner_{card} = HashMap::new();\n"
        else:
            rust_code += f"        let mut inner_{card} = HashMap::new();\n"
        inner_vars.append(f"inner_{card}")

        # Check if we need to skip processing entries for this card
        if NoneMode is False and card == 0:
            if Comments:
                rust_code += f"        // Skipping cardinality {card} due to NoneMode\n"
            continue

        if Comments:
            rust_code += (
                f"        // Processing cardinality {card} {CARDINALITIES[card]}\n"
            )

        card_data = tables.FORTE[card]
        if card != 0:
            for forte_idx in range(1, len(card_data)):
                entry = card_data[forte_idx]
                if entry is None:
                    continue

                pcs, icv, inv_vec, z_rel = entry
                has_distinct = inv_vec[1] == 0  # Adjust condition if necessary

                pcs_vec = [False] * 12
                for i in pcs:
                    pcs_vec[i] = True
                pcs_vec = f"{list(pcs_vec)}".replace("True", "true").replace(
                    "False", "false"
                )

                # Insert original entry
                key = f"({forte_idx}, {"SuperBool::One" if has_distinct else "SuperBool::Zero"})"
                rust_code += f"        inner_{card}.insert({key}, ({pcs_vec}, {list(inv_vec)}, {list(icv)}));\n"

                if has_distinct:
                    # Insert inverted entry
                    inv_pcs = tables.inversionDefaultPitchClasses.get(
                        (card, forte_idx), []
                    )
                    inv_pcs_vec = [False] * 12
                    for i in inv_pcs_vec:
                        inv_pcs_vec[i] = True
                    inv_pcs_vec = f"{list(inv_pcs_vec)}".replace(
                        "True", "true"
                    ).replace("False", "false")
                    rust_code += f"        inner_{card}.insert(({forte_idx}, SuperBool::NegativeOne), ({inv_pcs_vec}, {list(inv_vec)}, {list(icv)}));\n"

    # Build the vec! with all inner variables
    rust_code += "        vec![\n"
    for var in inner_vars:
        rust_code += f"            {var},\n"
    rust_code += "        ]\n    });\n"

    return rust_code


def generate_maximum_index_number_without_inversion_equivalence():
    rust_code = "pub(super) static MAXIMUM_INDEX_NUMBER_WITHOUT_INVERSION_EQUIVALENCE: LazyLock<Vec<u8>> = LazyLock::new(|| vec!["
    for idx in range(0, len(tables.maximumIndexNumberWithoutInversionEquivalence)):
        rust_code += f"{tables.maximumIndexNumberWithoutInversionEquivalence[idx]}, "
    rust_code = rust_code.rstrip(", ")
    rust_code += "]);"
    return rust_code


def generate_maximum_index_number_with_inversion_equivalence():
    rust_code = "pub(super) static MAXIMUM_INDEX_NUMBER_WITH_INVERSION_EQUIVALENCE: LazyLock<Vec<u8>> = LazyLock::new(|| vec!["
    for idx in range(0, len(tables.maximumIndexNumberWithInversionEquivalence)):
        rust_code += f"{tables.maximumIndexNumberWithInversionEquivalence[idx]}, "
    rust_code = rust_code.rstrip(", ")
    rust_code += "]);"
    return rust_code


def generate_forte_number_with_inversion_to_tn_index():
    rust_code = "pub(super) static FORTE_NUMBER_WITH_INVERSION_TO_INDEX: LazyLock<HashMap<U8U8SB, u8>> = LazyLock::new(|| {"
    rust_code += "let mut m = HashMap::new();"
    for key, i in tables.forteNumberWithInversionToTnIndex.items():
        card, idx, inv = key
        if inv == -1:
            inv = "SuperBool::NegativeOne"
        elif inv == 0:
            inv = "SuperBool::Zero"
        elif inv == 1:
            inv = "SuperBool::One"
        else:
            print(inv)
        rust_code += f"m.insert(({card}, {idx}, {inv}), {i});"
    rust_code += "m"
    rust_code += "});"
    return rust_code


def generate_tn_index_to_chord_info():
    if NoneMode:
        rust_code = "pub(super) static TN_INDEX_TO_CHORD_INFO: LazyLock<HashMap<U8U8SB, Option<Vec<&'static str>>>> = LazyLock::new(|| {"
    else:
        rust_code = "pub(super) static TN_INDEX_TO_CHORD_INFO: LazyLock<HashMap<U8U8SB, Vec<&'static str>>> = LazyLock::new(|| {"

    rust_code += "let mut m = HashMap::new();"
    for key, info in tables.tnIndexToChordInfo.items():
        card, idx, inv = key
        if inv == -1:
            inv = "SuperBool::NegativeOne"
        elif inv == 0:
            inv = "SuperBool::Zero"
        elif inv == 1:
            inv = "SuperBool::One"
        else:
            print(inv)
        names = info.get("name", [])
        if names:
            names_str = ", ".join(f'"{n}"' for n in names)
            if NoneMode:
                rust_code += (
                    f"m.insert(({card}, {idx}, {inv}),Some(vec![{names_str}]));"
                )
            else:
                rust_code += f"m.insert(({card},{idx},{inv}),vec![{names_str}]);"
        elif NoneMode:
            rust_code += f"m.insert(({card},{idx},{inv}),None);"
    rust_code += "m"
    rust_code += "});"
    return rust_code


def generate_rust_tables():
    rust_code = """
/*
This file is autogenerated from the tables in the original music21 library
*/  
use super::{Pcivicv, PitchClasses, SuperBool, TNIStructure, U8SB, U8U8SB};
use std::{collections::HashMap, sync::LazyLock};

"""
    rust_code += generate_forte_table()
    rust_code += generate_inversion_default_pitch_class()
    rust_code += generate_cardinality_to_chord_members()
    rust_code += generate_forte_number_with_inversion_to_tn_index()
    rust_code += generate_tn_index_to_chord_info()
    rust_code += generate_maximum_index_number_without_inversion_equivalence()
    rust_code += generate_maximum_index_number_with_inversion_equivalence()
    rust_code += "\n"
    return rust_code


NoneMode: bool = False
Comments: bool = False


def main():
    parser = argparse.ArgumentParser(description="Generate Rust chord tables.")
    parser.add_argument(
        "--NoneMode",
        "-n",
        default=True,
        action="store_true",
        help="Enable NoneMode functionality",
    )
    parser.add_argument(
        "--output",
        "-o",
        type=str,
        default="src/chord/tables/generated.rs",
        help="Output file path",
    )
    parser.add_argument(
        "--Comments",
        "-c",
        default=False,
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
            f.write(rust)
            f.truncate()

        print("Rust tables generated successfully.")

    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
