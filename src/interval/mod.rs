pub(crate) mod chromaticinterval;
pub(crate) mod diatonicinterval;
pub(crate) mod genericinterval;
pub(crate) mod intervalbase;
pub(crate) mod specifier;

use chromaticinterval::ChromaticInterval;
use diatonicinterval::DiatonicInterval;
use genericinterval::GenericInterval;
use intervalbase::IntervalBaseTrait;

use std::sync::Mutex;
use std::{collections::HashMap, sync::LazyLock};

use crate::base::Music21ObjectTrait;

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
}

pub(crate) enum PitchOrNote {
    Pitch(Pitch),
    Note(Note),
}

pub(crate) enum Arg1 {
    Str(String),
    Int(IntegerType),
    Float(FloatType),
}

static PYTHAGOREAN_CACHE: LazyLock<Mutex<HashMap<String, (Pitch, FractionType)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

impl Interval {
    pub(crate) fn between(
        arg0: Option<PitchOrNote>,
        arg1: Option<PitchOrNote>,
    ) -> ExceptionResult<Self> {
        todo!()
    }

    fn new(arg: Arg1) -> Interval {
        todo!()
    }

    pub(crate) fn generic(&self) -> &GenericInterval {
        &self.diatonic.generic
    }
}

impl IntervalBaseTrait for Interval {}

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
            &FractionType::new(2i64, 1i64),
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
            end_pitch_up = end_pitch_up.transpose(Interval::new(Arg1::Str("P5".to_string())));
            end_pitch_down = end_pitch_down.transpose(Interval::new(Arg1::Str("-P5".to_string())));
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
