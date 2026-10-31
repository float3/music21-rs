//! music21's `omr.correctors`: a score read by an optical reader mended
//! against itself.
//!
//! A measure whose notes do not fill its bar is flagged, and the likeliest
//! reading of it is found twice over -- among the other measures of its own
//! part, and among the measures at the same place in the other parts. Every
//! number in that is the crate's `omr::correctors`, which `omr_parity`
//! holds to music21 on corpus scores and on music21's own pair of K. 525:
//! the rhythm of a measure as text (`measure_hash`, and `hash_note`,
//! `hash_rest`, `hash_grace` and `hash_quarter_length` for one element of
//! one), how unlike two rhythms are (`measure_difference`), the steps that
//! turn one into the other (`opcodes`) and how likely each is
//! (`probability_for_one_opcode`, `probability_on_substitute`,
//! `probability_from_one_char_sub`, and the three constants), the whole
//! correction's odds (`probability_based_on_changes_between`), a part's
//! measures and the ones flagged (`part_hashes`, `incorrect_measures`),
//! and the search across the parts (`slice_search`).
//!
//! **`ScoreCorrector` stays music21's, and that is the harder test.** It is
//! the driver: it builds a `SinglePart` for each part, a `MeasureSlice` for
//! each place, and a `MeasureHash` for each measure -- all three of them
//! this package's, since `rebind` swaps the names its own module holds --
//! and it is the one member that *edits the score*, deleting a flagged
//! measure's contents and putting the correcting measure's in with as many
//! of the old pitches as it can. That surgery is on music21's own measure
//! objects, and the crate's `substitute_measure` does the same over the
//! crate's values; running music21's over ours is the `MetricModulation`
//! bargain, and it means every one of its own examples -- which print the
//! whole corrected score with `show('text')` -- runs as written.
//!
//! **A part is read afresh for every question.** music21's `measureStream`
//! is a live view of the part, and its own example inserts a time
//! signature into a measure and asks again, so `getIncorrectMeasureIndices`
//! reads the part into the crate each time it is asked rather than keeping
//! what it read.
//!
//! **A `MeasureHash` keeps which rhythm it compares from.** music21 keeps a
//! `difflib.SequenceMatcher` on the object, whose first sequence
//! `setSequenceMatcher` sets and whose second every comparison sets; the
//! comparison here is `search::difflib`, ported, so the facade keeps the
//! two strings instead and builds music21's own matcher beside them for a
//! caller who reads it. The two strings are told apart because music21
//! tells them apart: it reads its steps off the matcher's first sequence
//! and a replaced stretch off its own `hashString`, which
//! `probability_based_on_changes_between` takes separately.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};

use music21_rs_crate::FloatType;
use music21_rs_crate::omr::correctors::{
    MeasureRelationship as RsMeasureRelationship, Opcode, OpcodeTag, PROBABILITY_ON_ADDITION,
    PROBABILITY_ON_EQUALITY, PROBABILITY_ON_OMISSION, hash_grace, hash_note, hash_quarter_length,
    hash_rest, incorrect_measures, measure_difference, measure_hash, opcodes,
    probability_based_on_changes_between, probability_for_one_opcode,
    probability_from_one_char_sub, probability_on_substitute, slice_search,
};

use crate::Walkable;

/// The names this module hands out.
pub const NAMES: &[&str] = &["SinglePart", "MeasureSlice", "MeasureHash"];

pyo3::create_exception!(music21_rs_facade, OmrException, crate::Music21Exception);

/// music21's `MeasureRelationship`, built once where there is no music21
/// to ask for its own.
static RELATIONSHIP: pyo3::sync::PyOnceLock<Py<PyAny>> = pyo3::sync::PyOnceLock::new();

/// Which measure corrects which, as music21 answers it: a named tuple
/// holding nothing of the crate.
fn relationship(py: Python<'_>, found: &RsMeasureRelationship) -> PyResult<Py<PyAny>> {
    let class = crate::music21_or_namedtuple(
        py,
        "music21.omr.correctors",
        "MeasureRelationship",
        "flaggedMeasurePart flaggedMeasureIndex correctMeasurePart correctMeasureIndex \
         correctionProbability",
        &RELATIONSHIP,
    )?;
    Ok(class
        .call1((
            found.flagged_measure_part,
            found.flagged_measure_index,
            found.correct_measure_part,
            found.correct_measure_index,
            found.correction_probability,
        ))?
        .unbind())
}

