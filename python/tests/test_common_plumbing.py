"""The `music21.common` plumbing modules the extension builds whole, as
`provide_music21()` answers them where music21 is not installed: each
checked against what music21's own module does."""

import os
import pathlib
import subprocess
import sys
import weakref

import pytest

import music21_rs as m


def _run_provided(code, cwd):
    """Runs `code` after `provide_music21()`, in an interpreter that can
    import this package and not music21."""
    package_root = pathlib.Path(m.__file__).resolve().parents[1]
    if (package_root / "music21").exists():
        pytest.skip("music21 is installed beside this package")
    env = dict(os.environ, PYTHONPATH=str(package_root), PYTHONIOENCODING="utf-8")
    prelude = "import music21_rs\nmusic21_rs.provide_music21()\n"
    done = subprocess.run(
        [sys.executable, "-S", "-c", prelude + code],
        capture_output=True,
        text=True,
        encoding="utf-8",
        cwd=cwd,
        env=env,
    )
    assert done.returncode == 0, done.stderr
    return done.stdout.splitlines()


def test_class_tools_judge_objects_as_music21_does(tmp_path):
    code = """
from decimal import Decimal
from music21.common import classTools as ct
print(ct.isNum(3.0), ct.isNum(True), ct.isNum(None), ct.isNum('3'), ct.isNum(Decimal('2')))
print(ct.isInt(3), ct.isInt(False), ct.isInt(2.0))
print(ct.isListLike((1,)), ct.isListLike({1}), ct.isListLike('ab'))
print(ct.isIterable(range(3)), ct.isIterable('ab'), ct.isIterable(list))
r = iter(range(1, 5))
print(ct.holdsType([], float), ct.holdsType((4, 'a'), str), ct.holdsType(5, int))
print(ct.classToClassStr(Decimal))
class A: pass
class B(A): pass
cs = ct.getClassSet(B())
print(B in cs, A in cs, 'B' in cs, 'object' in cs, f'{__name__}.A' in cs)
named = ct.getClassSet(B(), classNameTuple=('X',))
print('X' in named, 'B' in named, B in named)
class P:
    a = 1
    b = 2
p = P()
with ct.tempAttribute(p, 'a', 5):
    print(p.a)
with ct.tempAttribute(p, 'a'):
    p.a = 9
try:
    with ct.saveAttributes(p, 'a', 'b'):
        p.a, p.b = 7, 8
        raise KeyError
except KeyError:
    pass
print(p.a, p.b)
with ct.tempAttribute(p, 'a', None):
    print(p.a)
"""
    assert _run_provided(code, tmp_path) == [
        "True False False False True",
        "True False False",
        "True False False",
        "True False False",
        "True False False",
        "Decimal",
        "True True True True True",
        "True False True",
        "5",
        "1 2",
        "None",
    ]


def test_decorators_warn_once_and_cache(tmp_path):
    code = """
import warnings
from music21 import common
from music21.common import decorators

@common.deprecated
def hi(msg):
    return msg

@decorators.deprecated('February 1972', 'September 2099', 'You should be okay.')
def bye(msg):
    return msg

class Holder:
    def __init__(self):
        self._cache = {}
        self.calls = 0

    @decorators.cacheMethod
    def value(self):
        self.calls += 1
        return [self.calls]

    @common.deprecated
    def old(self, x):
        return x * 2

with warnings.catch_warnings(record=True) as caught:
    warnings.simplefilter('always')
    print(hi('a'), hi('b'), bye('c'), Holder().old(2))
for warning in caught:
    print(warning.category.__name__, warning.message, warning.filename == '<string>')
h = Holder()
print(h.value() is h.value(), h.calls, list(h._cache), Holder.value.__name__)
print(hi.__name__, hi.__wrapped__.__name__, hi._isDeprecated)
"""
    assert _run_provided(code, tmp_path) == [
        "a b c 4",
        "Music21DeprecationWarning hi was deprecated and will disappear soon. "
        "Find alternative methods. True",
        "Music21DeprecationWarning bye was deprecated on February 1972 and will "
        "disappear at or after September 2099. You should be okay. True",
        "Music21DeprecationWarning Holder.old was deprecated and will disappear soon. "
        "Find alternative methods. True",
        "True 1 ['Holder.value'] value",
        "hi hi True",
    ]


