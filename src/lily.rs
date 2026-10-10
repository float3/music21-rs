//! LilyPond, written as music21's `lily.translate` writes it.
//!
//! music21 builds a tree of LilyPond objects (`lily.lilyObjects`) and writes
//! it out, each object indented by how many objects stand above it -- which
//! depends on the order the tree was put together in, since an object takes
//! the parent it was last given as an attribute, or the first list it was
//! put in, and none when it is appended to a list afterwards. The tree is
//! kept here as music21 builds it, parents and all, so the text comes out
//! as music21's does, space for space.
//!
//! ```
//! use music21_rs::lily::{LilyOptions, to_lilypond};
//! use music21_rs::tinynotation::from_tiny_notation;
//!
//! let line = from_tiny_notation("4/4 c4 d8 e f#4. g8")?;
//! let text = to_lilypond(&line, &LilyOptions::default())?;
//! assert!(text.starts_with("\\version \"2.24\""));
//! assert!(text.contains("fis' 4."));
//! # Ok::<(), music21_rs::Error>(())
//! ```

use crate::bar::BarlineType;
use crate::chord::Chord;
use crate::clef::Clef;
use crate::defaults::FloatType;
use crate::duration::Duration;
use crate::error::{Error, Result};
use crate::expressions::Expression;
use crate::key::Key;
use crate::metadata::Metadata;
use crate::meter::TimeSignature;
use crate::notation::{Beams, StemDirection, Syllabic, TieType};
use crate::note::Note;
use crate::pitch::Pitch;
use crate::rest::Rest;
use crate::stream::{Stream, StreamElement, StreamKind};
use crate::tempo::MetronomeMark;

/// How a LilyPond file is written.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LilyOptions {
    /// The LilyPond version the file says it is for, major and minor:
    /// music21 asks the LilyPond it finds installed.
    pub version: (u32, u32),
}

impl Default for LilyOptions {
    fn default() -> Self {
        Self { version: (2, 24) }
    }
}

fn lily_error(message: impl Into<String>) -> Error {
    Error::Notation(message.into())
}

// ------------------------------------------------------------------ tree

/// A child of a LilyPond object: another object, or text as it stands.
#[derive(Clone, Debug)]
enum Item {
    Node(usize),
    Text(String),
}

/// The kinds of music21's LilyPond objects this writer makes.
#[derive(Clone, Debug)]
enum Kind {
    Top {
        contents: Vec<Item>,
    },
    Header {
        body: Option<usize>,
    },
    HeaderBody {
        assignments: Vec<Item>,
    },
    Assignment {
        id: String,
        value: String,
    },
    Embedded(String),
    ScoreBlock {
        body: usize,
    },
    ScoreBody {
        music: usize,
    },
    OutputDef,
    Layout,
    MusicList {
        contents: Vec<Item>,
    },
    Music {
        composite: usize,
    },
    Sequential {
        list: usize,
        start_staff: bool,
    },
    Simultaneous {
        list: usize,
    },
    SimpleMusic {
        chord: usize,
    },
    ContextModification {
        list: Vec<String>,
    },
    Composite {
        prefix: Option<usize>,
        grouped: Option<usize>,
        lyrics: Option<usize>,
    },
    Grouped {
        simultaneous: Option<usize>,
        sequential: Option<usize>,
    },
    OptionalId(String),
    Prefix {
        kind: &'static str,
        context: String,
        id: Option<usize>,
        modification: Option<usize>,
        music: usize,
        fraction: String,
    },
    NewLyrics {
        lists: Vec<usize>,
    },
    PropertySet {
        property: String,
        value: String,
    },
    EventChord {
        simple: Option<usize>,
        post: Vec<String>,
        note_chord: Option<usize>,
    },
    NoteChordElement {
        body: usize,
        duration: usize,
        post: Vec<String>,
    },
    ChordBody {
        elements: Vec<usize>,
    },
    ChordBodyElement {
        parts: Vec<Item>,
    },
    Pitch {
        name: String,
        quotes: String,
    },
    Duration {
        number: String,
        dots: u32,
    },
    SimpleElement {
        parts: Vec<Item>,
    },
    LyricElement(String),
}

#[derive(Clone, Debug)]
struct Node {
    parent: Option<usize>,
    kind: Kind,
}

#[derive(Default)]
struct Tree {
    nodes: Vec<Node>,
}

impl Tree {
    /// A new object, its object attributes taking it as their parent and
    /// those in its lists taking it where they have none yet: music21's
    /// `__setattr__` as `__init__` sets each attribute.
    fn add(&mut self, kind: Kind) -> usize {
        let index = self.nodes.len();
        let (attributes, listed) = children(&kind);
        self.nodes.push(Node { parent: None, kind });
        for child in attributes {
            self.nodes[child].parent = Some(index);
        }
        for child in listed {
            self.adopt(index, child);
        }
        index
    }

    /// An attribute set to an object: it takes this parent, whatever it had.
    fn set_parent(&mut self, child: usize, parent: usize) {
        self.nodes[child].parent = Some(parent);
    }

    /// A list set with an object in it: it takes this parent only where it
    /// has none.
    fn adopt(&mut self, parent: usize, child: usize) {
        if self.nodes[child].parent.is_none() {
            self.nodes[child].parent = Some(parent);
        }
    }

    fn contents_mut(&mut self, node: usize) -> Result<&mut Vec<Item>> {
        match &mut self.nodes[node].kind {
            Kind::MusicList { contents } | Kind::Top { contents } => Ok(contents),
            _ => Err(lily_error(
                "Cannot get a currentMusicList from contextObject",
            )),
        }
    }

    /// `newlineIndent`: a newline and a space for every object above.
    fn newline_indent(&self, node: usize) -> String {
        let mut depth = 0;
        let mut current = self.nodes[node].parent;
        while let Some(parent) = current {
            depth += 1;
            current = self.nodes[parent].parent;
        }
        format!("\n{}", " ".repeat(depth))
    }

    /// `str()`: the object written out, a blank line closed up.
    fn str_of(&self, node: usize) -> String {
        self.string_output(node).replace("\n\n", "\n")
    }

    fn item(&self, item: &Item) -> String {
        match item {
            Item::Node(node) => self.str_of(*node),
            Item::Text(text) => text.clone(),
        }
    }

    /// `newlineSeparateStringOutputIfNotNone`.
    fn newline_separated(&self, node: usize, items: &[Item]) -> String {
        let indent = self.newline_indent(node);
        items.iter().map(|item| self.item(item) + &indent).collect()
    }

