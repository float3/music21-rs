#!/usr/bin/env python3
"""Regenerates data/scale_expectations.toml from the music21 submodule.

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
from music21 import scale, pitch

RS_TO_M21 = {
 "Major":"MajorScale","Minor":"MinorScale","Dorian":"DorianScale","Phrygian":"PhrygianScale",
 "Lydian":"LydianScale","Mixolydian":"MixolydianScale","Locrian":"LocrianScale",
 "Hypodorian":"HypodorianScale","Hypophrygian":"HypophrygianScale","Hypolydian":"HypolydianScale",
 "Hypomixolydian":"HypomixolydianScale","Hypolocrian":"HypolocrianScale","Hypoaeolian":"HypoaeolianScale",
 "HarmonicMinor":"HarmonicMinorScale","MelodicMinor":"MelodicMinorScale",
 "Chromatic":"ChromaticScale","WholeTone":"WholeToneScale","Octatonic":"OctatonicScale",
 "RagAsawari":"RagAsawari",
}
TONICS = ["C4","G4","D4","A4","E4","B4","F#4","C#4","F4","B-4","E-4","A-4","D-4","G-4","C-4"]

out = ["# Expected scale realizations, generated from music21 by",
       "# `python-parity/generate_scale_expectations.py`. Checked in so the",
       "# library can be verified without importing music21.",
       "#",
       f"# music21 {__import__('music21').__version__}",
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
