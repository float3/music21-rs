#!/usr/bin/env python3
from __future__ import annotations

import subprocess
import sys
import types
from pathlib import Path

ROOT = Path(__file__).resolve().parent
MUSIC21_SRC = ROOT / "music21" / "music21"
sys.path.insert(0, str(MUSIC21_SRC.parent))

# Avoid importing music21.__init__ (which pulls optional deps like requests).
music21_pkg = types.ModuleType("music21")
music21_pkg.__path__ = [str(MUSIC21_SRC)]
sys.modules["music21"] = music21_pkg

from music21.chord import Chord  # type: ignore  # noqa: E402


STR_CASES: list[tuple[str, str]] = [
    ("C E G", "C-major triad"),
    ("C E- G", "C-minor triad"),
    ("C E G B-", "C-dominant seventh chord"),
    ("C E G B", "C-major seventh chord"),
    ("C E- G B-", "C-minor seventh chord"),
    ("C E- G- B-", "C-half-diminished seventh chord"),
    ("C E- G- B--", "C-diminished seventh chord"),
    ("C E G B- D", "C-dominant-ninth"),
    ("C E G B D", "C-major-ninth chord"),
    ("C E- G B- D", "C-minor-ninth chord"),
    ("G2 B2 D3 F3", "G-dominant seventh chord"),
    ("B2 D3 F3 A3", "B-half-diminished seventh chord"),
]

INT_CASES: list[tuple[list[int], str]] = [
    ([1, 2, 3, 4, 5, 10], "forte class 6-36B above C#"),
]


def run_python_cases() -> dict[str, str]:
    outputs: dict[str, str] = {}

    for notes, expected in STR_CASES:
        got = Chord(notes).pitchedCommonName
        assert (
            got == expected
        ), f"Python mismatch for {notes!r}: expected {expected!r}, got {got!r}"
        outputs[notes] = got
        print(f"PY PASS {notes:<22} -> {got}")

    for notes, expected in INT_CASES:
        got = Chord(notes).pitchedCommonName
        key = str(notes)
        assert (
            got == expected
        ), f"Python mismatch for {notes!r}: expected {expected!r}, got {got!r}"
        outputs[key] = got
        print(f"PY PASS {key} -> {got}")

    return outputs


def run_rust_bin_and_parse() -> dict[str, str]:
    cmd = ["cargo", "run", "--bin", "test"]
    proc = subprocess.run(
        cmd,
        cwd=ROOT,
        check=True,
        text=True,
        capture_output=True,
    )

    outputs: dict[str, str] = {}
    for line in proc.stdout.splitlines():
        if not line.startswith("PASS "):
            continue
        payload = line.removeprefix("PASS ")
        if "->" not in payload:
            continue
        key_raw, value_raw = payload.split("->", 1)
        outputs[key_raw.strip()] = value_raw.strip()

    return outputs


def compare_python_to_rust(python_outputs: dict[str, str], rust_outputs: dict[str, str]) -> None:
    missing_from_rust = sorted(set(python_outputs) - set(rust_outputs))
    missing_from_python = sorted(set(rust_outputs) - set(python_outputs))
    assert not missing_from_rust, f"Missing from Rust output: {missing_from_rust}"
    assert not missing_from_python, f"Extra in Rust output: {missing_from_python}"

    for key, py_value in python_outputs.items():
        rust_value = rust_outputs[key]
        assert (
            py_value == rust_value
        ), f"Rust/Python mismatch for {key!r}: python={py_value!r} rust={rust_value!r}"
        print(f"MATCH {key!r} -> {py_value}")


def main() -> None:
    python_outputs = run_python_cases()
    rust_outputs = run_rust_bin_and_parse()
    compare_python_to_rust(python_outputs, rust_outputs)
    print("All Python and Rust chord outputs match.")


if __name__ == "__main__":
    main()