def test_enums_are_music21s(tmp_path):
    code = """
import enum
from music21.common import enums
print(repr(enums.OffsetSpecial.AT_END), str(enums.OffsetSpecial.AT_END))
print('highestTime' == enums.OffsetSpecial.AT_END, 'lowestOffset' in enums.OffsetSpecial, 6.0 in enums.OffsetSpecial)
# Python before 3.11 has no enum.StrEnum, and a str enum stands in for it.
standard = getattr(enum, 'StrEnum', str)
print(issubclass(enums.ElementSearch, enums.StrEnum), issubclass(enums.StrEnum, standard))
print([m.value for m in enums.MeterDivision], enums.OrnamentDelay.DEFAULT_DELAY.value)
print(repr(enums.GatherSpanners.ALL), bool(enums.GatherSpanners.NONE), bool(enums.GatherSpanners.COMPLETE_ONLY))
print(enums.AppendSpanners.RELATED_ONLY == 'related_only', enums.ContainsEnum is enums.HexEnum)

class Maybe(enums.BooleanEnum):
    YES = True
    NO = False
    MAYBE = 0.5
    NOT_A_CHANCE = (False, 'not a chance')

print(bool(Maybe.MAYBE), bool(Maybe.NOT_A_CHANCE), Maybe.MAYBE == 0.5)
print(Maybe.NOT_A_CHANCE == 'not a chance', Maybe.NOT_A_CHANCE == False, Maybe.NOT_A_CHANCE == True)
print(Maybe.NOT_A_CHANCE == (False, 'not a chance'), Maybe.YES == Maybe.YES, Maybe.YES == Maybe.NO)

class Messages(enums.HexEnum):
    NOTE_ON = 0x90

print(repr(Messages.NOTE_ON), 0x90 in Messages, Messages.hasValue(0x91))
"""
    assert _run_provided(code, tmp_path) == [
        "<OffsetSpecial.AT_END> highestTime",
        "True True False",
        "True True",
        "['fast', 'slow', 'none'] defaultDelay",
        "<GatherSpanners.ALL> False True",
        "True True",
        "True False True",
        "True True False",
        "True True False",
        "<Messages.NOTE_ON: 0x90> True False",
    ]


