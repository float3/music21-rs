//! Repeats played out: music21's `repeat.Expander` and `expandRepeats`.

use super::{Stream, StreamElement, StreamEvent, StreamKind};
use crate::bar::{Barline, BarlineType, RepeatDirection};
use crate::defaults::{FloatType, IntegerType};
use crate::error::{Error, Result};
use crate::repeat::RepeatExpressionKind;

impl Stream {
    /// This stream with its repeats played out, as music21's
    /// `expandRepeats` plays them: every passage between repeat barlines
    /// written out as many times as its end says (twice where it says
    /// nothing), each first and second ending in its turn, and a *da capo*
    /// or *dal segno* jump taken -- to the *fine* or through the *coda*
    /// where it says so.
    ///
    /// A score has each part played out and keeps everything else it holds;
    /// a part keeps what stands outside its measures where it stood. A
    /// measure written out again is numbered as it was, with `a`, `b` and
    /// on after its number the second time and later; one played after a
    /// jump is renumbered on from the first. The repeat barlines become
    /// double ones and the jumps and markers are taken out. A spanner joins
    /// what it joined the first time through. A part with no repeats comes
    /// back as it is.
    ///
    /// ```
    /// use music21_rs::bar::{Barline, RepeatDirection};
    /// use music21_rs::{Note, Stream, StreamKind};
    ///
    /// let mut part = Stream::with_kind(StreamKind::Part);
    /// for (index, name) in ["C4", "D4"].into_iter().enumerate() {
    ///     let mut measure = Stream::with_kind(StreamKind::Measure);
    ///     measure.set_number(index as i32 + 1);
    ///     measure.push(Note::from_name(name)?.with_duration(music21_rs::Duration::whole()));
    ///     if index == 0 {
    ///         measure.set_right_barline(Some(Barline::repeat(RepeatDirection::End, None)));
    ///     }
    ///     part.insert(index as f64 * 4.0, measure);
    /// }
    /// let played = part.expand_repeats()?;
    /// let numbers: Vec<String> = played
    ///     .measures()
    ///     .iter()
    ///     .map(|measure| measure.number_with_suffix())
    ///     .collect();
    /// assert_eq!(numbers, ["1", "1a", "2"]);
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// A stream holding no measures, and repeats music21 cannot make sense
    /// of: barlines that do not pair up, endings that are not numbered one
    /// on from another or not closed by a repeat, and more than one jump.
    pub fn expand_repeats(&self) -> Result<Stream> {
        if self.kind == StreamKind::Score || self.kind == StreamKind::Opus {
            let mut expanded = Vec::new();
            // Where each leaf of the score first sounds in what is played.
            let mut moved: Vec<Option<usize>> = Vec::new();
            let mut new_count = 0;
            for event in &self.events {
                match &event.element {
                    StreamElement::Stream(part)
                        if matches!(part.kind, StreamKind::Part | StreamKind::PartStaff) =>
                    {
                        let (played, part_moved) = part.expand_part()?;
                        let first = new_count;
                        moved.extend(
                            part_moved
                                .into_iter()
                                .map(|place| place.map(|place| place + first)),
                        );
                        new_count += played.leaves().len();
                        expanded.push(StreamEvent::new(
                            event.offset,
                            StreamElement::Stream(Box::new(played)),
                        ));
                    }
                    element => {
                        let count = match element {
                            StreamElement::Stream(inner) => inner.leaves().len(),
                            _ => 1,
                        };
                        moved.extend((new_count..new_count + count).map(Some));
                        new_count += count;
                        expanded.push(StreamEvent::new(event.offset, element.clone()));
                    }
                }
            }
            let mut score = self.with_events(expanded);
            score.labels.spanners = self.labels.spanners.clone();
            for spanner in &mut score.labels.spanners {
                spanner.move_places(&moved);
            }
            return Ok(score);
        }
        self.expand_part().map(|(played, _)| played)
    }

