//! Rows and sets of pitch classes found in the notes of a stream, and
//! labelled there: music21's `search.serial`.

use std::collections::HashMap;

use crate::{
    defaults::{FloatType, IntegerType},
    error::{Error, Result},
    notation::{Tie, TieType},
    pitch::Pitch,
    serial::{IndexedTransformation, ToneRow, TransformationConvention},
    spanner::Spanner,
    stream::{Stream, StreamElement, StreamKind},
};

/// How a [`ContiguousSegmentSearcher`] reads notes that repeat a pitch
/// class: music21's `reps`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Repetitions {
    /// A note or chord sounding just what the one before sounded is passed
    /// over: music21's `skipConsecutive`.
    #[default]
    SkipConsecutive,
    /// Every note counts, and a segment is a run of distinct pitch classes:
    /// music21's `rowsOnly`.
    RowsOnly,
    /// Every note counts: music21's `includeAll`.
    IncludeAll,
    /// A segment is as many notes as sound its number of distinct pitch
    /// classes: music21's `ignoreAll`.
    IgnoreAll,
}

/// One note or chord of a [`ContiguousSegment`], and where it stands.
#[derive(Clone, Copy, Debug)]
pub struct SegmentNote<'a> {
    /// The note or chord.
    pub element: &'a StreamElement,
    /// The number of the measure it is in, if it is in one.
    pub measure: Option<IntegerType>,
    /// Its offset in the stream that holds it, a measure or a voice:
    /// music21's `offset`.
    pub offset: FloatType,
    /// Its position among the leaves of its part, or of the stream searched
    /// where that has no parts: see [`Stream::leaves`].
    pub leaf: usize,
}

/// A run of notes and chords of one part: music21's
/// `ContiguousSegmentOfNotes`.
#[derive(Clone, Debug)]
pub struct ContiguousSegment<'a> {
    /// The notes and chords, in order.
    pub notes: Vec<SegmentNote<'a>>,
    /// The part they are in, counted from nought, or nothing where the
    /// stream searched has no parts.
    pub part: Option<usize>,
    /// The pitch classes a [`SegmentMatcher`] read the run as, where it
    /// matched one: music21's `activeSegment`, by its pitch classes.
    pub active: Vec<u8>,
    /// What the run was matched to, where it was: music21's
    /// `matchedSegment`.
    pub matched: Option<Vec<IntegerType>>,
}

/// The pitches an element sounds, a chord symbol's included.
fn pitches_of(element: &StreamElement) -> Vec<Pitch> {
    match element {
        StreamElement::ChordSymbol(symbol) => symbol.pitches().unwrap_or_default(),
        _ => element.pitches(),
    }
}

/// music21's `pitchClass`: the pitch space rounded half to even, folded
/// into the octave.
fn pitch_class(pitch: &Pitch) -> u8 {
    (pitch.ps().round_ties_even() as i64).rem_euclid(12) as u8
}

impl ContiguousSegment<'_> {
    /// The measure of the first note: music21's `startMeasureNumber`.
    pub fn start_measure(&self) -> Option<IntegerType> {
        self.notes.first().and_then(|note| note.measure)
    }

    /// The offset of the first note in the stream holding it: music21's
    /// `startOffset`.
    pub fn start_offset(&self) -> Option<FloatType> {
        self.notes.first().map(|note| note.offset)
    }

    /// Every pitch class of every note and chord, each chord from its
    /// bottom: music21's `readPitchClassesFromBottom`.
    pub fn pitch_classes_from_bottom(&self) -> Vec<u8> {
        self.notes
            .iter()
            .flat_map(|note| pitches_of(note.element))
            .map(|pitch| pitch_class(&pitch))
            .collect()
    }

    /// The same, each pitch class once: music21's
    /// `getDistinctPitchClasses`.
    pub fn distinct_pitch_classes(&self) -> Vec<u8> {
        let mut distinct = Vec::new();
        for class in self.pitch_classes_from_bottom() {
            if !distinct.contains(&class) {
                distinct.push(class);
            }
        }
        distinct
    }

    fn rows(&self) -> Option<(ToneRow, ToneRow)> {
        let matched = self.matched.as_ref()?;
        Some((
            ToneRow::new(self.active.iter().map(|class| IntegerType::from(*class))),
            ToneRow::new(matched.iter().copied()),
        ))
    }

    /// The zero-centred transformations taking the row matched to the row
    /// found: music21's `zeroCenteredTransformationsFromMatched`. Nothing
    /// for a run not matched.
    pub fn zero_centered_transformations_from_matched(&self) -> Vec<IndexedTransformation> {
        self.rows()
            .map(|(active, matched)| matched.find_zero_centered_transformations(&active))
            .unwrap_or_default()
    }

    /// The original-centred transformations taking the row matched to the
    /// row found: music21's `originalCenteredTransformationsFromMatched`.
    pub fn original_centered_transformations_from_matched(&self) -> Vec<IndexedTransformation> {
        self.rows()
            .map(|(active, matched)| matched.find_original_centered_transformations(&active))
            .unwrap_or_default()
    }
}

