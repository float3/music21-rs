//! A small element tree that writes XML exactly as Python's ElementTree
//! writes the tree music21 builds, so the two exporters' output can be
//! compared byte for byte.
//!
//! What music21's `helpers.dumpString` does is reproduced here: indent two
//! spaces a level, sort the attributes of any element carrying more than one,
//! write an element with neither text nor children as `<tag />`, and escape
//! text and attribute values the way ElementTree's serializer does.

use std::fmt::Write as _;

/// One node of the tree: an element or a comment.
#[derive(Clone, Debug)]
pub(crate) enum Node {
    Element(Element),
    Comment(String),
}

/// An element: a tag, its attributes in the order they were set, its text
/// and its children.
#[derive(Clone, Debug)]
pub(crate) struct Element {
    tag: &'static str,
    attributes: Vec<(&'static str, String)>,
    text: Option<String>,
    children: Vec<Node>,
}

impl Element {
    pub(crate) fn new(tag: &'static str) -> Self {
        Self {
            tag,
            attributes: Vec::new(),
            text: None,
            children: Vec::new(),
        }
    }

    /// An element holding text and nothing else.
    pub(crate) fn with_text(tag: &'static str, text: impl Into<String>) -> Self {
        let mut element = Self::new(tag);
        element.text = Some(text.into());
        element
    }

    /// Sets an attribute, replacing one of the same name where it stands.
    pub(crate) fn set(&mut self, name: &'static str, value: impl Into<String>) {
        let value = value.into();
        match self.attributes.iter_mut().find(|(key, _)| *key == name) {
            Some(slot) => slot.1 = value,
            None => self.attributes.push((name, value)),
        }
    }

    /// Takes an attribute away, where it stands.
    pub(crate) fn remove_attribute(&mut self, name: &str) {
        self.attributes.retain(|(key, _)| *key != name);
    }

    pub(crate) fn set_text(&mut self, text: impl Into<String>) {
        self.text = Some(text.into());
    }

    pub(crate) fn has_attributes(&self) -> bool {
        !self.attributes.is_empty()
    }

    pub(crate) fn push(&mut self, child: Element) {
        self.children.push(Node::Element(child));
    }

    pub(crate) fn push_comment(&mut self, comment: impl Into<String>) {
        self.children.push(Node::Comment(comment.into()));
    }

    /// Adds a child and hands it back to be filled in: ElementTree's
    /// `SubElement`.
    pub(crate) fn sub(&mut self, tag: &'static str) -> &mut Element {
        self.push(Element::new(tag));
        match self.children.last_mut() {
            Some(Node::Element(child)) => child,
            _ => unreachable!("an element was pushed just now"),
        }
    }

    /// Adds a child holding text.
    pub(crate) fn sub_text(&mut self, tag: &'static str, text: impl Into<String>) {
        self.push(Element::with_text(tag, text));
    }

    /// How many children, comments included: Python's `len(element)`.
    pub(crate) fn len(&self) -> usize {
        self.children.len()
    }

    pub(crate) fn tag(&self) -> &'static str {
        self.tag
    }

