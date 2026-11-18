//! Several staves written as one part: music21's `PartStaffExporterMixin`.
//!
//! A piano's two staves are two `PartStaff` streams in music21 and one
//! `<part>` in MusicXML. Each staff is written as a part of its own first;
//! then every note, direction and forward is told which staff it is on, the
//! later staves' measures are moved into the first's behind a `<backup>`,
//! and the first measure's `<attributes>` is given the number of staves and
//! a clef for each.
//!
//! The steps work on any tree that is an [`XmlTree`]: the crate's own, as
//! its writer joins staves, and Python's `ElementTree`, as music21's mixin
//! is called on one.

use super::tree::{Element, Node};
use crate::error::{Error, Result};

fn export_error(message: impl Into<String>) -> Error {
    Error::music_xml(message.into())
}

/// An XML element the joining steps can read and change: its tag,
/// attributes, text and children, counted as ElementTree counts them,
/// comments included.
///
/// [`XmlTree::child`] hands back a child to read. What it hands back may be
/// a copy, so a child is changed through [`XmlTree::with_child`].
pub trait XmlTree: Sized {
    /// The tag, or nothing for a comment.
    fn tag(&self) -> Option<String>;
    /// An attribute, where it is set.
    fn attribute(&self, name: &str) -> Option<String>;
    /// Sets an attribute.
    fn set_attribute(&mut self, name: &str, value: &str) -> Result<()>;
    /// The text, where there is some.
    fn text(&self) -> Option<String>;
    /// Sets the text.
    fn set_text(&mut self, text: &str) -> Result<()>;
    /// How many children, comments included.
    fn len(&self) -> usize;
    /// Whether it has no children.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// The child at an index, to read.
    fn child(&self, index: usize) -> Option<Self>;
    /// Runs `change` on the child at an index, where it is an element.
    fn with_child<R>(&mut self, index: usize, change: impl FnOnce(&mut Self) -> R) -> Option<R>;
    /// Puts a node in at an index.
    fn insert(&mut self, index: usize, child: Self) -> Result<()>;
    /// Takes away the child at an index.
    fn remove(&mut self, index: usize) -> Result<()>;
    /// A new element of this tag, of the same kind of tree.
    fn element(&self, tag: &str) -> Result<Self>;
    /// A new comment, of the same kind of tree.
    fn comment(&self, text: &str) -> Result<Self>;
    /// A copy of the whole element, unconnected to this one.
    fn deep_copy(&self) -> Result<Self>;
}

impl XmlTree for Element {
    fn tag(&self) -> Option<String> {
        Some(Element::tag(self).to_string())
    }

    fn attribute(&self, name: &str) -> Option<String> {
        self.get(name).map(str::to_string)
    }

    fn set_attribute(&mut self, name: &str, value: &str) -> Result<()> {
        self.set(name.to_string(), value);
        Ok(())
    }

    fn text(&self) -> Option<String> {
        Element::text(self).map(str::to_string)
    }

    fn set_text(&mut self, text: &str) -> Result<()> {
        Element::set_text(self, text);
        Ok(())
    }

    fn len(&self) -> usize {
        self.children().len()
    }

    fn child(&self, index: usize) -> Option<Self> {
        match self.children().get(index)? {
            Node::Element(element) => Some(element.clone()),
            Node::Comment(text) => Some(comment_carrier(text)),
        }
    }

    fn with_child<R>(&mut self, index: usize, change: impl FnOnce(&mut Self) -> R) -> Option<R> {
        match self.children_mut().get_mut(index)? {
            Node::Element(element) => Some(change(element)),
            Node::Comment(_) => None,
        }
    }

    fn insert(&mut self, index: usize, child: Self) -> Result<()> {
        let node = match carried_comment(&child) {
            Some(text) => Node::Comment(text),
            None => Node::Element(child),
        };
        self.children_mut().insert(index, node);
        Ok(())
    }

    fn remove(&mut self, index: usize) -> Result<()> {
        self.children_mut().remove(index);
        Ok(())
    }

    fn element(&self, tag: &str) -> Result<Self> {
        Ok(Element::new(tag.to_string()))
    }

    fn comment(&self, text: &str) -> Result<Self> {
        Ok(comment_carrier(text))
    }