def test_objects_behave_as_music21s(tmp_path):
    code = """
import copy
import pickle
import warnings
from music21 import common
from music21.common import objects

rc = objects.RelativeCounter(['b', 'b', 'a', 'a', 'a', 'a', 'c', 'd', 'd', 'd'] + ['e'] * 10)
print(list(rc), rc.asProportion()['b'], rc.asPercentage()['e'])
print(list(rc.asPercentage().items()))

class Glissdata(objects.SlottedObjectMixin):
    __slots__ = ('time', 'frequency')

class Loose(Glissdata):
    pass

g = Loose()
g.time = 0.125
g.frequency = 440.0
g.amplitude = 2
print(sorted(g.__getstate__().items()), sorted(g._getSlotsRecursive()))
back = copy.deepcopy(g)
print(back.time, back.frequency, back.amplitude)

class Pair(objects.EqualSlottedObjectMixin):
    __slots__ = ('a', 'id')

one, two = Pair(), Pair()
one.a = two.a = 1
one.id, two.id = 1, 2
print(one == two, one != two, one == 3, Pair.__hash__)

class Frozen(objects.FrozenObject):
    __slots__ = ('x', '_frozenHash')

    def __init__(self, x):
        self.x = x

f = Frozen(3)
for attempt in (lambda: setattr(f, 'x', 4), lambda: f.__setitem__(0, 1)):
    try:
        attempt()
    except TypeError as error:
        print(error)
print(f._frozenHash, hash(f) == hash(Frozen(3)), f._frozenHash is not None, f == Frozen(3))

counter = common.SingletonCounter()
print(counter() < objects.SingletonCounter()())
timer = common.Timer()
timer.stop()
print(timer() == timer(), str(timer))

with warnings.catch_warnings(record=True) as caught:
    warnings.simplefilter('always')
    filled = objects.defaultlist(lambda: True)
filled[2] = 'x'
print(filled[4], list(filled), caught[0].category.__name__)
"""
    lines = _run_provided(code, tmp_path)
    assert lines[:3] == [
        "['e', 'a', 'd', 'b', 'c'] 0.1 50.0",
        "[('e', 50.0), ('a', 20.0), ('d', 15.0), ('b', 10.0), ('c', 5.0)]",
        "[('amplitude', 2), ('frequency', 440.0), ('time', 0.125)] ['frequency', 'time']",
    ]
    assert lines[3:] == [
        "0.125 440.0 2",
        "True False False None",
        "This Frozen instance is immutable.",
        f"<class '__main__.Frozen'> object is not subscriptable",
        "None True True True",
        "True",
        "True 0.0",
        "True [True, True, 'x', True, True] Music21DeprecationWarning",
    ]


def test_types_and_weakrefs(tmp_path):
    code = """
import sys
import typing
from collections.abc import Callable, Iterable
from fractions import Fraction
from music21.common import enums, types, weakrefTools

if sys.version_info >= (3, 12):
    print(types.OffsetQL.__value__, types.OffsetQLIn.__value__)
    print(types.OffsetQLSpecial.__value__)
    print(types.StepName.__value__, types.StreamType.__bound__, types.M21ObjType.__name__)
    print(types.ClassListType.__value__, types.ClassListType.__type_params__)
    print(types.DocOrder.__value__)
else:
    # Before 3.12 there is no TypeAliasType, and an alias is its value.
    # ClassListType has a type variable of its own, as in its statement.
    (M,) = types.ClassListType.__parameters__
    print(types.OffsetQL == typing.Union[float, Fraction],
          types.OffsetQLIn == typing.Union[int, float, Fraction])
    print(types.OffsetQLSpecial == typing.Union[float, Fraction, enums.OffsetSpecial])
    print(types.StepName == typing.Literal['C', 'D', 'E', 'F', 'G', 'A', 'B'],
          types.StreamType.__bound__, types.M21ObjType.__name__)
    print(types.ClassListType == typing.Union[str, Iterable[str], type[M], Iterable[type[M]]],
          M is not types.M21ObjType, M.__name__)
    print(types.DocOrder == list[typing.Union[str, Callable]])

class Mock:
    pass

a = Mock()
ref = weakrefTools.wrapWeakref(a)
print(type(ref).__name__, weakrefTools.wrapWeakref(ref) is ref, weakrefTools.wrapWeakref(5))
print(weakrefTools.unwrapWeakref(ref) is a, weakrefTools.unwrapWeakref(a) is a)
"""
    if sys.version_info >= (3, 12):
        aliases = [
            "float | fractions.Fraction int | float | fractions.Fraction",
            "float | fractions.Fraction | music21.common.enums.OffsetSpecial",
            "typing.Literal['C', 'D', 'E', 'F', 'G', 'A', 'B'] "
            "ForwardRef('music21.stream.Stream') M21ObjType",
            "str | collections.abc.Iterable[str] | type[M21ObjType] | "
            "collections.abc.Iterable[type[M21ObjType]] (M21ObjType,)",
            "list[str | collections.abc.Callable]",
        ]
    else:
        aliases = [
            "True True",
            "True",
            "True ForwardRef('music21.stream.Stream') M21ObjType",
            "True True M21ObjType",
            "True",
        ]
    assert _run_provided(code, tmp_path) == [
        *aliases,
        # `weakref.ref` is named `weakref` before Python 3.11.
        f"{weakref.ref.__name__} True 5",
        "True True",
    ]