/// The measures of a music21 stream, as music21's own
/// `getElementsByClass` answers them.
fn measures_of<'py>(part: &Bound<'py, PyAny>) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let mut out = Vec::new();
    for measure in part
        .call_method1("getElementsByClass", ("Measure",))?
        .walk()?
    {
        out.push(measure?);
    }
    Ok(out)
}

/// One of difflib's steps read off the tuple music21 hands over.
fn opcode_read(step: &Bound<'_, PyAny>) -> PyResult<Opcode> {
    let tag: String = step.get_item(0)?.extract()?;
    let tag = match tag.as_str() {
        "equal" => OpcodeTag::Equal,
        "replace" => OpcodeTag::Replace,
        "insert" => OpcodeTag::Insert,
        "delete" => OpcodeTag::Delete,
        _ => {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "Incorrect opCodeType value.",
            ));
        }
    };
    Ok(Opcode {
        tag,
        i1: step.get_item(1)?.extract()?,
        i2: step.get_item(2)?.extract()?,
        j1: step.get_item(3)?.extract()?,
        j2: step.get_item(4)?.extract()?,
    })
}

/// One of difflib's steps as music21 hands it back.
fn opcode_tuple<'py>(py: Python<'py>, step: &Opcode) -> PyResult<Bound<'py, PyTuple>> {
    let tag = match step.tag {
        OpcodeTag::Equal => "equal",
        OpcodeTag::Replace => "replace",
        OpcodeTag::Insert => "insert",
        OpcodeTag::Delete => "delete",
    };
    PyTuple::new(
        py,
        [
            pyo3::types::PyString::new(py, tag).into_any(),
            step.i1.into_pyobject(py)?.into_any(),
            step.i2.into_pyobject(py)?.into_any(),
            step.j1.into_pyobject(py)?.into_any(),
            step.j2.into_pyobject(py)?.into_any(),
        ],
    )
}

/// The one length a note, rest or chord sounds.
fn quarter_length(element: &Bound<'_, PyAny>) -> PyResult<FloatType> {
    element
        .getattr("duration")?
        .getattr("quarterLength")?
        .extract()
}

/// music21's `omr.correctors.MeasureHash`: one measure's rhythm as text,
/// and every comparison of it with another's.
#[pyclass(
    name = "MeasureHash",
    module = "music21.omr.correctors",
    subclass,
    skip_from_py_object
)]
pub struct MeasureHash {
    /// The measure whose rhythm this is.
    #[pyo3(get, set)]
    measureObject: Py<PyAny>,
    /// That rhythm as text.
    #[pyo3(get, set)]
    hashString: Py<PyAny>,
    /// music21's own matcher, for a caller who reads it.
    #[pyo3(get, set)]
    sequenceMatcher: Py<PyAny>,
    /// The rhythm the comparisons are made from, which is music21's first
    /// sequence.
    matching: Option<String>,
    /// The rhythm last compared with, which is its second.
    other: String,
}

impl MeasureHash {
    /// The rhythm this hash says, or music21's failure where it says none.
    fn said(&self, py: Python<'_>) -> PyResult<String> {
        let hash = self.hashString.bind(py);
        if hash.is_none() {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "HashString has not yet been set!",
            ));
        }
        hash.extract()
    }
}

#[pymethods]
impl MeasureHash {
    /// music21's `MeasureHash(measureObject)`.
    #[new]
    #[pyo3(signature = (measureObject = None))]
    fn new(py: Python<'_>, measureObject: Option<Py<PyAny>>) -> Self {
        let given = measureObject.filter(|object| !object.bind(py).is_none());
        Self {
            measureObject: given.unwrap_or_else(|| py.None()),
            hashString: py.None(),
            sequenceMatcher: py.None(),
            matching: None,
            other: String::new(),
        }
    }

