"""music21's `common` package, as `provide_music21` answers it from Rust."""

import os
import pathlib
import subprocess
import sys

import pytest

import music21_rs as m


def _run_without_music21(code, cwd):
    """Runs `code` where this package can be imported and music21 cannot."""
    package_root = pathlib.Path(m.__file__).resolve().parents[1]
    if (package_root / "music21").exists():
        pytest.skip("music21 is installed beside this package")
    env = dict(os.environ, PYTHONPATH=str(package_root))
    done = subprocess.run(
        [sys.executable, "-S", "-c", code],
        capture_output=True,
        text=True,
        cwd=cwd,
        env=env,
    )
    assert done.returncode == 0, done.stderr
    return done.stdout.splitlines()


def test_common_answers_music21s_names_in_its_modules(tmp_path):
    code = """
import music21_rs
music21_rs.provide_music21()
from fractions import Fraction
from music21 import common
from music21.common.numberTools import opFrac
from music21.common import stringTools
print(opFrac(1 / 3), opFrac(0.25), opFrac(3), opFrac(Fraction(3, 4)))
print(common.opFrac is opFrac, common.numberTools.opFrac.__module__)
print(common.mixedNumeral(Fraction(-31, 7)), common.toRoman(1999))
print(stringTools.parenthesesMatch('[not [mix] very]', open='[', close=']'))
print(common.findFormat('.mxl'), common.getMd5('test'))
print(common.numberTools.__all__[:2], 'opFrac' in common.__all__, common.DEBUG_ALL)
print(sorted(common.__all__))
"""
    assert _run_without_music21(code, tmp_path) == [
        "1/3 0.25 3.0 0.75",
        "True music21.common.numberTools",
        "-4 3/7 MCMXCIX",
        "[ParenthesesMatch(start=1, end=15, text='not [mix] very', "
        "nested=[ParenthesesMatch(start=6, end=9, text='mix', nested=[])])]",
        "('musicxml', '.musicxml') 098f6bcd4621d373cade4e832627b4f6",
        "['addFloatPrecision', 'approximateGCD'] False 255",
        "['classTools', 'decorators', 'enums', 'fileTools', 'formats', 'misc', "
        "'numberTools', 'objects', 'parallel', 'pathTools', 'stringTools', "
        "'types', 'weakrefTools']",
    ]


def test_a_parentheses_match_is_a_value_like_music21s_dataclass(tmp_path):
    code = """
import music21_rs
music21_rs.provide_music21()
from music21.common.stringTools import ParenthesesMatch
found = ParenthesesMatch(1, 4, 'abc', [])
found.nested.append(ParenthesesMatch(2, 3, 'b', []))
print(found == ParenthesesMatch(1, 4, 'abc', [ParenthesesMatch(2, 3, 'b', [])]))
print(found != ParenthesesMatch(1, 4, 'abc', []), found == (1, 4, 'abc', []))
print(ParenthesesMatch.__module__, ParenthesesMatch.__match_args__)
"""
    assert _run_without_music21(code, tmp_path) == [
        "True",
        "True False",
        "music21.common.stringTools ('start', 'end', 'text', 'nested')",
    ]


def test_number_tools_raise_what_music21_raises(tmp_path):
    code = """
import music21_rs
music21_rs.provide_music21()
from music21 import common
for call in (
    lambda: common.opFrac(float('nan')),
    lambda: common.opFrac(float('inf')),
    lambda: common.opFrac('1'),
    lambda: common.toRoman('hi'),
    lambda: common.decimalToTuplet(-0.02),
    lambda: common.contiguousList([]),
    lambda: common.roundToHalfInteger('1.5'),
):
    try:
        call()
    except Exception as error:
        print(type(error).__name__, error)
"""
    assert _run_without_music21(code, tmp_path) == [
        "ValueError cannot convert NaN to integer ratio",
        "OverflowError cannot convert Infinity to integer ratio",
        "TypeError Cannot convert num: 1",
        "TypeError expected integer, got <class 'str'>",
        "ZeroDivisionError number must be greater than zero",
        "IndexError list index out of range",
        "TypeError unsupported operand type(s) for divmod(): 'str' and 'float'",
    ]