/// The parts of a stream, nested ones included, or the stream itself where
/// it has none.
fn parts_of(stream: &Stream) -> Vec<(Option<usize>, &Stream)> {
    let parts: Vec<&Stream> = stream
        .recurse()
        .into_iter()
        .filter_map(|(_, element)| match element {
            StreamElement::Stream(inner)
                if matches!(inner.kind(), StreamKind::Part | StreamKind::PartStaff) =>
            {
                Some(&**inner)
            }
            _ => None,
        })
        .collect();
    if parts.is_empty() {
        vec![(None, stream)]
    } else {
        parts
            .into_iter()
            .enumerate()
            .map(|(index, part)| (Some(index), part))
            .collect()
    }
}

fn tie_of(element: &StreamElement) -> Option<&Tie> {
    match element {
        StreamElement::Note(note) => note.tie(),
        StreamElement::Chord(chord) => chord.tie(),
        StreamElement::Unpitched(stroke) => stroke.written().tie(),
        StreamElement::PercussionChord(chord) => chord.written().tie(),
        _ => None,
    }
}

/// The notes and chords of a part, each with its measure, its own offset
/// and its leaf position, a note tied on from the one before left out:
/// what music21's searcher walks.
fn part_notes(part: &Stream) -> Vec<SegmentNote<'_>> {
    fn walk<'a>(
        stream: &'a Stream,
        measure: Option<IntegerType>,
        leaf: &mut usize,
        out: &mut Vec<SegmentNote<'a>>,
    ) {
        for event in stream.events() {
            let element = event.element();
            if let StreamElement::Stream(inner) = element {
                let measure = if inner.kind() == StreamKind::Measure {
                    Some(inner.number())
                } else {
                    measure
                };
                walk(inner, measure, leaf, out);
                continue;
            }
            let position = *leaf;
            *leaf += 1;
            if !matches!(
                element,
                StreamElement::Note(_)
                    | StreamElement::Chord(_)
                    | StreamElement::Unpitched(_)
                    | StreamElement::PercussionChord(_)
                    | StreamElement::ChordSymbol(_)
            ) {
                continue;
            }
            if tie_of(element).is_some_and(|tie| tie.tie_type() != TieType::Start) {
                continue;
            }
            out.push(SegmentNote {
                element,
                measure,
                offset: event.offset(),
                leaf: position,
            });
        }
    }
    let mut out = Vec::new();
    let measure = (part.kind() == StreamKind::Measure).then(|| part.number());
    walk(part, measure, &mut 0, &mut out);
    out
}

/// Finds every run of a stream's notes and chords that is a segment of a
/// given length, part by part: music21's `ContiguousSegmentSearcher`.
#[derive(Clone, Copy, Debug, Default)]
pub struct ContiguousSegmentSearcher {
    /// How repeated pitch classes are read.
    pub repetitions: Repetitions,
    /// Whether chords may be in a segment; where not, a chord breaks one.
    pub include_chords: bool,
    /// With [`Repetitions::IgnoreAll`], whether a run's first note is let go
    /// as soon as it makes one segment: music21's `trimToShortestLengthFast`.
    pub trim_to_shortest_length_fast: bool,
}

/// What a searcher keeps while it walks one part.
struct Walk<'a> {
    length: usize,
    part: Option<usize>,
    chords: Vec<SegmentNote<'a>>,
    total: usize,
    found: Vec<ContiguousSegment<'a>>,
}

impl<'a> Walk<'a> {
    fn add(&mut self, notes: Vec<SegmentNote<'a>>) {
        self.found.push(ContiguousSegment {
            notes,
            part: self.part,
            active: Vec::new(),
            matched: None,
        });
    }

