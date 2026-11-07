"""Reads a score through music21's reader and through the crate's, and says
where the two scores differ.

    PYTHONPATH=../music21 python tests/converter_parity.py musicxml FILE...

With the wheel installed, `converter.parse` reads a format through the
crate once `route_converters` routes it, so the two reads differ in the
reader alone. Each score is listed element by element -- where it stands,
its class, offset and length, what it says and how it looks -- with its
spanners naming their elements by place, its metadata, and what music21's
MusicXML writer makes of it, and the listings are compared line by line.
The wheel's tests import `listed` and `differences` from here.
"""

import contextlib
import difflib
import io
import re
import sys
import time

# What names an object by where it is in memory, which differs every read.
ADDRESS = re.compile(r'0x[0-9a-f]+|\b[0-9]{9,}\b')

# What an element says beyond its class, offset and length, by class.
ATTRIBUTES = {
    'GeneralNote': (
        'fullName', 'tie', 'lyrics', 'articulations', 'expressions',
        'stemDirection', 'beams', 'notehead', 'noteheadFill',
        'noteheadParenthesis', 'duration.tuplets', 'duration.isGrace',
        'duration.linked', 'volume.velocity', 'stemDirection',
        'hasVolumeInformation', 'editorial',
    ),
    'Rest': ('stepShift', 'fullMeasure'),
    'Clef': ('sign', 'line', 'octaveChange'),
    'KeySignature': ('sharps', 'mode'),
    'TimeSignature': ('ratioString', 'symbol', 'symbolizeDenominator'),
    'TempoIndication': ('number', 'text', 'referent', 'numberSounding', 'placement'),
    'Dynamic': ('value', 'placement'),
    'Expression': ('placement',),
    'TextExpression': ('content',),
    'Barline': ('type', 'location', 'pause'),
    'Repeat': ('direction', 'times'),
    'Instrument': (
        'partId', 'partName', 'instrumentName', 'instrumentAbbreviation',
        'midiProgram', 'midiChannel', 'instrumentId', 'transposition',
    ),
    'PageLayout': (
        'pageNumber', 'leftMargin', 'rightMargin', 'topMargin',
        'bottomMargin', 'pageHeight', 'pageWidth', 'isNew',
    ),
    'SystemLayout': ('leftMargin', 'rightMargin', 'distance', 'topDistance', 'isNew'),
    'StaffLayout': ('distance', 'staffNumber', 'staffSize', 'staffLines', 'hidden', 'staffType'),
    'ScoreLayout': ('scalingMillimeters', 'scalingTenths', 'musicFont', 'wordFont'),
    'TextBox': ('content', 'page'),
    'GlobalComment': ('comment',),
    'MiscTandem': ('tandem',),
    'Spanner': ('placement', 'type', 'idLocal', 'completeStatus'),
    'Measure': ('showNumber', 'layoutWidth', 'numberSuffix', 'paddingLeft', 'paddingRight'),
    'Part': ('partName', 'partAbbreviation'),
    'Music21Object': ('style',),
}

# What a style says, where it says anything.
STYLE = (
    'absoluteX', 'absoluteY', 'relativeX', 'relativeY', 'justify',
    'alignHorizontal', 'alignVertical', 'fontSize', 'fontFamily',
    'fontStyle', 'fontWeight', 'color', 'hideObjectOnPrint', 'enclosure',
    'placement',
)


def _value(element, name):
    value = element
    for part in name.split('.'):
        try:
            value = getattr(value, part)
        except Exception as error:  # noqa: BLE001 - a property may raise
            return f'<{type(error).__name__}>'
        if callable(value) and not isinstance(value, type):
            return '<callable>'
    if isinstance(value, (list, tuple)):
        return '[' + ', '.join(_shown(item) for item in value) + ']'
    return _shown(value)


def _shown(value):
    if hasattr(value, 'classes') and 'Style' in getattr(value, 'classes', ()):
        return _style(value)
    if type(value).__name__ == 'Beams':
        return '|'.join(f'{beam.type}/{beam.direction}' for beam in value.beamsList)
    if type(value).__name__ == 'Lyric':
        return f'{value.number}:{value.syllabic}:{value.text}:{value.identifier}'
    if type(value).__name__ == 'Editorial':
        return repr(dict(value)) if len(value) else '{}'
    if hasattr(value, 'classes'):
        # An object of music21's carries its id, which a read makes new.
        return _repr(value)
    return _repr(value)


# Whether to pass over where things are drawn and in what font and
# colour, to see what else differs: `--no-style`.
IGNORE_STYLE = False

# What MusicXML says of where and how a thing is drawn.
DRAWN = re.compile(
    r' (?:default-[xy]|relative-[xy]|color|font-[a-z]+|width|justify|halign|valign)="[^"]*"'
)


def _style(style):
    if IGNORE_STYLE:
        return 'style()'
    said = []
    for name in STYLE:
        value = getattr(style, name, None)
        if value not in (None, False):
            said.append(f'{name}={value!r}')
    return 'style(' + ' '.join(said) + ')'


