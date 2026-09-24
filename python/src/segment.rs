//! music21's `figuredBass.segment`, over the crate's.
//!
//! A music21 `Segment` is a plain object whose state is its attributes --
//! the bass note, the pitch names, the rules -- which music21's realizer
//! reads and writes as it goes. The facade keeps them as attributes too, and
//! builds the crate's segment from them for each question, so an edit
//! through any of them is seen. The voicings come back as tuples of the
//! segment's own pitch objects where the crate's answer is one of them.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple, PyType};

use music21_rs_crate::Pitch as RsPitch;
use music21_rs_crate::figuredbass::rules::Rules as RsRules;
use music21_rs_crate::figuredbass::segment::{self as rs, Consecutive, Segment as RsSegment};

use crate::pitch::{Pitch, pitch_from_any};

pyo3::create_exception!(music21_rs_facade, SegmentException, crate::Music21Exception);

error_into!(segment_error, SegmentException);

/// The names this facade replaces in `music21.figuredBass.segment`.
pub const NAMES: &[&str] = &[
    "Segment",
    "OverlaidSegment",
    "getPitches",
    "SegmentException",
];

/// The class music21 now has under `name` in `module`, or the wheel's own
/// where music21 is not there.
fn class_of<'py>(
    py: Python<'py>,
    module: &str,
    name: &str,
    fallback: Bound<'py, PyType>,
) -> Bound<'py, PyAny> {
    py.import(module)
        .and_then(|module| module.getattr(name))
        .unwrap_or_else(|_| fallback.into_any())
}

fn pitch_object(py: Python<'_>, pitch: RsPitch) -> PyResult<Py<PyAny>> {
    let inferred = pitch.spelling_is_inferred();
    Ok(
        crate::installed_new(py, "music21.pitch", "Pitch", Pitch::wrap(pitch, inferred))?
            .into_any(),
    )
}

/// Rules as the crate holds them, read off the wheel's rules or off
/// music21's own by attribute.
fn rules_of(rules: &Bound<'_, PyAny>) -> PyResult<RsRules> {
    let py = rules.py();
    if let Ok(ours) = rules.extract::<PyRef<'_, crate::fbrules::Rules>>() {
        return ours.synced(py);
    }
    let flag = |name: &str| -> PyResult<bool> { rules.getattr(name)?.extract() };
    let mut read = RsRules {
        forbid_incomplete_possibilities: flag("forbidIncompletePossibilities")?,
        upper_parts_max_semitone_separation: rules
            .getattr("upperPartsMaxSemitoneSeparation")?
            .extract()?,
        forbid_voice_crossing: flag("forbidVoiceCrossing")?,
        forbid_parallel_fifths: flag("forbidParallelFifths")?,
        forbid_parallel_octaves: flag("forbidParallelOctaves")?,
        forbid_hidden_fifths: flag("forbidHiddenFifths")?,
        forbid_hidden_octaves: flag("forbidHiddenOctaves")?,
        forbid_voice_overlap: flag("forbidVoiceOverlap")?,
        resolve_dominant_seventh_properly: flag("resolveDominantSeventhProperly")?,
        resolve_diminished_seventh_properly: flag("resolveDiminishedSeventhProperly")?,
        resolve_augmented_sixth_properly: flag("resolveAugmentedSixthProperly")?,
        doubled_root_in_dim7: flag("doubledRootInDim7")?,
        apply_single_possib_rules_to_resolution: flag("applySinglePossibRulesToResolution")?,
        apply_consecutive_possib_rules_to_resolution: flag(
            "applyConsecutivePossibRulesToResolution",
        )?,
        restrict_doublings_in_italian_a6_resolution: flag(
            "restrictDoublingsInItalianA6Resolution",
        )?,
        upper_parts_remain_same: flag("_upperPartsRemainSame")?,
        ..RsRules::default()
    };
    for limit in rules.getattr("partMovementLimits")?.try_iter()? {
        read.part_movement_limits
            .push(crate::fbrules::pair::<usize, i32>(&limit?)?);
    }
    for limit in rules.getattr("_partPitchLimits")?.try_iter()? {
        let (part, pitch) = crate::fbrules::pair::<usize, Bound<'_, PyAny>>(&limit?)?;
        read.part_pitch_limits.push((part, pitch_from_any(&pitch)?));
    }
    for part in rules.getattr("_partsToCheck")?.try_iter()? {
        read.parts_to_check.push(part?.extract()?);
    }
    Ok(read)
}

