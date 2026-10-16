//! music21's `analysis.windowed`: an analysis run over windows of a
//! stream, from a quarter note long to the whole of it.
//!
//! The windows are the crate's -- a stream cut into quarter-note measures,
//! with what runs past a barline tied, and the windows of each size put end
//! to end -- and the analysis is whatever `analysisProcessor` was handed in.
//! Each window is given to its `process` as a stream of this package's, so
//! the answer and the colour are the processor's own.
//!
//! **The colour is the processor's, which is why this carries `analyze` and
//! `process` where `discrete`'s own classes do not.** A window's answer is
//! `(data, colour)`, and nothing here knows the colour a key or a pitch
//! span is drawn in; whoever wrote the processor does. music21's own
//! processors work unchanged where music21 is installed, and so does a
//! processor a caller wrote.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyString, PyTuple};

use music21_rs_crate::Stream as RsStream;
use music21_rs_crate::analysis::windowed::{
    WindowStep, WindowType, WindowedAnalysis as RsWindowed,
};
use music21_rs_crate::makenotation::{make_measures_by, make_ties};
use music21_rs_crate::meter::TimeSignature as RsTimeSignature;
use music21_rs_crate::stream::StreamKind;

/// The names this module hands out.
pub const NAMES: &[&str] = &["WindowedAnalysis", "WindowedAnalysisException"];

pyo3::create_exception!(
    music21_rs_facade,
    WindowedAnalysisException,
    crate::Music21Exception
);

/// music21's `analysis.windowed.WindowedAnalysis`.
#[pyclass(
    name = "WindowedAnalysis",
    module = "music21.analysis.windowed",
    subclass,
    skip_from_py_object
)]
pub struct WindowedAnalysis {
    /// What each window is handed to: music21's `processor`.
    processor: Py<PyAny>,
    /// The stream being read, flattened where it holds parts, as music21
    /// flattens it -- part-like substreams are not supported there either.
    source: RsStream,
    /// The quarter-note measures every window is made of.
    windowed: RsWindowed,
}

#[pymethods]
impl WindowedAnalysis {
    /// music21's `WindowedAnalysis(streamObj, analysisProcessor)`, which
    /// refuses anything that is not a stream.
    #[new]
    fn make(streamObj: &Bound<'_, PyAny>, analysisProcessor: &Bound<'_, PyAny>) -> PyResult<Self> {
        if !streamObj
            .getattr("isStream")
            .and_then(|flag| flag.is_truthy())
            .unwrap_or(false)
        {
            return Err(WindowedAnalysisException::new_err(
                "non-stream provided as argument",
            ));
        }
        let read = crate::stream::crate_stream(streamObj)?;
        let source = if read.has_part_like_streams() {
            read.flatten()
        } else {
            read
        };
        let windowed = RsWindowed::new(&source)
            .map_err(|error| WindowedAnalysisException::new_err(crate::pitch::message(&error)))?;
        Ok(Self {
            processor: analysisProcessor.clone().unbind(),
            source,
            windowed,
        })
    }

    /// What each window is handed to: music21's `processor`.
    #[getter]
    fn get_processor(&self, py: Python<'_>) -> Py<PyAny> {
        self.processor.clone_ref(py)
    }

    #[setter]
    fn set_processor(&mut self, value: &Bound<'_, PyAny>) {
        self.processor = value.clone().unbind();
    }