    fn deep_copy(&self) -> Result<Self> {
        Ok(self.clone())
    }
}

/// The tag a comment of the crate's tree travels under while it is handed
/// about as an element: no XML tag can start with `!`.
const COMMENT_TAG: &str = "!--";

/// A comment, carried as an element.
fn comment_carrier(text: &str) -> Element {
    Element::with_text(COMMENT_TAG, text)
}

/// The text of a carried comment.
fn carried_comment(element: &Element) -> Option<String> {
    (Element::tag(element) == COMMENT_TAG).then(|| Element::text(element).unwrap_or("").to_string())
}

/// The child elements of a tree with their indices, comments passed over.
fn elements_of<T: XmlTree>(tree: &T) -> Vec<(usize, T)> {
    (0..tree.len())
        .filter_map(|index| tree.child(index).map(|child| (index, child)))
        .filter(|(_, child)| is_element(child))
        .collect()
}

/// Whether a node is an element rather than a comment.
fn is_element<T: XmlTree>(node: &T) -> bool {
    node.tag().is_some_and(|tag| tag != COMMENT_TAG)
}

/// Whether a node is an element of this tag.
fn tagged<T: XmlTree>(node: &T, tag: &str) -> bool {
    node.tag().as_deref() == Some(tag)
}

/// The first child of a tag, with its index: ElementTree's `find`.
fn find_in<T: XmlTree>(tree: &T, tag: &str) -> Option<(usize, T)> {
    elements_of(tree)
        .into_iter()
        .find(|(_, child)| tagged(child, tag))
}

/// A new element of a tag holding text.
fn with_text_of<T: XmlTree>(tree: &T, tag: &str, text: &str) -> Result<T> {
    let mut element = tree.element(tag)?;
    element.set_text(text)?;
    Ok(element)
}

/// music21's `helpers.insertBeforeElements`: `child` put in before the
/// first child tagged one of `tags`, or at the end.
fn insert_before<T: XmlTree>(tree: &mut T, child: T, tags: &[&str]) -> Result<()> {
    let at = elements_of(tree)
        .into_iter()
        .find(|(_, element)| {
            element
                .tag()
                .is_some_and(|tag| tags.contains(&tag.as_str()))
        })
        .map_or(tree.len(), |(index, _)| index);
    tree.insert(at, child)
}

/// Whether the staves of a group start in the same key, or meter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Across {
    /// Every staff starts the same.
    Same,
    /// Some staff starts differently, so each staff's is written.
    Different,
}

/// Which marks the staves of a group start with differently: music21's
/// `isMultiAttribute` for keys and meters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StaffDifferences {
    /// The keys.
    pub keys: Across,
    /// The meters.
    pub meters: Across,
}

/// The tags a `<staff>` goes before inside a note, direction or forward.
const BEFORE_STAFF: [&str; 5] = ["beam", "notations", "lyric", "play", "sound"];

/// The children of a measure told their staff when staves are joined.
pub const STAFF_TAGGED: [&str; 4] = ["note", "direction", "forward", "harmony"];

/// music21's `addStaffTags`: a `<staff>` in each child of a measure whose
/// tag is one of `tags`, before its beams, notations, lyrics and sound.
///
/// # Errors
///
/// A tree that is no `<measure>`, and a child that has a `<staff>` already.
pub fn add_staff_tags<T: XmlTree>(measure: &mut T, staff: usize, tags: &[&str]) -> Result<()> {
    if !tagged(measure, "measure") {
        return Err(export_error("addStaffTags() only accepts <measure> tags"));
    }
    let number = measure.attribute("number").unwrap_or_default();
    for (index, child) in elements_of(measure) {
        if !child.tag().is_some_and(|tag| tags.contains(&tag.as_str())) {
            continue;
        }
        if find_in(&child, "staff").is_some() {
            return Err(export_error(format!(
                "In part (), measure ({number}): Attempted to create a second <staff> tag"
            )));
        }
        let tag = with_text_of(&child, "staff", &staff.to_string())?;
        measure
            .with_child(index, |child| insert_before(child, tag, &BEFORE_STAFF))
            .transpose()?;
    }
    Ok(())
}

