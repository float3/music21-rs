//! music21's `abcFormat`: the ABC tokens, `ABCHandler`, `ABCHandlerBar`,
//! `mergeLeadingMetaData` and `ABCFile`, over the crate's `abc::step`.
//!
//! Each object keeps music21's attributes in its `__dict__` -- a token's
//! `src`, `tag`, `pitchName`, `applicableSpanners`; a handler's `tokens`,
//! `abcVersion`, `abcDirectives` -- so code reading or setting them works
//! as on music21's. Each step reads them into the crate's value, takes the
//! crate's step, and writes what it changed back. Objects music21 shares
//! between tokens (a tuplet, a key, a slur) are shared here too.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use music21::abc::step::{
    self, AbcMetadata, AbcNote, AbcTuplet, Bar, BrokenRhythm, Closes, Handler, Merge, Side,
    Spanning, Token,
};
use music21::notation::TieType;
use music21::spanner::SpannerKind;
use music21::stream::StreamElement;
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyList, PyModule, PyString, PyTuple};

use crate::subconverters::music21_name;

/// The module music21 keeps these in.
pub(crate) const MODULE: &str = "music21.abcFormat";

/// music21's `reMetadataTag` and the other patterns it exports, each name
/// beside its pattern.
const PATTERNS: [(&str, &str); 6] = [
    ("reMetadataTag", "[A-Zw]:"),
    ("rePitchName", "[a-gA-Gz]"),
    ("reChordSymbol", "\"[^\"]*\""),
    ("reChord", "[.*?]"),
    ("reAbcVersion", r"%abc-(\d+)\.(\d+)\.?(\d+)?"),
    ("reDirective", r"^%%([a-z\-]+)\s+(\S+)(.*)"),
];

/// What music21 says of a handler asked to cut tokens it has none of.
const NO_TOKENS: &str = "must process tokens before calling split";

static BUILT: PyOnceLock<Py<PyModule>> = PyOnceLock::new();

pyo3::create_exception!(
    music21_rs_facade,
    ABCTokenException,
    crate::Music21Exception
);
pyo3::create_exception!(
    music21_rs_facade,
    ABCHandlerException,
    crate::Music21Exception
);
pyo3::create_exception!(music21_rs_facade, ABCFileException, crate::Music21Exception);

/// The opening words of the crate's refusals music21 raises from a token's
/// own method rather than the handler's.
const TOKEN_REFUSALS: [&str; 7] = [
    "cannot handle tuplet",
    "must set numberNotesActual",
    "no time signature",
    "no key signature",
    "no tempo",
    "no quarter length",
    "cannot calculate quarter length",
];

/// A crate refusal as the exception music21 raises for it.
pub(crate) fn refusal(error: &music21::Error) -> PyErr {
    let message = crate::pitch::message(error);
    if TOKEN_REFUSALS
        .iter()
        .any(|start| message.starts_with(start))
    {
        return ABCTokenException::new_err(message);
    }
    ABCHandlerException::new_err(message)
}

// ------------------------------------------------------------ attributes

/// Each name set on `object` to its value.
fn set_all(object: &Bound<'_, PyAny>, pairs: &[(&str, Bound<'_, PyAny>)]) -> PyResult<()> {
    for (name, value) in pairs {
        object.setattr(*name, value)?;
    }
    Ok(())
}

/// A string attribute, empty where unset or not a string.
fn text(object: &Bound<'_, PyAny>, name: &str) -> PyResult<String> {
    let value = object.getattr(name)?;
    if value.is_none() {
        return Ok(String::new());
    }
    Ok(value.str()?.to_string())
}

/// An attribute that may be `None`, read as `T`.
macro_rules! optional {
    ($object:expr, $name:expr) => {{
        let value = $object.getattr($name)?;
        if value.is_none() {
            None
        } else {
            Some(value.extract()?)
        }
    }};
}

fn none(py: Python<'_>) -> Bound<'_, PyAny> {
    py.None().into_bound(py)
}

fn string<'py>(py: Python<'py>, value: &str) -> Bound<'py, PyAny> {
    PyString::new(py, value).into_any()
}

fn object<'py, T: pyo3::IntoPyObjectExt<'py>>(
    py: Python<'py>,
    value: T,
) -> PyResult<Bound<'py, PyAny>> {
    value.into_bound_py_any(py)
}

/// A crate element as this package's object.
fn element<'py>(py: Python<'py>, element: StreamElement) -> PyResult<Bound<'py, PyAny>> {
    Ok(crate::stream::element_object(py, &element)?.unwrap_or_else(|| none(py)))
}