    /// `encloseCurly` of one object.
    fn enclose(&self, node: usize, inner: Option<String>) -> String {
        let Some(inner) = inner else {
            return " { } ".to_string();
        };
        let indent = self.newline_indent(node);
        format!(" {{ {indent}{inner}{indent} }} {indent}")
    }

    fn string_output(&self, node: usize) -> String {
        match &self.nodes[node].kind {
            Kind::Top { contents } => self.newline_separated(node, contents),
            Kind::Header { body } => {
                format!(
                    "\\header{}",
                    self.enclose(node, body.map(|body| self.str_of(body)))
                )
            }
            Kind::HeaderBody { assignments } => self.newline_separated(node, assignments),
            Kind::Assignment { id, value } => [id.as_str(), "=", &quote(value), " "].join(" "),
            Kind::Embedded(content) => content.clone(),
            Kind::ScoreBlock { body } => {
                format!("\\score {}", self.enclose(node, Some(self.str_of(*body))))
            }
            Kind::ScoreBody { music } | Kind::Music { composite: music } => {
                self.string_output(*music)
            }
            Kind::OutputDef => "\\paper { }".to_string(),
            Kind::Layout => {
                let lines = [
                    "\\layout {",
                    " \\context {",
                    "   \\RemoveEmptyStaves",
                    "   \\override VerticalAxisGroup.remove-first = ##t",
                    " }",
                    "}",
                ];
                let indent = self.newline_indent(node);
                lines.iter().map(|line| format!("{line}{indent}")).collect()
            }
            Kind::MusicList { contents } => self.newline_separated(node, contents),
            Kind::Sequential { list, start_staff } => format!(
                "{{ {}{} }} {}",
                if *start_staff { "\\startStaff " } else { "" },
                self.string_output(*list),
                self.newline_indent(node)
            ),
            Kind::Simultaneous { list } => {
                let indent = self.newline_indent(node);
                format!("{indent}<< {} >>{indent}", self.string_output(*list))
            }
            Kind::SimpleMusic { chord } => self.string_output(*chord),
            Kind::ContextModification { list } => {
                let indent = self.newline_indent(node);
                format!(
                    "\\with  {{ {indent}{}{indent} }} {indent}",
                    list.join(&indent)
                )
            }
            Kind::Composite {
                prefix,
                grouped,
                lyrics,
            } => {
                let lyrics = lyrics.map(|lyrics| self.str_of(lyrics)).unwrap_or_default();
                let music = prefix.or(*grouped).expect("composite music holds music");
                format!("{}\n{lyrics}", self.str_of(music))
            }
            Kind::Grouped {
                simultaneous,
                sequential,
            } => self.str_of(
                simultaneous
                    .or(*sequential)
                    .expect("grouped music holds music"),
            ),
            Kind::OptionalId(content) => format!(" = {content}"),
            Kind::Prefix {
                kind,
                context,
                id,
                modification,
                music,
                fraction,
            } => {
                if *kind == "tuplet" {
                    return format!("\\tuplet {fraction} {} ", self.str_of(*music));
                }
                let mut out = format!("\\{kind} {context} ");
                if let Some(id) = id {
                    out += &(self.str_of(*id) + " ");
                }
                if let Some(modification) = modification {
                    out += &(self.str_of(*modification) + " ");
                }
                out + &self.str_of(*music) + " "
            }
            Kind::NewLyrics { lists } => lists
                .iter()
                .map(|list| format!("\\addlyrics {}", self.string_output(*list)))
                .collect(),
            Kind::PropertySet { property, value } => format!("\\set {property} = {value} "),
            Kind::EventChord {
                simple,
                post,
                note_chord,
            } => {
                if let Some(note_chord) = note_chord {
                    return self.str_of(*note_chord) + " ";
                }
                let mut out = simple.map(|simple| self.str_of(simple)).unwrap_or_default();
                for event in post {
                    out += &format!(" {event}");
                }
                out + " "
            }
            Kind::NoteChordElement {
                body,
                duration,
                post,
            } => {
                let mut out = self.str_of(*body);
                out += &(self.str_of(*duration) + " ");
                for event in post {
                    out += &format!("{event} ");
                }
                out
            }
            Kind::ChordBody { elements } => {
                let inner: Vec<String> = elements
                    .iter()
                    .map(|element| self.str_of(*element))
                    .collect();
                ["<", &inner.join(" "), "> "].join(" ")
            }
            Kind::ChordBodyElement { parts } => parts
                .iter()
                .map(|part| self.item(part))
                .collect::<Vec<_>>()
                .join(" "),
            Kind::Pitch { name, quotes } => format!("{name}{quotes} "),
            Kind::Duration { number, dots } => {
                format!("{number}{} ", ".".repeat(*dots as usize))
            }
            Kind::SimpleElement { parts } => parts.iter().map(|part| self.item(part)).collect(),
            Kind::LyricElement(text) => format!("{text} "),
        }
    }
}

/// The objects a kind holds as attributes, and those it holds in lists.
fn children(kind: &Kind) -> (Vec<usize>, Vec<usize>) {
    let nodes = |items: &[Item]| -> Vec<usize> {
        items
            .iter()
            .filter_map(|item| match item {
                Item::Node(node) => Some(*node),
                Item::Text(_) => None,
            })
            .collect()
    };
    match kind {
        Kind::Top { contents } | Kind::MusicList { contents } => (Vec::new(), nodes(contents)),
        Kind::HeaderBody { assignments } => (Vec::new(), nodes(assignments)),
        Kind::Header { body } => (body.iter().copied().collect(), Vec::new()),
        Kind::ScoreBlock { body } => (vec![*body], Vec::new()),
        Kind::ScoreBody { music } | Kind::Music { composite: music } => (vec![*music], Vec::new()),
        Kind::Sequential { list, .. } | Kind::Simultaneous { list } => (vec![*list], Vec::new()),
        Kind::SimpleMusic { chord } => (vec![*chord], Vec::new()),
        Kind::Composite {
            prefix,
            grouped,
            lyrics,
        } => (
            [*prefix, *grouped, *lyrics].into_iter().flatten().collect(),
            Vec::new(),
        ),
        Kind::Grouped {
            simultaneous,
            sequential,
        } => (
            [*simultaneous, *sequential].into_iter().flatten().collect(),
            Vec::new(),
        ),
        Kind::Prefix {
            id,
            modification,
            music,
            ..
        } => (
            [*id, *modification, Some(*music)]
                .into_iter()
                .flatten()
                .collect(),
            Vec::new(),
        ),
        Kind::NewLyrics { lists } => (Vec::new(), lists.clone()),
        Kind::EventChord {
            simple, note_chord, ..
        } => (
            [*simple, *note_chord].into_iter().flatten().collect(),
            Vec::new(),
        ),
        Kind::NoteChordElement { body, duration, .. } => (vec![*body, *duration], Vec::new()),
        Kind::ChordBody { elements } => (Vec::new(), elements.clone()),
        Kind::ChordBodyElement { parts } | Kind::SimpleElement { parts } => {
            (Vec::new(), nodes(parts))
        }
        _ => (Vec::new(), Vec::new()),
    }
}

