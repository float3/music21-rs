//! MusicXML, partwise, plain or compressed (`.mxl`), read by the crate's own
//! reader and turned into what the editor writes ABC from by
//! [`super::stream`].
//!
//! Three things the editor wants are no part of the score music21 models,
//! and are read off the document beside it: a tablature staff's tuning, a
//! rehearsal mark, and whether a part plays on the drum channel.

use super::{Imported, decode_text, stream::Extras, zip::Archive};
use music21_rs::musicxml::from_musicxml;
use roxmltree::{Document, Node, ParsingOptions};
pub(super) fn read_compressed(archive: &Archive<'_>) -> Result<Imported, String> {
    // The container names the score; failing that, the first XML file in the
    // archive is it.
    let named = match archive.read("META-INF/container.xml")? {
        Some(container) => {
            let text = decode_text(&container);
            let options = ParsingOptions {
                allow_dtd: true,
                ..ParsingOptions::default()
            };
            let document =
                Document::parse_with_options(&text, options).map_err(|err| err.to_string())?;
            document
                .descendants()
                .find(|node| node.has_tag_name("rootfile"))
                .and_then(|node| node.attribute("full-path"))
                .map(str::to_string)
        }
        None => None,
    };
    let name = named
        .or_else(|| {
            archive
                .names()
                .find(|name| {
                    !name.starts_with("META-INF")
                        && (name.ends_with(".xml") || name.ends_with(".musicxml"))
                })
                .map(str::to_string)
        })
        .ok_or("the archive holds no score")?;
    let score = archive.read(&name)?.ok_or("the archive holds no score")?;
    read(&decode_text(&score))
}

fn child<'a, 'input>(node: Node<'a, 'input>, name: &str) -> Option<Node<'a, 'input>> {
    node.children().find(|child| child.has_tag_name(name))
}

fn text_at<'a>(node: Node<'a, '_>, names: &[&str]) -> Option<&'a str> {
    let node = names
        .iter()
        .try_fold(node, |node, name| child(node, name))?;
    node.text().map(str::trim).filter(|text| !text.is_empty())
}

fn number<T: std::str::FromStr>(node: Node<'_, '_>, names: &[&str]) -> Option<T> {
    text_at(node, names)?.parse().ok()
}

fn step_class(step: &str) -> Option<i32> {
    Some(match step {
        "C" => 0,
        "D" => 2,
        "E" => 4,
        "F" => 5,
        "G" => 7,
        "A" => 9,
        "B" => 11,
        _ => return None,
    })
}

fn extras(text: &str) -> Extras {
    let mut extras = Extras::default();
    let options = ParsingOptions {
        allow_dtd: true,
        ..ParsingOptions::default()
    };
    let Ok(document) = Document::parse_with_options(text, options) else {
        return extras;
    };
    let root = document.root_element();
    for part in root
        .descendants()
        .filter(|node| node.has_tag_name("score-part"))
    {
        if let Some(id) = part.attribute("id")
            && number::<u8>(part, &["midi-instrument", "midi-channel"]) == Some(10)
        {
            extras.drums.insert(id.to_string());
        }
    }
    for part in root.children().filter(|node| node.has_tag_name("part")) {
        let Some(id) = part.attribute("id") else {
            continue;
        };
        let measures = part.children().filter(|node| node.has_tag_name("measure"));
        for (index, measure) in measures.enumerate() {
            for details in measure
                .descendants()
                .filter(|node| node.has_tag_name("staff-details"))
            {
                let staff = details
                    .attribute("number")
                    .and_then(|n| n.parse::<usize>().ok())
                    .unwrap_or(1);
                let mut strings: Vec<(i32, i32)> = details
                    .children()
                    .filter(|node| node.has_tag_name("staff-tuning"))
                    .filter_map(|tuning| {
                        let line = tuning.attribute("line")?.parse().ok()?;
                        let class = step_class(text_at(tuning, &["tuning-step"])?)?;
                        let alter = number::<f64>(tuning, &["tuning-alter"])
                            .unwrap_or(0.0)
                            .round() as i32;
                        let octave: i32 = number(tuning, &["tuning-octave"])?;
                        Some((line, (octave + 1) * 12 + class + alter))
                    })
                    .collect();
                strings.sort();
                if !strings.is_empty() {
                    extras.tuning.insert(
                        (id.to_string(), staff.max(1)),
                        strings.into_iter().map(|(_, midi)| midi).collect(),
                    );
                }
            }
            if let Some(mark) = measure
                .descendants()
                .find(|node| node.has_tag_name("rehearsal"))
                .and_then(|node| node.text())
                .map(str::trim)
                .filter(|text| !text.is_empty())
            {
                extras
                    .markers
                    .insert((id.to_string(), index), mark.to_string());
            }
        }
    }
    extras
}

pub(super) fn read(text: &str) -> Result<Imported, String> {
    let score = from_musicxml(text).map_err(|err| format!("the MusicXML does not read: {err}"))?;
    super::stream::read(&score, &extras(text))
}
