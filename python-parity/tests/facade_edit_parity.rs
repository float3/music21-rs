//! The package's classes against music21's under random edits.
//!
//! A seed makes one object -- a note, chord, pitch, key, scale, meter or
//! metronome mark -- of music21's classes and of the package's, and makes
//! the same one to six edits to each through the public setters and the
//! objects they hand out: a note's pitch's accidental, its duration's
//! type and dots, its volume, beams, tie and style, a key's tonic, a
//! mark's referent. Each is then read back -- repr, str, pitches and what
//! else its class answers -- as it stands, deep-copied and pickled, and
//! written as MusicXML in a stream of its own with notation made. Every
//! reading must be music21's.
//!
//! The generator only makes what music21 takes and means; what it leaves
//! out, and why, is said beside `EDITS` below.

use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use music21_rs_python_parity::music21_rs_facade;
use pyo3::prelude::*;
use utils::{init_py, prepare};

use crate::musicxml_common;
use musicxml_common::{first_difference, normalize_ids};

/// How many objects are made up: a hundred and thirty of each class,
/// about ten seconds of music21 writing them.
const GENERATED: u64 = 910;

/// The generator and both readings.
const CASES: &str = r#"
import copy
import os
import pickle
import random
import re
import tempfile
import types

from music21 import articulations, chord, duration, key, meter, note, pitch, scale, stream
from music21 import tempo, tie
from music21.musicxml import m21ToXml
import music21_rs_facade as rs

M21 = types.SimpleNamespace(
    Note=note.Note,
    Chord=chord.Chord,
    Pitch=pitch.Pitch,
    Accidental=pitch.Accidental,
    Tie=tie.Tie,
    Duration=duration.Duration,
    Tuplet=duration.Tuplet,
    Key=key.Key,
    TimeSignature=meter.TimeSignature,
    MetronomeMark=tempo.MetronomeMark,
    Stream=stream.Stream,
    Staccato=articulations.Staccato,
    Accent=articulations.Accent,
    MajorScale=scale.MajorScale,
    MinorScale=scale.MinorScale,
    DorianScale=scale.DorianScale,
    HarmonicMinorScale=scale.HarmonicMinorScale,
    MelodicMinorScale=scale.MelodicMinorScale,
    LydianScale=scale.LydianScale,
)

KINDS = ['Note', 'Chord', 'Pitch', 'Key', 'Scale', 'TimeSignature', 'MetronomeMark']
STEPS = 'CDEFGAB'
NAMES = ['C', 'C#', 'D-', 'D', 'E-', 'E', 'F', 'F#', 'G', 'A-', 'A', 'B-', 'B']
ACCIDENTALS = ['sharp', 'flat', 'natural', 'double-sharp', 'double-flat']
TYPES = ['whole', 'half', 'quarter', 'eighth', '16th']
LENGTHS = [0.25, 0.5, 0.75, 1.0, 1.5, 2.0, 3.0, 4.0]
COLORS = ['#FF0000', '#00AA00', 'blue']
METERS = ['4/4', '3/4', '2/4', '6/8', '3/8', '5/8', '2/2', '12/8', '7/8']
SCALES = [
    'MajorScale', 'MinorScale', 'DorianScale', 'HarmonicMinorScale', 'MelodicMinorScale',
    'LydianScale',
]


def named(r, low=3, high=5):
    return r.choice(NAMES) + str(r.randint(low, high))


