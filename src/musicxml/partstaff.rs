//! Several staves written as one part: music21's `PartStaffExporterMixin`.
//!
//! A piano's two staves are two `PartStaff` streams in music21 and one
//! `<part>` in MusicXML. Each staff is written as a part of its own first;
//! then every note, direction and forward is told which staff it is on, the
//! later staves' measures are moved into the first's behind a `<backup>`,
//! and the first measure's `<attributes>` is given the number of staves and
//! a clef for each.

use super::tree::{Element, Node};
use crate::error::{Error, Result};

fn export_error(message: impl Into<String>) -> Error {
    Error::MusicXml(message.into())
}

/// music21's `addStaffTags`, over every measure of a part: a `<staff>` in
/// each note, direction, forward and harmony, before its beams, notations,
/// lyrics and sound.
pub(super) fn add_staff_tags(part: &mut Element, staff: usize) -> Result<()> {
    for measure in part
        .elements_mut()
        .filter(|element| element.tag() == "measure")
    {
        let number = measure.get("number").unwrap_or_default().to_string();
        for child in measure.elements_mut() {
            if !matches!(child.tag(), "note" | "direction" | "forward" | "harmony") {
                continue;
            }
            if child.find("staff").is_some() {
                return Err(export_error(format!(
                    "In measure ({number}): Attempted to create a second <staff> tag"
                )));
            }
            child.insert_before(
                Element::with_text("staff", staff.to_string()),
                &["beam", "notations", "lyric", "play", "sound"],
            );
        }
    }
    Ok(())
}

/// music21's `movePartStaffMeasureContents`, `setEarliestAttributesAndClefs`
/// and the staff tags before them: the staves of `parts`, first to last,
/// joined into the first.
pub(super) fn join(parts: &mut [Element], multi_key: bool, multi_meter: bool) -> Result<()> {
    for (index, part) in parts.iter_mut().enumerate() {
        add_staff_tags(part, index + 1)?;
    }
    let Some((target, sources)) = parts.split_first_mut() else {
        return Ok(());
    };
    for (index, source) in sources.iter().enumerate() {
        let staff = index + 2;
        let insertions = process_subsequent_part_staff(target, source, staff)?;
        let mut inserted = 0;
        for (at, nodes) in insertions {
            for node in nodes {
                target.children_mut().insert(at + inserted, node);
                inserted += 1;
            }
        }
    }
    set_earliest_attributes_and_clefs(target, sources, multi_key, multi_meter)
}

