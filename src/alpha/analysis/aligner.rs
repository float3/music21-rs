//! Two streams aligned note by note, as an edit from one to the other:
//! music21's `alpha.analysis.aligner`.

use super::hasher::{HashReference, HashValue, Hasher, NoteHash};
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

/// Where a change stands: the places music21 reads the two hashes from,
/// and what became of the note.
///
/// The places are music21's own: it reads `hashedTargetStream[i - 1]` with
/// `i` as low as nought, which Python reads as the last hash, so the place
/// for the cell at the top or the left edge is the last of its list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChangePlace {
    /// Which of the target's hashes.
    pub target: usize,
    /// Which of the source's hashes.
    pub source: usize,
    /// What became of the note.
    pub op: ChangeOp,
}

/// A hasher for an alignment: music21's `StreamAligner.getDefaultHasher`,
/// which hashes no offset and says what each hash was made from.
pub fn default_hasher() -> Hasher {
    Hasher {
        hash_offset: false,
        include_reference: true,
        ..Hasher::default()
    }
}

/// A matrix of the right size for an alignment, every cost nought:
/// music21's `setupDistanceMatrix`.
///
/// # Errors
///
/// Either side with nothing hashed.
pub fn setup_distance_matrix(target: usize, source: usize) -> Result<Vec<Vec<i64>>> {
    if target == 0 {
        return Err(Error::Analysis(
            "Cannot perform alignment with empty target stream.".to_string(),
        ));
    }
    if source == 0 {
        return Err(Error::Analysis(
            "Cannot perform alignment with empty source stream.".to_string(),
        ));
    }
    Ok(vec![vec![0_i64; source + 1]; target + 1])
}

/// What it costs to put a note in that the source has not got: music21's
/// `insertCost`, which is how many values the hash says.
pub fn insert_cost(hash: &NoteHash) -> i64 {
    hash.keys.len() as i64
}

/// What it costs to take a note out that the target has not got: music21's
/// `deleteCost`, which is the same count.
pub fn delete_cost(hash: &NoteHash) -> i64 {
    hash.keys.len() as i64
}

/// How many of the values two hashes say alike: music21's
/// `calculateNumSimilarities`.
///
/// The target's keys are what is counted over, each read off the source by
/// name, as music21 reads it; a key the source has not got counts for
/// nothing.
pub fn num_similarities(target: &NoteHash, source: &NoteHash) -> usize {
    target
        .keys
        .iter()
        .zip(&target.values)
        .filter(|(key, value)| value_of(source, key) == Some(*value))
        .count()
}

/// Whether two hashes say the same of a note, whatever each was made from:
/// music21's `tupleEqualityWithoutReference`.
pub fn hashes_equal_without_reference(target: &NoteHash, source: &NoteHash) -> bool {
    target
        .keys
        .iter()
        .zip(&target.values)
        .all(|(key, value)| value_of(source, key) == Some(value))
}

/// The value a hash says for a key, where it says one.
fn value_of<'a>(hash: &'a NoteHash, key: &str) -> Option<&'a HashValue> {
    let at = hash.keys.iter().position(|theirs| theirs == key)?;
    hash.values.get(at)
}

/// What it costs to turn one note's hash into another's: music21's
/// `substitutionCost` -- nothing where the two say the same, otherwise how
/// many of the values differ.
pub fn substitution_cost(target: &NoteHash, source: &NoteHash) -> i64 {
    if hashes_equal_without_reference(target, source) {
        return 0;
    }
    (target.keys.len() - num_similarities(target, source)) as i64
}

/// The costs of the three moves onto a cell -- up, left and diagonally --
/// each nothing where that cell is off the matrix: music21's
/// `getPossibleMovesFromLocation`.
pub fn moves_from_location(matrix: &[Vec<i64>], i: usize, j: usize) -> [Option<i64>; 3] {
    let at = |i: usize, j: usize| matrix.get(i).and_then(|row| row.get(j)).copied();
    [
        (i >= 1).then(|| at(i - 1, j)).flatten(),
        (j >= 1).then(|| at(i, j - 1)).flatten(),
        (i >= 1 && j >= 1).then(|| at(i - 1, j - 1)).flatten(),
    ]
}