    /// The measure given is hashed at once, as music21's `__init__`
    /// hashes it -- and through the object, so a subclass's
    /// `getHashString` is the one that runs.
    #[pyo3(signature = (measureObject = None))]
    fn __init__(slf: &Bound<'_, Self>, measureObject: Option<Py<PyAny>>) -> PyResult<()> {
        let py = slf.py();
        let Some(given) = measureObject.filter(|object| !object.bind(py).is_none()) else {
            return Ok(());
        };
        slf.borrow_mut().measureObject = given;
        slf.call_method0("getHashString")?;
        Ok(())
    }

    /// The measure's rhythm as text, kept on the object: music21's
    /// `getHashString`, which is the crate's `measure_hash`.
    fn getHashString(slf: &Bound<'_, Self>) -> PyResult<String> {
        let py = slf.py();
        let measure = slf.borrow().measureObject.clone_ref(py);
        let measure = measure.bind(py);
        if measure.is_none() {
            return Ok(String::new());
        }
        let read = crate::stream::crate_stream(measure)?;
        let hash = measure_hash(&read).map_err(|error| crate::stream::stream_error(&error))?;
        slf.borrow_mut().hashString = pyo3::types::PyString::new(py, &hash).into_any().unbind();
        Ok(hash)
    }

    /// The character a note or chord of this length is written as:
    /// music21's `hashNote`.
    fn hashNote(&self, n: &Bound<'_, PyAny>) -> PyResult<String> {
        Ok(hash_note(quarter_length(n)?).to_string())
    }

    /// The character anything of no length is written as: music21's
    /// `hashGrace`, which writes a grace note as a 128th.
    fn hashGrace(&self, n: &Bound<'_, PyAny>) -> PyResult<String> {
        let _ = n;
        Ok(hash_grace().to_string())
    }

    /// The character a rest of this length is written as: music21's
    /// `hashRest`.
    fn hashRest(&self, r: &Bound<'_, PyAny>) -> PyResult<String> {
        Ok(hash_rest(quarter_length(r)?).to_string())
    }

    /// A length as the number from 1 to 127 it is written by: music21's
    /// `hashQuarterLength`.
    fn hashQuarterLength(&self, ql: FloatType) -> u8 {
        hash_quarter_length(ql)
    }

    /// The rhythm every later comparison is made from: music21's
    /// `setSequenceMatcher`, whose matcher this builds beside it.
    #[pyo3(signature = (hashes = None))]
    fn setSequenceMatcher(slf: &Bound<'_, Self>, hashes: Option<String>) -> PyResult<()> {
        let py = slf.py();
        let hashes = match hashes {
            Some(hashes) => hashes,
            None => {
                let said = slf.borrow().hashString.bind(py).clone();
                if said.is_none() {
                    Self::getHashString(slf)?
                } else {
                    said.extract()?
                }
            }
        };
        let matcher =
            py.import("difflib")?
                .getattr("SequenceMatcher")?
                .call1((py.None(), &hashes, ""))?;
        let mut me = slf.borrow_mut();
        me.matching = Some(hashes);
        me.other = String::new();
        me.sequenceMatcher = matcher.unbind();
        Ok(())
    }

    /// How unlike another measure's rhythm this one is: music21's
    /// `getMeasureDifference`, which is one less their likeness and one
    /// where they read alike.
    fn getMeasureDifference(slf: &Bound<'_, Self>, hashString: String) -> PyResult<FloatType> {
        let py = slf.py();
        let Some(matching) = slf.borrow().matching.clone() else {
            // music21 asks its matcher, which it has not built.
            return Err(pyo3::exceptions::PyAttributeError::new_err(
                "'NoneType' object has no attribute 'set_seq2'",
            ));
        };
        {
            let mut me = slf.borrow_mut();
            me.other = hashString.clone();
            let matcher = me.sequenceMatcher.bind(py).clone();
            if !matcher.is_none() {
                matcher.call_method1("set_seq2", (&hashString,))?;
            }
        }
        Ok(measure_difference(&matching, &hashString))
    }