/// music21's `addStaffTagsMultiStaffParts`: [`add_staff_tags`] over every
/// measure of each staff, the first staff numbered one.
///
/// # Errors
///
/// A child that has a `<staff>` already.
pub fn add_staff_tags_to_staves<T: XmlTree>(parts: &mut [T]) -> Result<()> {
    for (position, part) in parts.iter_mut().enumerate() {
        for (index, _) in elements_of(part)
            .into_iter()
            .filter(|(_, child)| tagged(child, "measure"))
        {
            part.with_child(index, |measure| {
                add_staff_tags(measure, position + 1, &STAFF_TAGGED)
            })
            .transpose()?;
        }
    }
    Ok(())
}

/// music21's `movePartStaffMeasureContents`, `setEarliestAttributesAndClefs`
/// and the staff tags before them: the staves of `parts`, first to last,
/// joined into the first.
///
/// # Errors
///
/// A child that has a `<staff>` already, measures whose numbers cannot be
/// put in order, more clefs than staves.
pub fn join<T: XmlTree>(parts: &mut [T], differences: StaffDifferences) -> Result<()> {
    add_staff_tags_to_staves(parts)?;
    move_part_staff_contents(parts)?;
    set_earliest_attributes_and_clefs(parts, differences)
}

/// music21's `movePartStaffMeasureContents`: every later staff's measures
/// moved into the first's, a measure the first lacks put in where it falls.
///
/// # Errors
///
/// Measures whose numbers cannot be put in order.
pub fn move_part_staff_contents<T: XmlTree>(parts: &mut [T]) -> Result<()> {
    let Some((target, sources)) = parts.split_first_mut() else {
        return Ok(());
    };
    for (index, source) in sources.iter().enumerate() {
        let staff = index + 2;
        let insertions = process_subsequent_part_staff(target, source, staff)?;
        let mut inserted = 0;
        for (at, nodes) in insertions {
            for node in nodes {
                target.insert(at + inserted, node)?;
                inserted += 1;
            }
        }
    }
    Ok(())
}

/// music21's `measureNumberComesBefore`: whether one measure number comes
/// strictly before another, suffixes and all.
///
/// ```
/// use music21::musicxml::measure_number_comes_before;
///
/// assert!(measure_number_comes_before("23", "23a"));
/// assert!(measure_number_comes_before("23b", "24a"));
/// assert!(!measure_number_comes_before("23b", "23a"));
/// ```
pub fn measure_number_comes_before(first: &str, second: &str) -> bool {
    if first == second {
        return false;
    }
    let split = |number: &str| {
        let digits: String = number.chars().take_while(char::is_ascii_digit).collect();
        let suffix = number[digits.len()..].to_string();
        (digits.parse::<i64>().unwrap_or(0), suffix)
    };
    let (first_number, first_suffix) = split(first);
    let (second_number, second_suffix) = split(second);
    if first_number != second_number {
        return first_number < second_number;
    }
    first_suffix <= second_suffix
}

