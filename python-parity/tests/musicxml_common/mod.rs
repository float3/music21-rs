//! What the MusicXML writer's and reader's parity tests share: the scores,
//! the layout a score is stripped of, and how two documents are compared.

use music21_rs::{Stream, StreamElement, StreamKind};
use pyo3::prelude::*;

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
    (
        "built:glissandi",
        "glissandi and slides, which no corpus score has",
    ),
    ("built:tremolo-spanners", "tremolos between notes"),
    (
        "built:pedals",
        "pedals drawn as lines, signs and both, with bounces and gaps",
    ),
    ("built:inner-barline", "a barline inside a measure"),
    (
        "built:lyric-styles",
        "lyrics justified, placed and left unprinted",
    ),
    ("built:rehearsal-marks", "rehearsal marks, boxed and not"),
    (
        "built:ottavas",
        "octave lines written where they are read, moved where they sound",
    ),
    (
        "built:partstaff-gap",
        "a piano's staves, the upper missing a measure the lower has",
    ),
    (
        "trecento/PMFC_12_19-Sanctus Barbitonsoris.xml",
        "rehearsal marks in a file",
    ),
    (
        "trecento/PMFC_23_16-Kyrie Apt 16.xml",
        "time signatures left unprinted",
    ),
    (
        "trecento/PMFC_06_8-In Verde Prato.xml",
        "a metric modulation",
    ),
    (
        "trecento/Fava_Dicant_nunc_iudei.xml",
        "a part named for a transposing instrument, written at its written pitch",
    ),
    (
        "trecento/PMFC_13_01-Kyrie-Summe-Clementissime.mxl",
        "the same, read from a compressed file",
    ),
    (
        "beethoven/opus59no3/movement3.mxl",
        "a hairpin started after a part's last note, which takes the next part's first",
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
// The MuseScore reader's test shares this module and asks music21 nothing.
#[allow(dead_code)]
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

def reset_placing(thing, keep=()):
    if not getattr(thing, 'hasStyleInformation', False):
        return
    fresh = fresh_style(thing)
    for name in PLACING:
        if name not in keep and hasattr(thing.style, name):
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

def quarters(bars):
    """A part of 4/4 bars of quarter notes, and the notes of each bar. A
    pitch of None is a rest, and a list of pitches a chord."""
    from music21 import chord, clef, meter, note, stream
    part = stream.Part()
    measures = []
    for index, pitches in enumerate(bars):
        measure = stream.Measure(number=index + 1)
        if index == 0:
            measure.insert(0, clef.TrebleClef())
            measure.insert(0, meter.TimeSignature('4/4'))
        for pitch in pitches:
            if pitch is None:
                measure.append(note.Rest(type='quarter'))
            elif isinstance(pitch, list):
                measure.append(chord.Chord(pitch, type='quarter'))
            else:
                measure.append(note.Note(pitch, type='quarter'))
        measures.append(measure)
    part.append(measures)
    return part, [list(measure.notesAndRests) for measure in measures]

def one_part(part):
    from music21 import stream
    score = stream.Score()
    score.insert(0, part)
    return score

def glissandi():
    """Glissandi and slides, of every line and every way of playing one."""
    from music21 import spanner
    part, bars = quarters([
        ['C4', 'G4', 'E4', 'C5'],
        ['D4', 'A4', ['F4', 'A4'], 'D5'],
        ['E4', 'B4', 'G4', 'E5'],
        ['F4', 'C5', 'A4', 'F5'],
    ])
    plain = spanner.Glissando(bars[0][0], bars[0][1])
    worded = spanner.Glissando(bars[0][2], bars[0][3], lineType='solid', label='gliss.')
    # Across a barline, and from a note another one ends on.
    slide = spanner.Glissando(bars[0][3], bars[1][0])
    slide.slideType = 'continuous'
    white = spanner.Glissando(bars[1][1], bars[1][2], lineType='dashed')
    white.slideType = 'white'
    # From a chord, which says it on its first note alone.
    dotted = spanner.Glissando(bars[1][2], bars[1][3], lineType='dotted', label='port.')
    dotted.slideType = 'continuous'
    # Over three notes, the middle one of which says nothing.
    harp = spanner.Glissando(bars[2][0], bars[2][1], bars[2][2])
    harp.slideType = 'diatonic'
    black = spanner.Glissando(bars[2][3], bars[3][0], label='black keys')
    black.slideType = 'black'
    # A seventh, which takes the first number again.
    last = spanner.Glissando(bars[3][2], bars[3][3])
    for made in (plain, worded, slide, white, dotted, harp, black, last):
        part.insert(0, made)
    return one_part(part)

def tremolos():
    """Tremolos between two notes, and one over three."""
    from music21 import expressions
    part, bars = quarters([
        ['C4', 'E4', 'G4', 'C5'],
        ['D4', 'F4', ['A4', 'C5'], ['D5', 'F5']],
        ['E4', 'G4', 'B4', None],
    ])
    plain = expressions.TremoloSpanner(bars[0][0], bars[0][1])
    above = expressions.TremoloSpanner(bars[0][2], bars[0][3])
    above.numberOfMarks = 2
    above.placement = 'above'
    below = expressions.TremoloSpanner(bars[1][0], bars[1][1])
    below.numberOfMarks = 4
    below.placement = 'below'
    chords = expressions.TremoloSpanner(bars[1][2], bars[1][3])
    chords.numberOfMarks = 1
    three = expressions.TremoloSpanner(bars[2][0], bars[2][1], bars[2][2])
    for made in (plain, above, below, chords, three):
        part.insert(0, made)
    return one_part(part)

def pedals():
    """Pedal marks drawn each way, with the bounces and gaps inside them."""
    from music21 import expressions
    Form, Type = expressions.PedalForm, expressions.PedalType
    part, bars = quarters([['C4', 'D4', 'E4', 'F4']] * 7)

    def held(bar, first, last, form, kind, inside=(), within=None, **said):
        """A pedal from one note of a bar to another, with what happens
        between them: (offset, class, placement)."""
        measure = part.getElementsByClass('Measure')[bar]
        mark = expressions.PedalMark(bars[bar][first])
        mark.pedalForm = form
        mark.pedalType = kind
        for name, value in said.items():
            setattr(mark, name, value)
        for offset, made, placement in inside:
            moment = made()
            moment.placement = placement
            measure.insert(offset, moment)
            mark.addSpannedElements(moment)
        mark.addSpannedElements(bars[bar][last])
        if within is None:
            part.insert(0, mark)
        else:
            # Standing in its measure, which is where its line resumes.
            measure.insert(within, mark)

    bounce = expressions.PedalBounce
    gap, back = expressions.PedalGapStart, expressions.PedalGapEnd
    held(0, 0, 3, Form.Line, Type.Sustain, [(2.0, bounce, None)])
    held(1, 0, 3, Form.Symbol, Type.Sustain, [(1.0, bounce, 'below')])
    held(2, 0, 3, Form.SymbolAlt, Type.Sostenuto, [(2.0, bounce, None)], abbreviated=True)
    held(3, 1, 3, Form.SymbolLine, Type.Sustain,
         [(2.0, gap, 'below'), (2.5, back, 'above')], within=1.0, placement='below')
    # Its line resumes where the pedal mark stands, at the start of the part.
    held(4, 2, 3, Form.SymbolLine, Type.Soft, [(2.5, bounce, None)])
    held(5, 0, 3, Form.Line, Type.Sostenuto, [(1.0, gap, None), (3.0, back, None)])
    held(6, 0, 2, Form.Symbol, Type.Silent, [(1.5, bounce, None), (0.5, bounce, None)])
    return one_part(part)

def inner_barline():
    """A barline standing inside a measure, which music21 keeps and does not
    write."""
    from music21 import bar
    part, _ = quarters([['C4', 'D4', 'E4', 'F4'], ['G4', 'A4', 'B4', 'C5']])
    part.getElementsByClass('Measure')[0].insert(2.0, bar.Barline('dashed'))
    part.getElementsByClass('Measure')[1].insert(3.0, bar.Barline('double'))
    return one_part(part)

def lyric_styles():
    """Lyrics that say how they line up, which side they are on and whether
    they are printed."""
    part, bars = quarters([['C4', 'D4', 'E4', 'F4']])
    said = [
        ('left', None, False),
        ('center', 'above', False),
        ('right', 'below', True),
        (None, 'above', True),
    ]
    for sung, (justify, placement, hidden) in zip(bars[0], said):
        sung.addLyric('la')
        sung.addLyric('li')
        lyric = sung.lyrics[0]
        if justify is not None:
            lyric.style.justify = justify
        if placement is not None:
            lyric.style.placement = placement
        if hidden:
            lyric.style.hideObjectOnPrint = True
    return one_part(part)

def rehearsal_marks():
    """Rehearsal marks, boxed and not, at the start of a bar and inside one."""
    from music21 import expressions
    part, _ = quarters([['C4', 'D4', 'E4', 'F4'], ['G4', 'A4', 'B4', 'C5']])
    measures = part.getElementsByClass('Measure')
    boxed = expressions.RehearsalMark('A')
    boxed.style.enclosure = 'square'
    boxed.style.placement = 'above'
    measures[0].insert(0, boxed)
    measures[0].insert(2.0, expressions.RehearsalMark(12))
    plain = expressions.RehearsalMark('B', numbering='alphabetical')
    plain.style.enclosure = 'none'
    plain.style.placement = 'below'
    measures[1].insert(0, plain)
    return one_part(part)

def ottavas():
    """Octave lines over notes written where they are read, which music21's
    writer moves to where they sound, and one over notes already there."""
    from music21 import spanner
    part, bars = quarters([
        ['C5', 'D5', 'E5', 'F5'],
        ['G3', ['C3', 'E3'], 'A3', 'B3'],
        ['C4', 'D4', 'E4', None],
    ])
    made = (
        spanner.Ottava(bars[0][0], bars[0][1], bars[0][2], type='8va'),
        spanner.Ottava(bars[1][0], bars[1][1], type='8vb', placement='below'),
        spanner.Ottava(bars[1][2], bars[1][3], type='15mb', transposing=False,
                       placement='below'),
        spanner.Ottava(bars[2][0], bars[2][1], type='15ma'),
    )
    for ottava in made:
        part.insert(0, ottava)
    return one_part(part)

def partstaff_gap():
    """A piano's two staves, the upper lacking the measure numbered 3, which
    music21's exporter joins measure by measure all the same."""
    from music21 import layout, note, stream
    upper = stream.PartStaff()
    lower = stream.PartStaff()
    for staff, numbers, name in ((upper, [1, 2, 4, 5], 'C5'), (lower, [1, 2, 3, 4, 5], 'C3')):
        for number in numbers:
            staff.append(stream.Measure([note.Note(name, type='whole')], number=number))
    group = layout.StaffGroup([upper, lower], symbol='brace')
    return stream.Score([group, upper, lower])

BUILT = {
    'built:partstaff-gap': partstaff_gap,
    'built:ottavas': ottavas,
    'built:rehearsal-marks': rehearsal_marks,
    'built:glissandi': glissandi,
    'built:tremolo-spanners': tremolos,
    'built:pedals': pedals,
    'built:inner-barline': inner_barline,
    'built:lyric-styles': lyric_styles,
}

def build(name):
    """A score made here rather than read from the corpus."""
    from music21 import instrument, stream
    if name == 'built:instrument-change':
        score = stream.Score()
        score.insert(0, two_measures(instrument.Violin(), instrument.Viola(), 'E3'))
        score.insert(0, two_measures(instrument.Flute(), instrument.Oboe(), 'F3'))
        return score
    if name in BUILT:
        return BUILT[name]()
    raise ValueError(name)

# What a built score's document gains that music21 reads and does not write.
ADDED = {
    'built:inner-barline': (
        '</note>',
        2,
        '</note>\n      <barline location="middle">\n'
        '        <bar-style>dashed</bar-style>\n      </barline>',
    ),
    'built:lyric-styles': ('<lyric name="2" number="2">', 1, '<lyric name="2" number="2" print-object="yes">'),
}

def built_source(name, directory):
    """The MusicXML music21 writes of a built score, as a file a reader can
    be given: its path and its text."""
    import os
    from music21.musicxml import m21ToXml
    exporter = m21ToXml.GeneralObjectExporter(strip_layout(build(name)))
    exporter.makeNotation = False
    text = exporter.parse().decode('utf-8')
    if name in ADDED:
        old, which, new = ADDED[name]
        at = -1
        for _ in range(which):
            at = text.index(old, at + 1)
        text = text[:at] + new + text[at + len(old):]
    os.makedirs(directory, exist_ok=True)
    path = os.path.join(directory, name.replace(':', '_') + '.musicxml')
    with open(path, 'w', encoding='utf-8') as handle:
        handle.write(text)
    return path, text

def strip_layout(score, keep_layouts=False):
    from music21 import text
    # The crate keeps the layouts and the text boxes, and where a text box
    # stands on its page.
    if not keep_layouts:
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
        if isinstance(element, text.TextBox):
            continue
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
            # How a syllable lines up under its note is the crate's to say.
            reset_placing(lyric, keep=('justify',))
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

/// Python that fixes the date music21's MusicXML exporter stamps in
/// `<encoding-date>`: `m21ToXml` reads it from `datetime.date.today()` at
/// export time, and is given a `datetime` whose `date.today()` is the day
/// `pin` was called.
const PIN_ENCODING_DATE: &str = r#"
import datetime
import types

from music21.musicxml import m21ToXml

def pin():
    day = datetime.date.today()

    class PinnedDate(datetime.date):
        @classmethod
        def today(cls):
            return day

    pinned = types.ModuleType('datetime')
    pinned.__dict__.update(vars(datetime))
    pinned.date = PinnedDate
    m21ToXml.datetime = pinned
    return str(day)
"#;

/// The date both writers put in `<encoding-date>`: today's, read once, with
/// music21's exporter held to it for the rest of the run, so a run that
/// crosses midnight still writes the same date on both sides.
#[allow(dead_code)]
pub fn pin_encoding_date(py: Python<'_>) -> PyResult<String> {
    PyModule::from_code(
        py,
        &std::ffi::CString::new(PIN_ENCODING_DATE).expect("no nul in the helper"),
        c"musicxml_encoding_date.py",
        c"musicxml_encoding_date",
    )?
    .getattr("pin")?
    .call0()?
    .extract()
}

/// Part and instrument ids renamed `PART1`, `ID1`... in the order they first
/// appear.
///
/// music21 gives a part or instrument whose id is missing or taken a random
/// uuid, which no second run can reproduce; the crate numbers them. What is
/// compared is that the same part or instrument is named the same wherever
/// it is named.
#[allow(dead_code)]
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

/// Scores music21's own exporter raises on, each with what it raises and
/// why. A test passes over a listed score while music21 still raises that,
/// and fails once music21 writes it, so the list cannot go stale.
#[allow(dead_code)]
pub const MUSIC21_CANNOT_WRITE: &[(&str, &str, &str)] = &[];

/// Whether music21 is known to raise `error` writing the score `name` (a
/// subject named `xml:` and a corpus name counts as that name).
#[allow(dead_code)]
pub fn music21_cannot_write(name: &str, error: &str) -> bool {
    let name = name.strip_prefix("xml:").unwrap_or(name);
    MUSIC21_CANNOT_WRITE
        .iter()
        .any(|(listed, raises, _)| *listed == name && error.contains(raises))
}

/// What to say of a listed score music21 has just written after all.
#[allow(dead_code)]
pub fn music21_writes_after_all(name: &str) -> Option<String> {
    let name = name.strip_prefix("xml:").unwrap_or(name);
    MUSIC21_CANNOT_WRITE
        .iter()
        .any(|(listed, _, _)| *listed == name)
        .then(|| format!("{name}: music21 writes it now; take it off MUSIC21_CANNOT_WRITE"))
}

/// The first line two documents differ on, with a little of what surrounds
/// it, so a failure says where to look.
#[allow(dead_code)]
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
#[allow(dead_code)]
pub fn music21_name(pitch: &music21_rs::Pitch) -> String {
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
