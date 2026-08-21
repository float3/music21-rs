#!/usr/bin/env python3
"""Regenerates the music21 expectation fixtures from the submodule.

Writes data/scale_expectations.toml and data/chord_type_expectations.toml.

The library's scale tables are verified against this file, so it is the
ground truth for what music21 actually produces. Unlike the chord-table
pipeline, this needs a full music21 import and therefore music21's own
dependencies, which is why the output is committed rather than generated
during a build:

    uv venv .m21venv --python 3.12
    uv pip install --python .m21venv chardet joblib jsonpickle more_itertools         numpy requests webcolors
    .m21venv/Scripts/python python-parity/generate_scale_expectations.py

Run from the repository root.
"""
import sys; sys.path.insert(0,'music21')
from music21 import scale, pitch, harmony, meter, interval, key

RS_TO_M21 = {
 "Major":"MajorScale","Minor":"MinorScale","Dorian":"DorianScale","Phrygian":"PhrygianScale",
 "Lydian":"LydianScale","Mixolydian":"MixolydianScale","Locrian":"LocrianScale",
 "Hypodorian":"HypodorianScale","Hypophrygian":"HypophrygianScale","Hypolydian":"HypolydianScale",
 "Hypomixolydian":"HypomixolydianScale","Hypolocrian":"HypolocrianScale","Hypoaeolian":"HypoaeolianScale",
 "HarmonicMinor":"HarmonicMinorScale","MelodicMinor":"MelodicMinorScale",
 "Chromatic":"ChromaticScale","WholeTone":"WholeToneScale","Octatonic":"OctatonicScale",
 "RagAsawari":"RagAsawari","RagMarwa":"RagMarwa",
}
TONICS = ["C4","G4","D4","A4","E4","B4","F#4","C#4","F4","B-4","E-4","A-4","D-4","G-4","C-4"]

out = ["# Expected scale realizations, generated from music21 by",
       "# `python-parity/generate_scale_expectations.py`. Checked in so the",
       "# library can be verified without importing music21.",
       "#",
       f"# music21 {__import__('music21').__version__}",
       "",
       f'music21_version = "{__import__("music21").__version__}"',
       ""]
for rs, m21 in RS_TO_M21.items():
    out.append("[[scale]]")
    out.append(f'scale_type = "{rs}"')
    out.append(f'music21_class = "{m21}"')
    out.append("cases = [")
    for t in TONICS:
        ps = getattr(scale, m21)(t).getPitches(t, pitch.Pitch(t).transpose("P8").nameWithOctave)
        names = ", ".join(f'"{p.nameWithOctave}"' for p in ps)
        out.append(f'    {{ tonic = "{t}", pitches = [{names}] }},')
    out.append("]")
    out.append("")
open("data/scale_expectations.toml","w",encoding="utf-8").write("\n".join(out))
print("wrote data/scale_expectations.toml:", len(RS_TO_M21), "scales x", len(TONICS), "tonics")


# ---------------------------------------------------------------- chord types
out = ["# Expected chord types, generated from music21 by",
       "# `python-parity/generate_scale_expectations.py`. The crate keeps its own",
       "# copy of this table in src/chordsymbol.rs; the parity test compares them",
       "# so the hand-maintained copy cannot drift from upstream unnoticed.",
       "#",
       f"# music21 {__import__('music21').__version__}",
       "",
       f'music21_version = "{__import__("music21").__version__}"',
       ""]
for kind, (notation, abbreviations) in harmony.CHORD_TYPES.items():
    out.append("[[chord_type]]")
    out.append(f'kind = "{kind}"')
    out.append(f'notation = "{notation}"')
    joined = ", ".join(f'"{a}"' for a in abbreviations)
    out.append(f"abbreviations = [{joined}]")
    out.append("")
out.append("[aliases]")
for alias, target in harmony.CHORD_ALIASES.items():
    out.append(f'"{alias}" = "{target}"')
out.append("")
open("data/chord_type_expectations.toml", "w", encoding="utf-8").write(chr(10).join(out))
print("wrote data/chord_type_expectations.toml:", len(harmony.CHORD_TYPES), "types,",
      len(harmony.CHORD_ALIASES), "aliases")


# --------------------------------------------------------------------- meters
out = ["# Expected time-signature properties, generated from music21 by",
       "# `python-parity/generate_scale_expectations.py`. The crate derives these",
       "# from the numerator and denominator alone; music21 derives them from a",
       "# MeterSequence partition tree, so this file is what proves the shortcut",
       "# agrees with the tree.",
       "#",
       f"# music21 {__import__('music21').__version__}",
       "",
       f'music21_version = "{__import__("music21").__version__}"',
       ""]
NUMERATORS = list(range(1, 17)) + [18, 20, 21, 24, 27]
DENOMINATORS = [1, 2, 4, 8, 16, 32]
count = 0
for d in DENOMINATORS:
    for n in NUMERATORS:
        ts = meter.TimeSignature(f"{n}/{d}")
        offsets = ", ".join(repr(float(o)) for o in ts.getBeatOffsets())
        out.append("[[meter]]")
        out.append(f'ratio = "{n}/{d}"')
        out.append(f"bar_quarter_length = {float(ts.barDuration.quarterLength)!r}")
        out.append(f"beat_count = {ts.beatCount}")
        out.append(f"beat_quarter_length = {float(ts.beatDuration.quarterLength)!r}")
        out.append(f"beat_division_count = {ts.beatDivisionCount}")
        out.append(f'beat_count_name = "{ts.beatCountName}"')
        out.append(f'beat_division_count_name = "{ts.beatDivisionCountName}"')
        out.append(f'classification = "{ts.classification}"')
        out.append(f"beat_offsets = [{offsets}]")
        out.append("")
        count += 1
open("data/meter_expectations.toml", "w", encoding="utf-8").write(chr(10).join(out))
print("wrote data/meter_expectations.toml:", count, "time signatures")


# --------------------------------------------------------------- small tables
# Tables the crate re-implements by hand. They are small enough to transcribe,
# which is exactly why they drift silently; these fixtures are what stops that.
M21V = __import__("music21").__version__
out = ["# Expected values for the small music21 tables the crate transcribes by",
       "# hand, generated by `python-parity/generate_scale_expectations.py`.",
       "# Verified by python-parity/tests/table_parity.rs.",
       "",
       f'music21_version = "{M21V}"',
       ""]

for name, modifier in pitch.accidentalNameToModifier.items():
    acc = pitch.Accidental(name)
    out.append("[[accidental]]")
    out.append(f'name = "{name}"')
    out.append(f'modifier = "{modifier}"')
    out.append(f"alter = {float(acc.alter)!r}")
    out.append(f'unicode = "{acc.unicode}"')
    out.append("")

for mode, alter in key.modeSharpsAlter.items():
    out.append("[[mode]]")
    out.append(f'name = "{mode}"')
    out.append(f"sharps_alter = {alter}")
    out.append("")

for prefix in interval.prefixSpecs[1:]:
    for number in range(1, 9):
        try:
            iv = interval.Interval(f"{prefix}{number}")
        except Exception:
            continue
        out.append("[[specifier]]")
        out.append(f'prefix = "{prefix}"')
        out.append(f"number = {number}")
        out.append(f"semitones = {iv.chromatic.semitones}")
        out.append("")

open("data/table_expectations.toml", "w", encoding="utf-8").write(chr(10).join(out))
print("wrote data/table_expectations.toml:",
      len(pitch.accidentalNameToModifier), "accidentals,",
      len(key.modeSharpsAlter), "modes,",
      sum(1 for line in out if line == "[[specifier]]"), "specifier combos")