/// music21's `processSubsequentPartStaff`: each measure of `source` moved
/// into the measure of `target` with the same number, and the measures
/// `target` has no number for gathered, with their divider comments, to be
/// inserted where they fall.
///
/// # Errors
///
/// Measures whose numbers cannot be put in order.
pub fn process_subsequent_part_staff<T: XmlTree>(
    target: &mut T,
    source: &T,
    staff: usize,
) -> Result<Vec<(usize, Vec<T>)>> {
    let divider = |number: &str| {
        source.comment(&format!(
            "========================= Measure {number} =========================="
        ))
    };
    let mut insertions: Vec<(usize, Vec<T>)> = Vec::new();
    let mut add =
        |at: usize, nodes: Vec<T>| match insertions.iter_mut().find(|(index, _)| *index == at) {
            Some((_, existing)) => existing.extend(nodes),
            None => insertions.push((at, nodes)),
        };
    let source_measures: Vec<T> = elements_of(source)
        .into_iter()
        .filter(|(_, child)| tagged(child, "measure"))
        .map(|(_, child)| child)
        .collect();
    let mut next = 0;
    let before = |first: &Option<String>, second: &Option<String>| match (first, second) {
        (Some(first), Some(second)) => measure_number_comes_before(first, second),
        _ => false,
    };
    for (index, target_measure) in elements_of(target) {
        if !tagged(&target_measure, "measure") {
            continue;
        }
        let Some(mut source_measure) = source_measures.get(next) else {
            return Ok(insertions);
        };
        let target_number = target_measure.attribute("number");
        let mut source_number = source_measure.attribute("number");
        if target_number == source_number {
            move_measure_into(target, index, source_measure, staff)?;
            next += 1;
            continue;
        }
        // A measure the later staff lacks: wait for it with the same source
        // measure.
        if before(&target_number, &source_number) {
            continue;
        }
        // Measures the first staff lacks go in on their own, until the
        // numbers meet again.
        while before(&source_number, &target_number) {
            let number = source_number.clone().unwrap_or_default();
            add(index, vec![divider(&number)?, source_measure.deep_copy()?]);
            next += 1;
            let Some(following) = source_measures.get(next) else {
                return Ok(insertions);
            };
            source_measure = following;
            source_number = source_measure.attribute("number");
        }
        if target_number == source_number {
            move_measure_into(target, index, source_measure, staff)?;
            next += 1;
            continue;
        }
        if before(&target_number, &source_number) {
            continue;
        }
        return Err(export_error(format!(
            "joinPartStaffs() was unable to order the measures {}, {}",
            target_number.as_deref().unwrap_or("None"),
            source_number.as_deref().unwrap_or("None")
        )));
    }
    let end = target.len();
    for remaining in &source_measures[next..] {
        let Some(number) = remaining.attribute("number") else {
            continue;
        };
        add(end, vec![divider(&number)?, remaining.deep_copy()?]);
    }
    Ok(insertions)
}

/// [`move_measure_contents`] into the target's child at an index.
fn move_measure_into<T: XmlTree>(
    target: &mut T,
    index: usize,
    source: &T,
    staff: usize,
) -> Result<()> {
    target
        .with_child(index, |measure| {
            move_measure_contents(source, measure, staff)
        })
        .transpose()
        .map(|_| ())
}

/// The tags a `<voice>` goes before inside a `<note>`.
const BEFORE_VOICE: [&str; 8] = [
    "type",
    "dot",
    "accidental",
    "time-modification",
    "stem",
    "notehead",
    "notehead-text",
    "staff",
];

/// A duration's text as a whole number, nought where it is none.
fn duration_of<T: XmlTree>(element: &T) -> i64 {
    find_in(element, "duration")
        .and_then(|(_, duration)| duration.text())
        .and_then(|text| text.trim().parse().ok())
        .unwrap_or(0)
}