/// A crate tuplet as music21's `Tuplet`.
fn tuplet_object(py: Python<'_>, tuplet: music21::duration::Tuplet) -> PyResult<Bound<'_, PyAny>> {
    Ok(crate::installed_new(
        py,
        "music21.duration",
        "Tuplet",
        crate::duration::Tuplet::wrap(tuplet),
    )?
    .into_bound(py)
    .into_any())
}

/// A quarter length as music21's `opFrac` hands it back.
fn quarters(py: Python<'_>, value: f64) -> PyResult<Bound<'_, PyAny>> {
    music21_name(py, "music21.common", "opFrac")?.call1((value,))
}

/// The ABC version a handler or file holds, as the crate's triple.
fn version_of(object: &Bound<'_, PyAny>) -> PyResult<(u32, u32, u32)> {
    let numbers: Vec<u32> = object.getattr("abcVersion")?.extract()?;
    let at = |place: usize| numbers.get(place).copied().unwrap_or(0);
    Ok((at(0), at(1), at(2)))
}

// ----------------------------------------------------------- conversions

/// The spanners of a run of tokens, each Python object beside the crate's
/// spanner standing for it, in the order they are met.
#[derive(Default)]
struct Spanners {
    objects: Vec<Py<PyAny>>,
    values: Vec<Spanning>,
}

impl Spanners {
    /// Where `spanner` stands, added where it is new.
    fn place_of(&mut self, spanner: &Bound<'_, PyAny>) -> PyResult<usize> {
        if let Some(at) = self.objects.iter().position(|held| held.is(spanner)) {
            return Ok(at);
        }
        let class: String = spanner.get_type().name()?.extract()?;
        let kind = match class.as_str() {
            "Crescendo" => SpannerKind::Crescendo,
            "Diminuendo" => SpannerKind::Diminuendo,
            _ => SpannerKind::Slur,
        };
        self.objects.push(spanner.clone().unbind());
        self.values.push(Spanning::new(kind));
        Ok(self.objects.len() - 1)
    }
}

/// How much of a token is read into the crate's value.
#[derive(Clone, Copy, PartialEq)]
enum Depth {
    /// Its kind and text, and a bar's or metadata's fields: what cutting a
    /// handler asks.
    Kind,
    /// Everything it holds.
    Whole,
}

/// A note's tie, by music21's name.
fn tie_named(name: Option<String>) -> Option<TieType> {
    TieType::from_name(&name?).ok()
}

/// A key signature or key's sharps, as the crate's signature.
fn key_value(key: &Bound<'_, PyAny>) -> PyResult<Option<music21::KeySignature>> {
    if key.is_none() {
        return Ok(None);
    }
    let sharps: i32 = key.getattr("sharps")?.extract()?;
    Ok(Some(music21::KeySignature::new(sharps)))
}

/// A music21 `Tuplet`, as the crate's.
fn tuplet_value(tuplet: &Bound<'_, PyAny>) -> Option<music21::duration::Tuplet> {
    tuplet
        .extract::<PyRef<'_, crate::duration::Tuplet>>()
        .ok()
        .map(|held| held.value())
}

/// An `ABCNote` or `ABCChord` read into the crate's note.
fn note_value(
    token: &Bound<'_, PyAny>,
    depth: Depth,
    spanners: &mut Spanners,
) -> PyResult<AbcNote> {
    let src = text(token, "src")?;
    let mut note = if token.is_instance_of::<ABCChord>() {
        AbcNote::chord(&src)
    } else {
        AbcNote::new(&src, &text(token, "carriedAccidental")?)
    };
    if depth == Depth::Kind {
        return Ok(note);
    }
    note.chord_symbols = token.getattr("chordSymbols")?.extract()?;
    note.in_grace = token.getattr("inGrace")?.is_truthy()?;
    note.default_quarter_length = optional!(token, "activeDefaultQuarterLength");
    let broken = token.getattr("brokenRhythmMarker")?;
    if !broken.is_none() {
        let (symbol, side): (String, String) = broken.extract()?;
        let side = if side == "left" {
            Side::Left
        } else {
            Side::Right
        };
        note.broken_rhythm = Some((symbol, side));
    }
    note.key = key_value(&token.getattr("activeKeySignature")?)?;
    note.tuplet = tuplet_value(&token.getattr("activeTuplet")?);
    for spanner in token.getattr("applicableSpanners")?.try_iter()? {
        note.spanners.push(spanners.place_of(&spanner?)?);
    }
    note.tie = tie_named(optional!(token, "tie"));
    note.articulations = token.getattr("articulations")?.extract()?;
    note.accidental_display = optional!(token, "accidentalDisplayStatus");
    note.is_rest = token.getattr("isRest")?.is_truthy()?;
    note.pitch_name = optional!(token, "pitchName");
    note.quarter_length = token.getattr("quarterLength")?.extract()?;
    if let Ok(subs) = token.getattr("subTokens") {
        for sub in subs.try_iter()? {
            note.sub_tokens.push(note_value(&sub?, depth, spanners)?);
        }
    }
    Ok(note)
}

/// The spanner a slur's, crescendo's or diminuendo's opening holds.
fn opened(
    token: &Bound<'_, PyAny>,
    member: &str,
    spanners: &mut Spanners,
) -> PyResult<Option<usize>> {
    let spanner = token.getattr(member)?;
    if spanner.is_none() {
        return Ok(None);
    }
    Ok(Some(spanners.place_of(&spanner)?))
}

/// A token of music21's read into the crate's.
fn token_value(token: &Bound<'_, PyAny>, depth: Depth, spanners: &mut Spanners) -> PyResult<Token> {
    let src = || text(token, "src");
    if token.is_instance_of::<ABCNote>() {
        return Ok(Token::Note(Box::new(note_value(token, depth, spanners)?)));
    }
    if token.is_instance_of::<ABCMetadata>() {
        return Ok(Token::Metadata(metadata_value(token)?));
    }
    if token.is_instance_of::<ABCBar>() {
        return Ok(Token::Bar(bar_value(token)?));
    }
    if token.is_instance_of::<ABCTuplet>() {
        return Ok(Token::Tuplet(tuplet_token(token)?));
    }
    if token.is_instance_of::<ABCSlurStart>() {
        let held = opened(token, "slurObj", spanners)?;
        return Ok(Token::SpannerStart(SpannerKind::Slur, held));
    }
    if token.is_instance_of::<ABCCrescStart>() {
        let held = opened(token, "crescObj", spanners)?;
        return Ok(Token::SpannerStart(SpannerKind::Crescendo, held));
    }
    if token.is_instance_of::<ABCDimStart>() {
        let held = opened(token, "dimObj", spanners)?;
        return Ok(Token::SpannerStart(SpannerKind::Diminuendo, held));
    }
    if token.is_instance_of::<ABCParenStop>() {
        let closes = if src()? == ")" {
            Closes::Slur
        } else {
            Closes::Hairpin
        };
        return Ok(Token::ParenStop(closes));
    }
    if token.is_instance_of::<ABCBrokenRhythmMarker>() {
        return Ok(Token::BrokenRhythm(BrokenRhythm {
            src: src()?,
            data: text(token, "data")?,
        }));
    }
    let fixed = [
        (token.is_instance_of::<ABCTie>(), Token::Tie),
        (token.is_instance_of::<ABCStaccato>(), Token::Staccato),
        (token.is_instance_of::<ABCUpbow>(), Token::Upbow),
        (token.is_instance_of::<ABCDownbow>(), Token::Downbow),
        (token.is_instance_of::<ABCAccent>(), Token::Accent),
        (token.is_instance_of::<ABCStraccent>(), Token::Straccent),
        (token.is_instance_of::<ABCTenuto>(), Token::Tenuto),
        (token.is_instance_of::<ABCGraceStart>(), Token::GraceStart),
        (token.is_instance_of::<ABCGraceStop>(), Token::GraceStop),
    ];
    match fixed.into_iter().find(|(is, _)| *is) {
        Some((_, value)) => Ok(value),
        None => Err(pyo3::exceptions::PyTypeError::new_err(format!(
            "an ABC handler holds tokens of a kind it reads, not {}",
            token.repr()?
        ))),
    }
}

fn metadata_value(token: &Bound<'_, PyAny>) -> PyResult<AbcMetadata> {
    Ok(AbcMetadata {
        src: text(token, "src")?,
        tag: text(token, "tag")?,
        data: text(token, "data")?,
        line: 0,
    })
}

fn bar_value(token: &Bound<'_, PyAny>) -> PyResult<Bar> {
    Ok(Bar {
        src: text(token, "src")?,
        bar_type: text(token, "barType")?,
        bar_style: text(token, "barStyle")?,
        repeat_form: text(token, "repeatForm")?,
    })
}

fn tuplet_token(token: &Bound<'_, PyAny>) -> PyResult<AbcTuplet> {
    let mut tuplet = AbcTuplet::new(&text(token, "src")?);
    tuplet.note_count = token.getattr("noteCount")?.extract()?;
    tuplet.actual = token.getattr("numberNotesActual")?.extract()?;
    tuplet.normal = token.getattr("numberNotesNormal")?.extract()?;
    Ok(tuplet)
}

/// The tokens of a list read into the crate's, and the spanners they hold.
fn tokens_value(tokens: &Bound<'_, PyAny>, depth: Depth) -> PyResult<(Vec<Token>, Spanners)> {
    let mut spanners = Spanners::default();
    let mut values = Vec::new();
    for token in tokens.try_iter()? {
        values.push(token_value(&token?, depth, &mut spanners)?);
    }
    Ok((values, spanners))
}

/// A handler of music21's read into the crate's.
fn handler_value(handler: &Bound<'_, PyAny>, depth: Depth) -> PyResult<(Handler, Spanners)> {
    let (tokens, spanners) = tokens_value(&handler.getattr("tokens")?, depth)?;
    let mut value = Handler::new(version_of(handler)?);
    value.tokens = tokens;
    for (key, item) in handler.getattr("abcDirectives")?.cast::<PyDict>()?.iter() {
        value
            .directives
            .push((key.str()?.to_string(), item.str()?.to_string()));
    }
    Ok((value, spanners))
}

/// The crate's handler and spanners for music21's `translate` to read.
pub(crate) fn processed_value(handler: &Bound<'_, PyAny>) -> PyResult<(Handler, Vec<Spanning>)> {
    let (value, spanners) = handler_value(handler, Depth::Whole)?;
    Ok((value, spanners.values))
}

/// The crate's note for music21's `translate` to read, and the spanner
/// objects its indices name.
pub(crate) fn note_for_translate(token: &Bound<'_, PyAny>) -> PyResult<(AbcNote, Vec<Py<PyAny>>)> {
    let mut spanners = Spanners::default();
    let note = note_value(token, Depth::Whole, &mut spanners)?;
    Ok((note, spanners.objects))
}

/// The crate's line of metadata for music21's `translate` to read.
pub(crate) fn metadata_for_translate(token: &Bound<'_, PyAny>) -> PyResult<AbcMetadata> {
    metadata_value(token)
}

/// A new token of music21's for a crate token, unread as `tokenize` makes
/// it.
fn token_object<'py>(py: Python<'py>, token: &Token) -> PyResult<Bound<'py, PyAny>> {
    let src = token.src();
    let class = match token {
        Token::Metadata(_) => py.get_type::<ABCMetadata>(),
        Token::Bar(_) => py.get_type::<ABCBar>(),
        Token::Tuplet(_) => py.get_type::<ABCTuplet>(),
        Token::Tie => py.get_type::<ABCTie>(),
        Token::SpannerStart(SpannerKind::Crescendo, _) => py.get_type::<ABCCrescStart>(),
        Token::SpannerStart(SpannerKind::Diminuendo, _) => py.get_type::<ABCDimStart>(),
        Token::SpannerStart(..) => py.get_type::<ABCSlurStart>(),
        Token::ParenStop(_) => py.get_type::<ABCParenStop>(),
        Token::Staccato => py.get_type::<ABCStaccato>(),
        Token::Upbow => py.get_type::<ABCUpbow>(),
        Token::Downbow => py.get_type::<ABCDownbow>(),
        Token::Accent => py.get_type::<ABCAccent>(),
        Token::Straccent => py.get_type::<ABCStraccent>(),
        Token::Tenuto => py.get_type::<ABCTenuto>(),
        Token::GraceStart => py.get_type::<ABCGraceStart>(),
        Token::GraceStop => py.get_type::<ABCGraceStop>(),
        Token::BrokenRhythm(_) => py.get_type::<ABCBrokenRhythmMarker>(),
        Token::Note(note) if note.is_chord => py.get_type::<ABCChord>(),
        Token::Note(note) => {
            return py
                .get_type::<ABCNote>()
                .call1((src, note.carried_accidental.as_str()));
        }
    };
    class.call1((src,))
}