    /// The steps that turn this rhythm into another: music21's
    /// `getOpCodes`, which are Python's own difflib opcodes.
    #[pyo3(signature = (otherHash = None))]
    fn getOpCodes<'py>(
        slf: &Bound<'py, Self>,
        otherHash: Option<String>,
    ) -> PyResult<Bound<'py, PyList>> {
        let py = slf.py();
        if slf.borrow().matching.is_none() {
            Self::setSequenceMatcher(slf, None)?;
        }
        if let Some(otherHash) = otherHash {
            let mut me = slf.borrow_mut();
            me.other = otherHash.clone();
            let matcher = me.sequenceMatcher.bind(py).clone();
            if !matcher.is_none() {
                matcher.call_method1("set_seq2", (&otherHash,))?;
            }
        }
        let (matching, other) = {
            let me = slf.borrow();
            (me.matching.clone().unwrap_or_default(), me.other.clone())
        };
        let out = PyList::empty(py);
        for step in opcodes(&matching, &other) {
            out.append(opcode_tuple(py, &step)?)?;
        }
        Ok(out)
    }

    /// How likely another measure was misread as this one: music21's
    /// `getProbabilityBasedOnChanges`, every step multiplied.
    fn getProbabilityBasedOnChanges(
        slf: &Bound<'_, Self>,
        otherHash: String,
    ) -> PyResult<FloatType> {
        let py = slf.py();
        Self::getOpCodes(slf, Some(otherHash.clone()))?;
        let me = slf.borrow();
        let matching = me.matching.clone().unwrap_or_default();
        let said = me.said(py)?;
        Ok(probability_based_on_changes_between(
            &matching, &otherHash, &said,
        ))
    }

    /// How likely one of those steps is: music21's
    /// `differenceProbabilityForOneOpCode`.
    #[pyo3(signature = (opCodeTuple, source, destination = None))]
    fn differenceProbabilityForOneOpCode(
        slf: &Bound<'_, Self>,
        opCodeTuple: &Bound<'_, PyAny>,
        source: String,
        destination: Option<String>,
    ) -> PyResult<FloatType> {
        let py = slf.py();
        let destination = match destination {
            Some(destination) => destination,
            None => slf.borrow().said(py)?,
        };
        let step = opcode_read(opCodeTuple)?;
        Ok(probability_for_one_opcode(&step, &source, &destination))
    }

    /// How likely a note is read right: music21's
    /// `getProbabilityOnEquality`.
    fn getProbabilityOnEquality(&self) -> FloatType {
        PROBABILITY_ON_EQUALITY
    }

    /// How likely a note is missed: music21's `getProbabilityOnOmission`.
    fn getProbabilityOnOmission(&self) -> FloatType {
        PROBABILITY_ON_OMISSION
    }

    /// How likely a note is read that is not there: music21's
    /// `getProbabilityOnAddition`.
    fn getProbabilityOnAddition(&self) -> FloatType {
        PROBABILITY_ON_ADDITION
    }

    /// How likely one stretch of rhythm was misread as another: music21's
    /// `getProbabilityOnSubstitute`.
    fn getProbabilityOnSubstitute(&self, source: String, destination: String) -> FloatType {
        probability_on_substitute(&source, &destination)
    }

    /// How likely one length was misread as another: music21's
    /// `getProbabilityFromOneCharSub`.
    fn getProbabilityFromOneCharSub(&self, source: &str, destination: &str) -> PyResult<FloatType> {
        let one = |text: &str| -> PyResult<char> {
            text.chars()
                .next()
                .ok_or_else(|| pyo3::exceptions::PyIndexError::new_err("string index out of range"))
        };
        Ok(probability_from_one_char_sub(
            one(source)?,
            one(destination)?,
        ))
    }
}