/// music21's `moveMeasureContents`: the contents of `measure` moved into
/// `other` behind a `<backup>` to the start, with voices numbered on from
/// `other`'s, mid-measure clefs told their staff, and a barline replacing
/// the one at the same end.
///
/// # Errors
///
/// Either tree no `<measure>`, and a voice that is no number.
pub fn move_measure_contents<T: XmlTree>(measure: &T, other: &mut T, staff: usize) -> Result<()> {
    if !tagged(measure, "measure") || !tagged(other, "measure") {
        return Err(export_error(format!(
            "moveMeasureContents() called on <{}> and <{}> (not measures).",
            measure.tag().unwrap_or_default(),
            other.tag().unwrap_or_default()
        )));
    }

    // The voices `other` has, or a first one for each of its notes.
    let mut max_voices: i64 = 0;
    for (_, child) in elements_of(other) {
        for (_, voice) in elements_of(&child)
            .into_iter()
            .filter(|(_, v)| tagged(v, "voice"))
        {
            if let Some(text) = voice.text() {
                let number: i64 = text.parse().map_err(|_| {
                    export_error(format!("invalid literal for int() with base 10: '{text}'"))
                })?;
                max_voices = max_voices.max(number);
            }
        }
    }
    let lacked_voice = max_voices == 0;
    if lacked_voice {
        for (index, note) in elements_of(other) {
            if !tagged(&note, "note") {
                continue;
            }
            let voice = with_text_of(&note, "voice", "1")?;
            other
                .with_child(index, |note| insert_before(note, voice, &BEFORE_VOICE))
                .transpose()?;
        }
        max_voices = 1;
    }

    // Back to the start of the measure.
    let mut backup: i64 = 0;
    for (_, child) in elements_of(other) {
        match child.tag().as_deref() {
            Some("note") if find_in(&child, "chord").is_none() => backup += duration_of(&child),
            Some("forward") => backup += duration_of(&child),
            Some("backup") => backup -= duration_of(&child),
            _ => {}
        }
    }
    if backup != 0 {
        let mut written = other.element("backup")?;
        written.insert(0, with_text_of(other, "duration", &backup.to_string())?)?;
        let end = other.len();
        other.insert(end, written)?;
    }

    // What `measure` holds, moved over.
    for (_, element) in elements_of(measure) {
        let mut element = element.deep_copy()?;
        match element.tag().as_deref() {
            Some("print") => continue,
            Some("attributes") => {
                if find_in(&element, "divisions").is_some() {
                    continue;
                }
                for (index, _) in elements_of(&element)
                    .into_iter()
                    .filter(|(_, child)| tagged(child, "clef"))
                {
                    element
                        .with_child(index, |clef| {
                            clef.set_attribute("number", &staff.to_string())
                        })
                        .transpose()?;
                }
            }
            Some("barline") => {
                let location = element.attribute("location");
                let existing: Vec<usize> = elements_of(other)
                    .into_iter()
                    .filter(|(_, child)| {
                        tagged(child, "barline") && child.attribute("location") == location
                    })
                    .map(|(index, _)| index)
                    .collect();
                for index in existing.into_iter().rev() {
                    other.remove(index)?;
                }
            }
            Some("note") => match find_in(&element, "voice") {
                Some((index, voice)) => {
                    if lacked_voice && let Some(text) = voice.text() {
                        let number: i64 = text.parse().unwrap_or(0);
                        element
                            .with_child(index, |voice| voice.set_text(&(number + 1).to_string()))
                            .transpose()?;
                    }
                }
                None => {
                    let voice = with_text_of(&element, "voice", &(max_voices + 1).to_string())?;
                    insert_before(&mut element, voice, &BEFORE_VOICE)?;
                }
            },
            _ => {}
        }
        let end = other.len();
        other.insert(end, element)?;
    }
    Ok(())
}

/// The index of the first `<attributes>` of any measure of a part, where
/// ElementTree's `find('measure/attributes')` finds it: the measure's
/// index and the attributes' index in it.
fn first_attributes<T: XmlTree>(part: &T) -> Option<(usize, usize)> {
    elements_of(part)
        .into_iter()
        .filter(|(_, child)| tagged(child, "measure"))
        .find_map(|(index, measure)| find_in(&measure, "attributes").map(|(at, _)| (index, at)))
}

/// The first child of a tag inside the first `<attributes>` that has one:
/// `find('measure/attributes/<tag>')`.
fn first_in_attributes<T: XmlTree>(part: &T, tag: &str) -> Option<T> {
    elements_of(part)
        .into_iter()
        .filter(|(_, child)| tagged(child, "measure"))
        .flat_map(|(_, measure)| elements_of(&measure))
        .filter(|(_, child)| tagged(child, "attributes"))
        .find_map(|(_, attributes)| find_in(&attributes, tag).map(|(_, found)| found))
}

/// The tags `<staves>` goes before in `<attributes>`.
const BEFORE_STAVES: [&str; 7] = [
    "part-symbol",
    "instruments",
    "clef",
    "staff-details",
    "transpose",
    "directive",
    "measure-style",
];

/// music21's `setEarliestAttributesAndClefsPartStaff`: the first staff's
/// first `<attributes>` told how many staves there are, given a clef for
/// each, and, where they differ across the staves, a key and a meter for
/// each.
///
/// # Errors
///
/// More clefs than staves.
pub fn set_earliest_attributes_and_clefs<T: XmlTree>(
    parts: &mut [T],
    differences: StaffDifferences,
) -> Result<()> {
    let Some((target, sources)) = parts.split_first_mut() else {
        return Ok(());
    };
    let Some((measure, at)) = first_attributes(target) else {
        return Ok(());
    };
    let staves = sources.len() + 1;
    target
        .with_child(measure, |measure| {
            measure
                .with_child(at, |attributes| {
                    earliest_attributes(attributes, sources, staves, differences)
                })
                .transpose()
        })
        .transpose()?;
    Ok(())
}