/// The change that leads to a cell: music21's `getOpFromLocation` -- none
/// where the cost standing there is the cheapest move's, otherwise the
/// cheapest move, up before left before diagonal.
///
/// # Errors
///
/// The origin, which nothing moves onto.
pub fn op_from_location(matrix: &[Vec<i64>], i: usize, j: usize) -> Result<ChangeOp> {
    let moves = moves_from_location(matrix, i, j);
    let Some(up) = moves[0] else {
        if moves[1].is_none() {
            return Err(Error::Value(
                "No movement possible from the origin".to_string(),
            ));
        }
        return Ok(ChangeOp::Deletion);
    };
    let Some(left) = moves[1] else {
        return Ok(ChangeOp::Insertion);
    };
    // Where the two above are costs, so is the diagonal: music21 compares
    // all three, which a `None` among them would refuse to do.
    let diagonal = moves[2].unwrap_or(i64::MAX);
    let current = matrix
        .get(i)
        .and_then(|row| row.get(j))
        .copied()
        .unwrap_or(i64::MAX);
    let (index, cheapest) =
        [up, left, diagonal]
            .iter()
            .enumerate()
            .fold((0, i64::MAX), |best, (index, cost)| {
                if *cost < best.1 { (index, *cost) } else { best }
            });
    if current == cheapest {
        Ok(ChangeOp::NoChange)
    } else {
        Ok(ChangeOp::from_index(index))
    }
}