    /// music21's `Stream.expandRepeats`, for one part, with where each of
    /// its leaves first sounds in what is played.
    fn expand_part(&self) -> Result<(Stream, Vec<Option<usize>>)> {
        let measures: Vec<(FloatType, &Stream)> = self
            .events
            .iter()
            .filter_map(|event| match &event.element {
                StreamElement::Stream(inner) if inner.kind == StreamKind::Measure => {
                    Some((event.offset, inner.as_ref()))
                }
                _ => None,
            })
            .collect();
        if measures.is_empty() {
            return Err(Error::Stream(
                "cannot process repeats on Stream that does not contain measures".to_string(),
            ));
        }
        let expander = Expander::new(&measures);
        // Nothing to play out leaves every measure where it stood.
        let Some(played) = expander.process()? else {
            return Ok((self.clone(), (0..self.leaves().len()).map(Some).collect()));
        };

        // Where each measure's leaves start among the part's, and what
        // stands outside the measures, with where it stood.
        let mut first_leaf: Vec<usize> = Vec::new();
        let mut loose: Vec<(StreamEvent, usize)> = Vec::new();
        let mut seen = 0;
        for event in &self.events {
            match &event.element {
                StreamElement::Stream(inner) => {
                    if inner.kind == StreamKind::Measure {
                        first_leaf.push(seen);
                    } else {
                        loose.push((event.clone(), seen));
                    }
                    seen += inner.leaves().len();
                }
                _ => {
                    loose.push((event.clone(), seen));
                    seen += 1;
                }
            }
        }

        // The measures appended as music21 appends them, each where the
        // furthest-reaching so far ends, then what stood outside them where
        // it stood, sorted in among them.
        let mut held: Vec<(StreamEvent, Origin)> = Vec::new();
        let mut end: FloatType = 0.0;
        for working in played {
            let length = working
                .appended_length
                .unwrap_or_else(|| working.measure.end_offset());
            let at = end;
            let origin = Origin::Measure {
                source: working.source,
                stripped: working.stripped,
            };
            held.push((
                StreamEvent::new(at, StreamElement::Stream(Box::new(working.measure))),
                origin,
            ));
            end = end.max(crate::makenotation::op_frac(at + length));
        }
        held.extend(
            loose
                .into_iter()
                .map(|(event, place)| (event, Origin::Loose(place))),
        );
        held.sort_by(|left, right| crate::makenotation::event_order(&left.0, &right.0));

        let mut moved: Vec<Option<usize>> = vec![None; seen];
        let mut placed = 0;
        for (event, origin) in &held {
            match origin {
                Origin::Loose(place) => {
                    let count = match event.element() {
                        StreamElement::Stream(inner) => inner.leaves().len(),
                        _ => 1,
                    };
                    for leaf in 0..count {
                        moved[place + leaf] = Some(placed + leaf);
                    }
                    placed += count;
                }
                Origin::Measure { source, stripped } => {
                    let source_measure = measures[*source].1;
                    for (index, (top, _, element)) in
                        source_measure.leaves_under_top().into_iter().enumerate()
                    {
                        // music21 takes out the jumps and markers standing in
                        // the measure itself.
                        let gone = *stripped
                            && matches!(element, StreamElement::RepeatExpression(_))
                            && !matches!(
                                source_measure.events[top].element,
                                StreamElement::Stream(_)
                            );
                        if gone {
                            continue;
                        }
                        let old = first_leaf[*source] + index;
                        if moved[old].is_none() {
                            moved[old] = Some(placed);
                        }
                        placed += 1;
                    }
                }
            }
        }
        let mut part =
            self.with_events(held.into_iter().map(|(event, _)| event).collect::<Vec<_>>());
        part.labels.spanners = self.labels.spanners.clone();
        for spanner in &mut part.labels.spanners {
            spanner.move_places(&moved);
        }
        Ok((part, moved))
    }
}

/// Where an element of what is played came from.
enum Origin {
    Measure { source: usize, stripped: bool },
    Loose(usize),
}

/// A measure being played out, with which measure of the part it is where
/// it is still that very measure rather than a copy of it: music21 tells
/// a measure from a copy by identity, and its endings name measures so.
#[derive(Clone, Debug)]
struct Working {
    measure: Stream,
    original: Option<usize>,
    /// The measure of the part it plays, copy or not.
    source: usize,
    /// Whether its jumps and markers were taken out.
    stripped: bool,
    /// How long it was when music21 appended it, where something has been
    /// taken out of it since: a jump standing past the barline still
    /// counts there.
    appended_length: Option<FloatType>,
}