/// music21's `omr.correctors.MeasureSlice`: the same measure across every
/// part, which is what a doubtful measure is compared against.
#[pyclass(
    name = "MeasureSlice",
    module = "music21.omr.correctors",
    subclass,
    skip_from_py_object
)]
pub struct MeasureSlice {
    /// The measure of each part at this place.
    #[pyo3(get, set)]
    arrayOfMeasureObjects: Py<PyAny>,
    /// The corrector this slice belongs to.
    #[pyo3(get, set)]
    score: Py<PyAny>,
    /// Which measure of each part it is.
    #[pyo3(get, set)]
    index: usize,
    /// A hash for each of those measures.
    #[pyo3(get, set)]
    sliceMeasureHashObjects: Py<PyAny>,
    /// How often each part's measures read like each other's.
    #[pyo3(get, set)]
    allProbabilities: Py<PyAny>,
    /// Which measure was found to correct the flagged one.
    #[pyo3(get, set)]
    correctingMeasure: Py<PyAny>,
}

#[pymethods]
impl MeasureSlice {
    /// music21's `MeasureSlice(score, i)`, which gathers the measure of
    /// every part at that place.
    #[new]
    #[pyo3(signature = (score = None, i = 0))]
    fn new(py: Python<'_>, score: Option<Py<PyAny>>, i: usize) -> PyResult<Self> {
        let measures = PyList::empty(py);
        if let Some(score) = score.as_ref().filter(|score| !score.bind(py).is_none()) {
            for part in score.bind(py).getattr("singleParts")?.walk()? {
                let found = part?.call_method0("getMeasures")?;
                measures.append(found.get_item(i)?)?;
            }
        }
        Ok(Self {
            arrayOfMeasureObjects: measures.into_any().unbind(),
            score: score.unwrap_or_else(|| py.None()),
            index: i,
            sliceMeasureHashObjects: PyList::empty(py).into_any().unbind(),
            allProbabilities: py.None(),
            correctingMeasure: py.None(),
        })
    }

    /// A hash of the measure in each part: music21's `getSliceHashes`,
    /// which appends to what it has and hands the whole back.
    fn getSliceHashes<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let (measures, hashes) = {
            let me = slf.borrow();
            (
                me.arrayOfMeasureObjects.clone_ref(py),
                me.sliceMeasureHashObjects.clone_ref(py),
            )
        };
        let class = crate::installed_class(py, "music21.omr.correctors", "MeasureHash")
            .unwrap_or_else(|| py.get_type::<MeasureHash>().into_any());
        for measure in measures.bind(py).try_iter()? {
            hashes
                .bind(py)
                .call_method1("append", (class.call1((measure?,))?,))?;
        }
        Ok(hashes.bind(py).clone())
    }

    /// Which measure of the slice most likely corrects the flagged one:
    /// music21's `runSliceSearch`, which is the crate's `slice_search`.
    fn runSliceSearch(slf: &Bound<'_, Self>, incorrectPartIndex: usize) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let hashed = Self::getSliceHashes(slf)?;
        let mut hashes = Vec::new();
        let count = slf.borrow().arrayOfMeasureObjects.bind(py).len()?;
        for at in 0..count {
            let hash = hashed.get_item(at)?.call_method0("getHashString")?;
            hashes.push(hash.extract::<String>()?);
        }
        let (score, probabilities, measure) = {
            let me = slf.borrow();
            (
                me.score.clone_ref(py),
                me.allProbabilities.clone_ref(py),
                me.index,
            )
        };
        let all = score.bind(py).call_method0("getAllIncorrectMeasures")?;
        let mut flagged = Vec::with_capacity(count);
        for at in 0..count {
            flagged.push(all.get_item(at)?.contains(measure)?);
        }
        let row = probabilities.bind(py).get_item(incorrectPartIndex)?;
        let distribution: Vec<FloatType> = row.extract()?;
        let found = slice_search(
            &hashes,
            &flagged,
            &distribution,
            incorrectPartIndex,
            measure,
        );
        let made = relationship(py, &found)?;
        slf.borrow_mut().correctingMeasure = made.clone_ref(py);
        Ok(made)
    }
}

