# Unreleased

Cleanup of the Python-shaped plumbing that remained in the pitch, note and
chord constructors, and a round of music21 features the crate had not
modelled: accidental display state, non-traditional key signatures, the
mutating half of `Chord`, and the interval halves as public types. Several
signatures change, so the next release needs a minor bump.

## Added

- `Interval` gains music21's compact names (`short_name`, `simple_name`,
  `semi_simple_name`, `directed_name`), the `is_step`, `is_diatonic_step`,
  `is_chromatic_step`, `is_skip` and `is_consonant` predicates,
  `complement`, `interval_class`, and `Interval::sum` / `Interval::difference`
  for `interval.add` and `interval.subtract`.
- The halves of an interval are public: `GenericInterval`, `DiatonicInterval`,
  `ChromaticInterval` and `Specifier`, with music21's full method set —
  `simple_directed`, `semi_simple_undirected`, `octaves`, `staff_distance`,
  `mod7`, `mod7_inversion`, `complement`, `reverse`, `nice_name` and the
  directed and simple name variants, `get_diatonic`/`get_chromatic`,
  `cents`, `interval_class`, and `DiatonicInterval::try_new`, which rejects
  a "Major Fifth" or a descending perfect unison the way music21 does.
  `Interval` exposes them as `generic()`, `diatonic()`, `chromatic()` and
  `specifier()`, and can be built from either half with `from_diatonic` and
  `from_chromatic`.
- `GenericInterval::transpose_pitch` is music21's generic transposition —
  it moves the staff position and keeps the accidental, so a third above
  `C#4` is `E#4` — and `transpose_pitch_key_aware` carries a pitch's offset
  from a key signature across, so `F` natural in G major steps up to `G-`.
  `GenericInterval::from_name` reads `"Third"`, `"3rd"` and
  `"Descending Fifth"`.
- Accidentals carry music21's display state: `Pitch::update_accidental_display`
  decides whether an accidental should be shown from the pitches before it,
  the key signature's altered pitches and the cautionary switches
  (`AccidentalDisplayOptions`), adding a cautionary natural where music21
  adds one. `Accidental::set_attribute_independently` writes one part of an
  accidental without the other two following, which is how a `sori` is built.
- A pitch now knows whether it carries an accidental object at all:
  `Pitch::has_accidental`, `explicit_accidental`, `explicit_accidental_mut`
  and `set_accidental`. music21 keeps no accidental on a bare `D` but does
  keep an explicit `Dn`, and the two are no longer equal.
  `Pitch::set_accidental_alter` splits a fractional alteration into an
  accidental and a microtone, and `set_microtone_cents` sets or clears the
  microtone. `Pitch::name_in_key_signature` and `step_in_key_signature` are
  music21's key-signature helpers.
- Non-traditional key signatures: `KeySignature::from_altered_pitches` takes
  the pitches a signature alters where no count of sharps can express it
  (`E-` with `G#`), with `is_non_traditional`, `set_altered_pitches`,
  `set_sharps`, `accidentals_apply_only_to_octave` and a `Display` that
  writes music21's `KeySignature of pitches: [E-, G#]`.
- `Chord` gains music21's mutating half: `add`, `remove`, `remove_named`,
  `set_root`, `set_bass` and `set_inversion`, with `found_root`,
  `found_bass`, `overridden_root` and `overridden_bass` to tell an inferred
  answer from one a caller fixed. `chord_step_with_root`,
  `semitones_from_chord_step_with_root` and `inversion_from_root` are
  music21's `testRoot` forms, and `note_pitch_classes` is music21's
  `pitchClasses`, in chord order with repeats kept.
- `format_vector_string` writes a pitch-class list the way music21's
  `Chord.formatVectorString` does, with ten and eleven as `A` and `B`.
- Notes and chords carry the notation music21 puts on them, in two new
  modules. `notation` holds `Tie` (with `TieType`, `TieStyle` and
  `Placement`), `Notehead`, `StemDirection` and `Lyric` (with `Syllabic`);
  `volume` holds `Volume`, which is a MIDI velocity and its 0-to-1 scalar
  together with music21's rule for reading them against the dynamics around
  them. `Note` gains `tie`, `notehead`, `notehead_fill`,
  `notehead_parenthesis`, `stem_direction`, `color`, `volume`, `lyrics`,
  `lyric` and `add_lyric`; `Accidental` gains `color`.