impl Working {
    /// A copy, which no ending names.
    fn copy(&self) -> Self {
        Self {
            original: None,
            ..self.clone()
        }
    }

    fn left_repeat(&self) -> Option<RepeatDirection> {
        self.measure
            .left_barline()
            .and_then(Barline::repeat_direction)
    }

    fn right_repeat(&self) -> Option<RepeatDirection> {
        self.measure
            .right_barline()
            .and_then(Barline::repeat_direction)
    }

    /// music21's `_stripRepeatBarlines`: a repeat barline made a double one.
    fn strip_repeat_barlines(&mut self) {
        if self.left_repeat().is_some() {
            self.measure
                .set_left_barline(Some(Barline::new(BarlineType::Double)));
        }
        if self.right_repeat().is_some() {
            self.measure
                .set_right_barline(Some(Barline::new(BarlineType::Double)));
        }
    }

    /// music21's `_stripRepeatExpressions`, on one measure.
    fn strip_repeat_expressions(&mut self) {
        let events: Vec<StreamEvent> = self
            .measure
            .events
            .iter()
            .filter(|event| !matches!(event.element, StreamElement::RepeatExpression(_)))
            .cloned()
            .collect();
        let spanners = self.measure.labels.spanners.clone();
        self.measure = self.measure.with_events(events);
        self.measure.labels.spanners = spanners;
        self.stripped = true;
    }

    /// The kinds of the jumps and markers standing in the measure itself.
    fn expressions(&self) -> Vec<RepeatExpressionKind> {
        self.measure
            .events
            .iter()
            .filter_map(|event| match &event.element {
                StreamElement::RepeatExpression(expression) => Some(expression.kind()),
                _ => None,
            })
            .collect()
    }
}

/// A first, second or later ending, by the measures of the part it spans.
#[derive(Clone, Debug)]
struct Bracket {
    numbers: Vec<u32>,
    measures: Vec<usize>,
}

impl Bracket {
    fn is_first(&self, working: &Working) -> bool {
        working.original.is_some() && working.original == self.measures.first().copied()
    }

    fn is_last(&self, working: &Working) -> bool {
        working.original.is_some() && working.original == self.measures.last().copied()
    }

    fn spans(&self, working: &Working) -> bool {
        working
            .original
            .is_some_and(|original| self.measures.contains(&original))
    }
}

/// A group of endings played in turn after one passage, with the places of
/// the measures from the first ending's start to each ending's end.
#[derive(Clone, Debug, Default)]
struct Group {
    brackets: Vec<usize>,
    indices: Vec<usize>,
}

struct Expander {
    source: Vec<Working>,
    brackets: Vec<Bracket>,
    counts: std::collections::HashMap<RepeatExpressionKind, usize>,
}

/// music21's `_daCapoOrSegno`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Jump {
    DaCapo,
    Segno,
}

impl Expander {
    fn new(measures: &[(FloatType, &Stream)]) -> Self {
        let source: Vec<Working> = measures
            .iter()
            .enumerate()
            .map(|(index, (_, measure))| Working {
                measure: (*measure).clone(),
                original: Some(index),
                source: index,
                stripped: false,
                appended_length: None,
            })
            .collect();
        // A bracket opens on a measure whose ending starts and closes on one
        // whose ending stops; music21's spanner spans those measures, and
        // only those, as the reader marks them.
        let mut brackets: Vec<Bracket> = Vec::new();
        let mut open = false;
        for (index, working) in source.iter().enumerate() {
            let Some(ending) = working.measure.ending() else {
                continue;
            };
            match brackets.last_mut() {
                Some(bracket) if open && !ending.starts() => bracket.measures.push(index),
                _ => brackets.push(Bracket {
                    numbers: ending.numbers().to_vec(),
                    measures: vec![index],
                }),
            }
            open = !ending.stops();
        }
        let mut counts = std::collections::HashMap::new();
        for working in &source {
            for (_, element) in working.measure.leaves() {
                if let StreamElement::RepeatExpression(expression) = element {
                    *counts.entry(expression.kind()).or_insert(0) += 1;
                }
            }
        }
        Self {
            source,
            brackets,
            counts,
        }
    }