    fn drop_front(&mut self, count: usize) {
        for removed in self.chords.drain(..count) {
            self.total -= pitches_of(removed.element).len().min(self.total);
        }
    }
}

fn count(note: &SegmentNote<'_>) -> usize {
    pitches_of(note.element).len()
}

fn index_error() -> Error {
    Error::Search("list index out of range".to_string())
}

impl ContiguousSegmentSearcher {
    /// A searcher reading repetitions so, chords included or not.
    pub fn new(repetitions: Repetitions, include_chords: bool) -> Self {
        Self {
            repetitions,
            include_chords,
            trim_to_shortest_length_fast: false,
        }
    }

    /// Every segment of `length` pitch classes in each part of a stream, in
    /// order: music21's `byLength`.
    ///
    /// # Errors
    ///
    /// A note sounding no pitch where music21 reads one, which music21
    /// cannot read either.
    pub fn by_length<'a>(
        &self,
        stream: &'a Stream,
        length: usize,
    ) -> Result<Vec<ContiguousSegment<'a>>> {
        let mut found = Vec::new();
        for (part, part_stream) in parts_of(stream) {
            let mut walk = Walk {
                length,
                part,
                chords: Vec::new(),
                total: 0,
                found: Vec::new(),
            };
            for note in part_notes(part_stream) {
                match (self.include_chords, self.repetitions) {
                    (false, Repetitions::SkipConsecutive) => {
                        if !same_as_last(&walk, &note) {
                            include_all_exclude(&mut walk, note);
                        }
                    }
                    (false, Repetitions::RowsOnly) => rows_only_exclude(&mut walk, note)?,
                    (false, Repetitions::IncludeAll) => include_all_exclude(&mut walk, note),
                    (false, Repetitions::IgnoreAll) => {
                        ignore_all_exclude(&mut walk, note, self.trim_to_shortest_length_fast);
                    }
                    (true, Repetitions::SkipConsecutive) => {
                        if !same_as_last(&walk, &note) {
                            include_all_include(&mut walk, note);
                        }
                    }
                    (true, Repetitions::RowsOnly) => rows_only_include(&mut walk, note),
                    (true, Repetitions::IncludeAll) => include_all_include(&mut walk, note),
                    (true, Repetitions::IgnoreAll) => {
                        ignore_all_include(&mut walk, note, self.trim_to_shortest_length_fast)?;
                    }
                }
            }
            found.append(&mut walk.found);
        }
        Ok(found)
    }
}

/// Whether a note sounds just the pitches of the last one kept.
fn same_as_last(walk: &Walk<'_>, note: &SegmentNote<'_>) -> bool {
    walk.chords
        .last()
        .is_some_and(|last| pitches_of(last.element) == pitches_of(note.element))
}

/// music21's `searchIncludeAllExclude`.
fn include_all_exclude<'a>(walk: &mut Walk<'a>, note: SegmentNote<'a>) {
    if count(&note) > 1 {
        walk.chords.clear();
        return;
    }
    walk.total += count(&note);
    walk.chords.push(note);
    if walk.chords.len() == walk.length + 1 {
        walk.chords.remove(0);
    }
    if walk.chords.len() == walk.length {
        let notes = walk.chords.clone();
        walk.add(notes);
    }
}

/// music21's `searchIncludeAllInclude`: each run ending on this note whose
/// inner notes and chords hold fewer pitches than the length, and which
/// holds at least as many, is a segment.
fn include_all_include<'a>(walk: &mut Walk<'a>, note: SegmentNote<'a>) {
    walk.total += count(&note);
    walk.chords.push(note);
    let mut active_length = walk.total;
    let mut to_delete = 0;
    for start in 0..walk.chords.len() {
        if start > 0 {
            active_length -= count(&walk.chords[start - 1]);
        }
        let active = &walk.chords[start..];
        let inner = active_length as isize
            - (count(&active[0]) + count(&active[active.len() - 1])) as isize;
        if active_length >= walk.length && inner <= walk.length as isize - 2 {
            let notes = active.to_vec();
            walk.add(notes);
        } else if active_length >= walk.length {
            to_delete += 1;
        } else {
            break;
        }
    }
    walk.drop_front(to_delete);
}

