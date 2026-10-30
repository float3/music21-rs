//! music21's `repeat`: the marks that say where the music goes back to
//! and where it ends.
//!
//! One `RepeatExpression` of the crate's stands for every one of music21's
//! eleven classes -- the kind says which -- so each class here is that
//! value under its own name, built with the text and the symbol music21
//! starts it with. What a mark *says* is the crate's: the words it may be
//! written with (`text_alternatives`), whether it is drawn as a symbol,
//! and where it is justified.
//!
//! `Expander` is not here. Playing a score's repeats out is one function
//! in the crate, `Stream::expand_repeats`, held to music21 by
//! `expand_repeats_parity` on every corpus file with repeats, and the
//! object music21 keeps that walk's state on has no counterpart.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::PyDict;

use music21_rs_crate::repeat::{
    RepeatExpression as RsRepeatExpression, RepeatExpressionKind as Kind,
};

pyo3::create_exception!(
    music21_rs_facade,
    RepeatExpressionException,
    crate::Music21Exception
);

/// music21's `repeat.RepeatExpression`: a mark saying where the music goes
/// back to, which this package holds as the one value every kind of it is.
#[pyclass(
    name = "RepeatExpression",
    module = "music21.repeat",
    subclass,
    skip_from_py_object
)]
pub struct RepeatExpression {
    pub(crate) inner: RsRepeatExpression,
}

impl RepeatExpression {
    /// The facade around a mark of the crate's.
    pub(crate) fn wrap(inner: RsRepeatExpression) -> Self {
        Self { inner }
    }

    /// A mark of a kind, with the text a caller gave where that is one of
    /// the words the kind may be written with -- which is how music21's
    /// own `__init__` reads the argument, falling back to the first of
    /// them and the symbol where it is not.
    fn of_kind(kind: Kind, text: Option<&str>) -> Self {
        let mut made = RsRepeatExpression::new(kind);
        if let Some(text) = text.filter(|text| made.is_valid_text(text)) {
            made.set_text(text);
            made.set_use_symbol(false);
        }
        Self::wrap(made)
    }
}

#[pymethods]
impl RepeatExpression {
    /// music21's `RepeatExpression()`, which is the base class its kinds
    /// extend; a mark with no kind is a `Coda`, as nothing else can be
    /// said of it.
    #[new]
    #[pyo3(signature = (**_keywords))]
    fn new(_keywords: Option<&Bound<'_, PyDict>>) -> Self {
        Self::wrap(RsRepeatExpression::new(Kind::Coda))
    }

    /// What music21 says this object is, its own chain down to
    /// `Music21Object`.
    #[getter]
    fn classes(&self) -> Vec<String> {
        let mut named = vec![self.inner.kind().class_name().to_string()];
        named.push(
            if self.inner.kind().is_marker() {
                "RepeatExpressionMarker"
            } else {
                "RepeatExpressionCommand"
            }
            .to_string(),
        );
        for name in [
            "RepeatExpression",
            "RepeatMark",
            "Expression",
            "Music21Object",
        ] {
            named.push(name.to_string());
        }
        named
    }

    /// The words the mark is written with: music21's `getText`.
    fn getText(&self) -> String {
        self.inner.text().to_string()
    }

    /// Writes the words, where they are ones the mark may be written with:
    /// music21's `setText`, which also stops it being drawn as a symbol.
    fn setText(&mut self, value: &str) {
        self.inner.set_text(value);
        self.inner.set_use_symbol(false);
    }

    /// Whether those are words this mark may be written with, whatever
    /// their case and however they are punctuated: music21's
    /// `isValidText`.
    fn isValidText(&self, value: &str) -> bool {
        self.inner.is_valid_text(value)
    }

    /// Whether the mark is drawn as its sign rather than written out:
    /// music21's `useSymbol`, which only a coda and a segno have.
    #[getter]
    fn useSymbol(&self) -> bool {
        self.inner.use_symbol()
    }

    #[setter]
    fn set_useSymbol(&mut self, #[pyo3(from_py_with = crate::truthy)] value: bool) {
        self.inner.set_use_symbol(value);
    }