/// `quoteString`: in double quotes, a quote inside escaped, a space after.
fn quote(text: &str) -> String {
    format!("\"{}\" ", text.replace('"', "\\\""))
}

/// `makeLettersOnlyId`: every character not a letter turned into one.
fn letters_only(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_alphabetic() {
                c
            } else {
                char::from_u32(c as u32 % 26 + 97).unwrap_or('a')
            }
        })
        .collect()
}

const COLOR_DEFINITION: &str = "color = #(define-music-function (color) (string?) #{\n        \\once \\override NoteHead.color = #(x11-color color)\n        \\once \\override Stem.color = #(x11-color color)\n        \\once \\override Rest.color = #(x11-color color)\n        \\once \\override Beam.color = #(x11-color color)\n     #})\n    ";
const BOOK_HEADER: &str = "\\include \"lilypond-book-preamble.ly\"\n    ";

// ------------------------------------------------------------ translator

/// music21's `LilypondConverter`.
struct Converter<'a> {
    tree: Tree,
    top: usize,
    context: usize,
    stored: Vec<usize>,
    current_measure: Option<&'a Stream>,
    in_word: bool,
    /// The time signature last written, which a pickup with none of its
    /// own is a part of.
    meter: Option<TimeSignature>,
    /// The number given to the next stream with no id of its own, where
    /// music21 writes the address of the object.
    unnamed: usize,
}

/// The id music21 writes for a stream: its own, or one made for it.
fn stream_id(stream: &Stream, unnamed: &mut usize) -> String {
    match stream.id() {
        Some(id) => id.to_string(),
        None => {
            *unnamed += 1;
            format!("{}", 1_000_000 + *unnamed)
        }
    }
}

fn is_part(stream: &Stream) -> bool {
    matches!(stream.kind(), StreamKind::Part | StreamKind::PartStaff)
}

impl<'a> Converter<'a> {
    fn new() -> Self {
        let mut tree = Tree::default();
        let top = tree.add(Kind::Top {
            contents: Vec::new(),
        });
        Self {
            tree,
            top,
            context: top,
            stored: Vec::new(),
            current_measure: None,
            in_word: false,
            meter: None,
            unnamed: 0,
        }
    }

    fn new_context(&mut self, context: usize) {
        self.stored.push(self.context);
        self.context = context;
    }

    fn restore_context(&mut self) {
        self.context = self.stored.pop().unwrap_or(self.top);
    }

    /// Appends an object to the context's contents and gives it the context
    /// as its parent, as music21 does after appending.
    fn append_to_context(&mut self, node: usize) -> Result<()> {
        let context = self.context;
        self.tree.contents_mut(context)?.push(Item::Node(node));
        self.tree.set_parent(node, context);
        Ok(())
    }

    fn embedded(&mut self, content: impl Into<String>) -> usize {
        self.tree.add(Kind::Embedded(content.into()))
    }

    /// `loadObjectFromScore`.
    fn load_score(&mut self, score: &'a Stream, options: &LilyOptions) -> Result<()> {
        let version = self.embedded(format!(
            "\\version {}",
            quote(&format!("{}.{}", options.version.0, options.version.1))
        ));
        let book = self.embedded(BOOK_HEADER);
        let colors = self.embedded(COLOR_DEFINITION);
        let header = self.tree.add(Kind::Header { body: None });
        let score_block = self.score_block(score)?;
        let output = self.tree.add(Kind::OutputDef);
        let layout = self.tree.add(Kind::Layout);
        let contents: Vec<Item> = [version, book, colors, header, score_block, output, layout]
            .into_iter()
            .map(Item::Node)
            .collect();
        if let Some(metadata) = score.metadata() {
            self.set_header(metadata, header);
        }
        // The contents go to the context music21 is left in, which is the
        // top unless a tuplet started and never stopped, or the other way.
        let context = self.context;
        if matches!(self.tree.nodes[context].kind, Kind::Composite { .. }) {
            return Err(lily_error(
                "AttributeError: property 'contents' of 'LyCompositeMusic' object has no setter",
            ));
        }
        for item in &contents {
            if let Item::Node(node) = item {
                self.tree.adopt(context, *node);
            }
        }
        *self.tree.contents_mut(context)? = contents;
        Ok(())
    }

    /// `setHeaderFromMetadata`: the best title and the alternative title.
    fn set_header(&mut self, metadata: &Metadata, header: usize) {
        let body = self.tree.add(Kind::HeaderBody {
            assignments: Vec::new(),
        });
        if let Kind::Header { body: held } = &mut self.tree.nodes[header].kind {
            *held = Some(body);
        }
        self.tree.set_parent(body, header);
        let best = ["title", "popularTitle", "alternativeTitle", "movementName"]
            .iter()
            .find_map(|name| {
                metadata
                    .string_value(name)
                    .filter(|title| !title.is_empty())
            });
        for (id, value) in [
            ("title", best),
            (
                "subtitle",
                metadata
                    .string_value("alternativeTitle")
                    .filter(|title| !title.is_empty()),
            ),
        ] {
            let Some(value) = value else {
                continue;
            };
            let assignment = self.tree.add(Kind::Assignment {
                id: id.to_string(),
                value,
            });
            if let Kind::HeaderBody { assignments } = &mut self.tree.nodes[body].kind {
                assignments.push(Item::Node(assignment));
            }
            self.tree.set_parent(assignment, body);
        }
    }