    fn count(&self, kind: RepeatExpressionKind) -> usize {
        self.counts.get(&kind).copied().unwrap_or(0)
    }

    /// music21's `process`: the measures played out, or `None` where there
    /// is nothing to play out.
    fn process(&self) -> Result<Option<Vec<Working>>> {
        match self.is_expandable()? {
            Some(false) => Err(Error::Stream(
                "cannot expand Stream: badly formed repeats or repeat expressions".to_string(),
            )),
            None => Ok(None),
            Some(true) => match self.da_capo_or_segno() {
                None => self
                    .process_recursive_repeat_bars(self.source.clone())
                    .map(Some),
                Some(jump) => self
                    .process_repeat_expression_and_repeats(self.source.clone(), jump)
                    .map(Some),
            },
        }
    }

    fn da_capo_or_segno(&self) -> Option<Jump> {
        use RepeatExpressionKind::*;
        let da_capo = self.count(DaCapo) + self.count(DaCapoAlFine) + self.count(DaCapoAlCoda);
        let segno = self.count(DalSegno)
            + self.count(DalSegnoAlCoda)
            + self.count(DalSegnoAlFine)
            + self.count(AlSegno);
        match (da_capo, segno) {
            (1, 0) => Some(Jump::DaCapo),
            (0, 1) => Some(Jump::Segno),
            _ => None,
        }
    }

    /// music21's `isExpandable`: `None` where there is nothing to play out.
    fn is_expandable(&self) -> Result<Option<bool>> {
        let jump = self.da_capo_or_segno();
        if jump.is_none() && !has_repeat(&self.source) {
            return Ok(None);
        }
        if !self.repeat_bars_are_coherent()? || !self.repeat_brackets_are_coherent() {
            return Ok(Some(false));
        }
        Ok(Some(match jump {
            Some(Jump::DaCapo) => self.da_capo_is_coherent(),
            Some(Jump::Segno) => self.dal_segno_is_coherent(),
            None => true,
        }))
    }

    fn repeat_bars_are_coherent(&self) -> Result<bool> {
        let (mut starts, mut ends, mut balance) = (0, 0, 0);
        for working in &self.source {
            match working.left_repeat() {
                Some(RepeatDirection::Start) => {
                    starts += 1;
                    balance += 1;
                }
                Some(RepeatDirection::End) => {
                    if balance == 0 {
                        starts += 1;
                        balance += 1;
                    }
                    ends += 1;
                    balance -= 1;
                }
                None => {}
            }
            match working.right_repeat() {
                Some(RepeatDirection::End) => {
                    if balance == 0 {
                        starts += 1;
                        balance += 1;
                    }
                    ends += 1;
                    balance -= 1;
                }
                Some(RepeatDirection::Start) => {
                    return Err(Error::Stream(format!(
                        "a right barline is found that cannot be processed: measure {}",
                        working.measure.number_with_suffix()
                    )));
                }
                None => {}
            }
        }
        Ok(matches!(balance, 0 | 1) && (starts == ends || starts + 1 == ends))
    }

    fn da_capo_is_coherent(&self) -> bool {
        use RepeatExpressionKind::*;
        let total = self.count(DaCapo) + self.count(DaCapoAlFine) + self.count(DaCapoAlCoda);
        if total > 1 {
            return false;
        }
        (self.count(DaCapo) == 1 && self.count(Coda) == 0)
            || (self.count(DaCapoAlFine) == 1 && self.count(Fine) == 1)
            || (self.count(DaCapoAlCoda) == 1 && self.count(Coda) == 2)
    }

