//! music21's `variant.py` doctests against the crate. See
//! `src/doctest.rs`. The merging and the fixing are the crate's
//! `variant`, which `variant_parity` holds to music21, and the streams
//! they answer are music21's own.
//!
//! `Variant` itself stays music21's: its members are readings of, or
//! delegations to, the stream it holds, so a facade for it would carry
//! almost nothing of the crate. What is swapped is the module functions,
//! which is where the crate's work is; the private helpers inside
//! music21's merge walk stay its own, and so do their examples.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    variant_doctests_against_the_crate,
    "music21.variant",
    "variant",
    [
        ("music21.pitch", doctest::PITCH_NAMES),
        ("music21.interval", doctest::INTERVAL_NAMES),
        ("music21.note", doctest::NOTE_NAMES),
        ("music21.duration", doctest::DURATION_NAMES),
        ("music21.chord", doctest::CHORD_NAMES),
        ("music21.tie", doctest::TIE_NAMES),
        ("music21.volume", doctest::VOLUME_NAMES),
        ("music21.beam", doctest::BEAM_NAMES),
        ("music21.tempo", doctest::TEMPO_NAMES),
        ("music21.clef", doctest::CLEF_NAMES),
        ("music21.key", doctest::KEY_NAMES),
        ("music21.meter.base", doctest::METER_NAMES),
        ("music21.meter.core", doctest::METER_CORE_NAMES),
        ("music21.instrument", doctest::INSTRUMENT_NAMES),
        ("music21.dynamics", doctest::DYNAMICS_NAMES),
        ("music21.expressions", doctest::EXPRESSION_NAMES),
        ("music21.articulations", doctest::ARTICULATION_NAMES),
        ("music21.bar", doctest::BAR_NAMES),
        ("music21.variant", doctest::VARIANT_NAMES),
    ]
);
