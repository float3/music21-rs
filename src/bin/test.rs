//! Chord-table smoke-check binary used during local development.

use music21_rs::{IntegerType, chord::Chord};

struct StrCase {
    notes: &'static str,
    // Hardcoded from Python music21 reference behavior.
    expected: &'static str,
}

struct IntCase {
    notes: &'static [IntegerType],
    // Hardcoded from Python music21 reference behavior.
    expected: &'static str,
}

fn run_string_cases() {
    let cases = [
        StrCase {
            notes: "C E G",
            expected: "C-major triad",
        },
        StrCase {
            notes: "C E- G",
            expected: "C-minor triad",
        },
        StrCase {
            notes: "C E G B-",
            expected: "C-dominant seventh chord",
        },
        StrCase {
            notes: "C E G B",
            expected: "C-major seventh chord",
        },
        StrCase {
            notes: "C E- G B-",
            expected: "C-minor seventh chord",
        },
        StrCase {
            notes: "C E- G- B-",
            expected: "C-half-diminished seventh chord",
        },
        StrCase {
            notes: "C E- G- B--",
            expected: "C-diminished seventh chord",
        },
        StrCase {
            notes: "C E G B- D",
            expected: "C-dominant-ninth",
        },
        StrCase {
            notes: "C E G B D",
            expected: "C-major-ninth chord",
        },
        StrCase {
            notes: "C E- G B- D",
            expected: "C-minor-ninth chord",
        },
        StrCase {
            notes: "G2 B2 D3 F3",
            expected: "G-dominant seventh chord",
        },
        StrCase {
            notes: "B2 D3 F3 A3",
            expected: "B-half-diminished seventh chord",
        },
    ];

    for case in cases {
        let chord = Chord::new(case.notes)
            .unwrap_or_else(|e| panic!("failed to construct {:?}: {e}", case.notes));
        let got = chord.pitched_common_name();
        assert_eq!(
            got, case.expected,
            "Mismatch for {:?}: expected {:?}, got {:?}",
            case.notes, case.expected, got
        );
        println!("PASS {:<22} -> {}", case.notes, got);
    }
}

fn run_integer_cases() {
    let cases = [IntCase {
        notes: &[1, 2, 3, 4, 5, 10],
        expected: "forte class 6-36B above C#",
    }];

    for case in cases {
        let chord = Chord::new(case.notes)
            .unwrap_or_else(|e| panic!("failed to construct {:?}: {e}", case.notes));
        let got = chord.pitched_common_name();
        assert_eq!(
            got, case.expected,
            "Mismatch for {:?}: expected {:?}, got {:?}",
            case.notes, case.expected, got
        );
        println!("PASS {:?} -> {}", case.notes, got);
    }
}

fn main() {
    run_string_cases();
    run_integer_cases();
    println!("All chord checks passed.");
}