/// What `parse` reads into a note written back onto it.
fn write_parsed(token: &Bound<'_, PyAny>, note: &AbcNote) -> PyResult<()> {
    let py = token.py();
    set_all(
        token,
        &[
            ("chordSymbols", object(py, note.chord_symbols.clone())?),
            ("pitchName", object(py, note.pitch_name.clone())?),
            (
                "accidentalDisplayStatus",
                object(py, note.accidental_display)?,
            ),
            ("isRest", object(py, note.is_rest)?),
            ("quarterLength", quarters(py, note.quarter_length)?),
        ],
    )?;
    if !note.is_chord {
        return Ok(());
    }

    // A chord's notes, each new and read alone, as music21 makes them.
    let subs = PyList::empty(py);
    for sub in &note.sub_tokens {
        let made = py
            .get_type::<ABCNote>()
            .call1((sub.src.as_str(), sub.carried_accidental.as_str()))?;
        write_parsed(&made, sub)?;
        subs.append(made)?;
    }
    token.setattr("subTokens", subs)
}

/// The shared objects `tokenProcess` hands notes as it walks the tokens.
struct InForce<'py> {
    spanners: Vec<Bound<'py, PyAny>>,
    key: Bound<'py, PyAny>,
    tuplet: Bound<'py, PyAny>,
}

/// What `tokenProcess` reads into a note written back onto it.
fn write_context(token: &Bound<'_, PyAny>, note: &AbcNote, held: &InForce<'_>) -> PyResult<()> {
    let py = token.py();
    let spanners = PyList::empty(py);
    for at in &note.spanners {
        spanners.append(&held.spanners[*at])?;
    }
    let key = if note.key.is_some() {
        held.key.clone()
    } else {
        none(py)
    };
    set_all(
        token,
        &[
            (
                "activeDefaultQuarterLength",
                object(py, note.default_quarter_length)?,
            ),
            ("activeKeySignature", key),
            ("applicableSpanners", spanners.into_any()),
            ("tie", object(py, note.tie.map(TieType::as_str))?),
            ("articulations", object(py, note.articulations.clone())?),
        ],
    )?;
    if note.in_grace {
        token.setattr("inGrace", true)?;
    }
    if note.tuplet.is_some() {
        token.setattr("activeTuplet", &held.tuplet)?;
    }
    if let Some((symbol, side)) = &note.broken_rhythm {
        token.setattr("brokenRhythmMarker", (symbol.as_str(), side.name()))?;
    }
    write_parsed(token, note)
}

/// A new spanner of music21's of `kind`.
fn spanner_object(py: Python<'_>, kind: SpannerKind) -> PyResult<Bound<'_, PyAny>> {
    match kind {
        SpannerKind::Crescendo => music21_name(py, "music21.dynamics", "Crescendo")?.call0(),
        SpannerKind::Diminuendo => music21_name(py, "music21.dynamics", "Diminuendo")?.call0(),
        _ => music21_name(py, "music21.spanner", "Slur")?.call0(),
    }
}

/// The member a spanner's opening keeps its spanner in.
fn opening_member(kind: SpannerKind) -> &'static str {
    match kind {
        SpannerKind::Crescendo => "crescObj",
        SpannerKind::Diminuendo => "dimObj",
        _ => "slurObj",
    }
}

/// The name music21 keeps in `activeParens` for a spanner.
fn paren_name(kind: SpannerKind) -> &'static str {
    match kind {
        SpannerKind::Crescendo => "Crescendo",
        SpannerKind::Diminuendo => "Diminuendo",
        _ => "Slur",
    }
}

/// The handler's processed tokens written back onto music21's, each
/// spanner, key and tuplet made once and shared as music21 shares it.
fn write_processed(
    handler: &Bound<'_, PyAny>,
    value: &Handler,
    spanners: &[Spanning],
) -> PyResult<()> {
    let py = handler.py();
    let tokens = handler.getattr("tokens")?;
    let made: Vec<Bound<'_, PyAny>> = spanners
        .iter()
        .map(|spanner| spanner_object(py, spanner.kind))
        .collect::<PyResult<_>>()?;
    let mut held = InForce {
        spanners: made,
        key: none(py),
        tuplet: none(py),
    };
    for (token, read) in tokens.try_iter()?.zip(&value.tokens) {
        let token = token?;
        match read {
            Token::Metadata(line) => {
                token.setattr("tag", &line.tag)?;
                token.setattr("data", &line.data)?;
                if line.tag == "K" {
                    held.key = element(py, line.key().map_err(|error| refusal(&error))?)?;
                }
            }
            Token::Bar(bar) => write_bar(&token, bar)?,
            Token::BrokenRhythm(mark) => token.setattr("data", &mark.data)?,
            Token::Tuplet(tuplet) => {
                held.tuplet = write_tuplet(&token, tuplet)?;
            }
            Token::SpannerStart(kind, Some(at)) => {
                token.setattr(opening_member(*kind), &held.spanners[*at])?;
            }
            Token::Note(note) => write_context(&token, note, &held)?,
            _ => {}
        }
    }

    // What is still open when the tokens end.
    let active = PyList::empty(py);
    let parens = PyList::empty(py);
    for at in &value.active {
        active.append(&held.spanners[*at])?;
        parens.append(paren_name(spanners[*at].kind))?;
    }
    handler.setattr("activeSpanners", active)?;
    handler.setattr("activeParens", parens)
}

fn write_bar(token: &Bound<'_, PyAny>, bar: &Bar) -> PyResult<()> {
    token.setattr("barType", &bar.bar_type)?;
    token.setattr("barStyle", &bar.bar_style)?;
    token.setattr("repeatForm", &bar.repeat_form)
}

/// A tuplet's counts written back, and its `Tuplet` made where it has a
/// ratio, which is handed back.
fn write_tuplet<'py>(token: &Bound<'py, PyAny>, tuplet: &AbcTuplet) -> PyResult<Bound<'py, PyAny>> {
    let py = token.py();
    token.setattr("noteCount", tuplet.note_count)?;
    token.setattr("numberNotesActual", tuplet.actual)?;
    token.setattr("numberNotesNormal", tuplet.normal)?;
    if tuplet.actual == -1 {
        return Ok(none(py));
    }
    let made = tuplet_object(
        py,
        music21::duration::Tuplet::ratio(tuplet.actual as u32, tuplet.normal.max(0) as u32),
    )?;
    token.setattr("tupletObj", &made)?;
    Ok(made)
}

// ---------------------------------------------------------------- tokens

/// music21's `ABCToken`: what every token has, its source text.
#[pyclass(
    name = "ABCToken",
    module = "music21.abcFormat",
    extends = crate::prebase::ProtoM21Object,
    subclass,
    dict
)]
pub struct ABCToken;

#[pymethods]
impl ABCToken {
    #[new]
    #[pyo3(signature = (*args, **kwargs))]
    fn new(
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyClassInitializer<Self> {
        let _ = (args, kwargs);
        PyClassInitializer::from(crate::prebase::ProtoM21Object::default()).add_subclass(Self)
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        crate::prebase::copied(slf.as_any(), None)
    }

    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        memo: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        crate::prebase::copied(slf.as_any(), Some(memo))
    }

    #[pyo3(signature = (src = ""))]
    fn __init__(slf: &Bound<'_, Self>, src: &str) -> PyResult<()> {
        slf.setattr("src", src)
    }

    /// music21's `_reprInternal`: the source text's repr.
    fn _reprInternal(slf: &Bound<'_, Self>) -> PyResult<String> {
        Ok(slf.getattr("src")?.repr()?.to_string())
    }

    /// music21's `EqualSlottedObjectMixin`: the same class, holding the same.
    fn __eq__(slf: &Bound<'_, Self>, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        if !slf.get_type().is(other.get_type()) {
            return Ok(false);
        }
        slf.getattr("__dict__")?.eq(other.getattr("__dict__")?)
    }

    /// music21's `stripComment`: the text before any `%`.
    #[staticmethod]
    fn stripComment(strSrc: &str) -> String {
        step::strip_comment(strSrc).to_string()
    }

    /// music21's `preParse`: nothing, for a token with nothing to read
    /// before its context is known.
    fn preParse(&self) {}

    /// music21's `parse`: nothing, for a token with nothing to read.
    fn parse(&self) {}
}