    fn dal_segno_is_coherent(&self) -> bool {
        use RepeatExpressionKind::*;
        let total = self.count(AlSegno)
            + self.count(DalSegno)
            + self.count(DalSegnoAlCoda)
            + self.count(DalSegnoAlFine);
        if total > 1 {
            return false;
        }
        let (segnos, codas, fines) = (self.count(Segno), self.count(Coda), self.count(Fine));
        (self.count(AlSegno) == 1 && segnos == 1 && codas == 0)
            || (self.count(DalSegno) == 1 && segnos == 1 && codas == 0)
            || (self.count(DalSegnoAlFine) == 1 && codas == 0 && segnos == 1 && fines == 1)
            || (self.count(DalSegnoAlCoda) == 1 && codas == 2 && segnos == 1 && fines == 0)
    }

    /// music21's `_groupRepeatBracketIndices`.
    fn group_bracket_indices(&self, measures: &[Working]) -> Vec<Group> {
        let mut groups = Vec::new();
        let mut found_numbers: Vec<u32> = Vec::new();
        let mut group = Group::default();
        for (index, working) in measures.iter().enumerate() {
            for (bracket_index, bracket) in self.brackets.iter().enumerate() {
                if !bracket.is_first(working) {
                    continue;
                }
                if bracket
                    .numbers
                    .first()
                    .is_some_and(|first| found_numbers.contains(first))
                {
                    groups.push(std::mem::take(&mut group));
                    found_numbers.clear();
                }
                found_numbers.extend(&bracket.numbers);
                group.brackets.push(bracket_index);
                for (sub, candidate) in measures.iter().enumerate() {
                    if bracket.is_last(candidate) {
                        group.indices.push(sub);
                        break;
                    }
                    if sub >= index {
                        group.indices.push(sub);
                    }
                }
            }
        }
        // music21 keeps the group it was building whatever it holds.
        groups.push(group);
        groups
    }

    fn repeat_brackets_are_coherent(&self) -> bool {
        for group in self.group_bracket_indices(&self.source) {
            if group.brackets.is_empty() {
                return true;
            }
            if group.brackets.len() > 1 {
                let numbers: Vec<u32> = group
                    .brackets
                    .iter()
                    .flat_map(|index| self.brackets[*index].numbers.clone())
                    .collect();
                let highest = numbers.iter().copied().max().unwrap_or(0);
                if numbers != (1..=highest).collect::<Vec<_>>() {
                    return false;
                }
            }
            let mut spanned: Vec<usize> = Vec::new();
            for (count, index) in group.brackets.iter().enumerate() {
                let bracket = &self.brackets[*index];
                for measure in &bracket.measures {
                    if spanned.contains(measure) {
                        return false;
                    }
                    spanned.push(*measure);
                }
                let last =
                    &self.source[*bracket.measures.last().expect("an ending spans a measure")];
                if last.right_repeat().is_none()
                    && (group.brackets.len() == 1 || count + 1 < group.brackets.len())
                {
                    return false;
                }
            }
        }
        true
    }

    /// music21's `findInnermostRepeatIndices`.
    fn innermost_repeat_indices(measures: &[Working]) -> Vec<usize> {
        let mut starts: Vec<usize> = Vec::new();
        for (index, working) in measures.iter().enumerate() {
            match working.left_repeat() {
                Some(RepeatDirection::Start) => starts.push(index),
                Some(RepeatDirection::End) => {
                    return (starts.last().copied().unwrap_or(0)..index).collect();
                }
                None => {}
            }
            if working.right_repeat() == Some(RepeatDirection::End) {
                return (starts.last().copied().unwrap_or(0)..=index).collect();
            }
        }
        Vec::new()
    }

    /// music21's `_getEndRepeatBar`: whether the measure closing the repeat
    /// is the last one repeated, and how many times it is played.
    fn end_repeat_bar(measures: &[Working], index: usize) -> Result<(bool, u32)> {
        let last = &measures[index];
        if last.right_repeat() == Some(RepeatDirection::End) {
            let times = last.measure.right_barline().and_then(Barline::repeat_times);
            return Ok((true, times.unwrap_or(2)));
        }
        let Some(after) = measures.get(index + 1) else {
            return Err(Error::Stream(format!(
                "cannot find an end Repeat bar after the given end: {index}"
            )));
        };
        if after.left_repeat() == Some(RepeatDirection::End) {
            let times = after.measure.left_barline().and_then(Barline::repeat_times);
            return Ok((false, times.unwrap_or(2)));
        }
        Err(Error::Stream(
            "cannot find an end Repeat bar in the expected position".to_string(),
        ))
    }