    /// An attribute's value, where it is set.
    pub(crate) fn get(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value.as_str())
    }

    pub(crate) fn text(&self) -> Option<&str> {
        self.text.as_deref()
    }

    pub(crate) fn children(&self) -> &[Node] {
        &self.children
    }

    pub(crate) fn children_mut(&mut self) -> &mut Vec<Node> {
        &mut self.children
    }

    /// The child elements, comments passed over.
    pub(crate) fn elements(&self) -> impl Iterator<Item = &Element> {
        self.children.iter().filter_map(|node| match node {
            Node::Element(element) => Some(element),
            Node::Comment(_) => None,
        })
    }

    /// The child elements, to be changed.
    pub(crate) fn elements_mut(&mut self) -> impl Iterator<Item = &mut Element> {
        self.children.iter_mut().filter_map(|node| match node {
            Node::Element(element) => Some(element),
            Node::Comment(_) => None,
        })
    }

    /// The first child of a tag: ElementTree's `find`.
    pub(crate) fn find(&self, tag: &str) -> Option<&Element> {
        self.elements().find(|element| element.tag == tag)
    }

    /// The same, to be changed.
    pub(crate) fn find_mut(&mut self, tag: &str) -> Option<&mut Element> {
        self.elements_mut().find(|element| element.tag == tag)
    }

    /// Inserts a child before the first child whose tag is among `tags`, or
    /// at the end if none is: music21's `helpers.insertBeforeElements`.
    pub(crate) fn insert_before(&mut self, child: Element, tags: &[&str]) {
        let at = self
            .children
            .iter()
            .position(|node| matches!(node, Node::Element(element) if tags.contains(&element.tag)))
            .unwrap_or(self.children.len());
        self.children.insert(at, Node::Element(child));
    }

    /// Writes the element as music21's `dumpString` does.
    pub(crate) fn dump(&self) -> String {
        let mut out = String::new();
        self.write(&mut out, 0, true);
        out.truncate(out.trim_end().len());
        out
    }

    /// Writes this element followed by the tail music21's `indent` gives it.
    fn write(&self, out: &mut String, level: usize, last: bool) {
        out.push('<');
        out.push_str(self.tag);
        let mut attributes: Vec<&(&'static str, String)> = self.attributes.iter().collect();
        if attributes.len() > 1 {
            attributes.sort_by(|left, right| left.0.cmp(right.0));
        }
        for (name, value) in attributes {
            let _ = write!(out, " {name}=\"{}\"", escape_attribute(value));
        }
        if self.children.is_empty() {
            match &self.text {
                Some(text) if !text.is_empty() => {
                    out.push('>');
                    out.push_str(&escape_text(text));
                    let _ = write!(out, "</{}>", self.tag);
                }
                _ => out.push_str(" />"),
            }
        } else {
            out.push('>');
            // An element with children has any whitespace-only text replaced
            // by the indentation of its first child.
            match &self.text {
                Some(text) if !text.trim().is_empty() => out.push_str(&escape_text(text)),
                _ => indentation(out, level + 1),
            }
            let count = self.children.len();
            for (index, child) in self.children.iter().enumerate() {
                let last_child = index + 1 == count;
                match child {
                    Node::Element(element) => element.write(out, level + 1, last_child),
                    Node::Comment(comment) => {
                        let _ = write!(out, "<!--{comment}-->");
                        tail(out, level + 1, last_child);
                    }
                }
            }
            let _ = write!(out, "</{}>", self.tag);
        }
        if level > 0 {
            tail(out, level, last);
        }
    }
}

/// The whitespace after an element: the indentation of the next sibling, or
/// of the parent's closing tag after the last one.
fn tail(out: &mut String, level: usize, last: bool) {
    indentation(out, if last { level - 1 } else { level });
}

fn indentation(out: &mut String, level: usize) {
    out.push('\n');
    for _ in 0..level {
        out.push_str("  ");
    }
}

/// ElementTree's `_escape_cdata`.
fn escape_text(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            other => escaped.push(other),
        }
    }
    escaped
}

/// ElementTree's `_escape_attrib`.
fn escape_attribute(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\r' => escaped.push_str("&#13;"),
            '\n' => escaped.push_str("&#10;"),
            '\t' => escaped.push_str("&#09;"),
            other => escaped.push(other),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_element_closes_itself() {
        assert_eq!(Element::new("accidental").dump(), "<accidental />");
        assert_eq!(
            Element::with_text("accidental", "∆").dump(),
            "<accidental>∆</accidental>"
        );
    }

    #[test]
    fn children_are_indented_and_attributes_sorted() {
        let mut note = Element::new("note");
        note.set("dynamics", "70.56");
        note.set("color", "#C0C0C0");
        let pitch = note.sub("pitch");
        pitch.sub_text("step", "D");
        pitch.sub_text("octave", "5");
        note.sub("dot");
        assert_eq!(
            note.dump(),
            "<note color=\"#C0C0C0\" dynamics=\"70.56\">\n  <pitch>\n    <step>D</step>\n    \
             <octave>5</octave>\n  </pitch>\n  <dot />\n</note>"
        );
    }

    #[test]
    fn text_and_attributes_are_escaped_as_elementtree_escapes_them() {
        let mut words = Element::with_text("words", "a < b & \"c\"");
        words.set("name", "x\"y\n");
        assert_eq!(
            words.dump(),
            "<words name=\"x&quot;y&#10;\">a &lt; b &amp; \"c\"</words>"
        );
    }

    #[test]
    fn a_comment_is_indented_like_an_element() {
        let mut part = Element::new("part");
        part.push_comment("= Measure 1 =");
        part.sub("measure");
        assert_eq!(
            part.dump(),
            "<part>\n  <!--= Measure 1 =-->\n  <measure />\n</part>"
        );
    }
}