/// A token class with nothing of its own beyond `src`.
macro_rules! plain_token {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[pyclass(module = "music21.abcFormat", extends = ABCToken, subclass)]
        pub struct $name;

        #[pymethods]
        impl $name {
            #[new]
            #[pyo3(signature = (*args, **kwargs))]
            fn new(
                args: &Bound<'_, PyTuple>,
                kwargs: Option<&Bound<'_, PyDict>>,
            ) -> PyClassInitializer<Self> {
                ABCToken::new(args, kwargs).add_subclass(Self)
            }
        }
    };
}

plain_token!(
    ABCParenStop,
    "music21's `ABCParenStop`: `)`, or a hairpin's end."
);
plain_token!(ABCStaccato, "music21's `ABCStaccato`: `.` before a note.");
plain_token!(ABCUpbow, "music21's `ABCUpbow`: `u` before a note.");
plain_token!(ABCDownbow, "music21's `ABCDownbow`: `v` before a note.");
plain_token!(ABCAccent, "music21's `ABCAccent`: `K` before a note.");
plain_token!(ABCStraccent, "music21's `ABCStraccent`: `k` before a note.");
plain_token!(ABCTenuto, "music21's `ABCTenuto`: `M` before a note.");
plain_token!(ABCGraceStart, "music21's `ABCGraceStart`: `{`.");
plain_token!(ABCGraceStop, "music21's `ABCGraceStop`: `}`.");

/// A token class holding one member besides `src`, `None` at first, and
/// for an opening the step that fills it with a new spanner.
macro_rules! holding_token {
    ($name:ident, $doc:literal, $member:literal) => {
        holding_token!(@class $name, $doc, $member, {});
    };
    ($name:ident, $doc:literal, $member:literal, $fill:ident, $kind:expr) => {
        holding_token!(@class $name, $doc, $member, {
            /// music21's fill: a new spanner for the notes after it.
            fn $fill(slf: &Bound<'_, Self>) -> PyResult<()> {
                slf.setattr($member, spanner_object(slf.py(), $kind)?)
            }
        });
    };
    (@class $name:ident, $doc:literal, $member:literal, { $($extra:tt)* }) => {
        #[doc = $doc]
        #[pyclass(module = "music21.abcFormat", extends = ABCToken, subclass)]
        pub struct $name;

        #[pymethods]
        impl $name {
            #[new]
            #[pyo3(signature = (*args, **kwargs))]
            fn new(
                args: &Bound<'_, PyTuple>,
                kwargs: Option<&Bound<'_, PyDict>>,
            ) -> PyClassInitializer<Self> {
                ABCToken::new(args, kwargs).add_subclass(Self)
            }

            #[pyo3(signature = (src = ""))]
            fn __init__(slf: &Bound<'_, Self>, src: &str) -> PyResult<()> {
                slf.setattr("src", src)?;
                slf.setattr($member, slf.py().None())
            }

            $($extra)*
        }
    };
}

holding_token!(ABCTie, "music21's `ABCTie`: `-` between notes.", "noteObj");
holding_token!(
    ABCSlurStart,
    "music21's `ABCSlurStart`: `(` opening a slur.",
    "slurObj",
    fillSlur,
    SpannerKind::Slur
);
holding_token!(
    ABCCrescStart,
    "music21's `ABCCrescStart`: `!crescendo(!`.",
    "crescObj",
    fillCresc,
    SpannerKind::Crescendo
);
holding_token!(
    ABCDimStart,
    "music21's `ABCDimStart`: `!diminuendo(!`.",
    "dimObj",
    fillDim,
    SpannerKind::Diminuendo
);

/// music21's `ABCBrokenRhythmMarker`: `>` to `<<<` between notes.
#[pyclass(module = "music21.abcFormat", extends = ABCToken, subclass)]
pub struct ABCBrokenRhythmMarker;

#[pymethods]
impl ABCBrokenRhythmMarker {
    #[new]
    #[pyo3(signature = (*args, **kwargs))]
    fn new(
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyClassInitializer<Self> {
        ABCToken::new(args, kwargs).add_subclass(Self)
    }

    #[pyo3(signature = (src = ""))]
    fn __init__(slf: &Bound<'_, Self>, src: &str) -> PyResult<()> {
        slf.setattr("src", src)?;
        slf.setattr("data", "")
    }

    /// music21's `preParse`: the mark, trimmed, as `data`.
    fn preParse(slf: &Bound<'_, Self>) -> PyResult<()> {
        let mut mark = BrokenRhythm {
            src: text(slf.as_any(), "src")?,
            data: String::new(),
        };
        mark.pre_parse();
        slf.setattr("data", mark.data)
    }
}

/// music21's `ABCMetadata`: a line such as `T:Title`, its `tag` and `data`
/// empty until `preParse`.
#[pyclass(module = "music21.abcFormat", extends = ABCToken, subclass)]
pub struct ABCMetadata;

impl ABCMetadata {
    fn value(slf: &Bound<'_, Self>) -> PyResult<AbcMetadata> {
        metadata_value(slf.as_any())
    }

    fn tagged(slf: &Bound<'_, Self>, tag: &str) -> PyResult<bool> {
        Ok(text(slf.as_any(), "tag")? == tag)
    }
}

#[pymethods]
impl ABCMetadata {
    #[new]
    #[pyo3(signature = (*args, **kwargs))]
    fn new(
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyClassInitializer<Self> {
        ABCToken::new(args, kwargs).add_subclass(Self)
    }

    #[pyo3(signature = (src = ""))]
    fn __init__(slf: &Bound<'_, Self>, src: &str) -> PyResult<()> {
        slf.setattr("src", src)?;
        slf.setattr("tag", "")?;
        slf.setattr("data", "")
    }

    /// music21's `preParse`: the tag and the data split apart.
    fn preParse(slf: &Bound<'_, Self>) -> PyResult<()> {
        let mut line = Self::value(slf)?;
        line.pre_parse();
        slf.setattr("tag", line.tag)?;
        slf.setattr("data", line.data)
    }

    /// music21's `isDefaultNoteLength`: an `L:` line.
    fn isDefaultNoteLength(slf: &Bound<'_, Self>) -> PyResult<bool> {
        Self::tagged(slf, "L")
    }

    /// music21's `isReferenceNumber`: an `X:` line.
    fn isReferenceNumber(slf: &Bound<'_, Self>) -> PyResult<bool> {
        Self::tagged(slf, "X")
    }

    /// music21's `isMeter`: an `M:` line.
    fn isMeter(slf: &Bound<'_, Self>) -> PyResult<bool> {
        Self::tagged(slf, "M")
    }

    /// music21's `isTitle`: a `T:` line.
    fn isTitle(slf: &Bound<'_, Self>) -> PyResult<bool> {
        Self::tagged(slf, "T")
    }

    /// music21's `isComposer`: a `C:` line.
    fn isComposer(slf: &Bound<'_, Self>) -> PyResult<bool> {
        Self::tagged(slf, "C")
    }

    /// music21's `isOrigin`: an `O:` line.
    fn isOrigin(slf: &Bound<'_, Self>) -> PyResult<bool> {
        Self::tagged(slf, "O")
    }

    /// music21's `isVoice`: a `V:` line.
    fn isVoice(slf: &Bound<'_, Self>) -> PyResult<bool> {
        Self::tagged(slf, "V")
    }

    /// music21's `isKey`: a `K:` line.
    fn isKey(slf: &Bound<'_, Self>) -> PyResult<bool> {
        Self::tagged(slf, "K")
    }

    /// music21's `isTempo`: a `Q:` line.
    fn isTempo(slf: &Bound<'_, Self>) -> PyResult<bool> {
        Self::tagged(slf, "Q")
    }

    /// music21's `isVersion`: `I:abc-version ...`.
    fn isVersion(slf: &Bound<'_, Self>) -> PyResult<bool> {
        let line = Self::value(slf)?;
        Ok(line.tag == "I" && line.data.starts_with("abc-version "))
    }

    /// music21's `getTimeSignatureParameters`: numerator, denominator and
    /// symbol, or `None` for `M:none`.
    fn getTimeSignatureParameters(
        slf: &Bound<'_, Self>,
    ) -> PyResult<Option<(u32, u32, &'static str)>> {
        Self::value(slf)?
            .meter_parameters()
            .map_err(|error| refusal(&error))
    }

    /// music21's `getTimeSignatureObject`.
    fn getTimeSignatureObject<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        match Self::value(slf)?.meter().map_err(|error| refusal(&error))? {
            Some(meter) => element(py, meter.into()),
            None => Ok(none(py)),
        }
    }