def test_parallel_runs_in_order_and_reports(tmp_path):
    code = """
from music21.common import parallel

def report(position, total, output, given=None):
    print(position, total, output, given)

print(parallel.runNonParallel([1, -2, 3], abs, updateFunction=True))
parallel.runNonParallel([1, -2, 3, -4], abs, updateFunction=report, updateMultiply=2,
                        updateSendsIterable=True)
print(parallel.runNonParallel([(2, 3), (4, 1)], pow, unpackIterable=True))
print(parallel.cpus() >= 1, isinstance(parallel.safeToParallize(), bool))
"""
    assert _run_provided(code, tmp_path) == [
        "Done 0 tasks of 3",
        "Done 3 tasks of 3",
        "[1, 2, 3]",
        "0 4 1 1",
        "1 4 2 -2",
        "2 4 3 3",
        "3 4 4 -4",
        "[8, 4]",
        "True True",
    ]


def test_paths_and_files(tmp_path):
    (tmp_path / "real").mkdir()
    (tmp_path / "lines.txt").write_bytes(b"a\r\nb\rc")
    code = f"""
import gzip, io, os, pathlib, pickle, zipfile
from music21.common import pathTools, fileTools
from music21 import exceptions21
root = {str(tmp_path)!r}
print(pathTools.cleanpath(root + '/real/../x') == os.path.join(root, 'x'))
# Windows collapses 'none/..' before asking the disk, and music21 with it.
missing = os.path.join(root, 'x') if os.name == 'nt' else root + '/none/../x'
print(pathTools.cleanpath(root + '/none/../x') == missing)
print(isinstance(pathTools.cleanpath(pathlib.Path(root)), pathlib.Path),
      isinstance(pathTools.cleanpath(pathlib.Path(root), returnPathlib=False), str))
print(pathTools.cleanpath('rel') == os.path.join(os.getcwd(), 'rel'))
print(pathTools.relativepath(root + '/real', root))
with fileTools.cd(root + '/real'):
    print(os.path.basename(os.getcwd()))
print(os.getcwd() == root)
print(repr(fileTools.readFileEncodingSafe(root + '/lines.txt')))
buffer = io.BytesIO()
with zipfile.ZipFile(buffer, 'w', zipfile.ZIP_DEFLATED) as zf:
    zf.writestr('score.xml', '<score/>')
with zipfile.ZipFile(buffer) as zf:
    print(fileTools.readZipMember(zf, 'score.xml'), fileTools.MAX_UNCOMPRESSED_BYTES)
with gzip.open(root + '/data.p.gz', 'wb') as out:
    out.write(pickle.dumps({{'a': pathlib.PurePosixPath('x')}}))
print(fileTools.readPickleGzip(root + '/data.p.gz'))
with gzip.open(root + '/bad.p.gz', 'wb') as out:
    out.write(b'not a pickle')
try:
    fileTools.readPickleGzip(root + '/bad.p.gz')
except exceptions21.Music21Exception as error:
    print(str(error).startswith('Cannot load file'), error.__cause__ is not None)
print(pathlib.WindowsPath is fileTools._storedPathlibClasses['windowsPath'])
fileTools.runSubprocessCapturingStderr(['true'])
try:
    fileTools.runSubprocessCapturingStderr(['sh', '-c', 'echo oops >&2; exit 3'])
except OSError as error:
    print(repr(str(error)))
"""
    assert _run_provided(code, tmp_path) == [
        "True",
        "True",
        "True True",
        "True",
        "real",
        "real",
        "True",
        "'a\\nb\\nc'",
        "b'<score/>' 268435456",
        "{'a': PurePosixPath('x')}",
        "True True",
        "True",
        "'oops\\n'",
    ]