    /// music21's `processInnermostRepeatBars`.
    fn process_innermost_repeat_bars(
        measures: &mut [Working],
        forced: Option<(&[usize], u32)>,
        expansion_only: bool,
    ) -> Result<Vec<Working>> {
        let indices: Vec<usize> = match forced {
            Some((indices, _)) => indices.to_vec(),
            None => Self::innermost_repeat_indices(measures),
        };
        let mut out = Vec::new();
        let mut strip_next = false;
        let mut times_given = forced.map(|(_, times)| times);
        let mut index = 0;
        while index < measures.len() {
            let (Some(&first), Some(&last)) = (indices.first(), indices.last()) else {
                break;
            };
            if index == first {
                let (closes_itself, times_found) = match Self::end_repeat_bar(measures, last) {
                    Ok(found) => found,
                    Err(error) if forced.is_none() => return Err(error),
                    // A forced run with no repeat bar at its end: music21
                    // leaves its last and its end-barline measure as `None`,
                    // which counts as the same measure.
                    Err(_) => (true, 2),
                };
                let times = *times_given.get_or_insert(times_found);
                for time in 0..times {
                    for &place in &indices {
                        let mut copy = measures[place].copy();
                        if place == first || place == last {
                            copy.strip_repeat_barlines();
                        }
                        if times >= 2 && time < times - 1 {
                            copy.strip_repeat_expressions();
                        }
                        if time != 0 {
                            let letter = char::from(b'a' + ((time - 1) % 26) as u8);
                            copy.measure.set_number_suffix(Some(letter.to_string()));
                        }
                        out.push(copy);
                    }
                }
                if !closes_itself {
                    strip_next = true;
                }
                index = last + 1;
            } else {
                if !expansion_only {
                    if strip_next {
                        measures[index].strip_repeat_barlines();
                        strip_next = false;
                    }
                    out.push(measures[index].clone());
                }
                index += 1;
            }
        }
        Ok(out)
    }

    /// music21's `_processInnermostRepeatsAndBrackets`.
    fn process_innermost_repeats_and_brackets(
        &self,
        mut measures: Vec<Working>,
        done: &mut Vec<usize>,
    ) -> Result<Vec<Working>> {
        let groups = self.group_bracket_indices(&measures);
        let innermost = Self::innermost_repeat_indices(&measures);
        let mut focus: Option<Group> = None;
        if let (Some(&first), Some(&last)) = (innermost.first(), innermost.last()) {
            for group in &groups {
                for &bracket in &group.brackets {
                    if done.contains(&bracket) {
                        break;
                    }
                    let spans = &self.brackets[bracket];
                    if spans.spans(&measures[first]) || spans.spans(&measures[last]) {
                        focus = Some(group.clone());
                        break;
                    }
                }
                if focus.is_some() {
                    break;
                }
            }
        }
        let Some(focus) = focus else {
            return Self::process_innermost_repeat_bars(&mut measures, None, false);
        };
        let start = innermost[0];
        let mut boundaries: Vec<(usize, Vec<usize>, Vec<usize>)> = Vec::new();
        for &bracket_index in &focus.brackets {
            done.push(bracket_index);
            let bracket = &self.brackets[bracket_index];
            let mut end = None;
            let mut bracket_start = None;
            for (index, working) in measures.iter().enumerate() {
                if bracket.is_last(working) {
                    end = Some(index);
                }
                if bracket.is_first(working) {
                    bracket_start = Some(index);
                }
            }
            let (Some(end), Some(bracket_start)) = (end, bracket_start) else {
                return Err(Error::Stream(
                    "failed to find start or end index of bracket expansion".to_string(),
                ));
            };
            let mut indices: Vec<usize> = (start..=end).collect();
            for (_, _, earlier) in &boundaries {
                indices.retain(|index| !earlier.contains(index));
            }
            boundaries.push((bracket_index, indices, (bracket_start..=end).collect()));
        }
        let mut repeats: Vec<Vec<Working>> = Vec::new();
        let mut highest = None;
        for (bracket_index, indices, _) in &boundaries {
            let times = self.brackets[*bracket_index].numbers.len() as u32;
            repeats.push(Self::process_innermost_repeat_bars(
                &mut measures,
                Some((indices, times)),
                true,
            )?);
            highest = indices.iter().copied().max();
        }
        let mut out: Vec<Working> = measures[..start].to_vec();
        for sub in repeats {
            for mut working in sub {
                working.strip_repeat_barlines();
                out.push(working);
            }
        }
        if let Some(highest) = highest {
            out.extend(measures[highest + 1..].iter().cloned());
        }
        Ok(out)
    }