    /// music21's `getKeySignatureParameters`: the sharps and the mode.
    fn getKeySignatureParameters(slf: &Bound<'_, Self>) -> PyResult<(i64, Option<&'static str>)> {
        let (sharps, mode) = Self::value(slf)?
            .key_parameters()
            .map_err(|error| refusal(&error))?;
        Ok((sharps.into(), mode))
    }

    /// music21's `getKeySignatureObject`: a key, or a signature where no
    /// mode is named.
    fn getKeySignatureObject<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let key = Self::value(slf)?.key().map_err(|error| refusal(&error))?;
        element(slf.py(), key)
    }

    /// music21's `getClefObject`: the clef a key line names and the
    /// transposition it implies, or `(None, None)`.
    fn getClefObject<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyTuple>> {
        let py = slf.py();
        let read = Self::value(slf)?.clef().map_err(|error| refusal(&error))?;
        match read {
            Some((clef, transposition)) => PyTuple::new(
                py,
                [
                    element(py, clef.into())?,
                    object(py, i64::from(transposition))?,
                ],
            ),
            None => PyTuple::new(py, [none(py), none(py)]),
        }
    }

    /// music21's `getMetronomeMarkObject`.
    fn getMetronomeMarkObject<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let mark = Self::value(slf)?.tempo().map_err(|error| refusal(&error))?;
        element(slf.py(), mark.into())
    }

    /// music21's `getDefaultQuarterLength`.
    fn getDefaultQuarterLength(slf: &Bound<'_, Self>) -> PyResult<f64> {
        Self::value(slf)?
            .default_quarter_length()
            .map_err(|error| refusal(&error))
    }
}

/// music21's `ABCBar`: a barline, its type and style empty until `parse`.
#[pyclass(module = "music21.abcFormat", extends = ABCToken, subclass)]
pub struct ABCBar;

#[pymethods]
impl ABCBar {
    #[new]
    #[pyo3(signature = (*args, **kwargs))]
    fn new(
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyClassInitializer<Self> {
        ABCToken::new(args, kwargs).add_subclass(Self)
    }

    #[pyo3(signature = (src = ""))]
    fn __init__(slf: &Bound<'_, Self>, src: &str) -> PyResult<()> {
        slf.setattr("src", src)?;
        write_bar(slf.as_any(), &Bar::new(src))
    }

    /// music21's `parse`: the type, style and repeat form its text names.
    fn parse(slf: &Bound<'_, Self>) -> PyResult<()> {
        let mut bar = bar_value(slf.as_any())?;
        bar.parse();
        write_bar(slf.as_any(), &bar)
    }

    /// music21's `isRepeat`.
    fn isRepeat(slf: &Bound<'_, Self>) -> PyResult<bool> {
        Ok(bar_value(slf.as_any())?.is_repeat())
    }

    /// music21's `isRegular`: a single light barline.
    fn isRegular(slf: &Bound<'_, Self>) -> PyResult<bool> {
        Ok(bar_value(slf.as_any())?.is_regular())
    }

    /// music21's `isRepeatBracket`: the ending it opens, or `False`.
    fn isRepeatBracket<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        match bar_value(slf.as_any())?.repeat_bracket() {
            Some(number) => object(py, number),
            None => object(py, false),
        }
    }

    /// music21's `getBarObject`: a barline or repeat, or `None` for a plain
    /// barline or an ending.
    fn getBarObject<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        match bar_value(slf.as_any())?.barline() {
            Some(barline) => Ok(crate::bar::Barline::object(py, barline)?.into_bound(py)),
            None => Ok(none(py)),
        }
    }
}

/// music21's `ABCTuplet`: a tuplet's opening, its ratio unknown until
/// `updateRatio`.
#[pyclass(module = "music21.abcFormat", extends = ABCToken, subclass)]
pub struct ABCTuplet;

#[pymethods]
impl ABCTuplet {
    #[new]
    #[pyo3(signature = (*args, **kwargs))]
    fn new(
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyClassInitializer<Self> {
        ABCToken::new(args, kwargs).add_subclass(Self)
    }

    #[pyo3(signature = (src = ""))]
    fn __init__(slf: &Bound<'_, Self>, src: &str) -> PyResult<()> {
        let tuplet = AbcTuplet::new(src);
        slf.setattr("src", src)?;
        slf.setattr("noteCount", tuplet.note_count)?;
        slf.setattr("numberNotesActual", tuplet.actual)?;
        slf.setattr("numberNotesNormal", tuplet.normal)?;
        slf.setattr("tupletObj", slf.py().None())
    }

    /// music21's `updateRatio`: the ratio, compound meters taking three in
    /// the time of 5, 7 and 9.
    #[pyo3(signature = (timeSignatureObj = None))]
    fn updateRatio(
        slf: &Bound<'_, Self>,
        timeSignatureObj: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let meter = match timeSignatureObj.filter(|given| !given.is_none()) {
            Some(given) => {
                let numerator: u32 = given.getattr("numerator")?.extract()?;
                let denominator: u32 = given.getattr("denominator")?.extract()?;
                Some(
                    music21::TimeSignature::new(numerator, denominator)
                        .map_err(|error| refusal(&error))?,
                )
            }
            None => None,
        };
        let mut tuplet = tuplet_token(slf.as_any())?;
        tuplet
            .update_ratio(meter.as_ref())
            .map_err(|error| refusal(&error))?;
        slf.setattr("numberNotesActual", tuplet.actual)?;
        slf.setattr("numberNotesNormal", tuplet.normal)
    }

    /// music21's `updateNoteCount`: how many notes the tuplet takes, and
    /// its `Tuplet`.
    fn updateNoteCount(slf: &Bound<'_, Self>) -> PyResult<()> {
        let mut tuplet = tuplet_token(slf.as_any())?;
        let made = tuplet
            .update_note_count()
            .map_err(|error| refusal(&error))?;
        slf.setattr("noteCount", tuplet.note_count)?;
        slf.setattr("tupletObj", tuplet_object(slf.py(), made)?)
    }
}

/// music21's `ABCNote`: a note or rest, its context filled by the handler
/// and its pitch and length by `parse`.
#[pyclass(module = "music21.abcFormat", extends = ABCToken, subclass)]
pub struct ABCNote;

/// The attributes a new `ABCNote` starts with beyond `src` and
/// `carriedAccidental`.
fn fresh_note(token: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = token.py();
    set_all(
        token,
        &[
            ("chordSymbols", PyList::empty(py).into_any()),
            ("inBar", none(py)),
            ("inBeam", none(py)),
            ("inGrace", none(py)),
            ("activeDefaultQuarterLength", none(py)),
            ("brokenRhythmMarker", none(py)),
            ("activeKeySignature", none(py)),
            ("activeTuplet", none(py)),
            ("applicableSpanners", PyList::empty(py).into_any()),
            ("tie", none(py)),
            ("articulations", PyList::empty(py).into_any()),
            ("accidentalDisplayStatus", none(py)),
            ("isRest", object(py, false)?),
            ("pitchName", none(py)),
            ("quarterLength", object(py, 0.0)?),
        ],
    )
}

#[pymethods]
impl ABCNote {
    #[new]
    #[pyo3(signature = (*args, **kwargs))]
    fn new(
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyClassInitializer<Self> {
        ABCToken::new(args, kwargs).add_subclass(Self)
    }

    #[pyo3(signature = (src = "", carriedAccidental = ""))]
    fn __init__(slf: &Bound<'_, Self>, src: &str, carriedAccidental: &str) -> PyResult<()> {
        slf.setattr("src", src)?;
        slf.setattr("carriedAccidental", carriedAccidental)?;
        fresh_note(slf.as_any())
    }

    /// music21's `_splitChordSymbols`: the quoted chord symbols, and what
    /// follows the last.
    #[staticmethod]
    fn _splitChordSymbols(strSrc: &str) -> (Vec<String>, String) {
        AbcNote::split_chord_symbols(strSrc)
    }

    /// music21's `getPitchName`: the pitch name, or `None` for a rest, and
    /// whether the accidental is shown.
    #[pyo3(signature = (strSrc, forceKeySignature = None))]
    fn getPitchName(
        slf: &Bound<'_, Self>,
        strSrc: &str,
        forceKeySignature: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<(Option<String>, Option<bool>)> {
        let mut spanners = Spanners::default();
        let note = note_value(slf.as_any(), Depth::Whole, &mut spanners)?;
        let forced = match forceKeySignature {
            Some(given) => key_value(given)?,
            None => None,
        };
        note.pitch_name(strSrc, forced.as_ref())
            .map_err(|error| ABCHandlerException::new_err(crate::pitch::message(&error)))
    }

    /// music21's `getQuarterLength`: the length `strSrc` writes against the
    /// default length, stretched by a broken rhythm.
    #[pyo3(signature = (strSrc, forceDefaultQuarterLength = None))]
    fn getQuarterLength<'py>(
        slf: &Bound<'py, Self>,
        strSrc: &str,
        forceDefaultQuarterLength: Option<f64>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let mut spanners = Spanners::default();
        let note = note_value(slf.as_any(), Depth::Whole, &mut spanners)?;
        let length = note
            .quarter_length_of(strSrc, forceDefaultQuarterLength)
            .map_err(|error| refusal(&error))?;
        quarters(slf.py(), length)
    }

    /// music21's `parse`: the chord symbols, pitch and length, or for a
    /// chord its notes and length.
    #[pyo3(signature = (forceDefaultQuarterLength = None, forceKeySignature = None))]
    fn parse(
        slf: &Bound<'_, Self>,
        forceDefaultQuarterLength: Option<f64>,
        forceKeySignature: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let mut spanners = Spanners::default();
        let mut note = note_value(slf.as_any(), Depth::Whole, &mut spanners)?;
        let forced = match forceKeySignature {
            Some(given) => key_value(given)?,
            None => None,
        };
        note.parse(forceDefaultQuarterLength, forced.as_ref())
            .map_err(|error| refusal(&error))?;
        write_parsed(slf.as_any(), &note)
    }
}

/// music21's `ABCChord`: notes in brackets, read into `subTokens` by
/// `parse`.
#[pyclass(module = "music21.abcFormat", extends = ABCNote, subclass)]
pub struct ABCChord;

#[pymethods]
impl ABCChord {
    #[new]
    #[pyo3(signature = (*args, **kwargs))]
    fn new(
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyClassInitializer<Self> {
        ABCNote::new(args, kwargs).add_subclass(Self)
    }

    #[pyo3(signature = (src = ""))]
    fn __init__(slf: &Bound<'_, Self>, src: &str) -> PyResult<()> {
        slf.setattr("src", src)?;
        slf.setattr("carriedAccidental", "")?;
        fresh_note(slf.as_any())?;
        slf.setattr("subTokens", PyList::empty(slf.py()))
    }
}

// --------------------------------------------------------------- handler

/// music21's `ABCHandler`: the tokens of a text and what reading them
/// keeps.
#[pyclass(module = "music21.abcFormat", subclass, dict)]
pub struct ABCHandler;

/// The attributes a new handler starts with.
fn fresh_handler(
    handler: &Bound<'_, PyAny>,
    version: &Bound<'_, PyAny>,
    phrases: bool,
) -> PyResult<()> {
    let py = handler.py();
    set_all(
        handler,
        &[
            ("abcVersion", version.clone()),
            ("abcDirectives", PyDict::new(py).into_any()),
            ("tokens", PyList::empty(py).into_any()),
            ("activeParens", PyList::empty(py).into_any()),
            ("activeSpanners", PyList::empty(py).into_any()),
            ("lineBreaksDefinePhrases", object(py, phrases)?),
            ("pos", object(py, -1)?),
            ("skipAhead", object(py, 0)?),
            ("strSrc", string(py, "")),
            ("srcLen", object(py, 0)?),
            ("currentCollectStr", string(py, "")),
        ],
    )
}

/// The default ABC version as music21's tuple.
fn default_version(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    object(py, step::DEFAULT_VERSION)
}

/// A new handler of `handler`'s own class, holding `tokens`.
fn same_class<'py>(
    handler: &Bound<'py, PyAny>,
    tokens: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let made = handler.get_type().call0()?;
    made.setattr("tokens", tokens)?;
    Ok(made)
}

/// Some of a handler's tokens, by index, as a new list.
fn picked<'py>(
    tokens: &Bound<'py, PyAny>,
    indices: impl IntoIterator<Item = usize>,
) -> PyResult<Bound<'py, PyAny>> {
    let list = PyList::empty(tokens.py());
    for at in indices {
        list.append(tokens.get_item(at)?)?;
    }
    Ok(list.into_any())
}

