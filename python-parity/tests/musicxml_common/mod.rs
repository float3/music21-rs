//! What the MusicXML writer's and reader's parity tests share: the scores,
//! the layout a score is stripped of, and how two documents are compared.

use music21_rs::{Stream, StreamElement, StreamKind};

/// Scores that are written the same by both, and what each exercises.
// The ABC reader's test shares this module and brings its own tunes.
#[allow(dead_code)]
pub const SCORES: &[(&str, &str)] = &[
    ("bach/bwv66.6", "a four-part chorale"),
    ("demos/multiple-verses.xml", "lyrics on several verses"),
    ("demos/two-voices.xml", "two voices in one staff"),
    ("demos/voices_with_chords.xml", "chords inside voices"),
    ("demos/two-parts.xml", "two parts"),
    (
        "demos/nested_tuplet_finale_test.xml",
        "tuplets inside tuplets",
    ),
    (
        "corelli/opus3no1/1grave",
        "metadata with contributors and a copyright",
    ),
    ("schoenberg/opus19/movement2", "a piano score on two staves"),
    ("bach/bwv269", "another chorale"),
    (
        "joplin/maple_leaf_rag",
        "a rag: repeats, endings, piano staves",
    ),
    ("haydn/opus74no1/movement3", "a string quartet movement"),
    ("mozart/k80/movement2", "another string quartet movement"),
    (
        "schumann_robert/opus41no1/movement2",
        "a romantic quartet movement",
    ),
    ("beethoven/opus18no4", "a whole quartet"),
    (
        "leadSheet/berlinAlexandersRagtime",
        "a lead sheet: chord symbols",
    ),
    ("leadSheet/fosterBrownHair", "another lead sheet"),
    (
        "demos/ComprehensiveChordSymbolsTestFile.mxl",
        "every kind of chord symbol",
    ),
    (
        "beach/prayer_of_a_tired_child",
        "a pedal mark, a choir and piano",
    ),
    (
        "cpebach/h186.mxl",
        "durations rounded to a file's divisions",
    ),
    (
        "handel/rinaldo/Lascia_chio_pianga",
        "a segno, a fine and a dal segno",
    ),
    ("johnson_j_r/lift_every_voice", "a hymn"),
    ("liliuokalani/aloha_oe", "a song with piano"),
    ("schubert/Lindenbaum", "coloured lyrics"),
    (
        "schumann_clara/opus17/movement3",
        "grace notes in tuplets, an arpeggio across two staves",
    ),
    ("schumann_clara/polonaise_op1n1", "a fine in words"),
    ("verdi/laDonnaEMobile", "an aria"),
    ("weber/concertino_clarinet", "a transposing solo part"),
    (
        "webern/webern_dormi_jesu_op_16_no_2",
        "a dashed line, marks at triplet offsets",
    ),
    ("monteverdi/madrigal.3.1.mxl", "a five-part madrigal"),
    ("ciconia/quod_jactatur.xml", "a canon"),
    ("luca/gloria", "ligature brackets and cue-sized notes"),
    ("theoryExercises/checker_demo", "a theory exercise"),
    ("demos/chorale_with_parallels.mxl", "a chorale"),
    (
        "demos/nested_tuplet_finale_test2.xml",
        "more nested tuplets",
    ),
    ("demos/layoutTest.xml", "a score that is mostly layout"),
    (
        "chopin/mazurka06-2",
        "a Humdrum score, whose part has no id",
    ),
    (
        "demos/drum_sample.xml",
        "unpitched strokes and chords of them",
    ),
    (
        "beethoven/opus59no3/movement1.mxl",
        "a trill carried on by a wavy line",
    ),
    (
        "schumann_clara/polonaise_op1n2.mxl",
        "a rest placed by the clef of its own voice's staff",
    ),
    (
        "schumann_robert/opus48no2.mxl",
        "notes of a chord hidden one by one",
    ),
    (
        "demos/chord_realization_exercise.mxl",
        "a copyright that says nothing",
    ),
    (
        "built:instrument-change",
        "parts that change instrument, which no corpus score does",
    ),
];