    /// music21's `_processRecursiveRepeatBars`.
    fn process_recursive_repeat_bars(&self, mut measures: Vec<Working>) -> Result<Vec<Working>> {
        let mut done: Vec<usize> = Vec::new();
        for _ in 0..100 {
            measures = self.process_innermost_repeats_and_brackets(measures, &mut done)?;
            if !has_repeat(&measures) {
                break;
            }
        }
        Ok(measures)
    }

    /// music21's `_processRepeatExpressionAndRepeats`: the passage up to the
    /// jump, then from where it jumps to on to the fine, the end or the
    /// first coda and on from the second.
    fn process_repeat_expression_and_repeats(
        &self,
        measures: Vec<Working>,
        jump: Jump,
    ) -> Result<Vec<Working>> {
        use RepeatExpressionKind::*;
        let command = [
            DaCapo,
            DaCapoAlCoda,
            DaCapoAlFine,
            AlSegno,
            DalSegno,
            DalSegnoAlCoda,
            DalSegnoAlFine,
        ]
        .into_iter()
        .find(|kind| self.count(*kind) == 1)
        .ok_or_else(|| Error::Stream("no repeat command found".to_string()))?;
        let index_of = |kind: RepeatExpressionKind| -> Vec<usize> {
            measures
                .iter()
                .enumerate()
                .flat_map(|(index, working)| {
                    working
                        .expressions()
                        .into_iter()
                        .filter(move |found| *found == kind)
                        .map(move |_| index)
                })
                .collect()
        };
        let missing =
            || Error::Stream("a repeat expression is not where it is expected".to_string());
        let jump_back = *index_of(command).first().ok_or_else(missing)?;
        let start = match jump {
            Jump::DaCapo => 0,
            Jump::Segno => *index_of(Segno).first().ok_or_else(missing)?,
        };
        let last = measures.len() - 1;
        let end = if matches!(command, DaCapoAlFine | DalSegnoAlFine) {
            *index_of(Fine).first().ok_or_else(missing)?
        } else {
            last
        };
        let mut segments = vec![(0, jump_back)];
        if matches!(command, DaCapoAlCoda | DalSegnoAlCoda) {
            let codas = index_of(Coda);
            let (Some(&jump_from), Some(&resume)) = (codas.first(), codas.get(1)) else {
                return Err(missing());
            };
            segments.push((start, jump_from));
            segments.push((resume, last));
        } else {
            segments.push((start, end));
        }

        let mut number: IntegerType = measures[0].measure.number();
        let mut out = Vec::new();
        for (count, (from, to)) in segments.into_iter().enumerate() {
            let mut segment: Vec<Working> = Vec::new();
            for working in measures.iter().take(to + 1).skip(from) {
                let mut copy = working.copy();
                copy.measure.set_number(number);
                segment.push(copy);
                number += 1;
            }
            if has_repeat(&segment) {
                // music21 plays the repeats after the jump only where the
                // jump says to, and none of its jumps do.
                if count != 1 {
                    segment = self.process_recursive_repeat_bars(segment)?;
                    number = segment
                        .last()
                        .map_or(number, |last| last.measure.number() + 1);
                }
                for working in &mut segment {
                    working.strip_repeat_barlines();
                }
            }
            out.extend(segment);
        }
        for working in &mut out {
            working.appended_length = Some(working.measure.end_offset());
            working.strip_repeat_expressions();
        }
        Ok(out)
    }
}