    /// How many quarter-note windows the stream was cut into.
    #[getter]
    fn get__windowedStream<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        Self::windows_of(py, &self.source, "1/4")
    }

    /// The stream cut into measures one quarter note long, with what runs
    /// past a barline tied: music21's `getMinimumWindowStream`.
    #[pyo3(signature = (timeSignature = "1/4".to_string()))]
    fn getMinimumWindowStream<'py>(
        &self,
        py: Python<'py>,
        timeSignature: String,
    ) -> PyResult<Bound<'py, PyAny>> {
        Self::windows_of(py, &self.source, &timeSignature)
    }

    /// The analysis run over every window of `windowSize` quarters: the
    /// answers and the colours, as music21's `analyze` answers them.
    ///
    /// A window the processor cannot read -- one of nothing but rests --
    /// answers `((None, None, 0), '#ffffff')`, which is what music21 puts
    /// there when its processor raises.
    #[pyo3(signature = (windowSize, windowType = "overlap".to_string()))]
    fn analyze<'py>(
        slf: &Bound<'py, Self>,
        windowSize: usize,
        windowType: String,
    ) -> PyResult<(Bound<'py, PyList>, Bound<'py, PyList>)> {
        let py = slf.py();
        let kind = exact_window_type(&windowType)?;
        let me = slf.borrow();
        if kind == WindowType::NoOverlap
            && windowSize > 0
            && !me.windowed.window_count().is_multiple_of(windowSize)
        {
            // music21 warns and carries on, so this does too.
            py.import("warnings")?.call_method1(
                "warn",
                ("maxWindowCount is not divisible by windowSize, possibly undefined behavior",),
            )?;
        }
        let processor = me.processor.bind(py);
        let found = me
            .windowed
            .analyze(windowSize, kind, |window| asked(processor, window));
        let data = PyList::empty(py);
        let colours = PyList::empty(py);
        for answer in found {
            let (value, colour) = answer?;
            data.append(value)?;
            colours.append(colour)?;
        }
        Ok((data, colours))
    }

    /// The analysis run over windows of every size from `minWindow` to
    /// `maxWindow`, stepping by `windowStepSize`: music21's `process`,
    /// which answers the solutions, the colours and a note of each window
    /// size.
    #[pyo3(signature = (
        minWindow = Some(1),
        maxWindow = Some(1),
        windowStepSize = None,
        windowType = None,
        includeTotalWindow = true,
    ))]
    fn process<'py>(
        slf: &Bound<'py, Self>,
        minWindow: Option<usize>,
        maxWindow: Option<usize>,
        windowStepSize: Option<&Bound<'py, PyAny>>,
        windowType: Option<&Bound<'py, PyAny>>,
        includeTotalWindow: bool,
    ) -> PyResult<(Bound<'py, PyList>, Bound<'py, PyList>, Bound<'py, PyList>)> {
        let py = slf.py();
        let kind = loose_window_type(windowType)?;
        let step = read_step(windowStepSize)?;
        let me = slf.borrow();
        let processor = me.processor.bind(py);
        let found = me
            .windowed
            .process(
                minWindow,
                maxWindow,
                step,
                kind,
                includeTotalWindow,
                |window| asked(processor, window),
            )
            .map_err(|error| WindowedAnalysisException::new_err(crate::pitch::message(&error)))?;
        let solutions = PyList::empty(py);
        let colours = PyList::empty(py);
        let meta = PyList::empty(py);
        for (size, answers) in found {
            let row = PyList::empty(py);
            let painted = PyList::empty(py);
            for answer in answers {
                let (value, colour) = answer?;
                row.append(value)?;
                painted.append(colour)?;
            }
            solutions.append(row)?;
            colours.append(painted)?;
            let said = PyDict::new(py);
            said.set_item("windowSize", size)?;
            meta.append(said)?;
        }
        Ok((solutions, colours, meta))
    }
}

impl WindowedAnalysis {
    /// A stream cut into measures of one meter, with what runs past a
    /// barline tied and nothing but the measures kept, as music21's
    /// `getMinimumWindowStream` cuts it.
    fn windows_of<'py>(
        py: Python<'py>,
        source: &RsStream,
        ratio: &str,
    ) -> PyResult<Bound<'py, PyAny>> {
        let refuse = |error: music21_rs_crate::Error| {
            WindowedAnalysisException::new_err(crate::pitch::message(&error))
        };
        let meter = RsTimeSignature::from_ratio_string(ratio).map_err(refuse)?;
        let measured = make_measures_by(source, &[(0.0, meter)]).map_err(refuse)?;
        // Nothing but the measures is kept, as music21's
        // `removeByNotOfClass('Measure')` keeps nothing else -- a staff
        // group read with the stream is not a window.
        let mut kept = RsStream::with_kind(measured.kind());
        for event in measured.events() {
            if let Some(inner) = event
                .element()
                .as_stream()
                .filter(|inner| inner.kind() == StreamKind::Measure)
            {
                kept.insert(event.offset(), inner.clone());
            }
        }
        if kept.events().is_empty() {
            return Err(WindowedAnalysisException::new_err("Making measures failed"));
        }
        make_ties(&mut kept).map_err(refuse)?;
        crate::stream::from_crate(py, &kept)
    }
}