/// ABC tunes of the corpus both readers read alike, and what each exercises.
/// A tune is named by its corpus file, with `#n` after it for the tune
/// numbered `n` of a file holding several.
#[allow(dead_code)]
pub const ABC_TUNES: &[(&str, &str)] = &[
    ("ryansMammoth/7thRegimentReel.abc", "a reel with repeats"),
    (
        "ryansMammoth/42dHighlandRegimentStrathspey.abc",
        "measures holding more than their bar, cut in two",
    ),
    (
        "ryansMammoth/BullDozerReel.abc",
        "a pickup of triplets, bowings and fingerings",
    ),
    (
        "ryansMammoth/CzarOfRussiasFavoriteHornpipe.abc",
        "chords written highest note first, endings",
    ),
    (
        "ryansMammoth/LafricansJig.abc",
        "a repeat moved onto the measure cut from its own",
    ),
    ("ryansMammoth/RisingSunReel.abc", "an ending never closed"),
    ("oneills1850/0001-0050.abc#1", "one tune of a file of fifty"),
    (
        "oneills1850/0001-0050.abc#25",
        "a run of nine in the time of two",
    ),
    (
        "oneills1850/1376-1475.abc#1427",
        "a grace note inside a triplet",
    ),
    ("essenFolksong/han1.abc#10", "a folk song changing meter"),
    ("essenFolksong/erk20.abc#169", "a tie written on a rest"),
    ("airdsAirs/book1.abc#5", "an air with a tempo"),
    (
        "airdsAirs/book4.abc#0722",
        "barlines in the header, so the meter stands outside the measures",
    ),
];

/// Python reading the text of a corpus file.
#[allow(dead_code)]
pub const ABC_SOURCE_TEXT: &str = r#"
from music21 import corpus

def source_text(name):
    path = corpus.getWork(name)
    if isinstance(path, (list, tuple)):
        path = path[0]
    with open(str(path), encoding='utf-8') as handle:
        return handle.read()
"#;

/// Takes out of a parsed score what belongs to a page rather than to the
/// music, which the crate does not model.
pub const STRIP_LAYOUT: &str = r#"
from music21 import expressions, layout, repeat, tempo

# The style attributes that place a thing on the page.
PLACING = (
    'absoluteX', 'absoluteY', 'relativeX', 'relativeY',
    'justify', 'alignHorizontal', 'alignVertical',
    'fontSize', 'fontFamily', 'fontStyle', 'fontWeight', 'letterSpacing', 'lineHeight',
    'stemStyle', 'dashLength', 'spaceLength', 'bezierX', 'bezierY', 'bezierX2', 'bezierY2', 'bezierOffset', 'bezierOffset2',
)

def fresh_style(thing):
    """The style a new object of this class starts with."""
    if isinstance(thing, tempo.MetronomeMark):
        # A tempo word is set 4.5 staff lines up whatever its mark says.
        style = thing._styleClass()
        style.absoluteY = 45
        return style
    if isinstance(thing, tempo.TempoText):
        # A tempo text formats its words when it is given them.
        return tempo.TempoText(thing.text).style
    try:
        return type(thing)().style
    except Exception:
        return thing._styleClass()

def reset_placing(thing):
    if not getattr(thing, 'hasStyleInformation', False):
        return
    fresh = fresh_style(thing)
    for name in PLACING:
        if hasattr(thing.style, name):
            setattr(thing.style, name, getattr(fresh, name, None))

def items(thing, name):
    """An attribute that is a list of things, or nothing."""
    value = getattr(thing, name, ())
    return value if isinstance(value, (list, tuple)) else ()

def two_measures(first, second, low):
    """A part of three bars that changes from one instrument to another."""
    from music21 import chord, clef, meter, note, stream
    part = stream.Part()
    one = stream.Measure(number=1)
    one.insert(0, first)
    one.insert(0, clef.TrebleClef())
    one.insert(0, meter.TimeSignature('4/4'))
    one.append(note.Note('C4', type='half'))
    one.append(note.Note('D4', type='half'))
    two = stream.Measure(number=2)
    two.insert(0, second)
    two.append(note.Note(low, type='half'))
    two.append(chord.Chord(['C3', 'E3'], type='half'))
    three = stream.Measure(number=3)
    back = note.Note('G4', type='half')
    back.storedInstrument = type(first)()
    three.append(back)
    three.append(note.Rest(type='half'))
    part.append([one, two, three])
    return part