/// Pitch objects handed back for the crate's pitches: a segment's own where
/// the value is one of them, and new ones, made once each, otherwise.
struct Objects {
    known: Vec<(RsPitch, Py<PyAny>)>,
}

impl Objects {
    fn of(pitches: &Bound<'_, PyAny>) -> PyResult<Self> {
        let mut known = Vec::new();
        for pitch in pitches.try_iter()? {
            let pitch = pitch?;
            known.push((pitch_from_any(&pitch)?, pitch.unbind()));
        }
        Ok(Self { known })
    }

    fn object(&mut self, py: Python<'_>, pitch: &RsPitch) -> PyResult<Py<PyAny>> {
        if let Some((_, object)) = self.known.iter().find(|(value, _)| value == pitch) {
            return Ok(object.clone_ref(py));
        }
        let made = pitch_object(py, pitch.clone())?;
        self.known.push((pitch.clone(), made.clone_ref(py)));
        Ok(made)
    }

    fn tuple<'py>(
        &mut self,
        py: Python<'py>,
        voicing: &[RsPitch],
    ) -> PyResult<Bound<'py, PyTuple>> {
        let objects = voicing
            .iter()
            .map(|pitch| self.object(py, pitch))
            .collect::<PyResult<Vec<_>>>()?;
        PyTuple::new(py, objects)
    }
}

/// A resolved voicing, every pitch a new object, as music21's transposed
/// pitches are.
fn fresh_tuple<'py>(py: Python<'py>, voicing: &[RsPitch]) -> PyResult<Bound<'py, PyTuple>> {
    let objects = voicing
        .iter()
        .map(|pitch| pitch_object(py, pitch.clone()))
        .collect::<PyResult<Vec<_>>>()?;
    PyTuple::new(py, objects)
}

/// music21's `figuredBass.segment.Segment`.
#[pyclass(
    name = "Segment",
    module = "music21.figuredBass.segment",
    subclass,
    dict
)]
pub struct Segment;

impl Segment {
    /// The crate's segment, built from what the attributes say now.
    fn value(slf: &Bound<'_, PyAny>) -> PyResult<RsSegment> {
        let bass_note = slf.getattr("bassNote")?;
        let bass = pitch_from_any(&bass_note.getattr("pitch")?)?;
        let quarter_length: f64 = bass_note.getattr("quarterLength")?.extract()?;
        let names = slf
            .getattr("pitchNamesInChord")?
            .try_iter()?
            .map(|name| name?.extract::<String>())
            .collect::<PyResult<Vec<_>>>()?;
        let rules = rules_of(&slf.getattr("fbRules")?)?;
        let parts: usize = slf.getattr("numParts")?.extract()?;
        let top = pitch_from_any(&slf.getattr("_maxPitch")?)?;
        let segment = RsSegment::from_pitch_names(bass, quarter_length, names, rules, parts, top)
            .map_err(segment_error)?;
        Ok(if slf.is_instance_of::<OverlaidSegment>() {
            segment.overlaid()
        } else {
            segment
        })
    }

