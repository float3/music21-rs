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
    rust_code = "static FORTE: LazyLock<Vec<Vec<Option<TNIStructure>>>> = LazyLock::new(|| {vec!["

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
                pcs, icv, inv_vec, z_relation = entry
                pcs_vec = f"vec!{list(pcs)}"
                icv_vec = f"{list(icv)}"
                inv_vec_vec = f"{list(inv_vec)}"
                z_rel = "None" if z_relation is None else f"{z_relation}"
                rust_code += f"Some(TNIStructure{{ pitch_classes:{pcs_vec}, interval_class_vector: {icv_vec}, invariance_vector: {inv_vec_vec}, z_relation: {z_rel}}}),"
        rust_code += "],"
    rust_code += "]});"

    return rust_code


def generate_inversion_default_pitch_class():
    rust_code = "static INVERSION_DEFAULT_PITCH_CLASSES: LazyLock<HashMap<(u8, u8), Vec<u8>>> = LazyLock::new(|| {"

    rust_code += "let mut m = HashMap::new();"
    for card_forte, pcs in tables.inversionDefaultPitchClasses.items():
        card, forte = card_forte
        rust_code += f"m.insert(({card}, {forte}), vec!{list(pcs)});"
    rust_code += "m"
    rust_code += "});"

    return rust_code


# def generate_cardinality_to_chord_members():
#     rust_code = (
#         "    static ref CARDINALITY_TO_CHORD_MEMBERS: CardinalityToChordMembers = {"
#     )

#     rust_code += "        let mut outer = HashMap::new();"

#     for card in range(len(tables.FORTE)):
#         if NoneMode == False and card == 0:
#             continue
#         if Comments:
#             rust_code += f"        // Cardinality {card} {CARDINALITIES[card]}"
#         if card == 0:
#             rust_code += f"        let inner_{card} = HashMap::new();"
#         else:
#             rust_code += f"        let mut inner_{card} = HashMap::new();"

#         card_data = tables.FORTE[card]
#         if card != 0:
#             for forte_idx in range(1, len(card_data)):
#                 entry = card_data[forte_idx]
#                 if entry is None:
#                     continue

#                 pcs, icv, inv_vec, z_rel = entry
#                 has_distinct = inv_vec[1] == 0

#                 # Original entry
#                 key = (forte_idx, 1 if has_distinct else 0)
#                 rust_code += f"        inner_{card}.insert({key}, (vec!{list(pcs)}, vec!{list(inv_vec)}, vec!{list(icv)}));"

#                 if has_distinct:
#                     # Inverted entry
#                     inv_pcs = tables.inversionDefaultPitchClasses.get(
#                         (card, forte_idx), []
#                     )
#                     rust_code += f"        inner_{card}.insert(({forte_idx}, -1), (vec!{list(inv_pcs)}, vec!{list(inv_vec)}, vec!{list(icv)}));"

#         rust_code += f"        outer.insert({card}, inner_{card});"

#     rust_code += "        outer"
#     rust_code += "    };"
#     return rust_code


def generate_maximum_index_number_without_inversion_equivalence():
    rust_code = "static MAXIMUM_INDEX_NUMBER_WITHOUT_INVERSION_EQUIVALENCE: LazyLock<Vec<u8>> = LazyLock::new(|| vec!["
    for idx in range(0, len(tables.maximumIndexNumberWithoutInversionEquivalence)):
        rust_code += f"{tables.maximumIndexNumberWithoutInversionEquivalence[idx]}, "
    rust_code = rust_code.rstrip(", ")
    rust_code += "]);"
    return rust_code


def generate_maximum_index_number_with_inversion_equivalence():
    rust_code = "static MAXIMUM_INDEX_NUMBER_WITH_INVERSION_EQUIVALENCE: LazyLock<Vec<u8>> = LazyLock::new(|| vec!["
    for idx in range(0, len(tables.maximumIndexNumberWithInversionEquivalence)):
        rust_code += f"{tables.maximumIndexNumberWithInversionEquivalence[idx]}, "
    rust_code = rust_code.rstrip(", ")
    rust_code += "]);"
    return rust_code


def generate_forte_number_with_inversion_to_tn_index():
    rust_code = "static FORTE_NUMBER_WITH_INVERSION_TO_INDEX: LazyLock<HashMap<U8U8SB, u8>> = LazyLock::new(|| {"
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
        rust_code = "static TN_INDEX_TO_CHORD_INFO: LazyLock<HashMap<U8U8SB, Option<Vec<&'static str>>>> = LazyLock::new(|| {"
    else:
        rust_code = "static TN_INDEX_TO_CHORD_INFO: LazyLock<HashMap<U8U8SB, Vec<&'static str>>> = LazyLock::new(|| {"

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
    rust_code = generate_forte_table()
    rust_code += generate_inversion_default_pitch_class()
    # rust_code += generate_cardinality_to_chord_members()
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
        default="src/chord/tables.rs",
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