impl ABCHandler {
    /// The handler's tokens read for cutting, refused where there are none.
    fn cut_value(slf: &Bound<'_, PyAny>, refused: &str) -> PyResult<Handler> {
        if slf.getattr("tokens")?.len()? == 0 {
            return Err(ABCHandlerException::new_err(refused.to_owned()));
        }
        Ok(handler_value(slf, Depth::Kind)?.0)
    }
}

#[pymethods]
impl ABCHandler {
    #[new]
    #[pyo3(signature = (*args, **kwargs))]
    fn new(args: &Bound<'_, PyTuple>, kwargs: Option<&Bound<'_, PyDict>>) -> Self {
        let _ = (args, kwargs);
        Self
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        crate::prebase::copied(slf.as_any(), None)
    }

    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        memo: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        crate::prebase::copied(slf.as_any(), Some(memo))
    }

    #[pyo3(signature = (abcVersion = None, lineBreaksDefinePhrases = false))]
    fn __init__(
        slf: &Bound<'_, Self>,
        abcVersion: Option<&Bound<'_, PyAny>>,
        lineBreaksDefinePhrases: bool,
    ) -> PyResult<()> {
        let py = slf.py();
        let version = match abcVersion {
            Some(given) => given.clone(),
            None => default_version(py)?,
        };
        fresh_handler(slf.as_any(), &version, lineBreaksDefinePhrases)
    }

    /// music21's `_getLinearContext`: the items before, at, after and two
    /// after `i`.
    #[staticmethod]
    fn _getLinearContext<'py>(
        source: &Bound<'py, PyAny>,
        i: usize,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let items: Vec<Bound<'py, PyAny>> = source.try_iter()?.collect::<PyResult<_>>()?;
        let (before, here, after, later) =
            step::linear_context(&items, i).map_err(|error| refusal(&error))?;
        let py = source.py();
        let or_none = |item: Option<Bound<'py, PyAny>>| item.unwrap_or_else(|| none(py));
        PyTuple::new(py, [or_none(before), here, or_none(after), or_none(later)])
    }

    /// music21's `_getNextLineBreak`: where the next line break after `i`
    /// stands, or the length.
    #[staticmethod]
    fn _getNextLineBreak(strSrc: &str, i: usize) -> usize {
        let chars: Vec<char> = strSrc.chars().collect();
        step::next_line_break(&chars, i)
    }

    /// music21's `barlineTokenFilter`: a bar written as two in one, as two
    /// tokens.
    #[staticmethod]
    fn barlineTokenFilter<'py>(py: Python<'py>, token: &str) -> PyResult<Bound<'py, PyList>> {
        let made = PyList::empty(py);
        for bar in Handler::bar_tokens(token) {
            made.append(token_object(py, &Token::Bar(bar))?)?;
        }
        Ok(made)
    }

    /// music21's `_accidentalPropagation`: `not`, `pitch` or `octave`.
    fn _accidentalPropagation(slf: &Bound<'_, Self>) -> PyResult<String> {
        let (handler, _) = handler_value(slf.as_any(), Depth::Kind)?;
        Ok(handler.accidental_propagation().to_string())
    }

    /// music21's `parseHeaderForVersionInformation`: the version `%abc-2.1`
    /// names.
    fn parseHeaderForVersionInformation(slf: &Bound<'_, Self>, inputSearch: &str) -> PyResult<()> {
        let Some(version) = step::version_in(inputSearch) else {
            return Ok(());
        };
        slf.setattr("abcVersion", version)
    }

    /// music21's `returnAbcVersionFromMatch`: a match's three numbers, the
    /// third nought where missing.
    #[staticmethod]
    fn returnAbcVersionFromMatch(verMats: &Bound<'_, PyAny>) -> PyResult<(i64, i64, i64)> {
        let number = |group: usize| -> PyResult<Option<i64>> {
            let found = verMats.call_method1("group", (group,))?;
            if found.is_none() || found.str()?.to_str()?.is_empty() {
                return Ok(None);
            }
            Ok(Some(found.str()?.to_str()?.parse().map_err(|_| {
                pyo3::exceptions::PyValueError::new_err("an ABC version is not a number")
            })?))
        };
        Ok((
            number(1)?.unwrap_or(0),
            number(2)?.unwrap_or(0),
            number(3)?.unwrap_or(0),
        ))
    }

    /// music21's `processComment`: how far the comment at `pos` runs, and
    /// a directive it sets.
    fn processComment(slf: &Bound<'_, Self>) -> PyResult<()> {
        let source: Vec<char> = text(slf.as_any(), "strSrc")?.chars().collect();
        let pos: usize = slf.getattr("pos")?.extract()?;
        let comment = step::comment_at(&source, pos);
        slf.setattr("skipAhead", comment.skip)?;
        if let Some((key, value)) = comment.directive {
            slf.getattr("abcDirectives")?.set_item(key, value)?;
        }
        Ok(())
    }

    /// music21's `startsMetadata`: a capital letter or `w`, a colon, then
    /// anything but a barline.
    #[staticmethod]
    #[pyo3(signature = (c, cNext, cNextNext))]
    fn startsMetadata(c: &str, cNext: Option<&str>, cNextNext: Option<&str>) -> bool {
        let first = |given: Option<&str>| given.and_then(|text| text.chars().next());
        let Some(c) = first(Some(c)) else {
            return false;
        };
        step::starts_metadata(c, first(cNext), first(cNextNext))
    }

    /// music21's `tokenize`: the tokens of `strSrc` added to the handler's.
    fn tokenize(slf: &Bound<'_, Self>, strSrc: &str) -> PyResult<()> {
        let py = slf.py();
        let (mut handler, _) = handler_value(slf.as_any(), Depth::Kind)?;
        handler.tokens.clear();
        handler.tokenize(strSrc).map_err(|error| refusal(&error))?;
        let tokens = slf.getattr("tokens")?;
        for token in &handler.tokens {
            tokens.call_method1("append", (token_object(py, token)?,))?;
        }

        // What reading the text leaves on the handler.
        let directives = slf.getattr("abcDirectives")?;
        for (key, value) in &handler.directives {
            directives.set_item(key, value)?;
        }
        let length = strSrc.chars().count();
        let last = handler.tokens.last().map(Token::src).unwrap_or_default();
        set_all(
            slf.as_any(),
            &[
                ("strSrc", string(py, strSrc)),
                ("srcLen", object(py, length)?),
                ("pos", object(py, length as i64 - 1)?),
                ("skipAhead", object(py, 0)?),
                ("currentCollectStr", string(py, &last)),
            ],
        )
    }

    /// music21's `tokenProcess`: each token pre-parsed, given its context,
    /// and parsed.
    fn tokenProcess(slf: &Bound<'_, Self>) -> PyResult<()> {
        let (mut handler, _) = handler_value(slf.as_any(), Depth::Whole)?;
        let spanners = handler.process().map_err(|error| refusal(&error))?;
        write_processed(slf.as_any(), &handler, &spanners)
    }

    /// music21's `process`: the version read, then the text tokenized and
    /// processed.
    fn process(slf: &Bound<'_, Self>, strSrc: &str) -> PyResult<()> {
        let head: String = strSrc.chars().take(100).collect();
        slf.call_method1("parseHeaderForVersionInformation", (head,))?;
        slf.setattr("tokens", PyList::empty(slf.py()))?;
        slf.call_method1("tokenize", (strSrc,))?;
        slf.call_method0("tokenProcess")?;
        Ok(())
    }

    fn __len__(slf: &Bound<'_, Self>) -> PyResult<usize> {
        slf.getattr("tokens")?.len()
    }

    /// music21's `__add__`: a new handler of the same class holding both's
    /// tokens.
    fn __add__<'py>(
        slf: &Bound<'py, Self>,
        other: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let tokens = slf.getattr("tokens")?.add(other.getattr("tokens")?)?;
        same_class(slf.as_any(), tokens)
    }

    /// music21's `definesReferenceNumbers`: more than one `X:` line.
    fn definesReferenceNumbers(slf: &Bound<'_, Self>) -> PyResult<bool> {
        Ok(Self::cut_value(slf.as_any(), NO_TOKENS)?.defines_reference_numbers())
    }

    /// music21's `splitByReferenceNumber`: a handler for each tune by its
    /// number, the header in front of each; under `None` where no tune is
    /// numbered.
    fn splitByReferenceNumber<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyDict>> {
        let py = slf.py();
        let handler = Self::cut_value(slf.as_any(), NO_TOKENS)?;
        let tunes = handler
            .split_by_reference_number()
            .map_err(|error| refusal(&error))?;
        let tokens = slf.getattr("tokens")?;
        let version = slf.getattr("abcVersion")?;
        let made = PyDict::new(py);
        for (number, indices) in tunes {
            let tune = py.get_type::<ABCHandler>().call1((&version,))?;
            tune.setattr("tokens", picked(&tokens, indices)?)?;
            made.set_item(number, tune)?;
        }
        Ok(made)
    }

    /// music21's `getReferenceNumber`: the first `X:` line's data.
    fn getReferenceNumber(slf: &Bound<'_, Self>) -> PyResult<Option<String>> {
        let handler = Self::cut_value(slf.as_any(), NO_TOKENS)?;
        Ok(handler.reference_number().map(str::to_owned))
    }

    /// music21's `definesMeasures`: two plain barlines or more.
    fn definesMeasures(slf: &Bound<'_, Self>) -> PyResult<bool> {
        Ok(Self::cut_value(slf.as_any(), NO_TOKENS)?.defines_measures())
    }

    /// music21's `splitByVoice`: the header, then a handler for each voice.
    fn splitByVoice<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyList>> {
        let py = slf.py();
        let handler = Self::cut_value(slf.as_any(), NO_TOKENS)?;
        let tokens = slf.getattr("tokens")?;
        let pieces = handler.split_by_voice();
        let made = PyList::empty(py);
        if pieces.len() == 1 {
            // music21 hands the one voice the very list.
            made.append(same_class(slf.as_any(), tokens)?)?;
            return Ok(made);
        }
        for piece in pieces {
            made.append(same_class(slf.as_any(), picked(&tokens, piece)?)?)?;
        }
        Ok(made)
    }

    /// music21's `_buildMeasureBoundaryIndices`: each measure's first and
    /// last place from the places of the barlines.
    #[staticmethod]
    fn _buildMeasureBoundaryIndices(
        positionList: Vec<usize>,
        lastValidIndex: usize,
    ) -> Vec<[usize; 2]> {
        step::measure_boundaries(&positionList, lastValidIndex)
            .into_iter()
            .map(|(start, end)| [start, end])
            .collect()
    }

    /// music21's `splitByMeasure`: an `ABCHandlerBar` for each measure, with
    /// the bars either side.
    fn splitByMeasure<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyList>> {
        let py = slf.py();
        let handler = Self::cut_value(slf.as_any(), NO_TOKENS)?;
        let tokens = slf.getattr("tokens")?;
        let made = PyList::empty(py);
        for split in handler.split_by_measure() {
            let bar = py.get_type::<ABCHandlerBar>().call0()?;
            bar.setattr("tokens", picked(&tokens, split.tokens)?)?;
            if let Some(at) = split.left {
                bar.setattr("leftBarToken", tokens.get_item(at)?)?;
            }
            if let Some(at) = split.right {
                bar.setattr("rightBarToken", tokens.get_item(at)?)?;
            }
            made.append(bar)?;
        }
        Ok(made)
    }

    /// music21's `tokensToBarIndices`: where each bar stands, and each line
    /// of metadata a note follows.
    fn tokensToBarIndices(slf: &Bound<'_, Self>) -> PyResult<Vec<usize>> {
        Ok(handler_value(slf.as_any(), Depth::Kind)?.0.bar_indices())
    }

    /// music21's `hasNotes`.
    fn hasNotes(slf: &Bound<'_, Self>) -> PyResult<bool> {
        Ok(Self::cut_value(slf.as_any(), "must process tokens before calling")?.has_notes())
    }

    /// music21's `getTitle`: the first `T:` line's data.
    fn getTitle(slf: &Bound<'_, Self>) -> PyResult<Option<String>> {
        let handler = Self::cut_value(slf.as_any(), NO_TOKENS)?;
        Ok(handler.title().map(str::to_owned))
    }
}