- `Chord` gains the same notation through its notes — `notes_mut`,
  `note_for_pitch`, `note_for_pitch_mut` — plus `tie`, `color`,
  `color_of_pitch`, `volume`, `set_volumes`, `has_component_volumes`,
  `lyrics` and `add_lyric`. `annotate_intervals` names the interval from the
  lowest pitch up to each of the others, and `annotated_with_intervals`
  writes those names on as lyrics.
- `Chord::string_harmonic` reads a written string harmonic: a chord whose
  second note has a diamond head comes back with the pitch it actually
  sounds added on top. This is music21's `Pitch.getStringHarmonic`, which
  was previously left out because it depends on noteheads.
- `Chord::remove_redundant_pitches_reporting` and its name and pitch-class
  variants hand back the pitches they dropped, as music21's do in place.
- `Accidental::set_attribute_independently` takes an `AccidentalAttribute`,
  so only the three parts music21 allows can be named.
- The module functions `notes_to_generic`, `notes_to_chromatic`,
  `intervals_to_diatonic`, `convert_diatonic_number_to_step`,
  `convert_semitone_to_specifier_generic`,
  `convert_semitone_to_specifier_generic_microtone`, `convert_generic` and
  `parse_specifier`, and `Interval::transpose_pitch_with_options` with
  music21's `reverse` and `maxAccidental` arguments, are public.
- `Chord` gains `root`, `bass`, `chord_step`, `third`, `fifth`, `seventh`,
  `semitones_from_chord_step`, `has_repeated_chord_step`,
  `has_any_enharmonic_spelled_pitches`, the `is_triad` / `is_major_triad` /
  `is_minor_triad` / `is_diminished_triad` / `is_augmented_triad` /
  `is_seventh` / `is_dominant_seventh` / `is_half_diminished_seventh` /
  `is_diminished_seventh` / `is_incomplete_major_triad` /
  `is_incomplete_minor_triad` / `contains_triad` / `contains_seventh` /
  `is_consonant` predicates, `quality` returning the new `TriadQuality`, and
  `closed_position`, `remove_redundant_pitches`,
  `remove_redundant_pitch_names`, `remove_redundant_pitch_classes` and
  `sort_ascending`.
- `voiceleading::VoiceLeadingQuartet` and `MotionType`, a port of music21's
  two-voice motion classification with parallel and hidden fifth and octave
  checks, voice crossing and voice overlap.
- `analysis::KeyProfile` with the five key-finding weight sets music21 ships
  (Krumhansl-Schmuckler, Aarden-Essen, Simple, Bellman-Budge,
  Temperley-Kostka-Payne), verified against music21 by the parity fixture,
  and `estimate_key_from_pitches_with` / `estimate_key_from_chords_with` to
  choose one.
- `Chord::transpose`, the augmented-sixth family (`is_augmented_sixth`,
  `is_italian_augmented_sixth`, `is_french_augmented_sixth`,
  `is_german_augmented_sixth`, `is_swiss_augmented_sixth`), `is_ninth`,
  `is_transpositionally_symmetrical`, `can_be_dominant_v`, `can_be_tonic`,
  `has_any_repeated_diatonic_note`, `prime_form`, `prime_form_string`,
  `forte_class_tni`, `pitch_class_cardinality` and
  `ordered_pitch_classes_string`.
- `Interval::directed_simple_name`.
- `Scale::degree_of`, `degree_of_pitch_class`, `next_pitch_above` and
  `next_pitch_below`, the realized-scale counterparts of music21's
  `getScaleDegreeFromPitch` and `nextPitch`.
- `tempo::MetronomeMark`, `convert_tempo_by_referent` and the
  `DEFAULT_TEMPO_VALUES` word table, verified against music21 by the parity
  fixture.
- `Error::Duration`, `Error::Key`, `Error::Scale` and `Error::Tempo`
  variants. Duration, mode and scale-degree errors used to be reported as
  `Error::Ordinal`.
- `Pitch::from_frequency`, `harmonic`, `harmonic_from_fundamental`,
  `harmonic_string`, `is_enharmonic`, `all_common_enharmonics`,
  `transpose_below_target`, `transpose_above_target`, `is_twelve_tone` and
  `diatonic_note_number`.