# Left out on purpose:
# - `Key.mode = ...`: music21 keeps the scale the key was made with, so a
#   key told it is minor goes on listing a major scale's pitches.
# - A tuplet before `quarterLength`, `type` or `dots` is set: music21 keeps
#   the tuplet over the new value, so a note set to 1.5 is written as 1.2.
#   A tuplet is only ever the last edit.
# - A fermata: the package has no `Fermata` of its own to hand a note.
EDITS = {
    'Note': lambda r: r.choice([
        ('accidental', r.choice(ACCIDENTALS)),
        ('name', r.choice(NAMES)),
        ('octave', r.randint(2, 6)),
        ('type', r.choice(TYPES)),
        ('dots', r.randint(0, 2)),
        ('quarterLength', r.choice(LENGTHS)),
        ('velocity', r.randint(1, 127)),
        ('tie', r.choice(['start', 'stop', 'continue'])),
        ('color', r.choice(COLORS)),
        ('lyric', r.choice(['la', 'ß', 'é'])),
        ('stem', r.choice(['up', 'down'])),
        ('articulation', r.choice(['Staccato', 'Accent'])),
        ('transpose', r.choice(['M2', 'm3', '-P4', 'P8', 'A1'])),
        ('notehead', r.choice(['x', 'diamond', 'slash'])),
        ('display', r.choice([True, False])),
        ('beam', r.choice(['start', 'stop', 'continue'])),
    ]),
    'Chord': lambda r: r.choice([
        ('add', named(r)),
        ('remove', r.randint(0, 3)),
        ('note_accidental', r.randint(0, 2), r.choice(ACCIDENTALS)),
        ('pitch_octave', r.randint(0, 2), r.randint(3, 5)),
        ('note_tie', r.randint(0, 2), r.choice(['start', 'stop'])),
        ('note_color', r.randint(0, 2), r.choice(COLORS)),
        ('type', r.choice(TYPES)),
        ('dots', r.randint(0, 2)),
        ('quarterLength', r.choice(LENGTHS)),
        ('velocity', r.randint(1, 127)),
        ('color', r.choice(COLORS)),
        ('transpose', r.choice(['M2', 'm3', '-P4', 'P8'])),
    ]),
    'Pitch': lambda r: r.choice([
        ('accidental', r.choice(ACCIDENTALS)),
        ('step', r.choice(STEPS)),
        ('octave', r.randint(1, 7)),
        ('name', r.choice(NAMES)),
        ('transpose', r.choice(['M2', 'm3', '-P4', 'P8', 'd5'])),
        ('microtone', r.choice([-30, 20, 50])),
        ('display', r.choice([True, False])),
    ]),
    'Key': lambda r: ('tonic_name', r.choice(NAMES)),
    'Scale': lambda r: r.choice([
        ('tonic_name', r.choice(NAMES)),
        ('tonic_octave', r.randint(2, 5)),
    ]),
    'TimeSignature': lambda r: r.choice([
        ('numerator', r.randint(1, 12)),
        ('denominator', r.choice([2, 4, 8, 16])),
        ('ratioString', r.choice(METERS)),
        ('symbol', r.choice(['common', 'cut', 'normal', ''])),
        ('symbolizeDenominator', r.random() < 0.5),
        ('barDuration', r.choice([1.0, 2.5, 3.0])),
        ('load', r.choice(METERS + ['c', 'cut'])),
        ('resetValues', r.choice(METERS + ['c'])),
    ]),
    'MetronomeMark': lambda r: r.choice([
        ('number', r.choice([40, 60, 72.5, 96, 120, 200])),
        ('text', r.choice(['Allegro', 'Adagio', 'Presto'])),
        ('referent_type', r.choice(['half', 'eighth', 'quarter'])),
        ('referent_dots', r.randint(0, 1)),
        ('numberSounding', r.choice([None, 80, 100])),
    ]),
}


def plan(seed):
    """What a seed makes, the kind turn and turn about, and one to six
    edits; printed, it is the case a failure names."""
    r = random.Random(seed)
    kind = KINDS[seed % len(KINDS)]
    if kind == 'Note':
        start = (named(r), r.choice(LENGTHS))
    elif kind == 'Chord':
        start = ([named(r) for _ in range(r.randint(1, 4))], r.choice(LENGTHS))
    elif kind == 'Pitch':
        start = (named(r, 2, 6),)
    elif kind == 'Key':
        start = (r.choice(NAMES + [name.lower() for name in NAMES]),)
    elif kind == 'Scale':
        start = (r.choice(SCALES), named(r))
    elif kind == 'TimeSignature':
        start = (r.choice(METERS),)
    else:
        start = (
            r.choice([None, 60, 90, 132]),
            r.choice([None, 'Andante']),
            r.choice([None, 0.5, 1.5, 2.0]),
        )
    edits = [EDITS[kind](r) for _ in range(r.randint(1, 6))]
    if kind in ('Note', 'Chord') and r.random() < 0.3:
        edits.append(('tuplet', r.choice([(3, 2), (5, 4)])))
    return kind, start, edits


