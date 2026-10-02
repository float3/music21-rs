//! A small XML reader: enough of the format to read a MusicXML or an MEI
//! document into a tree shaped like the one Python's ElementTree hands
//! music21.
//!
//! Each element keeps its tag, its attributes, its children, and the text
//! standing before its first child, which is what ElementTree calls `.text`
//! and the only text a MusicXML reader asks for. The prolog, the doctype,
//! comments and processing instructions are passed over.

use crate::error::{Error, Result};

/// One element of a parsed document.
#[derive(Clone, Debug, Default)]
pub(crate) struct Xml {
    pub(crate) tag: String,
    attributes: Vec<(String, String)>,
    /// The text before the first child; `None` where there is none at all.
    text: Option<String>,
    /// The text after this element's end tag, up to its next sibling or its
    /// parent's end: ElementTree's `.tail`.
    tail: Option<String>,
    pub(crate) children: Vec<Xml>,
}

impl Xml {
    /// An attribute's value, where it is set.
    pub(crate) fn get(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    /// The text before the first child, as written.
    pub(crate) fn text(&self) -> Option<&str> {
        self.text.as_deref()
    }

    /// Every piece of text inside the element, its children's included, in
    /// document order: ElementTree's `itertext`.
    pub(crate) fn all_text(&self) -> String {
        let mut out = String::new();
        self.collect_text(&mut out);
        out
    }

    fn collect_text(&self, out: &mut String) {
        if let Some(text) = &self.text {
            out.push_str(text);
        }
        for child in &self.children {
            child.collect_text(out);
            if let Some(tail) = &child.tail {
                out.push_str(tail);
            }
        }
    }

    /// The text after this element's end tag, as written.
    pub(crate) fn tail(&self) -> Option<&str> {
        self.tail.as_deref()
    }

    /// The text with the space around it taken off, empty where there is
    /// none: music21's `strippedText`.
    pub(crate) fn stripped(&self) -> &str {
        self.text.as_deref().map_or("", str::trim)
    }

    /// The first child of a tag: ElementTree's `find`.
    pub(crate) fn find(&self, tag: &str) -> Option<&Xml> {
        self.children.iter().find(|child| child.tag == tag)
    }

    /// Every child of a tag, in order: ElementTree's `findall`.
    pub(crate) fn find_all<'a>(&'a self, tag: &'a str) -> impl Iterator<Item = &'a Xml> {
        self.children.iter().filter(move |child| child.tag == tag)
    }

    /// The stripped text of the first child of a tag, empty where the child
    /// or its text is missing.
    pub(crate) fn child_text(&self, tag: &str) -> &str {
        self.find(tag).map_or("", Xml::stripped)
    }

    /// Reads a document and hands back its root element.
    pub(crate) fn parse(document: &str) -> Result<Xml> {
        // A line ends in a line feed whatever the file wrote, as the XML
        // standard has a reader take it.
        let normalized;
        let document = if document.contains('\r') {
            normalized = document.replace("\r\n", "\n").replace('\r', "\n");
            normalized.as_str()
        } else {
            document
        };
        let mut reader = Reader {
            text: document,
            at: 0,
        };
        reader.skip_misc()?;
        let root = reader.element()?;
        Ok(root)
    }
}

struct Reader<'a> {
    text: &'a str,
    at: usize,
}

fn malformed(what: impl std::fmt::Display) -> Error {
    Error::Xml(format!("the document is not well-formed XML: {what}"))
}