    /// The pairs the crate found, as music21 hands them: a voicing of this
    /// segment of its own pitch objects beside the next segment's.
    fn pairs<'py>(
        slf: &Bound<'py, PyAny>,
        next: &Bound<'py, PyAny>,
        found: Consecutive,
        special: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        if let Some(warning) = &found.fallback {
            warn(py, warning)?;
        }
        let mut mine = Self::own_objects(slf)?;
        let mut theirs = Self::own_objects(next)?;
        let list = PyList::empty(py);
        for (from, to) in &found.pairs {
            let from = mine.tuple(py, from)?;
            let to = if special {
                fresh_tuple(py, to)?
            } else {
                theirs.tuple(py, to)?
            };
            list.append(PyTuple::new(py, [from, to])?)?;
        }
        list.as_any().try_iter().map(Bound::into_any)
    }

    /// The segment's pitch objects: its pitches above the bass.
    fn own_objects(slf: &Bound<'_, PyAny>) -> PyResult<Objects> {
        Objects::of(&slf.getattr("allPitchesAboveBass")?)
    }

    /// Runs a resolution of this segment into `next`, writing back the one
    /// thing a resolution may change on `next`: whether its voicings must
    /// be complete.
    fn resolved<'py>(
        slf: &Bound<'py, PyAny>,
        next: &Bound<'py, PyAny>,
        special: bool,
        run: impl FnOnce(&RsSegment, &mut RsSegment) -> music21_rs_crate::Result<Consecutive>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let me = Self::value(slf)?;
        let mut other = Self::value(next)?;
        let before = other.rules.forbid_incomplete_possibilities;
        let found = run(&me, &mut other).map_err(segment_error)?;
        if other.rules.forbid_incomplete_possibilities != before {
            next.getattr("fbRules")?.setattr(
                "forbidIncompletePossibilities",
                other.rules.forbid_incomplete_possibilities,
            )?;
        }
        let special = special && found.fallback.is_none();
        Self::pairs(slf, next, found, special)
    }

    /// The function of music21's `possibility` module named `name`, as
    /// music21's rule tables hold it.
    fn rule(py: Python<'_>, name: &str) -> PyResult<Py<PyAny>> {
        match py.import("music21.figuredBass.possibility") {
            Ok(module) => Ok(module.getattr(name)?.unbind()),
            Err(_) => Ok(py.import("music21_rs")?.getattr(name)?.unbind()),
        }
    }

    fn rules_or_default<'py>(
        py: Python<'py>,
        fbRules: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        match fbRules.filter(|rules| !rules.is_none()) {
            Some(rules) => Ok(rules.clone()),
            None => Ok(class_of(
                py,
                "music21.figuredBass.rules",
                "Rules",
                py.get_type::<crate::fbrules::Rules>(),
            )
            .call0()?),
        }
    }
}

/// Says what music21's segment warns, where music21's environment is there
/// to say it through.
fn warn(py: Python<'_>, message: &str) -> PyResult<()> {
    if let Ok(environment) = py.import("music21.environment") {
        environment
            .getattr("Environment")?
            .call1(("figuredBass.segment",))?
            .call_method1("warn", (message,))?;
    }
    Ok(())
}

#[pymethods]
impl Segment {
    #[new]
    #[pyo3(signature = (*_arguments, **_keywords))]
    fn new(_arguments: &Bound<'_, PyTuple>, _keywords: Option<&Bound<'_, PyDict>>) -> Self {
        Self
    }