/// The cheapest edit from the target to the source read out of a matrix
/// already filled, each change with the places of the two hashes it is
/// between: the by-position twin of `calculateChangesList`.
///
/// # Errors
///
/// Either side with nothing hashed, or a matrix nothing can be traced
/// through.
pub fn change_places(
    matrix: &[Vec<i64>],
    target: usize,
    source: usize,
) -> Result<Vec<ChangePlace>> {
    if target == 0 || source == 0 {
        return Err(Error::Analysis(
            "Cannot trace an alignment of an empty stream.".to_string(),
        ));
    }
    // music21 reads the hash before the first as Python reads index -1:
    // the last.
    let before = |index: usize, count: usize| if index == 0 { count - 1 } else { index - 1 };
    let (mut i, mut j) = (target, source);
    let mut places = Vec::new();
    while i != 0 || j != 0 {
        let op = op_from_location(matrix, i, j)?;
        places.push(ChangePlace {
            target: before(i, target),
            source: before(j, source),
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
    places.reverse();
    Ok(places)
}

/// How much of an edit changes nothing: music21's `similarityScore`.
pub fn similarity_score(places: &[ChangePlace]) -> FloatType {
    let unchanged = places
        .iter()
        .filter(|place| place.op == ChangeOp::NoChange)
        .count();
    unchanged as FloatType / places.len() as FloatType
}

/// The cheapest edit onto every opening of the two, written into a matrix
/// already set up: music21's `populateDistanceMatrix`.
///
/// Inserting and deleting both cost what the source's *first* hash says,
/// which is music21's reading and not a cost per note.
///
/// # Errors
///
/// A source with nothing hashed, or a matrix not of the size
/// [`setup_distance_matrix`] gives.
pub fn fill_distance_matrix(
    matrix: &mut [Vec<i64>],
    target: &[NoteHash],
    source: &[NoteHash],
) -> Result<()> {
    let (n, m) = (target.len(), source.len());
    let Some(first) = source.first() else {
        return Err(Error::Analysis(
            "Cannot perform alignment with empty source stream.".to_string(),
        ));
    };
    if matrix.len() != n + 1 || matrix.iter().any(|row| row.len() != m + 1) {
        return Err(Error::Analysis(format!(
            "A matrix of {} by {} cannot hold the alignment of {n} with {m}",
            matrix.len(),
            matrix.first().map_or(0, Vec::len)
        )));
    }
    let insertion = insert_cost(first);
    let deletion = delete_cost(first);
    for i in 1..=n {
        matrix[i][0] = matrix[i - 1][0] + insertion;
    }
    for j in 1..=m {
        matrix[0][j] = matrix[0][j - 1] + deletion;
    }
    for i in 1..=n {
        for j in 1..=m {
            let substitution = substitution_cost(&target[i - 1], &source[j - 1]);
            matrix[i][j] = (matrix[i - 1][j] + insertion)
                .min(matrix[i][j - 1] + deletion)
                .min(matrix[i - 1][j - 1] + substitution);
        }
    }
    Ok(())
}

/// A matrix set up and filled in one: music21's `setupDistanceMatrix` and
/// `populateDistanceMatrix` together.
///
/// # Errors
///
/// Either side with nothing hashed.
pub fn populate_distance_matrix(target: &[NoteHash], source: &[NoteHash]) -> Result<Vec<Vec<i64>>> {
    let mut matrix = setup_distance_matrix(target.len(), source.len())?;
    fill_distance_matrix(&mut matrix, target, source)?;
    Ok(matrix)
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
        Self::with_hasher(default_hasher())
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
        self.populate_distance_matrix()?;
        self.calculate_changes()
    }

    fn populate_distance_matrix(&mut self) -> Result<()> {
        self.distance_matrix = populate_distance_matrix(&self.hashed_target, &self.hashed_source)?;
        Ok(())
    }

    fn calculate_changes(&mut self) -> Result<()> {
        let places = change_places(
            &self.distance_matrix,
            self.hashed_target.len(),
            self.hashed_source.len(),
        )?;
        self.similarity_score = similarity_score(&places);
        self.changes = places
            .iter()
            .map(|place| Change {
                target: self.hashed_target[place.target].reference,
                source: self.hashed_source[place.source].reference,
                op: place.op,
            })
            .collect();
        Ok(())
    }

    /// How many changes of each kind there are: music21's `changesCount`.
    pub fn changes_count(&self, op: ChangeOp) -> usize {
        self.changes.iter().filter(|change| change.op == op).count()
    }
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

    /// A line of notes, each as long as it is given.
    fn lengths(names: &[(&str, FloatType)]) -> Result<Stream> {
        let mut stream = Stream::new();
        for (name, length) in names {
            let mut note = Note::from_name(name)?;
            note.set_duration(crate::duration::Duration::new(*length)?);
            stream.push(note);
        }
        Ok(stream)
    }

    #[test]
    fn the_matrix_is_filled_as_music21_fills_it() -> Result<()> {
        // music21's own `populateDistanceMatrix` examples.
        let hasher = default_hasher();
        let filled = |target: &Stream, source: &Stream| -> Result<Vec<Vec<i64>>> {
            populate_distance_matrix(&hasher.hash_stream(target)?, &hasher.hash_stream(source)?)
        };
        let two = line(&["C#4", "C4"])?;
        assert_eq!(
            filled(&two, &two)?,
            vec![vec![0, 2, 4], vec![2, 0, 2], vec![4, 2, 0]]
        );
        assert_eq!(
            filled(&two, &line(&["C#4"])?)?,
            vec![vec![0, 2], vec![2, 0], vec![4, 2]]
        );
        assert_eq!(
            filled(
                &line(&["C#4", "C4", "E3"])?,
                &lengths(&[("D5", 3.0), ("C#4", 1.0), ("E3", 1.0)])?,
            )?,
            vec![
                vec![0, 2, 4, 6],
                vec![2, 2, 2, 4],
                vec![4, 4, 3, 3],
                vec![6, 6, 5, 3],
            ]
        );
        Ok(())
    }

    #[test]
    fn a_cell_says_which_moves_lead_onto_it() -> Result<()> {
        // music21 writes its own product table into the matrix and reads
        // the moves back off it.
        let matrix: Vec<Vec<i64>> = (0..=4).map(|i| (0..=3).map(|j| i * j).collect()).collect();
        assert_eq!(moves_from_location(&matrix, 0, 0), [None, None, None]);
        assert_eq!(
            moves_from_location(&matrix, 1, 1),
            [Some(0), Some(0), Some(0)]
        );
        assert_eq!(
            moves_from_location(&matrix, 4, 3),
            [Some(9), Some(8), Some(6)]
        );
        assert_eq!(
            moves_from_location(&matrix, 2, 2),
            [Some(2), Some(2), Some(1)]
        );
        assert_eq!(moves_from_location(&matrix, 0, 2), [None, Some(0), None]);
        assert_eq!(moves_from_location(&matrix, 3, 0), [Some(0), None, None]);
        assert!(op_from_location(&matrix, 0, 0).is_err());
        Ok(())
    }

    #[test]
    fn a_cell_says_what_change_leads_onto_it() -> Result<()> {
        // music21's own `getOpFromLocation` example, on the matrix its
        // four-note target and three-note source fill.
        let hasher = default_hasher();
        let mut matrix = populate_distance_matrix(
            &hasher.hash_stream(&line(&["C4", "D4", "C4", "E4"])?)?,
            &hasher.hash_stream(&line(&["C4", "D4", "C4"])?)?,
        )?;
        assert_eq!(
            matrix,
            vec![
                vec![0, 2, 4, 6],
                vec![2, 0, 2, 4],
                vec![4, 2, 0, 2],
                vec![6, 4, 2, 0],
                vec![8, 6, 4, 2],
            ]
        );
        assert_eq!(op_from_location(&matrix, 4, 3)?, ChangeOp::Insertion);
        assert_eq!(op_from_location(&matrix, 2, 2)?, ChangeOp::NoChange);
        assert_eq!(op_from_location(&matrix, 0, 2)?, ChangeOp::Deletion);
        matrix[0][0] = 1;
        assert_eq!(op_from_location(&matrix, 1, 1)?, ChangeOp::Substitution);
        Ok(())
    }

    #[test]
    fn two_hashes_are_compared_as_music21_compares_them() -> Result<()> {
        // music21's own `substitutionCost` examples: the same note, a
        // note of another pitch, and a note of another pitch and length.
        let hasher = default_hasher();
        let one =
            |stream: &Stream| -> Result<NoteHash> { Ok(hasher.hash_stream(stream)?.remove(0)) };
        let c4 = one(&line(&["C4"])?)?;
        let same = one(&line(&["C4"])?)?;
        assert!(hashes_equal_without_reference(&c4, &same));
        assert_eq!(num_similarities(&c4, &same), 2);
        assert_eq!(substitution_cost(&c4, &same), 0);
        let d4 = one(&line(&["D4"])?)?;
        assert!(!hashes_equal_without_reference(&c4, &d4));
        assert_eq!(num_similarities(&c4, &d4), 1);
        assert_eq!(substitution_cost(&c4, &d4), 1);
        let e4 = one(&line(&["E4"])?)?;
        let e_sharp = one(&lengths(&[("E#4", 2.0)])?)?;
        assert_eq!(num_similarities(&e4, &e_sharp), 0);
        assert_eq!(substitution_cost(&e4, &e_sharp), 2);
        assert_eq!(insert_cost(&c4), 2);
        assert_eq!(delete_cost(&c4), 2);
        Ok(())
    }

    #[test]
    fn a_change_says_which_hashes_it_is_between() -> Result<()> {
        // Every change music21 lists is between one hash of each side,
        // and the twin says which.
        let mut aligner = StreamAligner::new();
        let target = line(&["C4", "D4", "E4", "F4"])?;
        let source = line(&["C4", "D4", "E4"])?;
        aligner.align(&target, &source)?;
        let places = change_places(
            &aligner.distance_matrix,
            aligner.hashed_target.len(),
            aligner.hashed_source.len(),
        )?;
        assert_eq!(places.len(), aligner.changes.len());
        for (place, change) in places.iter().zip(&aligner.changes) {
            assert_eq!(place.op, change.op);
            assert_eq!(aligner.hashed_target[place.target].reference, change.target);
            assert_eq!(aligner.hashed_source[place.source].reference, change.source);
        }
        assert!((similarity_score(&places) - 0.75).abs() < 1e-12);
        Ok(())
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
