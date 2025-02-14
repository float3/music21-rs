pub(crate) mod chromaticinterval;
pub(crate) mod diatonicinterval;
pub(crate) mod direction;
pub(crate) mod genericinterval;
pub(crate) mod intervalbase;
pub(crate) mod intervalstring;
pub(crate) mod specifier;

use chromaticinterval::ChromaticInterval;
use diatonicinterval::DiatonicInterval;
use genericinterval::GenericInterval;
use intervalbase::IntervalBaseTrait;
use regex::Regex;
use specifier::Specifier;

use std::sync::Mutex;
use std::{collections::HashMap, sync::LazyLock};

use crate::base::Music21ObjectTrait;

use crate::common::numbertools::MUSICAL_ORDINAL_STRINGS;
use crate::common::stringtools::get_num_from_str;
use crate::defaults::UnsignedIntegerType;
use crate::exception::{Exception, ExceptionResult};
use crate::prebase::ProtoM21ObjectTrait;
use crate::{
    defaults::{FloatType, FractionType, IntegerType},
    fraction_pow::FractionPow,
    note::Note,
    pitch::Pitch,
};

#[derive(Clone, Debug)]
pub(crate) struct Interval {
    pub(crate) implicit_diatonic: bool,
    pub(crate) diatonic: DiatonicInterval,
    pub(crate) chromatic: ChromaticInterval,
    pitch_start: Option<Pitch>,
    pitch_end: Option<Pitch>,
}

pub(crate) enum PitchOrNote {
    Pitch(Pitch),
    Note(Note),
}

pub(crate) enum IntervalArgument {
    Str(String),
    Int(UnsignedIntegerType),
    Pitch(Pitch),
    Note(Note),
}

static PYTHAGOREAN_CACHE: LazyLock<Mutex<HashMap<String, (Pitch, FractionType)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

impl Interval {
    pub(crate) fn between(start: PitchOrNote, end: PitchOrNote) -> ExceptionResult<Self> {
        todo!()
    }

    pub(crate) fn from_diatonic_and_chromatic(
        diatonic: DiatonicInterval,
        chromatic: ChromaticInterval,
    ) -> ExceptionResult<Interval> {
        todo!()
    }

    pub fn new(arg: IntervalArgument) -> ExceptionResult<Interval> {
        match arg {
            IntervalArgument::Str(str) => {
                let name = str;
                let (diatonic_new, chromatic_new, inferred) = _string_to_diatonic_chromatic(name)?;
                todo!()
            }
            IntervalArgument::Int(int) => {
                let chromatic = ChromaticInterval::new(int);
                let diatonic = chromatic.get_diatonic();

                Ok(Self {
                    implicit_diatonic: true,
                    diatonic,
                    chromatic,
                    pitch_start: None,
                    pitch_end: None,
                })
            }
            IntervalArgument::Pitch(pitch) => todo!(),
            IntervalArgument::Note(note) => todo!(),
        }
    }

    pub(crate) fn generic(&self) -> &GenericInterval {
        &self.diatonic.generic
    }

    /// reverse default is false
    /// maxAccidental default is 4
    pub(crate) fn transpose_pitch(
        self,
        p: &Pitch,
        reverse: bool,
        max_accidental: Option<IntegerType>,
    ) -> ExceptionResult<Pitch> {
        if reverse {
            return self.reverse()?.transpose_pitch(p, false, Some(4));
        }
        let max_accidental = max_accidental.unwrap_or(IntegerType::MIN);

        if self.implicit_diatonic {
            let p_out = self.chromatic.transpose_pitch(p.clone())?;
        }
        todo!()
    }

    pub(crate) fn transpose_pitch_in_place(
        &self,
        arg: &Pitch,
        reverse: bool,
        max_accidental: Option<IntegerType>,
    ) -> ExceptionResult<()> {
        todo!()
    }
}

fn _string_to_diatonic_chromatic(
    mut value: String,
) -> ExceptionResult<(DiatonicInterval, ChromaticInterval, bool)> {
    let mut inferred = false;
    let mut dir_scale = 1;

    // Check for '-' and remove them:
    if value.contains('-') {
        value = value.replace('-', "");
        dir_scale = -1;
    }
    let mut value_lower = value.to_lowercase();

    // Remove directional words:
    {
        let descending_re = Regex::new(r"(?i)descending\s*").unwrap();
        if descending_re.is_match(&value) {
            value = descending_re.replace_all(&value, "").to_string();
            dir_scale = -1;
        } else {
            let ascending_re = Regex::new(r"(?i)ascending\s*").unwrap();
            if ascending_re.is_match(&value) {
                value = ascending_re.replace_all(&value, "").to_string();
            }
        }
    }
    value_lower = value.to_lowercase();

    // Handle whole/half abbreviations:
    if value_lower == "w" || value_lower == "whole" || value_lower == "tone" {
        value = "M2".to_string();
        inferred = true;
    } else if value_lower == "h" || value_lower == "half" || value_lower == "semitone" {
        value = "m2".to_string();
        inferred = true;
    }

    // Replace any music ordinal in the string with its index.
    for (i, ordinal) in MUSICAL_ORDINAL_STRINGS.iter().enumerate() {
        if value.to_lowercase().contains(&ordinal.to_lowercase()) {
            let pattern = format!(r"(?i)\s*{}\s*", regex::escape(ordinal));
            let re = Regex::new(&pattern).unwrap();
            value = re.replace_all(&value, i.to_string().as_str()).to_string();
        }
    }

    // Extract number and remaining spec:
    let (found, remain) = get_num_from_str(&value, "0123456789");
    let generic_number: IntegerType = found
        .parse::<IntegerType>()
        .expect("Failed to parse number")
        * dir_scale;
    let spec_name = Specifier::parse(remain);

    let g_interval = GenericInterval::from_int(generic_number)?;
    let d_interval = g_interval.get_diatonic(spec_name);
    let c_interval = d_interval.get_chromatic();
    Ok((d_interval, c_interval, inferred))
}