/// music21's `searchIgnoreAllExclude`.
fn ignore_all_exclude<'a>(walk: &mut Walk<'a>, note: SegmentNote<'a>, trim: bool) {
    if count(&note) > 1 {
        walk.chords.clear();
        return;
    }
    walk.chords.push(note);
    let mut to_delete = 0;
    for start in 0..walk.chords.len() {
        let active = &walk.chords[start..];
        let mut classes: Vec<u8> = active
            .iter()
            .flat_map(|note| pitches_of(note.element))
            .map(|pitch| pitch_class(&pitch))
            .collect();
        classes.sort_unstable();
        classes.dedup();
        if classes.len() == walk.length {
            let notes = active.to_vec();
            walk.add(notes);
            if trim {
                to_delete += 1;
            }
        } else if classes.len() > walk.length {
            to_delete += 1;
        }
    }
    walk.drop_front(to_delete);
}

/// music21's `searchIgnoreAllInclude`.
fn ignore_all_include<'a>(walk: &mut Walk<'a>, note: SegmentNote<'a>, trim: bool) -> Result<()> {
    walk.chords.push(note);
    let mut to_delete = 0;
    for start in 0..walk.chords.len() {
        let active = walk.chords[start..].to_vec();
        walk.add(active.clone());
        let mut superset = walk
            .found
            .last()
            .map(ContiguousSegment::pitch_classes_from_bottom)
            .unwrap_or_default();
        superset.sort_unstable();
        superset.dedup();
        if superset.len() >= walk.length {
            let first = pitches_of(active[0].element);
            let last = pitches_of(active[active.len() - 1].element);
            let mut check: Vec<u8> = active[1..active.len().saturating_sub(1).max(1)]
                .iter()
                .flat_map(|note| pitches_of(note.element))
                .map(|pitch| pitch_class(&pitch))
                .collect();
            if active.len() < 2 {
                check.clear();
            }
            check.push(pitch_class(first.last().ok_or_else(index_error)?));
            check.push(pitch_class(last.first().ok_or_else(index_error)?));
            check.sort_unstable();
            check.dedup();
            if check.len() > walk.length {
                walk.found.pop();
                to_delete += 1;
            } else if trim {
                to_delete += 1;
            }
        } else {
            walk.found.pop();
            break;
        }
    }
    walk.drop_front(to_delete);
    Ok(())
}

/// music21's `searchRowsOnlyExclude`, which reads the `pitch` of what is
/// left once chords are passed over, and so refuses anything but a note.
fn rows_only_exclude<'a>(walk: &mut Walk<'a>, note: SegmentNote<'a>) -> Result<()> {
    if count(&note) > 1 {
        walk.chords.clear();
        return Ok(());
    }
    if walk.chords.len() == walk.length && !walk.chords.is_empty() {
        walk.chords.remove(0);
    }
    let pitch_of = |element: &StreamElement| match element {
        StreamElement::Note(note) => Ok(pitch_class(note.pitch())),
        _ => Err(Error::Search(
            "only a note has a single pitch to read".to_string(),
        )),
    };
    let class = pitch_of(note.element)?;
    let existing = walk
        .chords
        .iter()
        .map(|old| pitch_of(old.element))
        .collect::<Result<Vec<u8>>>()?;
    if existing.contains(&class) {
        walk.chords = vec![note];
    } else {
        walk.chords.push(note);
    }
    if walk.chords.len() == walk.length {
        let notes = walk.chords.clone();
        walk.add(notes);
    }
    Ok(())
}

/// music21's `searchRowsOnlyInclude`.
fn rows_only_include<'a>(walk: &mut Walk<'a>, note: SegmentNote<'a>) {
    walk.total += count(&note);
    walk.chords.push(note);
    let mut active_length = walk.total;
    let mut to_delete = 0;
    for start in 0..walk.chords.len() {
        if start > 0 {
            active_length -= count(&walk.chords[start - 1]);
        }
        let active = walk.chords[start..].to_vec();
        let first_count = count(&active[0]);
        let last_count = count(&active[active.len() - 1]);
        let inner = active_length as isize - (first_count + last_count) as isize;
        if active_length >= walk.length && inner <= walk.length as isize - 2 {
            walk.add(active);
            let superset = walk
                .found
                .last()
                .map(ContiguousSegment::pitch_classes_from_bottom)
                .unwrap_or_default();
            let lower = (superset.len() as isize - walk.length as isize - last_count as isize + 1)
                .max(0) as usize;
            let upper =
                (first_count as isize).min(superset.len() as isize - walk.length as isize + 1);
            let is_row = (lower as isize..upper).any(|j| {
                let window = &superset[j as usize..j as usize + walk.length];
                let mut distinct = window.to_vec();
                distinct.sort_unstable();
                distinct.dedup();
                distinct.len() == walk.length
            });
            if !is_row {
                walk.found.pop();
            }
        } else if active_length >= walk.length {
            to_delete += 1;
        } else {
            break;
        }
    }
    walk.drop_front(to_delete);
}