impl Reader<'_> {
    fn rest(&self) -> &str {
        &self.text[self.at..]
    }

    /// Skips to just past the next occurrence of `end`.
    fn skip_past(&mut self, end: &str) -> Result<()> {
        match self.rest().find(end) {
            Some(found) => {
                self.at += found + end.len();
                Ok(())
            }
            None => Err(malformed(format!("no `{end}` closes what was opened"))),
        }
    }

    /// Skips a doctype, whose internal subset may hold `>` of its own.
    fn skip_doctype(&mut self) -> Result<()> {
        let mut depth = 0usize;
        for (index, character) in self.rest().char_indices() {
            match character {
                '[' => depth += 1,
                ']' => depth = depth.saturating_sub(1),
                '>' if depth == 0 => {
                    self.at += index + 1;
                    return Ok(());
                }
                _ => {}
            }
        }
        Err(malformed("the doctype is not closed"))
    }

    /// Skips whitespace, comments, processing instructions and the doctype,
    /// which may stand before and after the root element.
    fn skip_misc(&mut self) -> Result<()> {
        loop {
            self.skip_space();
            let trimmed = self.rest();
            if trimmed.starts_with("<?") {
                self.skip_past("?>")?;
            } else if trimmed.starts_with("<!--") {
                self.skip_past("-->")?;
            } else if trimmed.starts_with("<!DOCTYPE") {
                self.skip_doctype()?;
            } else {
                return Ok(());
            }
        }
    }

    fn name(&mut self) -> Result<String> {
        let rest = self.rest();
        let end = rest
            .find(|character: char| {
                character.is_whitespace() || matches!(character, '>' | '/' | '=')
            })
            .unwrap_or(rest.len());
        if end == 0 {
            return Err(malformed("a name was expected"));
        }
        let name = rest[..end].to_string();
        self.at += end;
        Ok(name)
    }

    fn skip_space(&mut self) {
        let remaining = self.rest().trim_start().len();
        self.at = self.text.len() - remaining;
    }

    /// One element, the reader standing on its `<`.
    fn element(&mut self) -> Result<Xml> {
        if !self.rest().starts_with('<') {
            return Err(malformed("an element was expected"));
        }
        self.at += 1;
        let mut element = Xml {
            tag: self.name()?,
            ..Xml::default()
        };
        loop {
            self.skip_space();
            let rest = self.rest();
            if rest.starts_with("/>") {
                self.at += 2;
                return Ok(element);
            }
            if rest.starts_with('>') {
                self.at += 1;
                break;
            }
            let name = self.name()?;
            self.skip_space();
            if !self.rest().starts_with('=') {
                return Err(malformed(format!("the attribute {name} has no value")));
            }
            self.at += 1;
            self.skip_space();
            let quote = self
                .rest()
                .chars()
                .next()
                .filter(|quote| matches!(quote, '"' | '\''))
                .ok_or_else(|| malformed(format!("the attribute {name} is not quoted")))?;
            self.at += 1;
            let end = self
                .rest()
                .find(quote)
                .ok_or_else(|| malformed(format!("the attribute {name} is not closed")))?;
            let value = unescape(&self.rest()[..end]);
            self.at += end + 1;
            element.attributes.push((name, value));
        }

        // Content: text, children, comments, until the closing tag.
        // The text before the first child, and then each child's tail.
        let mut text = String::new();
        let close = |element: &mut Xml, text: String| {
            if text.is_empty() {
                return;
            }
            match element.children.last_mut() {
                Some(last) => last.tail = Some(text),
                None => element.text = Some(text),
            }
        };
        loop {
            let rest = self.rest();
            let Some(open) = rest.find('<') else {
                return Err(malformed(format!("<{}> is not closed", element.tag)));
            };
            text.push_str(&unescape(&rest[..open]));
            self.at += open;
            let rest = self.rest();
            if rest.starts_with("</") {
                self.skip_past(">")?;
                close(&mut element, text);
                return Ok(element);
            } else if rest.starts_with("<!--") {
                self.skip_past("-->")?;
            } else if rest.starts_with("<![CDATA[") {
                self.at += "<![CDATA[".len();
                let end = self
                    .rest()
                    .find("]]>")
                    .ok_or_else(|| malformed("a CDATA section is not closed"))?;
                text.push_str(&self.rest()[..end]);
                self.at += end + 3;
            } else if rest.starts_with("<?") {
                self.skip_past("?>")?;
            } else {
                close(&mut element, std::mem::take(&mut text));
                element.children.push(self.element()?);
            }
        }
    }
}

/// Text with its character and entity references read.
fn unescape(text: &str) -> String {
    if !text.contains('&') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        rest = &rest[start..];
        let Some(end) = rest.find(';') else {
            break;
        };
        let reference = &rest[1..end];
        let read = match reference {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => reference
                .strip_prefix("#x")
                .or_else(|| reference.strip_prefix("#X"))
                .map(|hex| u32::from_str_radix(hex, 16))
                .or_else(|| reference.strip_prefix('#').map(str::parse::<u32>))
                .and_then(|code| code.ok())
                .and_then(char::from_u32),
        };
        match read {
            Some(character) => {
                out.push(character);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_document_is_read_into_a_tree() {
        let root = Xml::parse(
            "<?xml version='1.0'?>\n<!DOCTYPE a PUBLIC \"x\" \"y\">\n<!-- hi -->\n\
             <a version=\"4.0\"><b n='1'>one &amp; two</b><c/><!-- no --><b> 2 </b></a>\n",
        )
        .unwrap();
        assert_eq!(root.tag, "a");
        assert_eq!(root.get("version"), Some("4.0"));
        assert_eq!(root.children.len(), 3);
        assert_eq!(root.find("b").unwrap().text(), Some("one & two"));
        assert_eq!(root.find("b").unwrap().get("n"), Some("1"));
        assert!(root.find("c").unwrap().text().is_none());
        assert_eq!(root.find_all("b").nth(1).unwrap().stripped(), "2");
        assert_eq!(root.child_text("missing"), "");
    }

    #[test]
    fn references_and_cdata_are_read() {
        let root = Xml::parse("<a><b>&#233;&#xE9;&lt;</b><c><![CDATA[<raw>]]></c></a>").unwrap();
        assert_eq!(root.child_text("b"), "\u{e9}\u{e9}<");
        assert_eq!(root.child_text("c"), "<raw>");
    }

    #[test]
    fn text_between_children_is_kept_as_their_tails() {
        let root = Xml::parse("<a>one<b/>two<c>three</c>four</a>").unwrap();
        assert_eq!(root.text(), Some("one"));
        assert_eq!(root.find("b").unwrap().tail(), Some("two"));
        assert_eq!(root.find("c").unwrap().tail(), Some("four"));
        assert_eq!(root.all_text(), "onetwothreefour");
    }

    #[test]
    fn a_line_ends_in_a_line_feed_however_it_was_written() {
        let root = Xml::parse("<a>one\r\ntwo\rthree</a>").unwrap();
        assert_eq!(root.text(), Some("one\ntwo\nthree"));
    }

    #[test]
    fn a_document_that_does_not_close_is_refused() {
        assert!(Xml::parse("<a><b></a").is_err());
        assert!(Xml::parse("not xml").is_err());
    }
}