    /// `lyScoreBlockFromScore`.
    fn score_block(&mut self, score: &'a Stream) -> Result<usize> {
        let composite = self.tree.add(Kind::Composite {
            prefix: None,
            grouped: None,
            lyrics: None,
        });
        self.new_context(composite);
        let parts: Vec<&'a Stream> = score
            .events()
            .iter()
            .filter_map(|event| match event.element() {
                StreamElement::Stream(inner) if is_part(inner) => Some(&**inner),
                _ => None,
            })
            .collect();
        if parts.is_empty() {
            let prefix = self.prefix_from_stream(score, None, None, false)?;
            if let Kind::Composite { prefix: held, .. } = &mut self.tree.nodes[composite].kind {
                *held = Some(prefix);
            }
            self.tree.set_parent(prefix, composite);
        } else {
            let grouped = self.grouped_from_parts(&parts)?;
            if let Kind::Composite { grouped: held, .. } = &mut self.tree.nodes[composite].kind {
                *held = Some(grouped);
            }
            self.tree.set_parent(grouped, composite);
        }
        let music = self.tree.add(Kind::Music { composite });
        let body = self.tree.add(Kind::ScoreBody { music });
        let block = self.tree.add(Kind::ScoreBlock { body });
        self.restore_context();
        Ok(block)
    }