    /// Which side of the staff the mark is drawn on, where the score says.
    #[getter]
    fn placement(&self) -> Option<&'static str> {
        self.inner.placement().map(|placement| placement.as_str())
    }

    /// music21's `_reprInternal`: the words it is written with, cut at
    /// sixteen characters.
    fn __repr__(&self) -> String {
        let text = self.inner.text();
        let said = if text.chars().count() > 16 {
            let short: String = text.chars().take(16).collect();
            format!("{short}...")
        } else {
            text.to_string()
        };
        format!(
            "<music21.repeat.{} {said:?}>",
            self.inner.kind().class_name()
        )
    }

    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        _memo: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let copied = slf.get_type().call0()?;
        {
            let me = slf.borrow();
            let mut copy = copied.extract::<PyRefMut<'_, Self>>()?;
            copy.inner = me.inner.clone();
        }
        Ok(copied)
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        Self::__deepcopy__(slf, None)
    }

    /// music21 freezes a score by pickling it, and what this object is
    /// lives in Rust where a pickle cannot see it.
    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<crate::Pickled> {
        crate::pickled(slf, &slf.borrow().inner)
    }

    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        if let Some(inner) = crate::unpickled::<_, RsRepeatExpression>(slf, state)? {
            slf.borrow_mut().inner = inner;
        }
        Ok(())
    }
}

/// music21's kinds of repeat mark, each the same `RepeatExpression` under
/// its own name.
macro_rules! repeat_kinds {
    ($(($class:ident, $name:literal, $kind:expr, $doc:literal)),* $(,)?) => {
        $(
            #[doc = $doc]
            #[pyclass(
                name = $name,
                module = "music21.repeat",
                extends = RepeatExpression,
                subclass
            )]
            pub struct $class;

            #[pymethods]
            impl $class {
                #[new]
                #[pyo3(signature = (text = None, **_keywords))]
                fn new(
                    text: Option<&str>,
                    _keywords: Option<&Bound<'_, PyDict>>,
                ) -> PyClassInitializer<Self> {
                    PyClassInitializer::from(RepeatExpression::of_kind($kind, text))
                        .add_subclass($class)
                }
            }
        )*

        /// Adds the repeat classes to a module.
        pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
            m.add_class::<RepeatExpression>()?;
            $(m.add_class::<$class>()?;)*
            Ok(())
        }

        /// The names this module hands out.
        pub const NAMES: &[&str] = &[
            "RepeatExpression",
            "RepeatExpressionException",
            $($name),*
        ];
    };
}

repeat_kinds!(
    (
        Coda,
        "Coda",
        Kind::Coda,
        "music21's `Coda`: the place a *to Coda* jumps to."
    ),
    (
        Segno,
        "Segno",
        Kind::Segno,
        "music21's `Segno`: the sign a *Dal Segno* goes back to."
    ),
    (
        Fine,
        "Fine",
        Kind::Fine,
        "music21's `Fine`: where the piece ends after a jump back."
    ),
    (
        DaCapo,
        "DaCapo",
        Kind::DaCapo,
        "music21's `DaCapo`: back to the start."
    ),
    (
        DaCapoAlFine,
        "DaCapoAlFine",
        Kind::DaCapoAlFine,
        "music21's `DaCapoAlFine`: back to the start and on to *Fine*."
    ),
    (
        DaCapoAlCoda,
        "DaCapoAlCoda",
        Kind::DaCapoAlCoda,
        "music21's `DaCapoAlCoda`: back to the start and on to the coda."
    ),
    (
        AlSegno,
        "AlSegno",
        Kind::AlSegno,
        "music21's `AlSegno`: on to the sign."
    ),
    (
        DalSegno,
        "DalSegno",
        Kind::DalSegno,
        "music21's `DalSegno`: back to the sign."
    ),
    (
        DalSegnoAlFine,
        "DalSegnoAlFine",
        Kind::DalSegnoAlFine,
        "music21's `DalSegnoAlFine`: back to the sign and on to *Fine*."
    ),
    (
        DalSegnoAlCoda,
        "DalSegnoAlCoda",
        Kind::DalSegnoAlCoda,
        "music21's `DalSegnoAlCoda`: back to the sign and on to the coda."
    ),
);
