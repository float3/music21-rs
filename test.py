#!/usr/bin/env python3
import sys

sys.path.append("./music21")
from music21.chord import Chord, tables

print(Chord([0, 1, 2, 3, 4, 5, 12, 13]).pitchedCommonName)
print(Chord([0, 1, 2, 12, 13]).pitchedCommonName)
# print(Chord("C E G").pitchedCommonName)