    /// `lyGroupedMusicListFromScoreWithParts`.
    fn grouped_from_parts(&mut self, parts: &[&'a Stream]) -> Result<usize> {
        let grouped = self.tree.add(Kind::Grouped {
            simultaneous: None,
            sequential: None,
        });
        let list = self.tree.add(Kind::MusicList {
            contents: Vec::new(),
        });
        let simultaneous = self.tree.add(Kind::Simultaneous { list });
        if let Kind::Grouped {
            simultaneous: held, ..
        } = &mut self.tree.nodes[grouped].kind
        {
            *held = Some(simultaneous);
        }
        self.tree.set_parent(simultaneous, grouped);
        self.new_context(list);
        let mut composites = Vec::new();
        for part in parts {
            composites.push(self.prefix_from_stream(part, None, None, false)?);
        }
        self.restore_context();
        for composite in &composites {
            self.tree.adopt(list, *composite);
        }
        if let Kind::MusicList { contents } = &mut self.tree.nodes[list].kind {
            *contents = composites.into_iter().map(Item::Node).collect();
        }
        Ok(grouped)
    }

    /// `lyPrefixCompositeMusicFromStream`.
    fn prefix_from_stream(
        &mut self,
        stream: &'a Stream,
        context_type: Option<&str>,
        kind: Option<&'static str>,
        start_staff: bool,
    ) -> Result<usize> {
        let id_text = stream_id(stream, &mut self.unnamed);
        let (context, id) = match context_type {
            Some(context) => (
                context.to_string(),
                Some(self.tree.add(Kind::OptionalId(letters_only(&id_text)))),
            ),
            None if is_part(stream) => (
                "Staff".to_string(),
                Some(self.tree.add(Kind::OptionalId(letters_only(&id_text)))),
            ),
            None => ("Voice".to_string(), None),
        };
        let mut modifications = Vec::new();
        if crate::makenotation::beams_made(stream) {
            modifications.push("\\autoBeamOff ".to_string());
        }
        let lines = staff_lines(stream);
        if lines != 5 {
            modifications.push(format!("\\override StaffSymbol.line-count = #{lines}"));
        }
        let lyrics = self.new_lyrics(stream, &letters_only(&id_text));
        let sequential = self.sequential_from_stream(stream, start_staff)?;
        let grouped = self.tree.add(Kind::Grouped {
            simultaneous: None,
            sequential: Some(sequential),
        });
        let composite = self.tree.add(Kind::Composite {
            prefix: None,
            grouped: Some(grouped),
            lyrics: Some(lyrics),
        });
        let music = self.tree.add(Kind::Music { composite });
        let modification = (!modifications.is_empty()).then(|| {
            self.tree.add(Kind::ContextModification {
                list: modifications,
            })
        });
        Ok(self.tree.add(Kind::Prefix {
            kind: kind.unwrap_or("new"),
            context,
            id,
            modification,
            music,
            fraction: String::new(),
        }))
    }

    /// `lyNewLyricsFromStream`.
    fn new_lyrics(&mut self, stream: &Stream, id: &str) -> usize {
        let aligned = format!("#{}", quote(id));
        let mut lists = Vec::new();
        for (_, lyrics) in stream_lyrics(stream) {
            let mut items = vec![Item::Node(self.tree.add(Kind::PropertySet {
                property: "alignBelowContext".to_string(),
                value: aligned.clone(),
            }))];
            self.in_word = false;
            for lyric in lyrics {
                let text = self.lyric_text(lyric);
                items.push(Item::Node(self.tree.add(Kind::LyricElement(text))));
            }
            self.in_word = false;
            let list = self.tree.add(Kind::MusicList { contents: items });
            let sequential = self.tree.add(Kind::Sequential {
                list,
                start_staff: false,
            });
            lists.push(self.tree.add(Kind::Grouped {
                simultaneous: None,
                sequential: Some(sequential),
            }));
        }
        self.tree.add(Kind::NewLyrics { lists })
    }

    /// `lyLyricElementFromM21Lyric`.
    fn lyric_text(&mut self, lyric: Option<&crate::notation::Lyric>) -> String {
        let mut in_word = self.in_word;
        let text = match lyric {
            None => " _ ".to_string(),
            Some(lyric) if lyric.text().is_empty() => " _ ".to_string(),
            Some(lyric) => {
                let mut text = format!("\"{}\"", lyric.text());
                match lyric.syllabic() {
                    Syllabic::End => {
                        text += "__";
                        in_word = false;
                    }
                    Syllabic::Begin | Syllabic::Middle => {
                        text += " --";
                        in_word = true;
                    }
                    _ => {}
                }
                text
            }
        };
        self.in_word = in_word;
        text
    }

    /// `lySequentialMusicFromStream`.
    fn sequential_from_stream(&mut self, stream: &'a Stream, start_staff: bool) -> Result<usize> {
        let list = self.tree.add(Kind::MusicList {
            contents: Vec::new(),
        });
        let sequential = self.tree.add(Kind::Sequential { list, start_staff });
        self.new_context(list);
        self.append_stream(stream)?;
        if let Some(bar) = self.close_measure() {
            // Appended, so it takes no parent.
            self.tree.contents_mut(list)?.push(Item::Node(bar));
        }
        self.restore_context();
        Ok(sequential)
    }

    /// `appendObjectsToContextFromStream`: the stream's elements a group of
    /// those at one offset at a time, voices together at once.
    fn append_stream(&mut self, stream: &'a Stream) -> Result<()> {
        let events = stream.events();
        let mut start = 0;
        while start < events.len() {
            let offset = events[start].offset();
            let mut end = start + 1;
            while end < events.len() && events[end].offset() == offset {
                end += 1;
            }
            let group = &events[start..end];
            if group.len() == 1 {
                self.append_element(group[0].element())?;
            } else {
                let voices: Vec<&'a Stream> = group
                    .iter()
                    .filter_map(|event| match event.element() {
                        StreamElement::Stream(inner) if inner.kind() == StreamKind::Voice => {
                            Some(&**inner)
                        }
                        _ => None,
                    })
                    .collect();
                if !voices.is_empty() {
                    let grouped = self.tree.add(Kind::Grouped {
                        simultaneous: None,
                        sequential: None,
                    });
                    let list = self.tree.add(Kind::MusicList {
                        contents: Vec::new(),
                    });
                    let simultaneous = self.tree.add(Kind::Simultaneous { list });
                    if let Kind::Grouped {
                        simultaneous: held, ..
                    } = &mut self.tree.nodes[grouped].kind
                    {
                        *held = Some(simultaneous);
                    }
                    self.tree.set_parent(simultaneous, grouped);
                    let mut composites = Vec::new();
                    for voice in voices {
                        composites.push(self.prefix_from_stream(voice, None, None, false)?);
                    }
                    for composite in &composites {
                        self.tree.adopt(list, *composite);
                    }
                    if let Kind::MusicList { contents } = &mut self.tree.nodes[list].kind {
                        *contents = composites.into_iter().map(Item::Node).collect();
                    }
                    self.append_to_context(grouped)?;
                }
                for event in group {
                    if !matches!(event.element(), StreamElement::Stream(inner) if inner.kind() == StreamKind::Voice)
                    {
                        self.append_element(event.element())?;
                    }
                }
            }
            start = end;
        }
        Ok(())
    }

    /// `appendM21ObjectToContext`.
    fn append_element(&mut self, element: &'a StreamElement) -> Result<()> {
        if !matches!(element, StreamElement::Stream(_))
            && let Some(duration) = element.duration()
            && duration.is_complex()
        {
            for (_, piece) in crate::makenotation::element_at_durations(element, duration)? {
                self.append_owned(piece)?;
            }
            return Ok(());
        }
        match element {
            StreamElement::Stream(measure) if measure.kind() == StreamKind::Measure => {
                if let Some(bar) = self.close_measure() {
                    self.append_to_context(bar)?;
                }
                if let Some(pad) = self.padding(measure)? {
                    self.append_to_context(pad)?;
                }
                self.append_stream(measure)?;
                self.current_measure = Some(measure);
            }
            StreamElement::Stream(inner) => {
                let prefix = self.prefix_from_stream(inner, None, None, false)?;
                self.append_to_context(prefix)?;
            }
            other => self.append_owned(other.clone())?,
        }
        Ok(())
    }

    /// `appendM21ObjectToContext` for what is not a stream.
    fn append_owned(&mut self, element: StreamElement) -> Result<()> {
        match &element {
            StreamElement::Note(note) => self.append_note(note)?,
            StreamElement::Rest(rest) => self.append_rest(rest)?,
            StreamElement::Chord(chord) => self.append_chord(chord, chord.duration())?,
            // music21's chord symbol is a chord, with no length to write.
            StreamElement::ChordSymbol(symbol) => {
                let chord = Chord::new(symbol.pitches()?)?;
                self.append_chord(&chord, Some(symbol.duration()))?;
            }
            StreamElement::Clef(clef) => {
                let scheme = self.embedded(clef_scheme(clef));
                self.append_to_context(scheme)?;
            }
            StreamElement::KeySignature(signature) => {
                let scheme = self.embedded(key_scheme(&signature.as_key("major")));
                self.append_to_context(scheme)?;
            }
            StreamElement::Key(key) => {
                let scheme = self.embedded(key_scheme(key));
                self.append_to_context(scheme)?;
            }
            StreamElement::TimeSignature(meter) => {
                self.meter = Some(meter.clone());
                let scheme = self.embedded(time_scheme(meter));
                self.append_to_context(scheme)?;
            }
            StreamElement::MetronomeMark(mark) => {
                if let Some(scheme) = tempo_scheme(mark)? {
                    let scheme = self.embedded(scheme);
                    self.append_to_context(scheme)?;
                }
            }
            StreamElement::Layout(crate::layout::Layout::System(_)) => {
                let scheme = self.embedded("\\break");
                self.append_to_context(scheme)?;
            }
            StreamElement::Layout(crate::layout::Layout::Page(_)) => {
                let scheme = self.embedded("\\pageBreak");
                self.append_to_context(scheme)?;
            }
            _ => {}
        }
        Ok(())
    }

    /// `setContextForTupletStart`: a `\tuplet` the notes go into, where a
    /// tuplet's bracket starts.
    fn tuplet_start(&mut self, duration: Option<&Duration>) -> Result<()> {
        let Some(first) = duration.and_then(|duration| duration.tuplets().into_iter().next())
        else {
            return Ok(());
        };
        if first.tuplet_type() != Some(crate::duration::TupletType::Start) {
            return Ok(());
        }
        let fraction = format!("{}/{}", first.tuplet_actual().0, first.tuplet_normal().0);
        let list = self.tree.add(Kind::MusicList {
            contents: Vec::new(),
        });
        let sequential = self.tree.add(Kind::Sequential {
            list,
            start_staff: false,
        });
        let prefix = self.tree.add(Kind::Prefix {
            kind: "tuplet",
            context: String::new(),
            id: None,
            modification: None,
            music: sequential,
            fraction,
        });
        self.append_to_context(prefix)?;
        self.new_context(list);
        Ok(())
    }

    /// `setContextForTupletStop`.
    fn tuplet_stop(&mut self, duration: Option<&Duration>) {
        if duration
            .and_then(|duration| duration.tuplets().into_iter().next())
            .is_some_and(|first| first.tuplet_type() == Some(crate::duration::TupletType::Stop))
        {
            self.restore_context();
        }
    }

    /// `appendBeamCode`: how many beams leave the stem on each side.
    fn beam_code(&mut self, beams: &Beams) -> Result<()> {
        let (mut left, mut right) = (0, 0);
        for beam in beams.beams() {
            match beam.beam_type().map_or("", |kind| kind.as_str()) {
                "start" => right += 1,
                "continue" => {
                    right += 1;
                    left += 1;
                }
                "stop" => left += 1,
                "partial" => {
                    if beam
                        .direction()
                        .is_some_and(|direction| direction.as_str() == "left")
                    {
                        left += 1;
                    } else {
                        right += 1;
                    }
                }
                _ => {}
            }
        }
        if left > 0 {
            let scheme = self.embedded(format!("\\set stemLeftBeamCount = #{left}"));
            self.append_to_context(scheme)?;
        }
        if right > 0 {
            let scheme = self.embedded(format!("\\set stemRightBeamCount = #{right}"));
            self.append_to_context(scheme)?;
        }
        Ok(())
    }

    /// `appendStemCode`: a stem said to go up or down.
    fn stem_code(&mut self, direction: StemDirection) -> Result<()> {
        let said = match direction {
            StemDirection::Up => "UP",
            StemDirection::Down => "DOWN",
            _ => return Ok(()),
        };
        let scheme = self.embedded(format!("\\once \\override Stem.direction = #{said} "));
        self.append_to_context(scheme)
    }

    /// `appendContextFromNoteOrRest` for a note.
    fn append_note(&mut self, note: &Note) -> Result<()> {
        let duration = note.duration();
        if duration.is_some_and(Duration::is_grace) {
            return Ok(());
        }
        self.tuplet_start(duration)?;
        self.beam_code(note.beams())?;
        self.stem_code(note.stem_direction())?;
        let mut parts = Vec::new();
        if let Some(color) = note.color().filter(|color| !color.is_empty())
            && !note.hidden()
        {
            parts.push(Item::Text(format!(
                "\\override NoteHead.color = \"{color}\"\n"
            )));
            parts.push(Item::Text(format!("\\override Stem.color = \"{color}\"\n")));
        }
        if note.hidden() {
            parts.push(Item::Text("s ".to_string()));
        } else {
            parts.push(Item::Node(self.pitch_node(note.pitch())?));
            parts.extend(accidental_marks(note.pitch()));
        }
        let length = self.duration_node(&element_duration(duration))?;
        parts.push(Item::Node(length));
        if let Some(first) = note.beams().beams().first() {
            match first.beam_type().map_or("", |kind| kind.as_str()) {
                "start" => parts.push(Item::Text("[ ".to_string())),
                "stop" => parts.push(Item::Text("] ".to_string())),
                _ => {}
            }
        }
        let post = post_events(note.tie().map(|tie| tie.tie_type()), note.expressions());
        self.push_simple(parts, post)?;
        self.tuplet_stop(duration);
        Ok(())
    }

    /// `appendContextFromNoteOrRest` for a rest.
    fn append_rest(&mut self, rest: &Rest) -> Result<()> {
        let duration = Some(rest.duration());
        if rest.duration().is_grace() {
            return Ok(());
        }
        self.tuplet_start(duration)?;
        let mut parts = vec![Item::Text(
            if rest.hidden() { "s " } else { "r " }.to_string(),
        )];
        let length = self.duration_node(rest.duration())?;
        parts.push(Item::Node(length));
        let post = post_events(rest.tie().map(|tie| tie.tie_type()), rest.expressions());
        self.push_simple(parts, post)?;
        self.tuplet_stop(duration);
        Ok(())
    }

    fn push_simple(&mut self, parts: Vec<Item>, post: Vec<String>) -> Result<()> {
        let simple = self.tree.add(Kind::SimpleElement { parts });
        let chord = self.tree.add(Kind::EventChord {
            simple: Some(simple),
            post,
            note_chord: None,
        });
        let music = self.tree.add(Kind::SimpleMusic { chord });
        self.append_to_context(music)
    }

    /// `appendContextFromChord`, whose beams and stem music21 writes twice:
    /// once before the chord and once as it makes the chord.
    fn append_chord(&mut self, chord: &Chord, duration: Option<&Duration>) -> Result<()> {
        self.tuplet_start(duration)?;
        self.beam_code(chord.beams())?;
        self.stem_code(chord.stem_direction())?;
        // `lySimpleMusicFromChord`.
        self.beam_code(chord.beams())?;
        self.stem_code(chord.stem_direction())?;
        let mut elements = Vec::new();
        for note in chord.notes() {
            let mut parts = vec![Item::Node(self.pitch_node(note.pitch())?)];
            parts.extend(accidental_marks(note.pitch()));
            elements.push(self.tree.add(Kind::ChordBodyElement { parts }));
        }
        let body = self.tree.add(Kind::ChordBody { elements });
        let length = self.duration_node(&element_duration(duration))?;
        let post = post_events(chord.tie().map(|tie| tie.tie_type()), chord.expressions());
        let note_chord = self.tree.add(Kind::NoteChordElement {
            body,
            duration: length,
            post,
        });
        let event = self.tree.add(Kind::EventChord {
            simple: None,
            post: Vec::new(),
            note_chord: Some(note_chord),
        });
        let music = self.tree.add(Kind::SimpleMusic { chord: event });
        self.append_to_context(music)?;
        self.tuplet_stop(duration);
        Ok(())
    }

    /// `lyPitchFromPitch`.
    fn pitch_node(&mut self, pitch: &Pitch) -> Result<usize> {
        let quotes = octave_marks(pitch)?;
        Ok(self.tree.add(Kind::Pitch {
            name: base_name(pitch),
            quotes,
        }))
    }

    /// `lyMultipliedDurationFromDuration`.
    fn duration_node(&mut self, duration: &Duration) -> Result<usize> {
        let (number, dots) = duration_number(duration)?;
        Ok(self.tree.add(Kind::Duration { number, dots }))
    }

    /// `closeMeasure`: the barline closing the measure last written.
    fn close_measure(&mut self) -> Option<usize> {
        let measure = self.current_measure.take()?;
        let mut bar = match measure.right_barline() {
            None => format!("\\bar {}", quote("|")),
            Some(barline) => format!("\\bar {}", quote(barline_sign(barline.bar_type()))),
        };
        bar += &format!(" %{{ end measure {} %}} ", measure.number());
        Some(self.embedded(bar))
    }

    /// `getSchemeForPadding`: a pickup, as the 32nds of its bar it fills.
    fn padding(&mut self, measure: &Stream) -> Result<Option<usize>> {
        let padding = measure.padding_left();
        if padding == 0.0 {
            return Ok(None);
        }
        let bar =
            meter_of(measure, self.meter.clone()).map_or(4.0, |meter| meter.bar_quarter_length());
        let remaining = bar - padding;
        if remaining <= 0.0 {
            return Err(lily_error("your first pickup measure is non-existent!"));
        }
        Ok(Some(self.embedded(format!(
            "\\partial 32*{} ",
            (remaining * 8.0) as i64
        ))))
    }
}

/// The time signature a measure says, or the one in force before it.
fn meter_of(measure: &Stream, fallback: Option<TimeSignature>) -> Option<TimeSignature> {
    measure
        .events()
        .iter()
        .find_map(|event| match event.element() {
            StreamElement::TimeSignature(meter) => Some(meter.clone()),
            _ => None,
        })
        .or(fallback)
}

/// A duration, or a quarter where there is none.
fn element_duration(duration: Option<&Duration>) -> Duration {
    duration.cloned().unwrap_or_else(Duration::quarter)
}

/// The note-value number LilyPond writes for a duration and its dots.
fn duration_number(duration: &Duration) -> Result<(String, u32)> {
    let name = crate::braille::basic::duration_type(duration);
    let number: FloatType = match name.as_str() {
        "duplex-maxima" => 0.0625,
        "maxima" => 0.125,
        "longa" => 0.25,
        "breve" => 0.5,
        "zero" => 0.0,
        "inexpressible" | "complex" => {
            return Err(lily_error(format!(
                "DurationException for durationObject {}: Could not determine durationNumber from {name}",
                duration_repr(duration)
            )));
        }
        "whole" => 1.0,
        "half" => 2.0,
        "quarter" => 4.0,
        "eighth" => 8.0,
        other => other
            .trim_end_matches(|c: char| c.is_ascii_alphabetic())
            .parse()
            .unwrap_or(0.0),
    };
    if number == 0.0 {
        return Err(lily_error(format!(
            "Cannot translate an object of zero duration {}",
            duration_repr(duration)
        )));
    }
    let written = if number < 1.0 {
        if number == 0.5 {
            "\\breve".to_string()
        } else if number == 0.25 {
            "\\longa".to_string()
        } else {
            return Err(lily_error("Cannot support durations longer than longa"));
        }
    } else {
        format!("{}", number as i64)
    };
    Ok((written, crate::braille::basic::duration_dots(duration)))
}

fn duration_repr(duration: &Duration) -> String {
    format!(
        "<music21.duration.Duration {}>",
        crate::braille::basic::quarter_length_repr(duration.quarter_length())
    )
}

/// `baseNameFromPitch`: the letter, with LilyPond's Dutch accidental.
fn base_name(pitch: &Pitch) -> String {
    let mut name = pitch.step().as_char().to_ascii_lowercase().to_string();
    if let Some(accidental) = pitch.written_accidental() {
        name += match accidental.name() {
            "double-sharp" => "isis",
            "double-flat" => "eses",
            "one-and-a-half-sharp" => "isih",
            "one-and-a-half-flat" => "eseh",
            "sharp" => "is",
            "flat" => "es",
            "half-sharp" => "ih",
            "half-flat" => "eh",
            _ => "",
        };
    }
    name
}

/// `octaveCharactersFromPitch`: commas below the third octave, quotes above.
fn octave_marks(pitch: &Pitch) -> Result<String> {
    let Some(octave) = pitch.octave() else {
        return Err(lily_error(
            "a pitch with no octave has no place on the staff",
        ));
    };
    Ok(if octave < 3 {
        ",".repeat((3 - octave) as usize)
    } else {
        "'".repeat((octave - 3) as usize)
    })
}

/// An accidental always shown is forced, one in parentheses cautionary.
fn accidental_marks(pitch: &Pitch) -> Vec<Item> {
    let mut marks = Vec::new();
    if let Some(accidental) = pitch.written_accidental() {
        if accidental.display_type() == "always" {
            marks.push(Item::Text("! ".to_string()));
        }
        if accidental.display_style() == "parentheses" {
            marks.push(Item::Text("? ".to_string()));
        }
    }
    marks
}

/// `postEventsFromObject`: a tie going on, and fermatas.
fn post_events(tie: Option<TieType>, expressions: &[Expression]) -> Vec<String> {
    let mut post = Vec::new();
    if tie.is_some_and(|tie| tie != TieType::Stop) {
        post.push("~ ".to_string());
    }
    for expression in expressions {
        if matches!(expression, Expression::Fermata(_)) {
            post.push("\\fermata ".to_string());
        }
    }
    post
}

/// `lyEmbeddedScmFromClef`.
fn clef_scheme(clef: &Clef) -> String {
    let kind = clef.kind();
    let classes: Vec<&str> = std::iter::once(kind.class_name())
        .chain(kind.parents().iter().copied())
        .collect();
    let name = [
        ("Treble8vbClef", "treble_8"),
        ("TrebleClef", "treble"),
        ("BassClef", "bass"),
        ("AltoClef", "alto"),
        ("TenorClef", "tenor"),
        ("SopranoClef", "soprano"),
        ("PercussionClef", "percussion"),
    ]
    .iter()
    .find(|(class, _)| classes.contains(class))
    .map_or("", |(_, name)| name);
    format!("\\clef {}\n", quote(name))
}

/// `lyEmbeddedScmFromKeySignature`.
fn key_scheme(key: &Key) -> String {
    format!("\\key {} \\{} \n", base_name(&key.tonic()), key.mode())
}

/// `lyEmbeddedScmFromTimeSignature`.
fn time_scheme(meter: &TimeSignature) -> String {
    format!("\\time {}\n", meter.ratio_string())
}

/// `lyEmbeddedScmFromMetronomeMark`: nothing for a mark with no number.
fn tempo_scheme(mark: &MetronomeMark) -> Result<Option<String>> {
    let Some(number) = mark.number() else {
        return Ok(None);
    };
    let (steno, dots) = duration_number(mark.referent())?;
    let steno = format!("{steno}{} ", ".".repeat(dots as usize));
    Ok(Some(format!(
        "\\tempo {steno} = {}\n",
        if number.fract() == 0.0 {
            format!("{}", number as i64)
        } else {
            crate::statistics::python_repr(number)
        }
    )))
}

/// `barlineDict`: LilyPond's sign for a barline.
fn barline_sign(bar: BarlineType) -> &'static str {
    match bar {
        BarlineType::Regular => "|",
        BarlineType::Dotted => ";",
        BarlineType::Dashed => "!",
        BarlineType::Heavy => ".",
        BarlineType::Double => "||",
        BarlineType::Final => "|.",
        BarlineType::HeavyLight => ".|",
        BarlineType::HeavyHeavy => "..",
        BarlineType::Tick => "'",
        BarlineType::Short => ",",
        BarlineType::None => "",
    }
}