def _attributes(element):
    said = []
    classes = element.classes
    for owner, names in ATTRIBUTES.items():
        if owner not in classes:
            continue
        for name in names:
            said.append(f'{name}={_value(element, name)}')
    if isinstance(getattr(element, 'id', None), str):
        said.append(f'id={element.id}')
    return ' '.join(said)


def listed(score):
    """Every element of `score`, one line each, in the order its streams
    hold them, then its spanners, its metadata and its MusicXML."""
    lines = []
    places = {}
    _list_stream(score, '', lines, places)
    lines.append('-- spanners')
    for spanner in score.spannerBundle:
        joined = ', '.join(places.get(id(each), '?') for each in spanner.getSpannedElements())
        lines.append(f'{type(spanner).__name__} [{joined}] {_attributes(spanner)}')
    lines.append('-- metadata')
    metadata = score.metadata
    if metadata is not None:
        for name, value in metadata.all():
            lines.append(f'{name}: {value}')
    lines.append('-- musicxml')
    lines.extend(_musicxml(score))
    return lines


def _list_stream(stream, where, lines, places):
    # An element is named by where it stands rather than by how many came
    # before it, so one missing does not move every name after it.
    for element in stream.elements:
        offset = element.getOffsetBySite(stream)
        name = type(element).__name__
        pitches = None if element.isStream else getattr(element, 'pitches', None)
        if pitches:
            name += ' ' + ' '.join(p.nameWithOctave for p in pitches)
        places[id(element)] = f'{where}@{offset} {name}'
        line = (
            f'{where} {offset} {type(element).__name__}'
            f' {element.duration.quarterLength} {element.priority}'
        )
        if element.isStream:
            number = getattr(element, 'number', '')
            lines.append(f'{line} {number} {_attributes(element)}')
            inner = f'{type(element).__name__[0]}{number if number != "" else ""}'
            if getattr(element, 'id', None) and isinstance(element.id, str):
                inner = element.id
            _list_stream(element, f'{where}/{inner}', lines, places)
            continue
        lines.append(f'{line} {_attributes(element)} | {_repr(element)}')


def _repr(element):
    return ADDRESS.sub('', repr(element)).replace(' object at >', '>')


def _musicxml(score):
    from music21.musicxml.m21ToXml import GeneralObjectExporter

    try:
        with contextlib.redirect_stderr(io.StringIO()):
            written = GeneralObjectExporter(score).parse().decode()
    except Exception as error:  # noqa: BLE001 - the difference is the finding
        return [f'<write failed: {type(error).__name__}: {error}>']
    # The date written and every id are made new by each write.
    written = written.split('</identification>', 1)[-1]
    written = re.sub(r' id="[^"]*"', '', written)
    if IGNORE_STYLE:
        written = DRAWN.sub('', written)
    return ADDRESS.sub('', written).splitlines()


def read(path, fmt, routed):
    """`path` as `converter.parse` reads it, with `fmt` routed through the
    crate or not; and whether the crate read it, and how long it took."""
    import music21_rs
    from music21 import converter

    music21_rs.route_converters([fmt] if routed else [])
    made = converter.Converter()
    began = time.perf_counter()
    made.parseFile(path, forceSource=True, storePickle=False)
    took = time.perf_counter() - began
    return made.stream, getattr(made.subConverter, 'readByCrate', False), took


def differences(path, fmt, context=0):
    """The lines where the crate's score of `path` differs from music21's,
    as a unified diff, and the two read times."""
    theirs, _, their_time = read(path, fmt, routed=False)
    ours, by_crate, our_time = read(path, fmt, routed=True)
    if not by_crate:
        return ['<the crate did not read it>'], their_time, our_time
    diff = difflib.unified_diff(listed(theirs), listed(ours), 'music21', 'crate', n=context, lineterm='')
    return list(diff), their_time, our_time


def main(arguments):
    import warnings

    import music21_rs

    warnings.simplefilter('ignore')
    music21_rs.install_into_music21()
    fmt, paths = arguments[0], arguments[1:]
    global IGNORE_STYLE
    shown = 40
    if paths and paths[0] == '--no-style':
        IGNORE_STYLE = True
        paths = paths[1:]
    if paths and paths[0].startswith('--lines='):
        shown = int(paths[0].split('=', 1)[1])
        paths = paths[1:]
    matched = 0
    for path in paths:
        try:
            diff, their_time, our_time = differences(path, fmt)
        except Exception as error:  # noqa: BLE001 - report and go on
            diff, their_time, our_time = [f'<{type(error).__name__}: {error}>'], 0, 0
        name = path.rsplit('/', 1)[-1]
        if not diff:
            matched += 1
            print(f'SAME {name} music21 {their_time:.3f}s crate {our_time:.3f}s')
            continue
        changed = sum(1 for line in diff if line[:1] in '+-' and line[:3] not in ('+++', '---'))
        print(f'DIFF {name} ({changed} lines) music21 {their_time:.3f}s crate {our_time:.3f}s')
        for line in diff[2:2 + shown]:
            print('   ', line[:300])
    print(f'{matched}/{len(paths)} the same')


if __name__ == '__main__':
    main(sys.argv[1:])