/// music21's `ABCHandlerBar`: a measure's tokens and the bars either side.
#[pyclass(module = "music21.abcFormat", extends = ABCHandler, subclass)]
pub struct ABCHandlerBar;

#[pymethods]
impl ABCHandlerBar {
    #[new]
    #[pyo3(signature = (*args, **kwargs))]
    fn new(
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyClassInitializer<Self> {
        PyClassInitializer::from(ABCHandler::new(args, kwargs)).add_subclass(Self)
    }

    fn __init__(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py = slf.py();
        fresh_handler(slf.as_any(), &default_version(py)?, false)?;
        slf.setattr("leftBarToken", py.None())?;
        slf.setattr("rightBarToken", py.None())
    }

    /// music21's `ABCHandlerBar.__add__`: both's tokens, and each bar from
    /// the second where it has one.
    fn __add__<'py>(
        slf: &Bound<'py, Self>,
        other: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let tokens = slf.getattr("tokens")?.add(other.getattr("tokens")?)?;
        let made = same_class(slf.as_any(), tokens)?;
        for member in ["leftBarToken", "rightBarToken"] {
            let mine = slf.getattr(member)?;
            let theirs = other.getattr(member).unwrap_or_else(|_| none(slf.py()));
            made.setattr(member, if theirs.is_none() { mine } else { theirs })?;
        }
        Ok(made)
    }
}