/// `staffLines`: the first staff layout at the start that says, else five.
fn staff_lines(stream: &Stream) -> i64 {
    for (offset, element) in stream.recurse() {
        if offset > 0.0 {
            break;
        }
        if let StreamElement::Layout(crate::layout::Layout::Staff(staff)) = element
            && let Some(lines) = staff.staff_lines
        {
            return lines;
        }
    }
    5
}

/// `Stream.lyrics(skipTies=True)`: for each verse, the lyric of every note
/// the stream holds, or of its measures, a note tied over skipped.
fn stream_lyrics(stream: &Stream) -> Vec<(i64, Vec<Option<&crate::notation::Lyric>>)> {
    let mut verses: Vec<(i64, Vec<Option<&crate::notation::Lyric>>)> = Vec::new();
    let mut count = 0usize;
    let mut notes: Vec<&StreamElement> = Vec::new();
    for event in stream.events() {
        match event.element() {
            StreamElement::Stream(measure) if measure.kind() == StreamKind::Measure => {
                notes.extend(
                    measure
                        .events()
                        .iter()
                        .map(|event| event.element())
                        .filter(|element| is_not_rest(element)),
                );
            }
            element if is_not_rest(element) => notes.push(element),
            _ => {}
        }
    }
    for element in notes {
        let tie = match element {
            StreamElement::Note(note) => note.tie().map(|tie| tie.tie_type()),
            StreamElement::Chord(chord) => chord.tie().map(|tie| tie.tie_type()),
            _ => None,
        };
        if tie.is_some_and(|tie| tie != TieType::Start) {
            continue;
        }
        let lyrics: &[crate::notation::Lyric] = match element {
            StreamElement::Note(note) => note.lyrics(),
            StreamElement::Chord(chord) => chord.lyrics(),
            _ => &[],
        };
        let mut said: Vec<i64> = Vec::new();
        for lyric in lyrics {
            let number = lyric.number() as i64;
            match verses.iter_mut().find(|(known, _)| *known == number) {
                Some((_, list)) => list.push(Some(lyric)),
                None => {
                    let mut list = vec![None; count];
                    list.push(Some(lyric));
                    verses.push((number, list));
                }
            }
            said.push(number);
        }
        for (number, list) in &mut verses {
            if !said.contains(number) {
                list.push(None);
            }
        }
        count += 1;
    }
    verses.sort_by_key(|(number, _)| *number);
    verses
}