/// [`set_earliest_attributes_and_clefs`] on the first `<attributes>`.
fn earliest_attributes<T: XmlTree>(
    attributes: &mut T,
    sources: &[T],
    staves: usize,
    differences: StaffDifferences,
) -> Result<()> {
    let multi_key = differences.keys == Across::Different;
    let multi_meter = differences.meters == Across::Different;
    let number_first = |attributes: &mut T, tag: &str| -> Result<()> {
        if let Some((index, _)) = find_in(attributes, tag) {
            attributes
                .with_child(index, |found| found.set_attribute("number", "1"))
                .transpose()?;
        }
        Ok(())
    };
    number_first(attributes, "clef")?;
    let count = with_text_of(attributes, "staves", &staves.to_string())?;
    insert_before(attributes, count, &BEFORE_STAVES)?;
    if multi_key {
        number_first(attributes, "key")?;
    }
    if multi_meter {
        number_first(attributes, "time")?;
    }

    for (index, source) in sources.iter().enumerate() {
        let staff = index + 2;
        if let Some(old_clef) = first_in_attributes(source, "clef") {
            let clefs = elements_of(attributes)
                .into_iter()
                .filter(|(_, child)| tagged(child, "clef"))
                .count();
            if clefs >= staff {
                return Err(export_error("Attempted to add more clefs than staffs"));
            }
            let mut clef = attributes.element("clef")?;
            clef.set_attribute("number", &staff.to_string())?;
            let sign = find_in(&old_clef, "sign").and_then(|(_, sign)| sign.text());
            let mut written = attributes.element("sign")?;
            if let Some(sign) = sign {
                written.set_text(&sign)?;
            }
            clef.insert(0, written)?;
            let line = find_in(&old_clef, "line")
                .and_then(|(_, line)| line.text())
                .unwrap_or_default();
            clef.insert(1, with_text_of(attributes, "line", &line)?)?;
            if let Some((_, change)) = find_in(&old_clef, "clef-octave-change") {
                clef.insert(2, change.deep_copy()?)?;
            }
            insert_before(
                attributes,
                clef,
                &["staff-details", "transpose", "directive", "measure-style"],
            )?;
        }
        if multi_meter && let Some(old_meter) = first_in_attributes(source, "time") {
            let mut meter = old_meter.deep_copy()?;
            meter.set_attribute("number", &staff.to_string())?;
            insert_before(attributes, meter, &["staves"])?;
        }
        if multi_key && let Some(old_key) = first_in_attributes(source, "key") {
            let mut key = old_key.deep_copy()?;
            key.set_attribute("number", &staff.to_string())?;
            insert_before(attributes, key, &["time", "staves"])?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measure_numbers_order_by_number_then_suffix() {
        assert!(measure_number_comes_before("23", "24"));
        assert!(!measure_number_comes_before("23", "23"));
        assert!(measure_number_comes_before("23", "23a"));
        assert!(measure_number_comes_before("23a", "23b"));
        assert!(!measure_number_comes_before("23b", "23a"));
        assert!(measure_number_comes_before("23b", "24a"));
    }

    #[test]
    fn a_staff_tag_goes_before_beams_and_notations() {
        let mut part = Element::new("part");
        let measure = part.sub("measure");
        measure.set("number", "1");
        let note = measure.sub("note");
        note.sub_text("duration", "8");
        note.sub_text("beam", "begin");
        add_staff_tags_to_staves(std::slice::from_mut(&mut part)).unwrap();
        assert_eq!(
            part.dump(),
            "<part>\n  <measure number=\"1\">\n    <note>\n      <duration>8</duration>\n      \
             <staff>1</staff>\n      <beam>begin</beam>\n    </note>\n  </measure>\n</part>"
        );
        let error = add_staff_tags_to_staves(std::slice::from_mut(&mut part)).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("In part (), measure (1): Attempted to create a second <staff> tag"),
            "{error}"
        );
    }

    #[test]
    fn moving_a_measure_numbers_its_voices_on() {
        let mut measure = Element::new("measure");
        measure.sub("note");
        let mut other = Element::new("measure");
        other.sub("note");
        move_measure_contents(&measure, &mut other, 2).unwrap();
        assert_eq!(
            other.dump(),
            "<measure>\n  <note>\n    <voice>1</voice>\n  </note>\n  <note>\n    \
             <voice>2</voice>\n  </note>\n</measure>"
        );
    }
}