/// music21's `mergeLeadingMetaData`: a handler of nothing but metadata
/// joined to the measure after it; a tune of one measure, one handler.
#[pyfunction]
fn mergeLeadingMetaData<'py>(
    py: Python<'py>,
    barHandlers: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyList>> {
    let handlers: Vec<Bound<'py, PyAny>> = barHandlers.try_iter()?.collect::<PyResult<_>>()?;
    let holding: Vec<bool> = handlers
        .iter()
        .map(|handler| handler.call_method0("hasNotes")?.is_truthy())
        .collect::<PyResult<_>>()?;
    let merged = PyList::empty(py);
    for merge in step::merge_plan(&holding) {
        let made = match merge {
            Merge::Whole => {
                let mut whole = py.get_type::<ABCHandlerBar>().call0()?.into_any();
                for handler in &handlers {
                    whole = whole.add(handler)?;
                }
                whole
            }
            Merge::Kept(at) => handlers[at].clone(),
            Merge::Pair(at) => handlers[at].add(&handlers[at + 1])?,
        };
        merged.append(made)?;
    }
    Ok(merged)
}

// ------------------------------------------------------------------ file

/// music21's `ABCFile`: ABC read from a file or a string.
#[pyclass(
    module = "music21.abcFormat",
    extends = crate::prebase::ProtoM21Object,
    subclass,
    dict
)]
pub struct ABCFile;

#[pymethods]
impl ABCFile {
    #[new]
    #[pyo3(signature = (*args, **kwargs))]
    fn new(
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyClassInitializer<Self> {
        let _ = (args, kwargs);
        PyClassInitializer::from(crate::prebase::ProtoM21Object::default()).add_subclass(Self)
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        crate::prebase::copied(slf.as_any(), None)
    }

    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        memo: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        crate::prebase::copied(slf.as_any(), Some(memo))
    }

    #[pyo3(signature = (abcVersion = None))]
    fn __init__(slf: &Bound<'_, Self>, abcVersion: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let py = slf.py();
        let version = match abcVersion {
            Some(given) => given.clone(),
            None => default_version(py)?,
        };
        slf.setattr("abcVersion", version)?;
        slf.setattr("file", py.None())?;
        slf.setattr("filename", "")
    }

    /// music21's `open`: the file opened for reading as UTF-8.
    fn open(slf: &Bound<'_, Self>, filename: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        let keywords = PyDict::new(py);
        keywords.set_item("encoding", "utf-8")?;
        let file = py
            .import("io")?
            .getattr("open")?
            .call((filename,), Some(&keywords))?;
        slf.setattr("file", file)?;
        slf.setattr("filename", filename)
    }

    /// music21's `openFileLike`: an open file-like object read from.
    fn openFileLike(slf: &Bound<'_, Self>, fileLike: &Bound<'_, PyAny>) -> PyResult<()> {
        slf.setattr("file", fileLike)
    }

    /// music21's `_reprInternal`: the filename, a placeholder for a
    /// file-like object, or nothing.
    fn _reprInternal(slf: &Bound<'_, Self>) -> PyResult<String> {
        let filename = slf.getattr("filename")?;
        if filename.is_truthy()? {
            return Ok(filename.str()?.to_string());
        }
        if !slf.getattr("file")?.is_none() {
            return Ok("<filelike-object>".to_owned());
        }
        Ok(String::new())
    }

    /// music21's `close`.
    fn close(slf: &Bound<'_, Self>) -> PyResult<()> {
        let file = slf.getattr("file")?;
        if !file.is_none() {
            file.call_method0("close")?;
        }
        Ok(())
    }

    /// music21's `read`: the open file read and processed, or the tune
    /// numbered `number` in it.
    #[pyo3(signature = (number = None))]
    fn read<'py>(
        slf: &Bound<'py, Self>,
        number: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let file = slf.getattr("file")?;
        if file.is_none() {
            return Err(ABCFileException::new_err(
                "cannot read; no file or file-like object is open",
            ));
        }
        let text = file.call_method0("read")?;
        slf.call_method1("readstr", (text, number))
    }

    /// music21's `extractReferenceNumber`: the lines of the tune numbered
    /// `number`.
    #[staticmethod]
    fn extractReferenceNumber(strSrc: &str, number: i32) -> PyResult<String> {
        step::extract_reference_number(strSrc, number)
            .map(|(_, tune)| tune)
            .map_err(|error| ABCFileException::new_err(crate::pitch::message(&error)))
    }

    /// music21's `readstr`: a processed handler of the text, or of the tune
    /// numbered `number` in it.
    #[pyo3(signature = (strSrc, number = None))]
    fn readstr<'py>(
        slf: &Bound<'py, Self>,
        strSrc: &str,
        number: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let source = match number.filter(|given| !given.is_none()) {
            Some(number) => Self::extractReferenceNumber(strSrc, number.extract()?)?,
            None => strSrc.to_owned(),
        };
        let version = slf.getattr("abcVersion")?;
        let handler = py.get_type::<ABCHandler>().call1((&version,))?;
        if version.eq(default_version(py)?)? {
            let head: String = source.chars().take(100).collect();
            handler.call_method1("parseHeaderForVersionInformation", (head,))?;
        }
        handler.call_method1("process", (source,))?;
        Ok(handler)
    }
}

// ---------------------------------------------------------------- module

/// The module music21 calls `abcFormat`, `translate` included.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, MODULE)?;
        m.add_class::<ABCToken>()?;
        m.add_class::<ABCMetadata>()?;
        m.add_class::<ABCBar>()?;
        m.add_class::<ABCTuplet>()?;
        m.add_class::<ABCTie>()?;
        m.add_class::<ABCSlurStart>()?;
        m.add_class::<ABCParenStop>()?;
        m.add_class::<ABCCrescStart>()?;
        m.add_class::<ABCDimStart>()?;
        m.add_class::<ABCStaccato>()?;
        m.add_class::<ABCUpbow>()?;
        m.add_class::<ABCDownbow>()?;
        m.add_class::<ABCAccent>()?;
        m.add_class::<ABCStraccent>()?;
        m.add_class::<ABCTenuto>()?;
        m.add_class::<ABCGraceStart>()?;
        m.add_class::<ABCGraceStop>()?;
        m.add_class::<ABCBrokenRhythmMarker>()?;
        m.add_class::<ABCNote>()?;
        m.add_class::<ABCChord>()?;
        m.add_class::<ABCHandler>()?;
        m.add_class::<ABCHandlerBar>()?;
        m.add_class::<ABCFile>()?;
        m.add_function(wrap_pyfunction!(mergeLeadingMetaData, &m)?)?;
        for (name, class) in [
            ("ABCTokenException", py.get_type::<ABCTokenException>()),
            ("ABCHandlerException", py.get_type::<ABCHandlerException>()),
            ("ABCFileException", py.get_type::<ABCFileException>()),
        ] {
            class.setattr("__module__", MODULE)?;
            m.add(name, class)?;
        }

        // music21's tables and patterns.
        let bars = PyList::empty(py);
        let named = PyDict::new(py);
        for (symbol, name) in step::BARS {
            bars.append((symbol, name))?;
            named.set_item(symbol, name)?;
        }
        m.add("ABC_BARS", bars)?;
        m.add("ABC_BARS_DICT", named)?;
        let re = py.import("re")?;
        for (name, pattern) in PATTERNS {
            m.add(name, re.call_method1("compile", (pattern,))?)?;
        }
        m.add("translate", crate::abctranslate::module(py)?)?;
        m.add(
            "environLocal",
            crate::environment::module(py)?
                .getattr("Environment")?
                .call1(("abcFormat",))?,
        )?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}