/// music21's `omr.correctors.SinglePart`: one part of a score being
/// corrected, its measures' rhythms and those flagged.
#[pyclass(
    name = "SinglePart",
    module = "music21.omr.correctors",
    subclass,
    skip_from_py_object
)]
pub struct SinglePart {
    /// The part itself.
    #[pyo3(get, set)]
    scorePart: Py<PyAny>,
    /// Which part of the score it is.
    #[pyo3(get, set)]
    partNumber: Py<PyAny>,
    /// Where each measure stands against a flagged one.
    #[pyo3(get, set)]
    indexArray: Py<PyAny>,
    /// How likely a measure that far off corrects a flagged one.
    #[pyo3(get, set)]
    probabilityDistribution: Py<PyAny>,
    /// Which measure was found to correct the flagged one.
    #[pyo3(get, set)]
    correctingMeasure: Py<PyAny>,
    /// The part's measures.
    #[pyo3(get, set)]
    measureStream: Py<PyAny>,
    /// Each measure's rhythm as text.
    #[pyo3(get, set)]
    hashedNotes: Py<PyAny>,
    /// Which of them do not fill their bar.
    #[pyo3(get, set)]
    incorrectMeasures: Py<PyAny>,
}

impl SinglePart {
    /// The part as the crate holds one, read afresh: music21's
    /// `measureStream` is a live view of it, and its own example inserts
    /// a time signature and asks again.
    fn read(&self, py: Python<'_>) -> PyResult<music21_rs_crate::Stream> {
        crate::stream::crate_stream(self.scorePart.bind(py))
    }

    /// The part's measure rhythms and those flagged, as the crate reads
    /// them.
    fn crate_part(
        &self,
        py: Python<'_>,
    ) -> PyResult<music21_rs_crate::omr::correctors::SinglePart> {
        let part = self.read(py)?;
        let number = self.partNumber.bind(py).extract::<usize>().unwrap_or(0);
        music21_rs_crate::omr::correctors::SinglePart::from_part(&part, number)
            .map_err(|error| crate::stream::stream_error(&error))
    }
}

#[pymethods]
impl SinglePart {
    /// music21's `SinglePart(part, pn)`.
    #[new]
    #[pyo3(signature = (part = None, pn = None))]
    fn new(py: Python<'_>, part: Option<Py<PyAny>>, pn: Option<Py<PyAny>>) -> Self {
        let given = part.filter(|part| !part.bind(py).is_none());
        Self {
            scorePart: given.unwrap_or_else(|| py.None()),
            partNumber: pn.unwrap_or_else(|| py.None()),
            indexArray: py.None(),
            probabilityDistribution: py.None(),
            correctingMeasure: py.None(),
            measureStream: py.None(),
            hashedNotes: py.None(),
            incorrectMeasures: py.None(),
        }
    }

    /// The part given has its measures hashed and those that do not fill
    /// their bar flagged, as music21's `__init__` does both -- and
    /// through the object, so a subclass's overrides are the ones that
    /// run.
    #[pyo3(signature = (part = None, pn = None))]
    fn __init__(
        slf: &Bound<'_, Self>,
        part: Option<Py<PyAny>>,
        pn: Option<Py<PyAny>>,
    ) -> PyResult<()> {
        let py = slf.py();
        let Some(given) = part.filter(|part| !part.bind(py).is_none()) else {
            return Ok(());
        };
        {
            let mut me = slf.borrow_mut();
            me.scorePart = given;
            me.partNumber = pn.unwrap_or_else(|| py.None());
        }
        slf.call_method0("getMeasures")?;
        let hashed = slf.call_method0("getSequenceHashesFromMeasureStream")?;
        slf.setattr("hashedNotes", hashed)?;
        let keywords = PyDict::new(py);
        keywords.set_item("runFast", true)?;
        slf.call_method("getIncorrectMeasureIndices", (), Some(&keywords))?;
        Ok(())
    }

    /// The part's measures, kept on the object: music21's `getMeasures`,
    /// which is music21's own `getElementsByClass`.
    fn getMeasures(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let part = slf.borrow().scorePart.clone_ref(py);
        let found = part
            .bind(py)
            .call_method1("getElementsByClass", ("Measure",))?;
        slf.borrow_mut().measureStream = found.clone().unbind();
        Ok(found.unbind())
    }