/// music21's `NotRest`: a note, chord, stroke or chord symbol.
fn is_not_rest(element: &StreamElement) -> bool {
    matches!(
        element,
        StreamElement::Note(_)
            | StreamElement::Chord(_)
            | StreamElement::Unpitched(_)
            | StreamElement::PercussionChord(_)
            | StreamElement::ChordSymbol(_)
    )
}

/// A stream written as LilyPond, as music21's `LilypondConverter`'s
/// `textFromMusic21Object` writes it: a score's parts each on a staff, its
/// voices, measures with their barlines, pickups, clefs, keys, meters,
/// tempi, breaks, notes, rests and chords with their beams, stems, ties,
/// fermatas, tuplets and colours, and each part's lyrics. A stream that is
/// not a score is written as the one part of a score.
///
/// A stream with no id is named by a number of the writer's own, where
/// music21 takes the address of the object, which changes from run to run.
///
/// # Errors
///
/// A note of no length, or of one longer than a longa or no note value
/// writes, or a pickup longer than its bar, which music21 refuses.
pub fn to_lilypond(stream: &Stream, options: &LilyOptions) -> Result<String> {
    let wrapped;
    let score = if stream.kind() == StreamKind::Score {
        stream
    } else {
        let mut score = Stream::with_kind(StreamKind::Score);
        if matches!(stream.kind(), StreamKind::Measure | StreamKind::Voice) {
            let mut part = Stream::with_kind(StreamKind::Part);
            part.insert(0.0, StreamElement::Stream(Box::new(stream.clone())));
            score.insert(0.0, StreamElement::Stream(Box::new(part)));
        } else {
            score.insert(0.0, StreamElement::Stream(Box::new(stream.clone())));
        }
        wrapped = score;
        &wrapped
    };
    let mut converter = Converter::new();
    converter.load_score(score, options)?;
    let written = converter.tree.str_of(converter.top);
    Ok(collapse_blank_lines(&written).trim().to_string())
}