def made(lib, kind, start):
    if kind == 'Note':
        return lib.Note(start[0], quarterLength=start[1])
    if kind == 'Chord':
        return lib.Chord(start[0], quarterLength=start[1])
    if kind == 'Pitch':
        return lib.Pitch(start[0])
    if kind == 'Key':
        return lib.Key(start[0])
    if kind == 'Scale':
        return getattr(lib, start[0])(start[1])
    if kind == 'TimeSignature':
        return lib.TimeSignature(start[0])
    number, text, referent = start
    given = {}
    if number is not None:
        given['number'] = number
    if text is not None:
        given['text'] = text
    if referent is not None:
        given['referent'] = lib.Duration(referent)
    return lib.MetronomeMark(**given)


def edit_pitch(lib, obj, held, what, value):
    """An edit to a note's pitch or a pitch; False if it is none."""
    if what == 'accidental':
        held.accidental = lib.Accidental(value)
    elif what == 'name':
        held.name = value
    elif what == 'octave':
        held.octave = value
    elif what == 'step':
        held.step = value
    elif what == 'microtone':
        held.microtone = value
    elif what == 'display':
        if held.accidental is not None:
            held.accidental.displayStatus = value
    elif what == 'transpose':
        obj.transpose(value, inPlace=True)
    else:
        return False
    return True


def edit_sounding(lib, obj, what, value):
    """An edit to a note's or a chord's duration, volume or style; False
    if it is none."""
    if what == 'type':
        obj.duration.type = value
    elif what == 'dots':
        obj.duration.dots = value
    elif what == 'quarterLength':
        obj.quarterLength = value
    elif what == 'tuplet':
        obj.duration.appendTuplet(lib.Tuplet(*value))
    elif what == 'velocity':
        obj.volume.velocity = value
    elif what == 'color':
        obj.style.color = value
    elif what == 'transpose':
        obj.transpose(value, inPlace=True)
    else:
        return False
    return True


def edit_note(lib, obj, what, value):
    if what == 'tie':
        obj.tie = lib.Tie(value)
    elif what == 'lyric':
        obj.lyric = value
    elif what == 'stem':
        obj.stemDirection = value
    elif what == 'articulation':
        obj.articulations.append(getattr(lib, value)())
    elif what == 'notehead':
        obj.notehead = value
    elif what == 'beam':
        obj.beams.append(value)
    elif not edit_sounding(lib, obj, what, value):
        edit_pitch(lib, obj, obj.pitch, what, value)


def edit_chord(lib, obj, what, *values):
    if what == 'add':
        obj.add(values[0])
        return
    if edit_sounding(lib, obj, what, values[0]):
        return
    index = values[0] % len(obj.notes)
    if what == 'remove':
        if len(obj.notes) > 1:
            obj.remove(obj.pitches[index])
    elif what == 'note_accidental':
        obj.notes[index].pitch.accidental = lib.Accidental(values[1])
    elif what == 'pitch_octave':
        obj.pitches[index].octave = values[1]
    elif what == 'note_tie':
        obj.notes[index].tie = lib.Tie(values[1])
    elif what == 'note_color':
        obj.notes[index].style.color = values[1]


def edit(lib, obj, kind, edit):
    what, *values = edit
    if kind == 'Note':
        edit_note(lib, obj, what, values[0] if values else None)
    elif kind == 'Chord':
        edit_chord(lib, obj, what, *values)
    elif kind == 'Pitch':
        edit_pitch(lib, obj, obj, what, values[0])
    elif what == 'tonic_name':
        obj.tonic.name = values[0]
    elif what == 'tonic_octave':
        obj.tonic.octave = values[0]
    elif what == 'barDuration':
        obj.barDuration = lib.Duration(values[0])
    elif what in ('load', 'resetValues'):
        getattr(obj, what)(values[0])
    elif what == 'referent_type':
        obj.referent.type = values[0]
    elif what == 'referent_dots':
        obj.referent.dots = values[0]
    else:
        setattr(obj, what, values[0])