def build(name):
    """A score made here rather than read from the corpus."""
    from music21 import instrument, stream
    if name == 'built:instrument-change':
        score = stream.Score()
        score.insert(0, two_measures(instrument.Violin(), instrument.Viola(), 'E3'))
        score.insert(0, two_measures(instrument.Flute(), instrument.Oboe(), 'F3'))
        return score
    raise ValueError(name)

def strip_layout(score):
    from music21 import text
    for element in list(score.recurse().getElementsByClass((layout.LayoutBase, text.TextBox))):
        element.activeSite.remove(element)
    score.definesExplicitSystemBreaks = False
    score.definesExplicitPageBreaks = False
    # How the page is drawn as a whole: line widths, note sizes, fonts.
    score.style = score._styleClass()
    for spanner in score.spannerBundle:
        reset_placing(spanner)
    for element in score.recurse(includeSelf=True):
        if hasattr(element, 'layoutWidth'):
            element.layoutWidth = None
        reset_placing(element)
        if element.isStream:
            continue
        if isinstance(element, repeat.RepeatExpression) and element.getText() is not None:
            # A repeat mark read from a file writes its words out of a text
            # expression with a style of its own; a fresh one is formatted
            # as the mark's class formats it.
            old = element.getTextExpression()
            fresh = expressions.TextExpression(element.getText())
            fresh.placement = old.placement
            element.setTextExpression(fresh)
        for lyric in items(element, 'lyrics'):
            reset_placing(lyric)
        for expression in items(element, 'expressions'):
            reset_placing(expression)
        for articulation in items(element, 'articulations'):
            reset_placing(articulation)
        for pitch in items(element, 'pitches'):
            if pitch.accidental is not None:
                reset_placing(pitch.accidental)
        for note in items(element, 'notes'):
            reset_placing(note)
    return score
"#;

/// Part and instrument ids renamed `PART1`, `ID1`... in the order they first
/// appear.
///
/// music21 gives a part or instrument whose id is missing or taken a random
/// uuid, which no second run can reproduce; the crate numbers them. What is
/// compared is that the same part or instrument is named the same wherever
/// it is named.
pub fn normalize_ids(document: &str) -> String {
    const NAMED: [(&str, &str); 5] = [
        ("<score-part id=\"", "PART"),
        ("<part id=\"", "PART"),
        ("<score-instrument id=\"", "ID"),
        ("<midi-instrument id=\"", "ID"),
        ("<instrument id=\"", "ID"),
    ];
    let mut seen: Vec<(&str, String)> = Vec::new();
    document
        .lines()
        .map(|line| {
            let Some((start, prefix)) = NAMED
                .iter()
                .find_map(|(tag, prefix)| line.find(tag).map(|at| (at + tag.len(), *prefix)))
            else {
                return line.to_string();
            };
            let Some(length) = line[start..].find('"') else {
                return line.to_string();
            };
            let id = &line[start..start + length];
            let known = seen
                .iter()
                .filter(|(kind, _)| *kind == prefix)
                .position(|(_, known)| known == id);
            let number = match known {
                Some(index) => index + 1,
                None => {
                    seen.push((prefix, id.to_string()));
                    seen.iter().filter(|(kind, _)| *kind == prefix).count()
                }
            };
            format!(
                "{}{prefix}{number}{}",
                &line[..start],
                &line[start + length..]
            )
        })
        .collect::<Vec<_>>()
        .join(
            "
",
        )
}

