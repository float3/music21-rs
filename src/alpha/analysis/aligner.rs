//! Two streams aligned note by note, as an edit from one to the other:
//! music21's `alpha.analysis.aligner`.

use super::hasher::{HashReference, Hasher, NoteHash};
use crate::{
    defaults::FloatType,
    error::{Error, Result},
    stream::Stream,
};

/// What becomes of a note from the target to the source: music21's
/// `ChangeOps`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ChangeOp {
    /// A note in the target and not the source.
    Insertion,
    /// A note in the source and not the target.
    Deletion,
    /// A note changed.
    Substitution,
    /// A note the same in both.
    NoChange,
}

impl ChangeOp {
    /// The colour music21 shows a change in.
    pub fn color(self) -> Option<&'static str> {
        match self {
            Self::Insertion => Some("green"),
            Self::Deletion => Some("red"),
            Self::Substitution => Some("purple"),
            Self::NoChange => None,
        }
    }

    fn from_index(index: usize) -> Self {
        match index {
            0 => Self::Insertion,
            1 => Self::Deletion,
            2 => Self::Substitution,
            _ => Self::NoChange,
        }
    }
}

/// A change from the target to the source: the target's note, the
/// source's, and what became of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Change {
    /// The target's note.
    pub target: Option<HashReference>,
    /// The source's note.
    pub source: Option<HashReference>,
    /// What became of it.
    pub op: ChangeOp,
}

/// Two streams aligned by their notes' hashes, the cheapest edit from one to
/// the other: music21's `StreamAligner`.
///
/// Inserting or deleting a note costs as many as the values in its hash, and
/// changing one as many values as differ.
#[derive(Clone, Debug)]
pub struct StreamAligner {
    /// How notes are hashed: by default by MIDI pitch and rounded length,
    /// each with what it was made from.
    pub hasher: Hasher,
    /// The target's hashes.
    pub hashed_target: Vec<NoteHash>,
    /// The source's hashes.
    pub hashed_source: Vec<NoteHash>,
    /// The cost of the cheapest edit from each opening of the target to each
    /// of the source.
    pub distance_matrix: Vec<Vec<i64>>,
    /// The changes, in order.
    pub changes: Vec<Change>,
    /// The share of the changes that change nothing.
    pub similarity_score: FloatType,
}

impl Default for StreamAligner {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamAligner {
    /// An aligner with music21's default hasher: pitch and length, each
    /// hash with what it was made from.
    pub fn new() -> Self {
        Self::with_hasher(Hasher {
            hash_offset: false,
            include_reference: true,
            ..Hasher::default()
        })
    }

    /// An aligner hashing notes as this hasher does.
    pub fn with_hasher(hasher: Hasher) -> Self {
        Self {
            hasher,
            hashed_target: Vec::new(),
            hashed_source: Vec::new(),
            distance_matrix: Vec::new(),
            changes: Vec::new(),
            similarity_score: 0.0,
        }
    }

    /// Aligns two streams: music21's `align`.
    ///
    /// # Errors
    ///
    /// A stream with nothing to hash, or a hash the hasher refuses.
    pub fn align(&mut self, target: &Stream, source: &Stream) -> Result<()> {
        let target = self.hasher.hash_stream(target)?;
        let source = self.hasher.hash_stream(source)?;
        self.align_hashed(target, source)
    }

    /// Aligns two lists of hashes already made: music21's `align` with
    /// `preHashed`.
    ///
    /// # Errors
    ///
    /// An empty list.
    pub fn align_hashed(&mut self, target: Vec<NoteHash>, source: Vec<NoteHash>) -> Result<()> {
        if target.is_empty() {
            return Err(Error::Analysis(
                "Cannot perform alignment with empty target stream.".to_string(),
            ));
        }
        if source.is_empty() {
            return Err(Error::Analysis(
                "Cannot perform alignment with empty source stream.".to_string(),
            ));
        }
        self.hashed_target = target;
        self.hashed_source = source;
        self.populate_distance_matrix();
        self.calculate_changes()
    }