/// music21's `_hasRepeat`.
fn has_repeat(measures: &[Working]) -> bool {
    measures.iter().any(|working| {
        working.left_repeat().is_some() || working.right_repeat() == Some(RepeatDirection::End)
    })
}

#[cfg(test)]
mod tests {
    use crate::bar::{Barline, Ending, RepeatDirection};
    use crate::repeat::{RepeatExpression, RepeatExpressionKind};
    use crate::stream::{Stream, StreamElement, StreamKind};
    use crate::{Duration, Note};

    /// A part of whole-note measures numbered from one, each shaped by
    /// `shape` from its number.
    fn part(count: i32, shape: impl Fn(i32, &mut Stream)) -> Stream {
        let mut part = Stream::with_kind(StreamKind::Part);
        for number in 1..=count {
            let mut measure = Stream::with_kind(StreamKind::Measure);
            measure.set_number(number);
            measure.push(
                Note::from_name("C4")
                    .unwrap()
                    .with_duration(Duration::whole()),
            );
            shape(number, &mut measure);
            part.insert(f64::from(number - 1) * 4.0, measure);
        }
        part
    }

    fn played(part: &Stream) -> Vec<String> {
        part.expand_repeats()
            .unwrap()
            .measures()
            .iter()
            .map(|measure| measure.number_with_suffix())
            .collect()
    }

    #[test]
    fn a_passage_between_repeat_barlines_is_played_twice() {
        let part = part(4, |number, measure| match number {
            2 => measure.set_left_barline(Some(Barline::repeat(RepeatDirection::Start, None))),
            3 => measure.set_right_barline(Some(Barline::repeat(RepeatDirection::End, None))),
            _ => {}
        });
        assert_eq!(played(&part), ["1", "2", "3", "2a", "3a", "4"]);
        let expanded = part.expand_repeats().unwrap();
        // Laid end to end, and the repeat barlines made double ones.
        assert_eq!(expanded.end_offset(), 24.0);
        assert!(
            expanded.measures()[2]
                .right_barline()
                .unwrap()
                .repeat_direction()
                .is_none()
        );
    }

    #[test]
    fn a_first_ending_is_played_then_the_second() {
        let part = part(4, |number, measure| match number {
            2 => {
                measure.set_ending(Some(Ending::new(vec![1], true, true)));
                measure.set_right_barline(Some(Barline::repeat(RepeatDirection::End, None)));
            }
            3 => measure.set_ending(Some(Ending::new(vec![2], true, true))),
            _ => {}
        });
        // Each ending's run is played once, so music21 numbers neither
        // time through with a suffix.
        assert_eq!(played(&part), ["1", "2", "1", "3", "4"]);
    }

    #[test]
    fn da_capo_al_fine_goes_back_to_the_fine() {
        let part = part(3, |number, measure| {
            let mark = match number {
                1 => RepeatExpressionKind::Fine,
                3 => RepeatExpressionKind::DaCapoAlFine,
                _ => return,
            };
            measure.insert(
                0.0,
                StreamElement::RepeatExpression(RepeatExpression::new(mark)),
            );
        });
        // Renumbered on from the first, and the marks taken out.
        assert_eq!(played(&part), ["1", "2", "3", "4"]);
        let expanded = part.expand_repeats().unwrap();
        assert!(
            expanded
                .recurse()
                .iter()
                .all(|(_, element)| !matches!(element, StreamElement::RepeatExpression(_)))
        );
    }

    #[test]
    fn repeats_that_do_not_pair_up_are_refused() {
        let part = part(3, |number, measure| {
            if number != 2 {
                measure.set_left_barline(Some(Barline::repeat(RepeatDirection::Start, None)));
            }
        });
        assert!(part.expand_repeats().is_err());
    }

    #[test]
    fn a_stream_with_no_measures_is_refused() {
        assert!(
            Stream::with_kind(StreamKind::Part)
                .expand_repeats()
                .is_err()
        );
    }
}
