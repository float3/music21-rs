//! Every parity test, built as one binary.
//!
//! Each file here was a test binary of its own; linking 188 of them was
//! most of the build, and carrying them to CI's shards most of its run.
//! nextest still runs every test in a process of its own, which the suites
//! need: a module swapped into music21 lasts for the life of the
//! interpreter. A new file here needs its `mod` line below.

mod abc_read_parity;
mod abc_write_parity;
mod accidental_display_parity;
mod alpha_parity;
mod analysis_parity;
mod articulation_parity;
mod audiosearch_parity;
mod braille_basic_parity;
mod braille_parity;
mod built_streams;
mod capella_parity;
mod chord_name_parity;
mod chord_symbol_parity;
mod chord_type_parity;
mod chordify_parity;
mod class_lineage_parity;
mod clef_parity;
mod clercq_temperley_parity;
mod color_parity;
mod correlate_parity;
mod dataset_parity;
mod doctest_aligner;
mod doctest_articulations;
mod doctest_audiosearch;
mod doctest_bar;
mod doctest_beam;
mod doctest_braillebasic;
mod doctest_brailleexamples;
mod doctest_braillelookup;
mod doctest_braillenotegrouping;
mod doctest_braillesegment;
mod doctest_brailletext;
mod doctest_brailletranslate;
mod doctest_checker;
mod doctest_chord;
mod doctest_chordtables;
mod doctest_clef;
mod doctest_clercqtemperley;
mod doctest_correlate;
mod doctest_discrete;
mod doctest_duration;
mod doctest_dynamics;
mod doctest_elements;
mod doctest_enharmonics;
mod doctest_expressions;
mod doctest_fbexamples;
mod doctest_fbharmony;
mod doctest_features;
mod doctest_figuredbass;
mod doctest_fixer;
mod doctest_floatingkey;
mod doctest_harmonicfunction;
mod doctest_harmony;
mod doctest_harmparser;
mod doctest_hasher;
mod doctest_instrument;
mod doctest_interval;
mod doctest_jsymbolic;
mod doctest_key;
mod doctest_layout;
mod doctest_makenotation;
mod doctest_metadata;
mod doctest_meter;
mod doctest_metercore;
mod doctest_metrical;
mod doctest_midipercussion;
mod doctest_nativefeatures;
mod doctest_naturallanguage;
mod doctest_neoriemannian;
mod doctest_note;
mod doctest_omrcorrectors;
mod doctest_omrevaluators;
mod doctest_patel;
mod doctest_percussion;
mod doctest_pitch;
mod doctest_pitchanalysis;
mod doctest_possibility;
mod doctest_primitives;
mod doctest_realizer;
mod doctest_realizerscale;
mod doctest_reducechords;
mod doctest_reduction;
mod doctest_repeat;
mod doctest_resolution;
mod doctest_roman;
mod doctest_rules;
mod doctest_scala;
mod doctest_scale;
mod doctest_searchbase;
mod doctest_searchlyrics;
mod doctest_searchsegment;
mod doctest_searchserial;
mod doctest_segment;
mod doctest_segmentbyrests;
mod doctest_serial;
mod doctest_sieve;
mod doctest_spanner;
mod doctest_streambase;
mod doctest_style;
mod doctest_tablature;
mod doctest_tempo;
mod doctest_text;
mod doctest_tie;
mod doctest_transposition;
mod doctest_treefromstream;
mod doctest_treespans;
mod doctest_treetimespantree;
mod doctest_treetostream;
mod doctest_treeverticality;
mod doctest_tsvconverter;
mod doctest_variant;
mod doctest_voiceleading;
mod doctest_volume;
mod doctest_windowed;
mod expand_repeats_parity;
mod facade_edit_parity;
mod features_parity;
mod fixer_parity;
mod fixture_freshness;
mod flat_spelling_parity;
mod humdrum_parity;
mod instrument_parity;
mod instrument_streams_parity;
mod key_analysis_parity;
mod language_names_parity;
mod lily_parity;
mod mei_parity;
mod metadata_parity;
mod metadata_search_parity;
mod meter_parity;
mod midi_read_parity;
mod midi_write_parity;
mod mscx_parity;
mod musedata_parity;
mod music21_parity;
mod music21_suite;
mod musicxml_common;
mod musicxml_make_notation_parity;
mod musicxml_parity;
mod musicxml_plain_parity;
mod musicxml_read_parity;
mod notation_walks_parity;
mod noteworthy_parity;
mod omr_parity;
mod opus_parity;
mod ornament_parity;
mod ornament_recognizer_parity;
mod percussion_parity;
mod placed_parity;
mod realize_ornaments_parity;
mod realizer_parity;
mod reduce_chords_parity;
mod reduction_parity;
mod roman_figure_parity;
mod romantext_parity;
mod romantext_write_parity;
mod scala_archive;
mod scale_parity;
mod scale_search_parity;
mod search_base_parity;
mod search_parity;
mod search_segment_parity;
mod search_serial_parity;
mod serial_parity;
mod stream_analyze_parity;
mod stream_context_parity;
mod stream_editing_parity;
mod stream_ends_parity;
mod stream_filling_parity;
mod stream_lookups_parity;
mod stream_lyrics_parity;
mod stream_queries_parity;
mod stream_seconds_parity;
mod stream_slicing_parity;
mod stream_span_parity;
mod stream_split_parity;
mod stream_voices_parity;
mod tablature_parity;
mod table_parity;
mod template_parity;
mod text_parity;
mod tinynotation_parity;
mod tree_parity;
mod tree_voices_parity;
mod tsv_parity;
mod variant_parity;
mod voice_leading_parity;
mod volpiano_parity;
mod wheel_version;
mod windowed_parity;
mod zz_corpus_cache;

/// With `autotests` off, a file left out above would never run; this says
/// which.
#[test]
fn every_file_is_a_module() {
    let main = include_str!("main.rs");
    let tests = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut missing = Vec::new();
    for entry in std::fs::read_dir(&tests).expect("read tests/").flatten() {
        let path = entry.path();
        let name = if path.is_dir() {
            if !path.join("mod.rs").is_file() {
                continue;
            }
            path.file_name()
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            path.file_stem()
        } else {
            continue;
        };
        let Some(name) = name.and_then(|name| name.to_str()) else {
            continue;
        };
        if name != "main" && !main.contains(&format!("\nmod {name};\n")) {
            missing.push(name.to_string());
        }
    }
    missing.sort();
    assert!(missing.is_empty(), "not in tests/main.rs: {missing:?}");
}