    /// Which of the part's measures do not fill their bar, kept on the
    /// object: music21's `getIncorrectMeasureIndices`, which is the
    /// crate's `incorrect_measures`.
    #[pyo3(signature = (runFast = false))]
    fn getIncorrectMeasureIndices<'py>(
        slf: &Bound<'py, Self>,
        #[pyo3(from_py_with = crate::truthy)] runFast: bool,
    ) -> PyResult<Bound<'py, PyList>> {
        let py = slf.py();
        let part = slf.borrow().read(py)?;
        let found = incorrect_measures(&part, runFast)
            .map_err(|error| crate::stream::stream_error(&error))?;
        let out = PyList::new(py, found)?;
        slf.borrow_mut().incorrectMeasures = out.clone().into_any().unbind();
        Ok(out)
    }

    /// Each measure's rhythm as text: music21's
    /// `getSequenceHashesFromMeasureStream`, a `MeasureHash` of each.
    fn getSequenceHashesFromMeasureStream<'py>(
        slf: &Bound<'py, Self>,
    ) -> PyResult<Bound<'py, PyList>> {
        let py = slf.py();
        let stream = slf.borrow().measureStream.clone_ref(py);
        let stream = stream.bind(py);
        let out = PyList::empty(py);
        if stream.is_none() {
            return Ok(out);
        }
        let class = crate::installed_class(py, "music21.omr.correctors", "MeasureHash")
            .unwrap_or_else(|| py.get_type::<MeasureHash>().into_any());
        for measure in measures_of(stream)? {
            out.append(class.call1((measure,))?.call_method0("getHashString")?)?;
        }
        Ok(out)
    }

    /// How likely a measure that far from a flagged one corrects it, kept
    /// on the object: music21's `horizontalProbabilityDist`.
    #[pyo3(signature = (regenerate = false))]
    fn horizontalProbabilityDist<'py>(
        slf: &Bound<'py, Self>,
        #[pyo3(from_py_with = crate::truthy)] regenerate: bool,
    ) -> PyResult<Bound<'py, PyList>> {
        let py = slf.py();
        {
            let held = slf.borrow().probabilityDistribution.bind(py).clone();
            if !regenerate && !held.is_none() {
                return Ok(held.cast_into::<PyList>()?);
            }
        }
        let mut part = slf.borrow().crate_part(py)?;
        let distribution = part.horizontal_probability_dist().to_vec();
        let indices = part.index_array().unwrap_or_default().to_vec();
        let out = PyList::new(py, distribution)?;
        let mut me = slf.borrow_mut();
        me.probabilityDistribution = out.clone().into_any().unbind();
        me.indexArray = PyList::new(py, indices)?.into_any().unbind();
        Ok(out)
    }

    /// How likely the measure at one place corrects the one at another:
    /// music21's `getProbabilityDistribution`.
    fn getProbabilityDistribution(
        slf: &Bound<'_, Self>,
        sourceIndex: usize,
        destinationIndex: usize,
    ) -> PyResult<FloatType> {
        let py = slf.py();
        let mut part = slf.borrow().crate_part(py)?;
        Ok(part.probability_distribution(sourceIndex, destinationIndex))
    }

    /// Which measure of the part most likely corrects a flagged one:
    /// music21's `runHorizontalSearch`.
    fn runHorizontalSearch(slf: &Bound<'_, Self>, i: usize) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let mut part = slf.borrow().crate_part(py)?;
        let found = part.run_horizontal_search(i);
        let made = relationship(py, &found)?;
        slf.borrow_mut().correctingMeasure = made.clone_ref(py);
        Ok(made)
    }

    /// The same for every flagged measure of the part: music21's
    /// `runHorizontalCorrectionModel`.
    fn runHorizontalCorrectionModel<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyList>> {
        let py = slf.py();
        let mut part = slf.borrow().crate_part(py)?;
        let out = PyList::empty(py);
        for found in part.run_horizontal_correction_model() {
            out.append(relationship(py, &found)?)?;
        }
        if let Some(last) = out.iter().last() {
            slf.borrow_mut().correctingMeasure = last.unbind();
        }
        Ok(out)
    }
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<MeasureHash>()?;
    m.add_class::<MeasureSlice>()?;
    m.add_class::<SinglePart>()?;
    Ok(())
}