    fn populate_distance_matrix(&mut self) {
        let (n, m) = (self.hashed_target.len(), self.hashed_source.len());
        // music21 costs every insertion and deletion as the source's first
        // hash does.
        let cost = self.hashed_source[0].keys.len() as i64;
        let mut matrix = vec![vec![0_i64; m + 1]; n + 1];
        for i in 1..=n {
            matrix[i][0] = matrix[i - 1][0] + cost;
        }
        for j in 1..=m {
            matrix[0][j] = matrix[0][j - 1] + cost;
        }
        for i in 1..=n {
            for j in 1..=m {
                let substitution =
                    substitution_cost(&self.hashed_target[i - 1], &self.hashed_source[j - 1]);
                matrix[i][j] = (matrix[i - 1][j] + cost)
                    .min(matrix[i][j - 1] + cost)
                    .min(matrix[i - 1][j - 1] + substitution);
            }
        }
        self.distance_matrix = matrix;
    }

    /// The change that leads to a place of the matrix, as music21 reads it
    /// back: none where the cost is the cheapest neighbour's, otherwise the
    /// cheapest, up before left before diagonal.
    fn op_at(&self, i: usize, j: usize) -> Result<ChangeOp> {
        let matrix = &self.distance_matrix;
        let up = (i >= 1).then(|| matrix[i - 1][j]);
        let left = (j >= 1).then(|| matrix[i][j - 1]);
        let diagonal = (i >= 1 && j >= 1).then(|| matrix[i - 1][j - 1]);
        let Some(up) = up else {
            if left.is_none() {
                return Err(Error::Analysis(
                    "No movement possible from the origin".to_string(),
                ));
            }
            return Ok(ChangeOp::Deletion);
        };
        let Some(left) = left else {
            return Ok(ChangeOp::Insertion);
        };
        let diagonal = diagonal.unwrap_or(i64::MAX);
        let moves = [up, left, diagonal];
        let (index, cheapest) =
            moves
                .iter()
                .enumerate()
                .fold((0, i64::MAX), |best, (index, cost)| {
                    if *cost < best.1 { (index, *cost) } else { best }
                });
        if matrix[i][j] == cheapest {
            Ok(ChangeOp::NoChange)
        } else {
            Ok(ChangeOp::from_index(index))
        }
    }

    fn calculate_changes(&mut self) -> Result<()> {
        let (mut i, mut j) = (self.hashed_target.len(), self.hashed_source.len());
        let (n, m) = (i, j);
        // music21 reads the note before the first as Python reads index -1:
        // the last.
        let before = |index: usize, count: usize| if index == 0 { count - 1 } else { index - 1 };
        let mut changes = Vec::new();
        while i != 0 || j != 0 {
            let op = self.op_at(i, j)?;
            changes.push(Change {
                target: self.hashed_target[before(i, n)].reference,
                source: self.hashed_source[before(j, m)].reference,
                op,
            });
            match op {
                ChangeOp::Insertion => i -= 1,
                ChangeOp::Deletion => j -= 1,
                ChangeOp::Substitution | ChangeOp::NoChange => {
                    i -= 1;
                    j -= 1;
                }
            }
        }
        changes.reverse();
        let unchanged = changes
            .iter()
            .filter(|change| change.op == ChangeOp::NoChange)
            .count();
        self.similarity_score = unchanged as FloatType / changes.len() as FloatType;
        self.changes = changes;
        Ok(())
    }

    /// How many changes of each kind there are: music21's `changesCount`.
    pub fn changes_count(&self, op: ChangeOp) -> usize {
        self.changes.iter().filter(|change| change.op == op).count()
    }
}

/// What it costs to turn one note's hash into another's: nothing if they
/// are the same, otherwise how many of their values differ.
fn substitution_cost(target: &NoteHash, source: &NoteHash) -> i64 {
    let same = target
        .values
        .iter()
        .zip(&source.values)
        .filter(|(a, b)| a == b)
        .count();
    if same == target.values.len() && target.values.len() == source.values.len() {
        return 0;
    }
    (target.keys.len() - same) as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::note::Note;

    fn line(names: &[&str]) -> Result<Stream> {
        let mut stream = Stream::new();
        for name in names {
            stream.push(Note::from_name(name)?);
        }
        Ok(stream)
    }

    #[test]
    fn streams_are_aligned_as_music21_aligns_them() -> Result<()> {
        // music21's own tests.
        let mut aligner = StreamAligner::new();
        aligner.align(&line(&["C4", "D#4", "C4"])?, &line(&["C4", "D-4", "C4"])?)?;
        assert!((aligner.similarity_score - 2.0 / 3.0).abs() < 1e-12);
        let mut aligner = StreamAligner::new();
        aligner.align(
            &line(&["C4", "D4", "E4", "F4"])?,
            &line(&["C4", "D4", "E4"])?,
        )?;
        assert!((aligner.similarity_score - 0.75).abs() < 1e-12);
        Ok(())
    }
}