/// How a [`SegmentMatcher`] tells a run of pitch classes the same as one
/// searched for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Matching {
    /// The same pitch classes in the same order: music21's
    /// `SegmentMatcher`.
    #[default]
    Exact,
    /// The same intervals in the same order: music21's
    /// `TransposedSegmentMatcher`.
    Transposed,
    /// A transposition, inversion, retrograde or retrograde inversion:
    /// music21's `TransformedSegmentMatcher`.
    Transformed,
    /// The same pitch classes in any order: music21's
    /// `MultisetSegmentMatcher`.
    Multiset,
    /// The same pitch classes in any order, transposed: music21's
    /// `TransposedMultisetMatcher`.
    TransposedMultiset,
    /// The same pitch classes in any order, transposed or inverted:
    /// music21's `TransposedInvertedMultisetMatcher`.
    TransposedInvertedMultiset,
}

/// A segment as music21 normalizes it to compare.
#[derive(Clone, Debug, PartialEq)]
enum Normal {
    Classes(Vec<u8>),
    Intervals(String),
    Row(ToneRow),
}

impl Normal {
    fn len(&self) -> usize {
        match self {
            Self::Classes(classes) => classes.len(),
            Self::Intervals(intervals) => intervals.chars().count(),
            Self::Row(row) => row.len(),
        }
    }
}

/// A multiset of pitch classes, as a sorted list.
fn multiset(classes: impl IntoIterator<Item = i64>) -> Vec<i64> {
    let mut sorted: Vec<i64> = classes.into_iter().collect();
    sorted.sort_unstable();
    sorted
}

impl Matching {
    fn normalize(self, segment: &[IntegerType]) -> Normal {
        let row = ToneRow::new(segment.iter().copied());
        match self {
            Self::Transposed => Normal::Intervals(row.intervals_as_string()),
            Self::Transformed => Normal::Row(row),
            _ => Normal::Classes(row.pitch_classes().to_vec()),
        }
    }

    fn multiset_duplicates(self) -> bool {
        matches!(
            self,
            Self::Multiset | Self::TransposedMultiset | Self::TransposedInvertedMultiset
        )
    }

    fn equal(self, search: &Normal, subset: &Normal) -> bool {
        let classes = |normal: &Normal| -> Vec<i64> {
            match normal {
                Normal::Classes(classes) => classes.iter().map(|class| i64::from(*class)).collect(),
                _ => Vec::new(),
            }
        };
        match self {
            Self::Exact | Self::Transposed => search == subset,
            Self::Transformed => match (search, subset) {
                (Normal::Row(search), Normal::Row(subset)) => {
                    !subset.find_zero_centered_transformations(search).is_empty()
                }
                _ => false,
            },
            Self::Multiset => multiset(classes(search)) == multiset(classes(subset)),
            Self::TransposedMultiset | Self::TransposedInvertedMultiset => {
                let wanted = multiset(classes(subset));
                let search = classes(search);
                let transposed = (0..12)
                    .any(|i| multiset(search.iter().map(|p| (p + i).rem_euclid(12))) == wanted);
                transposed
                    || (self == Self::TransposedInvertedMultiset
                        && (0..12).any(|i| {
                            multiset(search.iter().map(|p| (-(p + i)).rem_euclid(12))) == wanted
                        }))
            }
        }
    }
}

/// What a matcher has searched for already in one [`SegmentMatcher::find`].
enum Searched {
    Normals(Vec<Normal>),
    Multisets(Vec<Vec<i64>>),
}