def said(obj, kind):
    """What a caller reads back, beyond the repr."""
    if kind in ('Note', 'Chord'):
        out = (
            [p.nameWithOctave for p in obj.pitches],
            float(obj.quarterLength),
            obj.duration.type,
            obj.duration.dots,
            obj.volume.velocity,
            obj.style.color,
        )
        if kind == 'Note':
            out += (repr(obj.tie), repr(obj.pitch.accidental))
        return out
    if kind == 'Pitch':
        return (obj.nameWithOctave, obj.ps, obj.midi, repr(obj.accidental))
    if kind == 'Key':
        return (obj.sharps, obj.mode, [p.name for p in obj.pitches], obj.tonic.nameWithOctave)
    if kind == 'Scale':
        return ([p.nameWithOctave for p in obj.pitches], obj.tonic.nameWithOctave)
    if kind == 'TimeSignature':
        return (
            obj.ratioString,
            obj.numerator,
            obj.denominator,
            obj.symbol,
            obj.symbolizeDenominator,
            obj.beatCount,
            float(obj.beatDuration.quarterLength),
            float(obj.barDuration.quarterLength),
        )
    return (obj.number, obj.text, float(obj.referent.quarterLength), obj.getQuarterBPM())


def read(obj, kind):
    return repr((repr(obj), str(obj), said(obj, kind)))


def written(lib, obj, kind):
    """The object written as MusicXML in a stream of its own, a note after
    a mark, with notation made; nothing for a pitch or a scale, which no
    stream holds. The date and the writer's name are taken out."""
    if kind in ('Pitch', 'Scale'):
        return ''
    held = lib.Stream()
    held.append(obj)
    if kind in ('Key', 'TimeSignature', 'MetronomeMark'):
        held.append(lib.Note('C4'))
    if lib is M21:
        text = m21ToXml.GeneralObjectExporter(held).parse().decode('utf-8')
    else:
        handle, path = tempfile.mkstemp(suffix='.musicxml')
        os.close(handle)
        try:
            held.write('musicxml', fp=path)
            with open(path, encoding='utf-8') as written_file:
                text = written_file.read()
        finally:
            os.remove(path)
    text = re.sub(r'<encoding-date>.*?</encoding-date>', '', text)
    text = re.sub(r'<software>.*?</software>', '', text)
    return '\n'.join(line for line in text.splitlines() if line.strip())


def described(lib, seed):
    """The object made and edited as the seed says, read as it stands, as
    deep-copied and as pickled and read back, and as written."""
    kind, start, edits = plan(seed)
    obj = made(lib, kind, start)
    for each in edits:
        edit(lib, obj, kind, each)
    return (
        read(obj, kind),
        read(copy.deepcopy(obj), kind),
        read(pickle.loads(pickle.dumps(obj)), kind),
        written(lib, obj, kind),
    )


def theirs(seed):
    return described(M21, seed)


def ours(seed):
    return described(rs, seed)
"#;

/// One object read: as it stands, deep-copied, pickled, and written.
type Reading = (String, String, String, String);

#[test]
fn edited_objects_read_as_music21s_do() {
    pyo3::append_to_inittab!(music21_rs_facade);
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let failures = Python::attach(|py| -> PyResult<Vec<String>> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let cases = PyModule::from_code(
            py,
            &std::ffi::CString::new(CASES).expect("no nul in the cases"),
            c"facade_edit_cases.py",
            c"facade_edit_cases",
        )?;
        let mut failures = Vec::new();
        for seed in 0..GENERATED {
            let plan = cases.getattr("plan")?.call1((seed,))?.repr()?.to_string();
            let theirs: Reading = match cases.getattr("theirs")?.call1((seed,)) {
                Ok(read) => read.extract()?,
                Err(error) => {
                    failures.push(format!("seed {seed} {plan}: music21 refused it: {error}"));
                    continue;
                }
            };
            let ours: Reading = match cases.getattr("ours")?.call1((seed,)) {
                Ok(read) => read.extract()?,
                Err(error) => {
                    failures.push(format!("seed {seed} {plan}: the package raised {error}"));
                    continue;
                }
            };
            for (what, theirs, ours) in [
                ("as made", &theirs.0, &ours.0),
                ("deep-copied", &theirs.1, &ours.1),
                ("pickled", &theirs.2, &ours.2),
            ] {
                if theirs != ours {
                    failures.push(format!(
                        "seed {seed} {plan}, {what}:\n  music21 {theirs}\n  ours    {ours}"
                    ));
                }
            }
            if let Some(difference) =
                first_difference(&normalize_ids(&ours.3), &normalize_ids(&theirs.3))
            {
                failures.push(format!("seed {seed} {plan}, written: {difference}"));
            }
        }
        Ok(failures)
    })
    .unwrap_or_else(|error| panic!("running the cases: {error}"));

    assert!(
        failures.is_empty(),
        "{} of {GENERATED} edited objects differ:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