/// `re.sub(r'\s*\n\s*\n', '\n', text)`: a run of whitespace holding two
/// newlines or more, up to its last newline, is one newline.
fn collapse_blank_lines(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut index = 0;
    while index < chars.len() {
        if !chars[index].is_whitespace() {
            out.push(chars[index]);
            index += 1;
            continue;
        }
        let mut end = index;
        while end < chars.len() && chars[end].is_whitespace() {
            end += 1;
        }
        let newlines: Vec<usize> = (index..end).filter(|&at| chars[at] == '\n').collect();
        if let [_, .., last] = newlines[..] {
            out.push('\n');
            out.extend(&chars[last + 1..end]);
        } else {
            out.extend(&chars[index..end]);
        }
        index = end;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tinynotation::from_tiny_notation;

    #[test]
    fn blank_lines_are_closed_up_as_python_closes_them() {
        assert_eq!(collapse_blank_lines("a \n \n b"), "a\n b");
        assert_eq!(collapse_blank_lines("a\n\n\nb"), "a\nb");
        assert_eq!(collapse_blank_lines("a\nb"), "a\nb");
    }

    #[test]
    fn ids_are_made_of_letters_as_music21_makes_them() {
        assert_eq!(letters_only("P1"), "Px");
        assert_eq!(letters_only("Soprano"), "Soprano");
    }

    #[test]
    fn a_line_is_written_as_music21_writes_it() {
        let line = from_tiny_notation("4/4 c4 d8 e f#4. g8 a2 r2").unwrap();
        let text = to_lilypond(&line, &LilyOptions::default()).unwrap();
        assert!(text.contains("\\time 4/4"));
        assert!(text.contains("fis' 4."));
        assert!(text.contains("\\bar \"|.\"  %{ end measure 2 %}"));
    }
}