impl Searched {
    /// music21's `checkSearchedAlready`: whether a segment is one searched
    /// for already, noting it if not.
    fn already(&mut self, matching: Matching, segment: &[IntegerType]) -> bool {
        match self {
            Self::Normals(seen) => {
                let normal = matching.normalize(segment);
                let found = if matching == Matching::Transformed {
                    seen.iter().any(|used| match (&normal, used) {
                        (Normal::Row(row), Normal::Row(used)) => {
                            !used.find_zero_centered_transformations(row).is_empty()
                        }
                        _ => false,
                    })
                } else {
                    seen.contains(&normal)
                };
                if !found {
                    seen.push(normal);
                }
                found
            }
            Self::Multisets(seen) => {
                let raw: Vec<i64> = segment.iter().map(|p| i64::from(*p)).collect();
                if matching == Matching::TransposedInvertedMultiset
                    && (0..12).any(|i| {
                        seen.contains(&multiset(raw.iter().map(|p| (-(p + i)).rem_euclid(12))))
                    })
                {
                    return true;
                }
                if (0..12)
                    .any(|i| seen.contains(&multiset(raw.iter().map(|p| (p + i).rem_euclid(12)))))
                {
                    return true;
                }
                // music21 keeps the last transposition it tried.
                seen.push(multiset(raw.iter().map(|p| (p + 11).rem_euclid(12))));
                false
            }
        }
    }
}

/// Finds the runs of a stream's notes and chords that are segments or
/// sets of pitch classes searched for: music21's `SegmentMatcher` and the
/// matchers built on it, chosen by [`Matching`].
#[derive(Clone, Copy, Debug, Default)]
pub struct SegmentMatcher {
    /// How a run is told the same as a segment searched for.
    pub matching: Matching,
    /// How repeated pitch classes are read.
    pub repetitions: Repetitions,
    /// Whether chords may be in a run.
    pub include_chords: bool,
}

impl SegmentMatcher {
    /// A matcher of this kind, reading repetitions so, chords included or
    /// not.
    pub fn new(matching: Matching, repetitions: Repetitions, include_chords: bool) -> Self {
        Self {
            matching,
            repetitions,
            include_chords,
        }
    }