- `KeySignature::altered_pitches`, `accidental_by_step`, `transpose` and
  `scale`; `Key::tonic_pitch_name_with_case`.
- `Duration::type_and_dots`, `dots` and `full_name`,
  `DurationType::type_number`, and `quarter_length_to_closest_type`.
- `ScaleType::derive`, `derive_ranked` and `derive_all` find the tonics on
  which a scale type contains a set of pitches; `Scale::degree_and_accidental_of`
  and `Chord::scale_degrees` report degrees with the accidental that departs
  from the scale's spelling.
- `Pitch::german`, `italian`, `french`, `spanish`, `unicode_name` and
  `unicode_name_with_octave`; `Pitch::transpose` is public and returns
  `Result` instead of swallowing failures.
- `serial::ToneRow`, a port of music21's `serial` module: zero- and
  original-centered `P`/`I`/`R`/`RI` transformations and their inverse
  search, `matrix` and `row_to_matrix`, `intervals_as_string`,
  `is_all_interval`, Link chord classification and hexachordal
  combinatoriality, plus the 71 historical rows as `HISTORICAL_ROWS` and
  `historical_row_by_name`. Both tables are verified against music21 by a
  new `serial_expectations.toml` fixture.
- `Error::Serial`.
- `Chord::normal_order` and `normal_order_string` (music21's `normalOrder`,
  on the chord's own pitch classes), `interval_vector_string`,
  `forte_class_number`, `forte_class_tn`, `multiset_cardinality`,
  `is_prime_form_inversion`, `has_z_relation`, `are_z_relations`,
  `is_false_diminished_seventh`, `inversion_text`,
  `interval_from_chord_step`, `semi_closed_position`,
  `sort_chromatic_ascending`, `sort_diatonic_ascending`,
  `sort_frequency_ascending`, `pitch_names` and `full_name`.
- `Interval::simple_nice_name`, `semi_simple_nice_name`,
  `directed_nice_name`, `directed_simple_nice_name`,
  `directed_semi_simple_nice_name`, `specific_name`, `cents`, `is_unison`,
  `is_perfectable`, `staff_distance`, `mod7`, `mod7_inversion` and `mod12`.
- `Pitch::full_name`, music21's `E-flat in octave 4 (+20c)` spelling.
- `TimeSignature::offset_from_beat`, `beat_progress`, `beat_proportion`,
  `beat_proportion_string`, `beat_division_quarter_lengths`,
  `beat_division_durations`, `beat_sub_division_durations`,
  `beat_division_count_name`, `ratio_equal` and the two beat-to-quarter
  ratios; the division lengths are checked against music21 by the meter
  fixture.
- `Scale::derive_by_degree` and `Key::derive_by_degree`,
  `KeySignature::transpose_pitch_from_c`, and `analysis::tonal_certainty`
  over a ranked list of key estimates.
- `Pitch::get_enharmonic`, `implicit_octave`, `pitch_class_string`,
  `cent_shift_from_midi`, `convert_quarter_tones_to_microtones`,
  `convert_microtones_to_quarter_tones`,
  `harmonic_and_fundamental_from_pitch` and
  `harmonic_and_fundamental_string_from_pitch`.
- `Scale::degree_count`, `transpose`, `chord`, `pitches_from_scale_degrees`,
  `interval_between_degrees`, `is_next`, `match_pitches`, `find_missing` and
  `solfeg` with `SolfegVariant`; the two syllable tables are pinned against
  music21 by the table fixture.
- `RomanNumeral::roman_numeral`, `figure_and_key`,
  `scale_degree_with_alteration`, `functionality_score` (with the
  `FUNCTIONALITY_SCORES` table, pinned against music21), `is_neapolitan`,
  `is_mixture` and `transpose`.
- `VoiceLeadingQuartet::with_key`, `key`, `is_proper_resolution`,
  `leap_not_set_with_step` and `clausula_vera`.
- `Note::step`, `octave`, `pitches` and `full_name`; `Duration::ordinal`,
  `augment_or_diminish` and `DurationType::ordinal`;
  `MetronomeMark::equivalent_by_referent` and
  `maintained_number_with_referent`; `Chord::geometric_normal_form`.
- `pitch::simplify_multiple_enharmonics` and `dissonance_score`, and
  `chordsymbol::chord_symbol_figure_from_chord` and `chord_symbol_from_chord`,
  the last three music21 members the feature map listed as missing.
- `ChordSymbol::find_figure`, `transpose` and `inversion_is_valid`;
  `Music21ChordType` now carries every abbreviation music21 accepts, with
  `abbreviations_for_kind`, `notation_for_kind` and
  `current_abbreviation_for_kind` to read them, and the parity fixture
  checks the whole list.
- `Chord::from_forte_class`, `from_forte_address` and `from_interval_vector`;
  `roman::roman_inversion_name` and `identify_as_tonic_or_dominant`;
  `key::convert_key_string_to_music21_key_string`;
  `DurationType::next_larger` and `next_smaller`; `convert_pitch_class_to_str`
  is public.
- `Interval::from_generic_and_chromatic`, `pitch_start`, `pitch_end`,
  `note_start`, `note_end`, and the free functions
  `written_higher_pitch`, `written_lower_pitch`, `absolute_higher_pitch`,
  `absolute_lower_pitch` and `staff_distance_to_generic_number`.
- `Note::transpose`, `Key::transpose` and `Key::as_scale`.

## Bug Fixes

- Pitches built from a name no longer count as having inferred spelling, so
  `Pitch::transpose` keeps `D-` a flat and `KeySignature::new(-7)` is C-flat
  major rather than B major.
- `tuningsystem::C4` is `440 * 2^(-9/12)` to full precision rather than the
  rounded `261.6256`.
- Transposing with no accidental limit errors past quadruple accidentals
  instead of silently respelling, as music21 does.
- A `PitchOptions` with both a name and an accidental keeps the accidental
  (`name("D").accidental("--")` is `D--`, not `D`), and a pitch built from a
  number stays marked as having inferred spelling, so transposing it
  respells (`Pitch::from_number(6.0)` down a minor second is `F`, not `E#`).
- `Pitch::midi` folds out-of-range pitches into 0 to 127 by octaves and
  rounds half up, as music21 does; `frequency_hz` is computed from A4 = 440
  exactly, so `A4` reads `440.0` rather than `439.99999999999994`.
- Transposing keeps the pitch's fundamental, transposed with it, and can
  reach octave -1 (`D2` down a minor 23rd is `C#-1`).
- The pitch-language errors, `transposeAboveTarget`/`transposeBelowTarget`
  and the unknown-accidental errors use music21's wording.
- Doubled, tripled and quadrupled specifiers are spelled `Doubly-Diminished`,
  `Triply-Augmented` and so on in interval names, as music21 spells them,
  rather than `Double Diminished`. The specifier fixture now checks the
  names as well as the semitone counts.

## Breaking Changes

- `Chord::inversion` now follows music21: it is the chord step the bass
  occupies above the music21 root, so `C F G` is second inversion and
  `A- C F#` is first inversion with root `F#`, and it is `None` only for an
  empty chord. `Chord::bass` picks the written-lowest pitch, so `B#3` is
  below `C4`.
- `Chord::empty` returns `Chord` rather than `Result<Chord>`; it cannot fail.
- `Note::from_pitch` returns `Note` rather than `Result<Note>`, and
  `From<Pitch>` / `From<&Pitch>` replace the `TryFrom` impls that could not
  fail. `Note::from_number` is new and takes a pitch-space number.
- Under the `serde` feature, `Pitch`, `Accidental`, `Microtone`, `Chord` and
  `Note` no longer serialize their fields with a leading underscore
  (`"step"` rather than `"_step"`). Data written by 0.3.0 will not
  deserialize unchanged.
- The `test` binary is gone. It was auto-discovered from `src/bin/test.rs`
  and shipped with the crate; its checks are now a unit test.
- `Interval::direction` is the direction of the chromatic interval, as
  music21's is: a diminished unison is `Descending` and an augmented one
  `Ascending`, where both used to read `Oblique` off the generic interval.
- `transpose_pitch_with_options(pitch, true, max_accidental)` honours
  `max_accidental` on the reversed transposition instead of resetting it to
  four.
- `KeySignature::sharps` returns `Option<IntegerType>`, since a
  non-traditional signature has no count of sharps. `altered_pitches`,
  `transpose_pitch_from_c` and `try_as_key` are errors on one.
- `Chord::inversion_name` is music21's `inversionName`: the figured-bass
  number (`53`, `6`, `64`, `7`, `65`, `43`, `42`) as
  `Result<Option<IntegerType>>`, an error for a chord that is neither a triad
  nor carries a seventh. The words it used to return are `inversion_text`.
- `Chord::closed_position` and `semi_closed_position` take
  `leave_redundant_pitches`, and `is_italian_augmented_sixth` takes
  `restrict_doublings`, matching music21's keyword arguments.
- `Duration::duration_type` reports the type a dotted value is dotted from,
  as music21 does: a dotted half is a half with one dot, where it used to be
  no type at all. `Chord::full_name` names the quarter every chord without a
  duration of its own implicitly has.
- `Chord::common_name` names the augmented sixths that share a set class with
  a seventh chord, and prefixes `enharmonic equivalent to` where the spelling
  does not match the name, both as music21 does.
- Pitch classes round half to even, as Python does, so the half-sharp `C~` is
  pitch class `0` rather than `1`.
- `Chord::remove` reports a pitch that is not there with music21's message,
  which does not name the pitch.
- `Specifier::parse` reports an unknown quality as
  `Cannot find a match for value: 'x'`, and a zero generic interval as
  `The Zeroth is not an interval`, music21's texts.

## Internal

- `python-parity` now builds a pyo3 module, `music21_rs_facade`, of
  music21-shaped `Pitch`, `Accidental` and `Microtone` classes, and a test
  runs the doctests of music21's own `pitch.py` with those classes swapped
  in: 890 of the 920 examples pass, and `python-parity/doctest/pitch.toml`
  pins the passing docstrings. Running them turned up the accidental,
  inferred-spelling, MIDI-folding and negative-octave fixes above.
- The same harness covers `interval.py` (759 of 771 examples), `chord.py`
  (916 of 971), `key.py` (230 of 253) and `serial.py` (148 of 149), each with
  a facade module and an expectation file under `python-parity/doctest/`. The
  interval facade is what made the interval halves public and found the
  reversed `maxAccidental` and key-aware transposition fixes;
  `IntervalBaseTrait`, which it replaced, is gone. The chord facade brought
  `note.Note`, `duration.Duration`, `tie.Tie`, `note.Lyric`, `volume.Volume`
  and a colour-only `style.Style` with it, and found the `commonName`,
  `inversionName`, dotted-duration and pitch-class rounding fixes above.
- The facade chord holds its notes as Python objects rather than values, so
  `chord[1].notehead = 'diamond'` sticks and `chord[1] is chord.notes[1]`
  holds. Pitches are still values, which is what most of the remaining
  failures come down to: `chord.pitches[0].getEnharmonic(inPlace=True)` does
  not reach the chord. The rest are music21 internals (`_overrides`,
  `chordTablesAddress`, `client`) and stream context.
- `cargo run -p xtask -- report` writes `target/reports`: the library's test
  coverage from `cargo llvm-cov`, and every public method of the music21
  classes named in `data/feature_map.toml` against the crate's `pub fn`s,
  with the deliberate omissions and their reasons. CI publishes it on the
  Pages site under `/reports/` and fails when the map goes stale.
- `Pitch::from_options` is the constructor; the nine-argument positional
  `Pitch::new` and the `IntoAccidental`, `IntoCentShift`, `IntoPitchName` and
  `IntoPitch` traits, half of whose impls were `panic!()` stubs, are gone.
- `Interval::from_name` and `from_semitones` build intervals directly rather
  than through an `IntervalArgument` enum. `IntervalBaseTrait` borrows its
  interval and pitch instead of consuming them, and `Specifier` is `Copy`.
- `interval_to_pythagorean_ratio` no longer keeps a process-wide
  `Mutex<HashMap>` of every ratio it has computed.
- The chord root-finding walk lives once in `chord::root`; `Chord`,
  `chordsymbol` and `roman` share it and its `pitch_class` helper.
- Private fields drop their `_` prefix, and `spelling_is_infered` is spelled
  correctly.
- `Note` is a `Pitch` plus an `Option<Duration>`. The `GeneralNote` and
  `NotRest` wrapper structs and `GeneralNoteTrait`, mirrored from music21's
  class hierarchy, are gone; nothing dispatched on them. Under `serde` a
  note now serializes as `{"pitch", "duration"}` rather than nesting the
  duration two wrappers deep.

# music21-rs 0.3.0

This release removes a layer of music21's Python runtime that had been
transliterated into Rust without ever being connected to anything, and fixes
two bugs that were hiding inside it. The minor bump is required because
`fraction` is a public dependency: `FractionType` is re-exported from the
crate root and returned by `Interval::pythagorean_ratio`, so moving to
`fraction` 0.16 is a breaking change for callers who name those types.

## Breaking Changes

- Updated `fraction` to 0.16 and `itertools` to 0.15. `FractionType`
  (`fraction::GenericFraction<IntegerType>`) appears in the public API, so
  downstream crates that mention it must move to `fraction` 0.16 as well.
- `Interval::from_name` and `Interval::new` now return `Error::Interval` for
  a name with no interval number in it, instead of panicking. Code that
  relied on the panic (`""`, `"X"`, `"perfect"`) now sees an `Err`.

## Bug Fixes

- Fixed `Chord::set_duration` and `Chord::with_duration`, which silently did
  nothing on any chord that had notes in it. The chord's duration lived
  behind an `Arc<ChordBase>` that every note in the chord also referenced, so
  the `Arc::get_mut` in the setter never succeeded. `Chord::new("C E G")
  .with_duration(Duration::whole())` returned a chord with no duration; only
  the empty chord worked. The same reference cycle leaked each chord's
  internal note list.
- Fixed `Interval::from_name` panicking out of a `Result`-returning API on
  malformed interval names.
- `PitchOptions::fundamental` now does something. The value was stored on
  `Pitch` and had no reader anywhere in the crate, so it could be set but
  never observed. Added `Pitch::fundamental()`.

## Highlights

- Added `Pitch::fundamental()`.
- Pitch names are no longer formatted out of a `#[derive(Debug)]` impl.
  `Pitch::name` and the ABC and scale helpers now spell the step letter
  explicitly, so the output no longer depends on `StepName`'s variant names.
- Transposition no longer clones the interval. `Interval::transpose_pitch`
  and `Pitch::transpose` read the interval through a reference internally
  instead of taking it by value, so a `Chord` or scale walk no longer copies
  a diatonic/chromatic pair per note. Public signatures are unchanged.
- `interval_to_pythagorean_ratio` no longer holds its cache lock across the
  circle-of-fifths walk, so concurrent callers queue only on the map access,
  and no longer re-parses `"P5"`/`"-P5"` from strings on every call.
- Removed roughly 600 lines of unreachable internals: the `_client` observer
  chains on `Pitch` and `Accidental` (only ever assigned `None`), the
  `HashMap<String, String>` caches on `Note` and `ChordBase` (never read),
  `ChordBase._overrides`, `Pitch._overriden_freq440`, and the `ChordBase` /
  `IntoNotRests` layer that duplicated the note list `Chord` already held.

# music21-rs 0.2.2

This release reworks the tuning-system API around context-dependent tunings
and moves the Python parity suite out of the main workspace.

## Breaking Changes

- Removed the `TuningSystem::StepMethod` and
  `TuningSystem::RecursiveEqualTemperament` variants, both of which computed
  plain equal temperament. `ALL_TUNING_SYSTEMS` is now 15 entries rather
  than 17.

## Highlights

- Added the `tuningsystem::adaptive` module and `AdaptiveTuningSystem` for
  tunings whose frequencies depend on harmonic context, plus the
  `AnyTuningSystem` enum unifying those with the fixed ratio-table systems,
  with `frequency_at`, `cents_at`, and `is_adaptive`.
- Added `TWELVE_TONE_NAMES_SHARP` and `TWELVE_TONE_NAMES_FLAT`.
- Improved Just Intonation ratio accuracy.
- Moved the Python parity tests into a separate `python-parity` package
  outside the cargo workspace, so `cargo test` and `cargo check` on the
  library no longer pull in pyo3.
- Added `xtask verify-tables`, which CI runs to assert the committed chord
  table source still matches `data/chord_tables.toml`.
- Removed the dormant internal `base` and `prebase` modules.

# music21-rs 0.2.1

This patch release fixes a Pythagorean tuning table ordering issue and improves
the Tuning Explorer browser workflow.

## Highlights

- Fixed the Pythagorean tuning ratios so the twelve-tone chromatic degrees stay
  in ascending frequency order, including `Bb` below `B`.
- Added shareable URLs to the Tuning Explorer, preserving the selected tuning
  system, root frequency, and selected degree.
- Added a major-scale playback button for twelve-tone tuning systems, with
  nearest-degree suggestions for non-twelve-tone systems.
- Updated twelve-tone Tuning Explorer labels to use unambiguous flat spellings
  such as `Bb4`.

# music21-rs 0.2.0

This release continues the browser-demo work from `0.1.x` and cleans up several
pre-1.0 pitch APIs so the Rust surface more closely resembles Python
`music21`.

## Breaking API Changes

- Removed the legacy `PitchAccidental` and `PitchMicrotone` builder wrapper
  types.
- Added public `Accidental`, `Microtone`, and `PitchClass` structs with
  companion `AccidentalSpecifier`, `MicrotoneSpecifier`, and
  `PitchClassSpecifier` input enums.
- Updated `PitchOptions` and pitch builders to use those specifier types
  directly.

## Highlights

- Added `Pitch::accidental()`, `Pitch::microtone()`, and
  `Pitch::pitch_class()` accessors.
- Added public accidental helpers for names, modifiers, unicode display,
  non-standard values, and display metadata.
- Added public microtone helpers for cents, harmonic shifts, and music21-style
  formatting.
- Added normalized public pitch-class values with music21-style `A`/`B`
  display for pitch classes 10 and 11.
- Added immediate playback when selecting a suggested resolution in the Chord
  Inspector.
- Added per-resolution preview buttons that play the current chord followed by
  the suggested resolution without changing the page.
- Added hover/focus notation previews for suggested resolutions, showing the
  current chord and hovered resolution side by side on one ABCJS staff.
- Added a CI TypeScript build step for the web demos and included generated web
  JavaScript in the GitHub Pages artifact checks.

# music21-rs 0.1.1

This patch release adds a few browser-facing theory workflow improvements on
top of `0.1.0`.

## Highlights

- Added MIDI-number input support to the Chord Inspector. Inputs like
  `60 64 67`, `60,64,67`, and `midi: 60 64 67` analyze as MIDI notes.
- Added Web MIDI support to the Chord Inspector so a connected MIDI device can
  feed the currently held notes into the analyzer.
- Added a MIDI column to the pitch table and changed pitch display spelling from
  music21 flats such as `A-5` to browser-facing names such as `Ab5`.
- Added a `Class` help widget in the Chord Inspector pitch table.
- Added a Chord Browser at `/chords` listing all 351 unpitched entries in the
  music21-derived chord table, with links back into the inspector.
- Expanded the Chord Browser with a root selector, realized pitches, and
  per-inversion inspector links.
- Added range filtering to the Chord Browser note-count control.
- Listed directed dyad inversions such as major second and minor seventh as
  separate Chord Browser rows, with interval-class labels kept as aliases.
- Moved the Chord Browser frontend source to TypeScript, with browser-served
  JavaScript generated from that source.
- Added resolution-chord links to Chord Browser rows when the realized chord has
  suggestions.

# music21-rs 0.1.0

This release expands `music21-rs` from a chord-name port into a broader set of
interactive music-theory tools and supporting Rust APIs.

## Highlights

- Added a browser demo suite published from `examples/web`:
  - Chord Inspector at `/chord`
  - Polyrhythm Lab at `/polyrhythm`
  - Tuning Explorer at `/tuning`
  - a root index page linking the demos and docs
- Added simple chord-resolution suggestions to the Rust `Chord` API and the
  Chord Inspector.
- Added chord playback, ABC staff notation, random chord generation, clickable
  keyboard toggles, history controls, shareable URLs, and an "open as
  polyrhythm" bridge to the Chord Inspector.
- Added a Polyrhythm Lab with playback, random rhythm settings, shareable URLs,
  history controls, track mute/edit/remove controls, ABC rhythm notation, and
  chord-equivalence links.
- Added a Tuning Explorer for the tuning systems exposed by the crate, including
  scale playback and per-degree ratio/frequency/cents data.
- Updated CI to build and smoke-check the full browser demo site on every run.
- Cleaned up the README and package description to reflect the current crate
  scope.

## Notes

- The browser demos share one WASM crate at `examples/web` rather than keeping
  the Rust glue under the chord demo.
- APIs are still pre-1.0 and may continue to change as more of `music21` is
  ported.
