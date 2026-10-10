# Where these scores come from

Each score here is kept twice: as the `.mscx` MuseScore 4 saved and as the
MusicXML MuseScore exported from that file (see `mscx_parity.rs`). Most were
made by opening a score from music21's corpus in MuseScore and saving it; the
rest were written for these tests.

| Files | Made from | Terms |
| --- | --- | --- |
| `bwv66.6` | music21 corpus `bach/bwv66.6` | Bach; the corpus's chorale encoding |
| `corelli-opus3no1-1grave` | music21 corpus `corelli/opus3no1/1grave` | © 2014, CC BY, as the encoding states |
| `foster-brown-hair` | music21 corpus `leadSheet/fosterBrownHair` | public domain, as the encoding states (from Wikifonia) |
| `mozart-k545-exposition` | music21 corpus `mozart/k545/movement1_exposition` | Mozart; the corpus's encoding |
| `mozart-k80-movement3` | music21 corpus `mozart/k80/movement3` | Mozart; the corpus's encoding |
| `schoenberg-opus19-movement2` | music21 corpus `schoenberg/opus19/movement2` | Schoenberg (1911); the corpus's encoding |
| `two-staves` | music21 corpus `cpebach/h186` | C. P. E. Bach; the corpus's encoding |
| `verdi-la-donna-e-mobile` | music21 corpus `verdi/laDonnaEMobile` | Verdi; the corpus's encoding |
| `multiple-verses` | music21 corpus `demos/multiple-verses.xml` | music21's test file |
| `nested-tuplets` | music21 corpus `demos/nested_tuplet_finale_test.xml` | music21's test file |
| `two-voices` | music21 corpus `demos/two-voices.xml` | music21's test file |
| `voices-with-chords` | music21 corpus `demos/voices_with_chords.xml` | music21's test file |
| `arpeggios`, `features`, `marks`, `pickup` | written for these tests | as this repository |

music21's corpus says of these encodings
([`corpus/license.txt`](https://github.com/cuthbertLab/music21/blob/master/music21/corpus/license.txt)):
they are distributed with the permission of their encoders, the music is
believed to be in the public domain, and some encodings may carry their own
terms, such as a restriction on commercial use, stated in the composition or
its directory. Where an encoding states terms, they are in the table above;
the others state none. They are kept here, as converted by MuseScore, only as
test data for the MuseScore reader, and are not part of the published crate.