    /// Each run that matches one of the segments searched for, by the
    /// first it matches, in the order searched and found: music21's `find`.
    /// A segment the same as one searched for already is passed over. A run
    /// matching two segments is listed for each, as music21 lists the one
    /// run twice, both times as it was matched last.
    ///
    /// # Errors
    ///
    /// As [`ContiguousSegmentSearcher::by_length`].
    pub fn find<'a>(
        &self,
        stream: &'a Stream,
        search: &[Vec<IntegerType>],
    ) -> Result<Vec<ContiguousSegment<'a>>> {
        let searcher = ContiguousSegmentSearcher::new(self.repetitions, self.include_chords);
        let mut by_length: HashMap<usize, Vec<ContiguousSegment<'a>>> = HashMap::new();
        let mut matched: Vec<(usize, usize)> = Vec::new();
        let mut searched = if matches!(
            self.matching,
            Matching::TransposedMultiset | Matching::TransposedInvertedMultiset
        ) {
            Searched::Multisets(Vec::new())
        } else {
            Searched::Normals(Vec::new())
        };
        for segment in search {
            if searched.already(self.matching, segment) {
                continue;
            }
            let normal = self.matching.normalize(segment);
            let length = segment.len();
            let candidates = match by_length.entry(length) {
                std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
                std::collections::hash_map::Entry::Vacant(entry) => {
                    entry.insert(searcher.by_length(stream, length)?)
                }
            };
            for (index, candidate) in candidates.iter_mut().enumerate() {
                let found = if self.repetitions == Repetitions::IgnoreAll {
                    self.match_ignore_all(candidate, &normal, length)
                } else {
                    self.match_other(candidate, &normal, length)
                };
                if let Some(active) = found {
                    candidate.active = active;
                    candidate.matched = Some(segment.clone());
                    matched.push((length, index));
                }
            }
        }
        Ok(matched
            .into_iter()
            .map(|(length, index)| by_length[&length][index].clone())
            .collect())
    }

    /// music21's `findOneIgnoreAll`: the pitch classes a run is read as,
    /// where it matches.
    fn match_ignore_all(
        &self,
        candidate: &ContiguousSegment<'_>,
        search: &Normal,
        length: usize,
    ) -> Option<Vec<u8>> {
        let classes = candidate.distinct_pitch_classes();
        let first: Vec<u8> = pitches_of(candidate.notes.first()?.element)
            .iter()
            .map(pitch_class)
            .collect();
        let before_last: Vec<u8> = candidate.notes[..candidate.notes.len() - 1]
            .iter()
            .flat_map(|note| pitches_of(note.element))
            .map(|pitch| pitch_class(&pitch))
            .collect();
        let starts = classes.len() as isize - search.len() as isize + 1;
        for start in 0..starts.max(0) as usize {
            let subset = &classes[start..(start + length).min(classes.len())];
            let as_ints: Vec<IntegerType> = subset
                .iter()
                .map(|class| IntegerType::from(*class))
                .collect();
            if !self
                .matching
                .equal(search, &self.matching.normalize(&as_ints))
            {
                continue;
            }
            if !first.contains(&subset[0]) {
                continue;
            }
            if self.matching.multiset_duplicates()
                || !before_last.contains(&subset[subset.len() - 1])
            {
                return Some(subset.to_vec());
            }
        }
        None
    }

    /// music21's `findOneOtherReps`.
    fn match_other(
        &self,
        candidate: &ContiguousSegment<'_>,
        search: &Normal,
        length: usize,
    ) -> Option<Vec<u8>> {
        let classes = candidate.pitch_classes_from_bottom();
        let first = count(candidate.notes.first()?);
        let last = count(candidate.notes.last()?);
        let lower = (classes.len() as isize - length as isize - last as isize + 1).max(0);
        let upper = (first as isize).min(classes.len() as isize + 1 - length as isize);
        for start in lower..upper {
            let start = start as usize;
            let subset = &classes[start..(start + length).min(classes.len())];
            let as_ints: Vec<IntegerType> = subset
                .iter()
                .map(|class| IntegerType::from(*class))
                .collect();
            if self
                .matching
                .equal(search, &self.matching.normalize(&as_ints))
            {
                return Some(subset.to_vec());
            }
        }
        None
    }

    /// A copy of the stream with each run found marked, as music21's
    /// `labelSegments` and its kin mark them: a line spanner from its first
    /// note to its last, in its part, and the name of the segment it matched
    /// as a lyric of its first note, unless the note already carries that
    /// lyric. Runs are marked part by part, measure by measure, offset by
    /// offset. With a `convention`, as music21's `labelTransformedSegments`
    /// takes one, each name is followed by the transformations taking the
    /// segment to the run, as ` ,T0` or ` ,P0`.
    ///
    /// # Errors
    ///
    /// As [`SegmentMatcher::find`].
    pub fn label(
        &self,
        stream: &Stream,
        named: &[(String, Vec<IntegerType>)],
        convention: Option<TransformationConvention>,
    ) -> Result<Stream> {
        let search: Vec<Vec<IntegerType>> =
            named.iter().map(|(_, segment)| segment.clone()).collect();
        let mut found: Vec<(Option<usize>, usize, usize, String)> = Vec::new();
        {
            let mut segments = self.find(stream, &search)?;
            segments.sort_by(|a, b| {
                (a.part, a.start_measure())
                    .cmp(&(b.part, b.start_measure()))
                    .then_with(|| {
                        a.start_offset()
                            .partial_cmp(&b.start_offset())
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
            });
            for segment in &segments {
                let (Some(first), Some(last)) = (segment.notes.first(), segment.notes.last())
                else {
                    continue;
                };
                let Some((name, _)) = named
                    .iter()
                    .find(|(_, wanted)| Some(wanted) == segment.matched.as_ref())
                else {
                    found.push((segment.part, first.leaf, last.leaf, String::new()));
                    continue;
                };
                let transformations = match convention {
                    Some(TransformationConvention::OriginalCentered) => {
                        segment.original_centered_transformations_from_matched()
                    }
                    Some(TransformationConvention::ZeroCentered) => {
                        segment.zero_centered_transformations_from_matched()
                    }
                    None => Vec::new(),
                };
                let mut label = name.clone();
                for (transformation, index) in transformations {
                    let convention = convention.unwrap_or(TransformationConvention::ZeroCentered);
                    label.push_str(&format!(" ,{}{index}", transformation.label(convention)));
                }
                found.push((segment.part, first.leaf, last.leaf, label));
            }
        }
        let mut labelled = stream.clone();
        for (part, first, last, label) in found {
            let target = match part {
                Some(part) => part_mut(&mut labelled, part).ok_or_else(index_error)?,
                None => &mut labelled,
            };
            let spanned = if first == last {
                vec![first]
            } else {
                vec![first, last]
            };
            target.add_spanner(Spanner::line(spanned));
            if label.is_empty() {
                continue;
            }
            let mut leaf = 0;
            let mut result = Ok(());
            target.for_each_mut(&mut |_, element| {
                if leaf == first {
                    let lyrics = match element {
                        StreamElement::Note(note) => Some(note.lyrics().to_vec()),
                        StreamElement::Chord(chord) => Some(chord.lyrics().to_vec()),
                        StreamElement::ChordSymbol(symbol) => Some(symbol.lyrics().to_vec()),
                        _ => None,
                    };
                    let present = lyrics
                        .is_some_and(|lyrics| lyrics.iter().any(|lyric| lyric.text() == label));
                    if !present {
                        result = match element {
                            StreamElement::Note(note) => note.add_lyric(&label, None, false),
                            StreamElement::Chord(chord) => chord.add_lyric(&label, None, false),
                            StreamElement::ChordSymbol(symbol) => {
                                symbol.add_lyric(&label, None, false);
                                Ok(())
                            }
                            _ => Ok(()),
                        };
                    }
                }
                leaf += 1;
            });
            result?;
        }
        Ok(labelled)
    }
}

/// The part a searcher numbered so, in a stream's walk.
fn part_mut(stream: &mut Stream, number: usize) -> Option<&mut Stream> {
    fn walk<'a>(stream: &'a mut Stream, number: usize, seen: &mut usize) -> Option<&'a mut Stream> {
        for event in stream.events_mut() {
            if let StreamElement::Stream(inner) = event.element_mut() {
                if matches!(inner.kind(), StreamKind::Part | StreamKind::PartStaff) {
                    if *seen == number {
                        return Some(inner);
                    }
                    *seen += 1;
                }
                if let Some(found) = walk(inner, number, seen) {
                    return Some(found);
                }
            }
        }
        None
    }
    walk(stream, number, &mut 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tinynotation::from_tiny_notation;

    #[test]
    fn segments_are_found_by_length() -> Result<()> {
        // Each read off music21's ContiguousSegmentSearcher for the line.
        let line = from_tiny_notation("4/4 c4 d d e f g a b")?;
        let skip = ContiguousSegmentSearcher::new(Repetitions::SkipConsecutive, true);
        let found = skip.by_length(&line, 3)?;
        assert_eq!(found.len(), 5);
        assert_eq!(found[0].pitch_classes_from_bottom(), [0, 2, 4]);
        let all = ContiguousSegmentSearcher::new(Repetitions::IncludeAll, true);
        let found = all.by_length(&line, 3)?;
        assert_eq!(found.len(), 6);
        assert_eq!(found[1].pitch_classes_from_bottom(), [2, 2, 4]);
        Ok(())
    }

    #[test]
    fn rows_are_matched_and_labelled() -> Result<()> {
        // Each read off music21's TransposedSegmentMatcher and
        // labelTransposedSegments for the same line.
        let line = from_tiny_notation("4/4 c4 d e f g a b c'")?;
        let matcher = SegmentMatcher::new(Matching::Transposed, Repetitions::SkipConsecutive, true);
        let found = matcher.find(&line, &[vec![0, 2, 4]])?;
        let actives: Vec<&[u8]> = found
            .iter()
            .map(|segment| segment.active.as_slice())
            .collect();
        assert_eq!(actives, [&[0, 2, 4][..], &[5, 7, 9], &[7, 9, 11]]);
        let labelled = matcher.label(&line, &[("A".to_string(), vec![0, 2, 4])], None)?;
        let spanners: usize = labelled.spanners().len()
            + labelled
                .parts()
                .iter()
                .map(|part| part.spanners().len())
                .sum::<usize>();
        assert_eq!(spanners, 3);
        let lyrics: Vec<Option<String>> = labelled
            .leaves()
            .into_iter()
            .filter_map(|(_, element)| match element {
                StreamElement::Note(note) => Some(note.lyrics().first().map(|lyric| lyric.text())),
                _ => None,
            })
            .collect();
        let a = Some("A".to_string());
        assert_eq!(
            lyrics,
            [a.clone(), None, None, a.clone(), a, None, None, None]
        );
        Ok(())
    }
}