    #[pyo3(signature = (
        bassNote = None,
        notationString = None,
        fbScale = None,
        fbRules = None,
        numParts = 4,
        maxPitch = None,
        listOfPitches = None,
    ))]
    #[allow(clippy::too_many_arguments)]
    fn __init__(
        slf: &Bound<'_, Self>,
        bassNote: Option<&Bound<'_, PyAny>>,
        notationString: Option<&str>,
        fbScale: Option<&Bound<'_, PyAny>>,
        fbRules: Option<&Bound<'_, PyAny>>,
        numParts: usize,
        maxPitch: Option<&Bound<'_, PyAny>>,
        listOfPitches: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let py = slf.py();
        let note_class = class_of(
            py,
            "music21.note",
            "Note",
            py.get_type::<crate::note::Note>(),
        );
        let pitch_class = class_of(py, "music21.pitch", "Pitch", py.get_type::<Pitch>());
        let bass_note = match bassNote.filter(|note| !note.is_none()) {
            Some(note) if note.extract::<String>().is_ok() => note_class.call1((note,))?,
            Some(note) => note.clone(),
            None => note_class.call1(("C3",))?,
        };
        let top = match maxPitch.filter(|pitch| !pitch.is_none()) {
            Some(pitch) if pitch.extract::<String>().is_ok() => pitch_class.call1((pitch,))?,
            Some(pitch) => pitch.clone(),
            None => pitch_class.call1(("B5",))?,
        };
        let scale = match fbScale.filter(|scale| !scale.is_none()) {
            Some(scale) => scale.clone(),
            None => class_of(
                py,
                "music21.figuredBass.realizerScale",
                "FiguredBassScale",
                py.get_type::<crate::realizerscale::FiguredBassScale>(),
            )
            .call0()?,
        };
        let rules = match fbRules.filter(|rules| !rules.is_none()) {
            Some(rules) => py.import("copy")?.getattr("deepcopy")?.call1((rules,))?,
            None => Self::rules_or_default(py, None)?,
        };
        let names = match (
            notationString,
            listOfPitches.filter(|given| !given.is_none()),
        ) {
            (None, Some(given)) => given.clone(),
            _ => scale.call_method1(
                "getPitchNames",
                (bass_note.getattr("pitch")?, notationString.unwrap_or("")),
            )?,
        };
        let me = slf.as_any();
        me.setattr("fbRules", &rules)?;
        me.setattr("bassNote", &bass_note)?;
        me.setattr("numParts", numParts)?;
        me.setattr("_maxPitch", &top)?;
        me.setattr("pitchNamesInChord", &names)?;
        let above = getPitches(
            py,
            Some(&names),
            Some(&bass_note.getattr("pitch")?),
            Some(&top),
        )?;
        me.setattr("allPitchesAboveBass", &above)?;
        let chord_class = class_of(
            py,
            "music21.chord",
            "Chord",
            py.get_type::<crate::chord::Chord>(),
        );
        let keywords = PyDict::new(py);
        keywords.set_item("quarterLength", bass_note.getattr("quarterLength")?)?;
        me.setattr("segmentChord", chord_class.call((above,), Some(&keywords))?)?;
        Ok(())
    }

    /// music21's table of the rules a single voicing is held to: whether
    /// each runs, the function, the answer that keeps a voicing, and its
    /// arguments.
    #[pyo3(signature = (fbRules = None))]
    fn singlePossibilityRules<'py>(
        slf: &Bound<'py, Self>,
        fbRules: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyList>> {
        let py = slf.py();
        let rules = Self::rules_or_default(py, fbRules)?;
        let names = slf.getattr("pitchNamesInChord")?;
        let table = PyList::empty(py);
        table.append((
            rules.getattr("forbidIncompletePossibilities")?,
            Self::rule(py, "isIncomplete")?,
            false,
            PyList::new(py, [names])?,
        ))?;
        table.append((
            true,
            Self::rule(py, "upperPartsWithinLimit")?,
            true,
            PyList::new(py, [rules.getattr("upperPartsMaxSemitoneSeparation")?])?,
        ))?;
        table.append((
            rules.getattr("forbidVoiceCrossing")?,
            Self::rule(py, "voiceCrossing")?,
            false,
        ))?;
        Ok(table)
    }

    /// music21's table of the rules moving from one voicing to the next is
    /// held to.
    #[pyo3(signature = (fbRules = None))]
    fn consecutivePossibilityRules<'py>(
        slf: &Bound<'py, Self>,
        fbRules: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyList>> {
        let py = slf.py();
        let rules = Self::rules_or_default(py, fbRules)?;
        let value = Self::value(slf.as_any())?;
        let chord = value.chord();
        let italian = chord.is_italian_augmented_sixth(false, false);
        let unpacked = PyList::empty(py);
        for pitch in [
            chord.bass(),
            chord.root(),
            chord.chord_step(3),
            chord.chord_step(5),
        ] {
            match pitch {
                Some(pitch) => unpacked.append(pitch_object(py, pitch.clone())?)?,
                None => unpacked.append(py.None())?,
            }
        }
        let table = PyList::empty(py);
        let arguments = |values: Vec<Bound<'py, PyAny>>| PyList::new(py, values);
        table.append((
            true,
            Self::rule(py, "partsSame")?,
            true,
            arguments(vec![rules.getattr("_partsToCheck")?])?,
        ))?;
        table.append((
            rules.getattr("_upperPartsRemainSame")?,
            Self::rule(py, "upperPartsSame")?,
            true,
        ))?;
        table.append((
            rules.getattr("forbidVoiceOverlap")?,
            Self::rule(py, "voiceOverlap")?,
            false,
        ))?;
        table.append((
            true,
            Self::rule(py, "partMovementsWithinLimits")?,
            true,
            arguments(vec![rules.getattr("partMovementLimits")?])?,
        ))?;
        for (switch, name) in [
            ("forbidParallelFifths", "parallelFifths"),
            ("forbidParallelOctaves", "parallelOctaves"),
            ("forbidHiddenFifths", "hiddenFifths"),
            ("forbidHiddenOctaves", "hiddenOctaves"),
        ] {
            table.append((rules.getattr(switch)?, Self::rule(py, name)?, false))?;
        }
        let resolves: bool = rules.getattr("resolveAugmentedSixthProperly")?.extract()?;
        table.append((
            resolves && italian,
            Self::rule(py, "couldBeItalianA6Resolution")?,
            true,
            arguments(vec![
                unpacked.into_any(),
                rules.getattr("restrictDoublingsInItalianA6Resolution")?,
            ])?,
        ))?;
        Ok(table)
    }

    /// music21's table of the resolutions a chord of its own kind takes.
    #[pyo3(signature = (fbRules = None))]
    fn specialResolutionRules<'py>(
        slf: &Bound<'py, Self>,
        fbRules: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyList>> {
        let py = slf.py();
        let rules = Self::rules_or_default(py, fbRules)?;
        let value = Self::value(slf.as_any())?;
        let chord = value.chord();
        let flag = |name: &str| -> PyResult<bool> { rules.getattr(name)?.extract() };
        let table = PyList::empty(py);
        table.append((
            flag("resolveDominantSeventhProperly")? && chord.is_dominant_seventh(),
            slf.getattr("resolveDominantSeventhSegment")?,
        ))?;
        table.append((
            flag("resolveDiminishedSeventhProperly")? && chord.is_diminished_seventh(),
            slf.getattr("resolveDiminishedSeventhSegment")?,
            PyList::new(py, [rules.getattr("doubledRootInDim7")?])?,
        ))?;
        table.append((
            flag("resolveAugmentedSixthProperly")? && chord.is_augmented_sixth(false),
            slf.getattr("resolveAugmentedSixthSegment")?,
        ))?;
        Ok(table)
    }

    fn resolveDominantSeventhSegment<'py>(
        slf: &Bound<'py, Self>,
        segmentB: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        Self::resolved(slf.as_any(), segmentB, true, |me, other| {
            me.resolve_dominant_seventh_segment(other)
        })
    }

    #[pyo3(signature = (segmentB, doubledRoot = false))]
    fn resolveDiminishedSeventhSegment<'py>(
        slf: &Bound<'py, Self>,
        segmentB: &Bound<'py, PyAny>,
        doubledRoot: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        Self::resolved(slf.as_any(), segmentB, true, |me, other| {
            me.resolve_diminished_seventh_segment(other, doubledRoot)
        })
    }

    fn resolveAugmentedSixthSegment<'py>(
        slf: &Bound<'py, Self>,
        segmentB: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let me = Self::value(slf.as_any())?;
        let italian = me.chord().is_italian_augmented_sixth(false, false);
        Self::resolved(slf.as_any(), segmentB, !italian, |me, other| {
            me.resolve_augmented_sixth_segment(other)
        })
    }

    /// Every voicing, acceptable or not, as music21's `itertools.product`
    /// of the segment's own pitches.
    fn allSinglePossibilities<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let above = slf.getattr("allPitchesAboveBass")?;
        let parts: usize = slf.getattr("numParts")?.extract()?;
        let bass_name = slf
            .getattr("bassNote")?
            .getattr("pitch")?
            .getattr("nameWithOctave")?;
        let pitch_class = class_of(py, "music21.pitch", "Pitch", py.get_type::<Pitch>());
        let mut choices: Vec<Bound<'py, PyAny>> = vec![above; parts.saturating_sub(1)];
        choices.push(PyList::new(py, [pitch_class.call1((bass_name,))?])?.into_any());
        if slf.is_instance_of::<OverlaidSegment>() {
            let limits = slf.getattr("fbRules")?.getattr("_partPitchLimits")?;
            for limit in limits.try_iter()? {
                let (part, pitch) = crate::fbrules::pair::<usize, Bound<'py, PyAny>>(&limit?)?;
                if let Some(slot) = part.checked_sub(1).and_then(|index| choices.get_mut(index)) {
                    *slot = PyList::new(
                        py,
                        [pitch_class.call1((pitch.getattr("nameWithOctave")?,))?],
                    )?
                    .into_any();
                }
            }
        }
        py.import("itertools")?
            .getattr("product")?
            .call1(PyTuple::new(py, choices)?)
    }

    /// The voicings the rules accept, as tuples of the segment's own
    /// pitches.
    fn allCorrectSinglePossibilities<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyList>> {
        let py = slf.py();
        let value = Self::value(slf.as_any())?;
        let voicings = value
            .all_correct_single_possibilities()
            .map_err(segment_error)?;
        let mut objects = Self::own_objects(slf.as_any())?;
        let list = PyList::empty(py);
        for voicing in &voicings {
            list.append(objects.tuple(py, voicing)?)?;
        }
        Ok(list)
    }

    /// Every pair of voicings `segmentB` may follow this segment by.
    fn allCorrectConsecutivePossibilities<'py>(
        slf: &Bound<'py, Self>,
        segmentB: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let me = Self::value(slf.as_any())?;
        let chord = me.chord();
        let rules = &me.rules;
        let special = (rules.resolve_dominant_seventh_properly && chord.is_dominant_seventh())
            || (rules.resolve_diminished_seventh_properly && chord.is_diminished_seventh())
            || (rules.resolve_augmented_sixth_properly
                && chord.is_augmented_sixth(false)
                && !chord.is_italian_augmented_sixth(false, false));
        Self::resolved(slf.as_any(), segmentB, special, |me, other| {
            me.all_correct_consecutive_possibilities(other)
        })
    }
}

