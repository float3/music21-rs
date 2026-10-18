//! music21's `layout.py` doctests against the crate. See `src/doctest.rs`.
//! The measurements are the crate's `layout` values, each written back the
//! way music21 writes it: `120` stays an `int` and `120.5` a `float`.
//!
//! `LayoutScore`, `Page`, `System` and `Staff` stay music21's -- they are
//! streams it builds a picture of a score out of, holding the page rather
//! than the music -- so their docstrings run music21's own code over these
//! layouts, which is the harder test of them and what the nested
//! `pageLayout`, `systemLayout` and `staffLayoutList` are for.
//!
//! The one example left failing is a `repr`: music21 keeps a staff size as
//! the number it was given, so `StaffLayout(staffSize=80)` says `80` and
//! `staffSize=113.0` says `113.0`, where the crate's `staff_size` is a
//! float and says `80.0` for both. `layout::Tenths` is the type that keeps
//! that difference, and giving the size one is a change to a public field,
//! so it waits for a minor version rather than riding along here.

use music21_rs_python_parity::doctest;

music21_rs_python_parity::doctest_suite!(
    layout_doctests_against_the_crate,
    "music21.layout",
    "layout",
    [
        ("music21.pitch", doctest::PITCH_NAMES),
        ("music21.interval", doctest::INTERVAL_NAMES),
        ("music21.note", doctest::NOTE_NAMES),
        ("music21.duration", doctest::DURATION_NAMES),
        ("music21.chord", doctest::CHORD_NAMES),
        ("music21.tie", doctest::TIE_NAMES),
        ("music21.volume", doctest::VOLUME_NAMES),
        ("music21.beam", doctest::BEAM_NAMES),
        ("music21.layout", doctest::LAYOUT_NAMES),
    ]
);