/// One window handed to the processor: its answer and its colour, and
/// music21's own pair where the processor has nothing to say about it.
fn asked(processor: &Bound<'_, PyAny>, window: &RsStream) -> PyResult<(Py<PyAny>, Py<PyAny>)> {
    let py = processor.py();
    let given = crate::stream::from_crate(py, window)?;
    match processor.call_method1("process", (given,)) {
        Ok(answer) => Ok((answer.get_item(0)?.unbind(), answer.get_item(1)?.unbind())),
        Err(error) if is_discrete(py, &error) => {
            // A window of nothing but rests, which music21 answers for.
            let nothing = PyTuple::new(
                py,
                [
                    py.None(),
                    py.None(),
                    0i32.into_pyobject(py)?.into_any().unbind(),
                ],
            )?;
            Ok((
                nothing.into_any().unbind(),
                PyString::new(py, "#ffffff").into_any().unbind(),
            ))
        }
        Err(error) => Err(error),
    }
}

/// Whether an error is the one music21 raises for a window it cannot
/// analyse, whosever class it is -- this package's or music21's.
fn is_discrete(py: Python<'_>, error: &PyErr) -> bool {
    if error.is_instance_of::<crate::discrete::DiscreteAnalysisException>(py) {
        return true;
    }
    py.import("music21.analysis.discrete")
        .and_then(|module| module.getattr("DiscreteAnalysisException"))
        .is_ok_and(|class| error.is_instance(py, &class))
}

/// How a window moves, as music21's `analyze` reads it: by its exact name,
/// and anything else refused.
fn exact_window_type(said: &str) -> PyResult<WindowType> {
    match said {
        "overlap" => Ok(WindowType::Overlap),
        "noOverlap" => Ok(WindowType::NoOverlap),
        "adjacentAverage" => Ok(WindowType::AdjacentAverage),
        // music21 raises its own base class here, which is what this is.
        other => Err(PyErr::new::<crate::Music21Exception, _>(format!(
            "Unknown windowType: {other}"
        ))),
    }
}

/// How a window moves, as music21's `process` reads it: any case, and the
/// two spellings of no overlap.
fn loose_window_type(said: Option<&Bound<'_, PyAny>>) -> PyResult<WindowType> {
    let Some(said) = said.filter(|given| !given.is_none()) else {
        return Ok(WindowType::Overlap);
    };
    let said = said.extract::<String>()?.to_lowercase();
    match said.as_str() {
        "overlap" => Ok(WindowType::Overlap),
        "nooverlap" | "nonoverlapping" => Ok(WindowType::NoOverlap),
        "adjacentaverage" => Ok(WindowType::AdjacentAverage),
        // music21 leaves an unknown name alone here and `analyze` refuses
        // it, which is the same refusal either way.
        other => Err(PyErr::new::<crate::Music21Exception, _>(format!(
            "Unknown windowType: {other}"
        ))),
    }
}

/// How the window size grows: a number of quarters, or a number written in
/// a string, which music21 multiplies by.
fn read_step(said: Option<&Bound<'_, PyAny>>) -> PyResult<WindowStep> {
    let Some(said) = said.filter(|given| !given.is_none()) else {
        return Ok(WindowStep::Add(1));
    };
    if let Ok(by) = said.extract::<usize>() {
        return Ok(WindowStep::Add(by));
    }
    let written = said.extract::<String>()?;
    let digits: String = written.chars().take_while(char::is_ascii_digit).collect();
    let by = digits.parse::<usize>().map_err(|_| {
        WindowedAnalysisException::new_err(format!("cannot read a window step from {written}"))
    })?;
    Ok(WindowStep::Multiply(by))
}

/// Adds the windowed-analysis class to a module.
///
/// The exception is not among them: building one builds music21's own
/// `Music21Exception` and so imports music21, which this package is not to
/// do for a name nobody has asked for. `lib.rs`'s table hands it out on
/// first asking.
pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<WindowedAnalysis>()?;
    Ok(())
}