/// The first line two documents differ on, with a little of what surrounds
/// it, so a failure says where to look.
pub fn first_difference(ours: &str, theirs: &str) -> Option<String> {
    let ours: Vec<&str> = ours.lines().collect();
    let theirs: Vec<&str> = theirs.lines().collect();
    let at = (0..ours.len().max(theirs.len())).find(|&line| ours.get(line) != theirs.get(line))?;
    let from = at.saturating_sub(4);
    let context = |lines: &[&str]| {
        lines
            .iter()
            .enumerate()
            .skip(from)
            .take(9)
            .map(|(number, line)| {
                format!(
                    "{:5}{} {line}",
                    number + 1,
                    if number == at { ">" } else { " " }
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    Some(format!(
        "line {}\n--- music21\n{}\n--- music21-rs\n{}",
        at + 1,
        context(&theirs),
        context(&ours)
    ))
}

/// Python writing the outline of a music21 score, as `outline` writes the
/// crate's: every part, measure and voice, and in each the elements in
/// order with offset, length and what they are.
#[allow(dead_code)]
pub const OUTLINE: &str = r#"
from fractions import Fraction

def number(value):
    return f'{float(value):.5f}'

def describe(e):
    classes = e.classes
    if 'Unpitched' in classes:
        return f'Unpitched {e.storedInstrument.classes[0] if e.storedInstrument else None}'
    if 'PercussionChord' in classes:
        return 'PercussionChord ' + ' '.join(
            str(n.storedInstrument.classes[0]) for n in e.notes)
    if 'Note' in classes:
        return f'Note {e.nameWithOctave} {e.tie.type if e.tie else None} {sung(e)}'
    if 'NoChord' in classes:
        return 'NoChord'
    if 'ChordSymbol' in classes:
        return f'ChordSymbol {e.figure}'
    if 'Chord' in classes:
        return 'Chord ' + ' '.join(
            f'{n.nameWithOctave}/{n.tie.type if n.tie else None}' for n in e.notes
        ) + f' {sung(e)}'
    if 'Rest' in classes:
        return f'Rest {sung(e)}'
    if 'Clef' in classes:
        return f'Clef {e.sign}{e.line}'
    if 'Dynamic' in classes:
        return f'Dynamic {e.value}'
    if 'Key' in classes:
        return f'Key {e.sharps} {e.mode}'
    if 'KeySignature' in classes:
        return f'KeySignature {e.sharps}'
    if 'TimeSignature' in classes:
        return f'TimeSignature {e.ratioString}'
    if 'MetronomeMark' in classes:
        return f'MetronomeMark {number(e.number)} {e.numberImplicit}'
    if 'Instrument' in classes:
        return f'Instrument {classes[0]} {e.partName} {e.instrumentName} {e.midiProgram}'
    return None

def sung(e):
    return None if e.lyric is None else e.lyric.replace('\n', '|')

def line(e, offset):
    said = describe(e)
    if said is None:
        return None
    return f'    {number(offset)} {number(e.duration.quarterLength)} {said}'

def outline(score):
    out = []
    for part in score.parts:
        out.append('Part')
        for measure in part.getElementsByClass('Measure'):
            out.append(f'  Measure {measure.number} {number(measure.offset)}')
            for e in measure:
                if 'Barline' in e.classes:
                    continue
                if 'Voice' in e.classes:
                    out.append('   Voice')
                    for inner in e:
                        out.append(line(inner, inner.getOffsetBySite(e)))
                else:
                    out.append(line(e, e.getOffsetBySite(measure)))
    return '\n'.join(said for said in out if said is not None)

def flat_outline(score):
    """Every part with all it holds, measures or none."""
    out = []
    for part in score.parts:
        out.append('Part')
        for e in part.recurse():
            if 'Measure' in e.classes:
                out.append(f'  Measure {e.number} {number(e.getOffsetInHierarchy(part))}')
            elif not e.isStream:
                out.append(line(e, e.getOffsetInHierarchy(part)))
    return '\n'.join(said for said in out if said is not None)
"#;

#[allow(dead_code)]
fn number(value: f64) -> String {
    format!("{value:.5}")
}

#[allow(dead_code)]
fn python_bool(value: bool) -> &'static str {
    if value { "True" } else { "False" }
}

#[allow(dead_code)]
fn option(value: Option<impl std::fmt::Display>) -> String {
    value.map_or_else(|| "None".to_string(), |value| value.to_string())
}

#[allow(dead_code)]
fn sung(lyric: Option<String>) -> String {
    option(lyric.map(|lyric| lyric.replace('\n', "|")))
}

#[allow(dead_code)]
fn describe(element: &StreamElement) -> String {
    let tie = |note: &music21_rs::Note| option(note.tie().map(|tie| tie.tie_type().as_str()));
    match element {
        StreamElement::Unpitched(stroke) => format!(
            "Unpitched {}",
            option(
                stroke
                    .stored_instrument()
                    .map(|instrument| instrument.kind())
            )
        ),
        StreamElement::PercussionChord(chord) => format!(
            "PercussionChord {}",
            chord
                .written()
                .notes()
                .iter()
                .map(|note| option(
                    note.stored_instrument()
                        .map(|instrument| instrument.kind().to_string())
                ))
                .collect::<Vec<_>>()
                .join(" ")
        ),
        StreamElement::Note(note) => format!(
            "Note {} {} {}",
            music21_name(note.pitch()),
            tie(note),
            sung(note.lyric())
        ),
        StreamElement::ChordSymbol(symbol) if symbol.figure() == "N.C." => "NoChord".to_string(),
        StreamElement::ChordSymbol(symbol) => format!("ChordSymbol {}", symbol.figure()),
        StreamElement::Chord(chord) => format!(
            "Chord {} {}",
            chord
                .notes()
                .iter()
                .map(|note| format!("{}/{}", music21_name(note.pitch()), tie(note)))
                .collect::<Vec<_>>()
                .join(" "),
            sung(chord.notes().first().and_then(music21_rs::Note::lyric))
        ),
        StreamElement::Rest(rest) => format!("Rest {}", sung(rest.lyric())),
        StreamElement::Dynamic(dynamic) => format!("Dynamic {}", dynamic.value()),
        StreamElement::KeySignature(signature) => {
            format!("KeySignature {}", option(signature.sharps()))
        }
        StreamElement::Clef(clef) => format!(
            "Clef {}{}",
            clef.sign().unwrap_or("None"),
            option(clef.line())
        ),
        StreamElement::Key(key) => format!("Key {} {}", key.sharps(), key.mode()),
        StreamElement::TimeSignature(meter) => {
            format!("TimeSignature {}", meter.ratio_string())
        }
        StreamElement::MetronomeMark(mark) => format!(
            "MetronomeMark {} {}",
            number(mark.number().unwrap_or(0.0)),
            python_bool(mark.number_implicit())
        ),
        StreamElement::Instrument(instrument) => format!(
            "Instrument {} {} {} {}",
            instrument.kind(),
            option(instrument.part_name()),
            option(instrument.name()),
            option(instrument.midi_program())
        ),
        other => format!("{other:?}"),
    }
}

/// A pitch's name and octave as music21 writes them, a flat as `-`: the
/// crate's `Bb4` is `B-4`, and its `Bb-1` `B--1`.
fn music21_name(pitch: &music21_rs::Pitch) -> String {
    let name = pitch.name_with_octave();
    let mut letters = name.chars();
    let Some(step) = letters.next() else {
        return String::new();
    };
    format!("{step}{}", letters.as_str().replace('b', "-"))
}

/// The score as the outline `HELPERS` writes of music21's.
#[allow(dead_code)]
pub fn outline(score: &Stream) -> String {
    let mut out = Vec::new();
    let line = |event: &music21_rs::StreamEvent| {
        format!(
            "    {} {} {}",
            number(event.offset()),
            number(event.element().quarter_length()),
            describe(event.element())
        )
    };
    for part in score.parts() {
        out.push("Part".to_string());
        for event in part.events() {
            let Some(measure) = event
                .element()
                .as_stream()
                .filter(|inner| inner.kind() == StreamKind::Measure)
            else {
                continue;
            };
            out.push(format!(
                "  Measure {} {}",
                measure.number(),
                number(event.offset())
            ));
            for held in measure.events() {
                match held.element().as_stream() {
                    Some(voice) if voice.kind() == StreamKind::Voice => {
                        out.push("   Voice".to_string());
                        out.extend(voice.events().iter().map(line));
                    }
                    _ => out.push(line(held)),
                }
            }
        }
    }
    out.join("\n")
}

/// A score as an outline of every part with all it holds, whether in
/// measures or not: the outline of a tune with no measures, which `outline`
/// says nothing of.
#[allow(dead_code)]
pub fn flat_outline(score: &Stream) -> String {
    fn walk(stream: &Stream, base: f64, out: &mut Vec<String>) {
        for event in stream.events() {
            let offset = base + event.offset();
            match event.element().as_stream() {
                Some(inner) => {
                    if inner.kind() == StreamKind::Measure {
                        out.push(format!("  Measure {} {}", inner.number(), number(offset)));
                    }
                    walk(inner, offset, out);
                }
                None => out.push(format!(
                    "    {} {} {}",
                    number(offset),
                    number(event.element().quarter_length()),
                    describe(event.element())
                )),
            }
        }
    }
    let mut out = Vec::new();
    for part in score.parts() {
        out.push("Part".to_string());
        walk(part, 0.0, &mut out);
    }
    out.join("\n")
}