impl IntervalBaseTrait for Interval {
    fn reverse(self) -> ExceptionResult<Self>
    where
        Self: Sized,
    {
        if let (Some(start), Some(end)) = (self.pitch_start, self.pitch_end) {
            Interval::between(PitchOrNote::Pitch(end), PitchOrNote::Pitch(start))
        } else {
            Interval::from_diatonic_and_chromatic(
                self.diatonic.reverse()?,
                self.chromatic.reverse()?,
            )
        }
    }

    fn transpose_note(self, note1: Note) -> ExceptionResult<Note> {
        todo!()
    }

    fn transpose_pitch(self, pitch1: Pitch) -> ExceptionResult<Pitch> {
        todo!()
    }

    fn transpose_pitch_in_place(self, pitch1: &mut Pitch) -> ExceptionResult<()> {
        todo!()
    }
}

impl Music21ObjectTrait for Interval {}

impl ProtoM21ObjectTrait for Interval {}

pub(crate) fn interval_to_pythagorean_ratio(interval: Interval) -> ExceptionResult<FractionType> {
    let start_pitch = Pitch::new(
        Some("C1".to_string()),
        None,
        None,
        Option::<i8>::None,
        Option::<IntegerType>::None,
        None,
        None,
        None,
        None,
    )?;

    let end_pitch_wanted = start_pitch.transpose((interval).clone());

    let mut cache = match PYTHAGOREAN_CACHE.lock() {
        Ok(cache) => cache,
        Err(poisoned) => poisoned.into_inner(),
    };

    if let Some((cached_pitch, cached_ratio)) = cache.get(&end_pitch_wanted.name()).cloned() {
        let octaves = (end_pitch_wanted.ps() - cached_pitch.ps()) / 12.0;
        let octave_multiplier = FractionPow::<IntegerType, FloatType, UnsignedIntegerType>::powi(
            &FractionType::new(2 as IntegerType, 1 as IntegerType),
            octaves as IntegerType,
        );
        return Ok(cached_ratio * octave_multiplier);
    }

    let mut end_pitch_up = start_pitch.clone();
    let mut end_pitch_down = start_pitch.clone();
    let mut found: Option<(Pitch, FractionType)> = None;

    for counter in 0..37 {
        if end_pitch_up.name() == end_pitch_wanted.name() {
            found = Some((
                end_pitch_up.clone(),
                FractionPow::<IntegerType, FloatType, UnsignedIntegerType>::powi(
                    &FractionType::new(3i32, 2i32),
                    counter,
                ),
            ));
            break;
        } else if end_pitch_down.name() == end_pitch_wanted.name() {
            found = Some((
                end_pitch_down.clone(),
                FractionPow::<IntegerType, FloatType, UnsignedIntegerType>::powi(
                    &FractionType::new(2i32, 3i32),
                    counter,
                ),
            ));
            break;
        } else {
            end_pitch_up =
                end_pitch_up.transpose(Interval::new(IntervalArgument::Str("P5".to_string()))?);
            end_pitch_down =
                end_pitch_down.transpose(Interval::new(IntervalArgument::Str("-P5".to_string()))?);
        }
    }

    let (found_pitch, found_ratio) = match found {
        Some(val) => val,
        None => {
            return Err(Exception::Interval(format!(
                "Could not find a pythagorean ratio for {:?}",
                interval
            )))
        }
    };

    cache.insert(
        end_pitch_wanted.name().clone(),
        (found_pitch.clone(), found_ratio),
    );

    let octaves = (end_pitch_wanted.ps() - found_pitch.ps()) / 12.0;
    let octave_multiplier = FractionPow::<IntegerType, FloatType, UnsignedIntegerType>::powi(
        &FractionType::new(2i32, 1i32),
        octaves as IntegerType,
    );

    Ok(found_ratio * octave_multiplier)
}

pub trait IntoInterval {
    fn into_interval_arg();
}