/// music21's `measureNumberComesBefore`: whether one measure number comes
/// strictly before another, suffixes and all.
fn measure_number_comes_before(first: &str, second: &str) -> bool {
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
/// `target` has no number for gathered to be inserted where they fall.
fn process_subsequent_part_staff(
    target: &mut Element,
    source: &Element,
    staff: usize,
) -> Result<Vec<(usize, Vec<Node>)>> {
    let divider = |number: &str| {
        Node::Comment(format!(
            "========================= Measure {number} =========================="
        ))
    };
    fn add(insertions: &mut Vec<(usize, Vec<Node>)>, at: usize, nodes: Vec<Node>) {
        match insertions.iter_mut().find(|(index, _)| *index == at) {
            Some((_, existing)) => existing.extend(nodes),
            None => insertions.push((at, nodes)),
        }
    }
    let mut source_measures = source
        .elements()
        .filter(|element| element.tag() == "measure")
        .peekable();
    let mut insertions: Vec<(usize, Vec<Node>)> = Vec::new();
    let count = target.children().len();
    for index in 0..count {
        let Node::Element(target_measure) = &mut target.children_mut()[index] else {
            continue;
        };
        if target_measure.tag() != "measure" {
            continue;
        }
        let Some(source_measure) = source_measures.peek().copied() else {
            return Ok(insertions);
        };
        let target_number = target_measure.get("number").map(str::to_string);
        let source_number = source_measure.get("number").map(str::to_string);
        if target_number == source_number {
            move_measure_contents(source_measure, target_measure, staff)?;
            source_measures.next();
            continue;
        }
        let (Some(target_number), Some(source_number)) = (target_number, source_number) else {
            return Err(export_error(
                "joinPartStaffs() was unable to order the measures",
            ));
        };
        if measure_number_comes_before(&target_number, &source_number) {
            continue;
        }
        if !measure_number_comes_before(&source_number, &target_number) {
            return Err(export_error(format!(
                "joinPartStaffs() was unable to order the measures {target_number}, \
                 {source_number}"
            )));
        }
        // music21 reads the number once and never again, so once the first
        // staff is missing a measure every measure the later staff has left
        // goes in here, each under the first one's number.
        for measure in source_measures {
            add(
                &mut insertions,
                index,
                vec![divider(&source_number), Node::Element(measure.clone())],
            );
        }
        return Ok(insertions);
    }
    let end = target.children().len();
    for remaining in source_measures {
        let Some(number) = remaining.get("number") else {
            continue;
        };
        add(
            &mut insertions,
            end,
            vec![divider(number), Node::Element(remaining.clone())],
        );
    }
    Ok(insertions)
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

/// music21's `moveMeasureContents`: the contents of `measure` moved into
/// `other` behind a `<backup>` to the start, with voices numbered on from
/// `other`'s, mid-measure clefs told their staff, and a barline replacing
/// the one at the same end.
fn move_measure_contents(measure: &Element, other: &mut Element, staff: usize) -> Result<()> {
    let mut max_voices: i64 = 0;
    for child in other.elements() {
        for voice in child.elements().filter(|element| element.tag() == "voice") {
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
        for note in other
            .elements_mut()
            .filter(|element| element.tag() == "note")
        {
            note.insert_before(Element::with_text("voice", "1"), &BEFORE_VOICE);
        }
        max_voices = 1;
    }

    let duration_of = |element: &Element| -> i64 {
        element
            .find("duration")
            .and_then(Element::text)
            .and_then(|text| text.parse().ok())
            .unwrap_or(0)
    };
    let mut backup: i64 = 0;
    for child in other.elements() {
        match child.tag() {
            "note" if child.find("chord").is_none() => backup += duration_of(child),
            "forward" => backup += duration_of(child),
            "backup" => backup -= duration_of(child),
            _ => {}
        }
    }
    if backup != 0 {
        other.sub("backup").sub_text("duration", backup.to_string());
    }

    for element in measure.elements() {
        let mut element = element.clone();
        match element.tag() {
            "print" => continue,
            "attributes" => {
                if element.find("divisions").is_some() {
                    continue;
                }
                for clef in element.elements_mut().filter(|child| child.tag() == "clef") {
                    clef.set("number", staff.to_string());
                }
            }
            "barline" => {
                let location = element.get("location").map(str::to_string);
                other.children_mut().retain(|node| {
                    !matches!(node, Node::Element(existing)
                        if existing.tag() == "barline"
                            && existing.get("location").map(str::to_string) == location)
                });
            }
            "note" => match element.find_mut("voice") {
                Some(voice) => {
                    if lacked_voice && let Some(text) = voice.text() {
                        let number: i64 = text.parse().unwrap_or(0);
                        voice.set_text((number + 1).to_string());
                    }
                }
                None => element.insert_before(
                    Element::with_text("voice", (max_voices + 1).to_string()),
                    &BEFORE_VOICE,
                ),
            },
            _ => {}
        }
        other.push(element);
    }
    Ok(())
}

/// The first `<attributes>` of any measure of a part, where ElementTree's
/// `find('measure/attributes')` finds it.
fn first_attributes(part: &mut Element) -> Option<&mut Element> {
    part.elements_mut()
        .filter(|element| element.tag() == "measure")
        .find_map(|measure| measure.find_mut("attributes"))
}

/// The first child of a tag inside the first `<attributes>` that has one:
/// `find('measure/attributes/<tag>')`.
fn first_in_attributes<'a>(part: &'a Element, tag: &str) -> Option<&'a Element> {
    part.elements()
        .filter(|element| element.tag() == "measure")
        .flat_map(|measure| {
            measure
                .elements()
                .filter(|child| child.tag() == "attributes")
        })
        .find_map(|attributes| attributes.find(tag))
}

/// music21's `setEarliestAttributesAndClefsPartStaff`.
fn set_earliest_attributes_and_clefs(
    target: &mut Element,
    sources: &[Element],
    multi_key: bool,
    multi_meter: bool,
) -> Result<()> {
    let staves = sources.len() + 1;
    let Some(attributes) = first_attributes(target) else {
        return Ok(());
    };
    if let Some(clef) = attributes.find_mut("clef") {
        clef.set("number", "1");
    }
    attributes.insert_before(
        Element::with_text("staves", staves.to_string()),
        &[
            "part-symbol",
            "instruments",
            "clef",
            "staff-details",
            "transpose",
            "directive",
            "measure-style",
        ],
    );
    if multi_key && let Some(key) = attributes.find_mut("key") {
        key.set("number", "1");
    }
    if multi_meter && let Some(time) = attributes.find_mut("time") {
        time.set("number", "1");
    }

    for (index, source) in sources.iter().enumerate() {
        let staff = index + 2;
        if let Some(old_clef) = first_in_attributes(source, "clef") {
            let clefs = attributes
                .elements()
                .filter(|element| element.tag() == "clef")
                .count();
            if clefs >= staff {
                return Err(export_error("Attempted to add more clefs than staffs"));
            }
            let mut clef = Element::new("clef");
            clef.set("number", staff.to_string());
            match old_clef.find("sign").and_then(Element::text) {
                Some(sign) => clef.sub_text("sign", sign),
                None => {
                    clef.sub("sign");
                }
            }
            clef.sub_text(
                "line",
                old_clef.find("line").and_then(Element::text).unwrap_or(""),
            );
            if let Some(change) = old_clef.find("clef-octave-change") {
                clef.push(change.clone());
            }
            attributes.insert_before(
                clef,
                &["staff-details", "transpose", "directive", "measure-style"],
            );
        }
        if multi_meter && let Some(old_meter) = first_in_attributes(source, "time") {
            let mut meter = old_meter.clone();
            meter.set("number", staff.to_string());
            attributes.insert_before(meter, &["staves"]);
        }
        if multi_key && let Some(old_key) = first_in_attributes(source, "key") {
            let mut key = old_key.clone();
            key.set("number", staff.to_string());
            attributes.insert_before(key, &["time", "staves"]);
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
        add_staff_tags(&mut part, 2).unwrap();
        assert_eq!(
            part.dump(),
            "<part>\n  <measure number=\"1\">\n    <note>\n      <duration>8</duration>\n      \
             <staff>2</staff>\n      <beam>begin</beam>\n    </note>\n  </measure>\n</part>"
        );
        assert!(add_staff_tags(&mut part, 2).is_err());
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