/// music21's `figuredBass.segment.OverlaidSegment`: a segment whose rules
/// hold some parts to a pitch in every voicing.
#[pyclass(
    name = "OverlaidSegment",
    module = "music21.figuredBass.segment",
    extends = Segment,
    subclass,
    dict
)]
pub struct OverlaidSegment;

#[pymethods]
impl OverlaidSegment {
    #[new]
    #[pyo3(signature = (*_arguments, **_keywords))]
    fn new(
        _arguments: &Bound<'_, PyTuple>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyClassInitializer<Self> {
        PyClassInitializer::from(Segment).add_subclass(OverlaidSegment)
    }
}

/// Every pitch named in `pitchNames` in every octave from `bassPitch` up to
/// `maxPitch`, lowest first.
#[pyfunction]
#[pyo3(signature = (pitchNames = None, bassPitch = None, maxPitch = None))]
pub(crate) fn getPitches<'py>(
    py: Python<'py>,
    pitchNames: Option<&Bound<'py, PyAny>>,
    bassPitch: Option<&Bound<'py, PyAny>>,
    maxPitch: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyList>> {
    let names: Vec<String> = match pitchNames.filter(|names| !names.is_none()) {
        Some(names) => names
            .try_iter()?
            .map(|name| name?.extract::<String>())
            .collect::<PyResult<_>>()?,
        None => vec!["C".to_string(), "E".to_string(), "G".to_string()],
    };
    let read = |given: Option<&Bound<'py, PyAny>>, default: &str| -> PyResult<RsPitch> {
        match given.filter(|given| !given.is_none()) {
            Some(given) => pitch_from_any(given),
            None => RsPitch::from_name(default).map_err(crate::pitch::pitch_error),
        }
    };
    let bass = read(bassPitch, "C3")?;
    let top = read(maxPitch, "C8")?;
    let found = rs::pitches(&names, &bass, &top).map_err(segment_error)?;
    let list = PyList::empty(py);
    for pitch in found {
        list.append(pitch_object(py, pitch)?)?;
    }
    Ok(list)
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Segment>()?;
    m.add_class::<OverlaidSegment>()?;
    m.add_function(wrap_pyfunction!(getPitches, m)?)?;
    Ok(())
}
