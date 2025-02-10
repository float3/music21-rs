use super::Pitch;

use crate::common::objects::slottedobjectmixin::{SlottedObjectMixin, SlottedObjectMixinTrait};
use crate::defaults::FloatType;
use crate::display::{DisplayLocation, DisplaySize, DisplayStyle, DisplayType};
use crate::exception::{Exception, ExceptionResult};
use crate::prebase::{ProtoM21Object, ProtoM21ObjectTrait};

use std::fmt::{Display, Formatter};
use std::sync::Arc;

enum AccidentalEnum {
    Natural,
    HalfSharp,
    Sharp,
    OneAndAHalfSharp,
    DoubleSharp,
    TripleSharp,
    QuadrupleSharp,
    HalfFlat,
    Flat,
    OneAndAHalfFlat,
    DoubleFlat,
    TripleFlat,
    QuadrupleFlat,
}

impl AccidentalEnum {
    fn to_name(&self) -> &'static str {
        match self {
            AccidentalEnum::Natural => "natural",
            AccidentalEnum::HalfSharp => "half-sharp",
            AccidentalEnum::Sharp => "sharp",
            AccidentalEnum::OneAndAHalfSharp => "one-and-a-half-sharp",
            AccidentalEnum::DoubleSharp => "double-sharp",
            AccidentalEnum::TripleSharp => "triple-sharp",
            AccidentalEnum::QuadrupleSharp => "quadruple-sharp",
            AccidentalEnum::HalfFlat => "half-flat",
            AccidentalEnum::Flat => "flat",
            AccidentalEnum::OneAndAHalfFlat => "one-and-a-half-flat",
            AccidentalEnum::DoubleFlat => "double-flat",
            AccidentalEnum::TripleFlat => "triple-flat",
            AccidentalEnum::QuadrupleFlat => "quadruple-flat",
        }
    }

    fn from_name(s: &str) -> Option<Self> {
        match s {
            "natural" => Some(AccidentalEnum::Natural),
            "half-sharp" => Some(AccidentalEnum::HalfSharp),
            "sharp" => Some(AccidentalEnum::Sharp),
            "one-and-a-half-sharp" => Some(AccidentalEnum::OneAndAHalfSharp),
            "double-sharp" => Some(AccidentalEnum::DoubleSharp),
            "triple-sharp" => Some(AccidentalEnum::TripleSharp),
            "quadruple-sharp" => Some(AccidentalEnum::QuadrupleSharp),
            "half-flat" => Some(AccidentalEnum::HalfFlat),
            "flat" => Some(AccidentalEnum::Flat),
            "one-and-a-half-flat" => Some(AccidentalEnum::OneAndAHalfFlat),
            "double-flat" => Some(AccidentalEnum::DoubleFlat),
            "triple-flat" => Some(AccidentalEnum::TripleFlat),
            "quadruple-flat" => Some(AccidentalEnum::QuadrupleFlat),
            _ => None,
        }
    }

    fn to_alter(&self) -> FloatType {
        match self {
            AccidentalEnum::Natural => 0.0,
            AccidentalEnum::HalfSharp => 0.5,
            AccidentalEnum::Sharp => 1.0,
            AccidentalEnum::OneAndAHalfSharp => 1.5,
            AccidentalEnum::DoubleSharp => 2.0,
            AccidentalEnum::TripleSharp => 3.0,
            AccidentalEnum::QuadrupleSharp => 4.0,
            AccidentalEnum::HalfFlat => -0.5,
            AccidentalEnum::Flat => -1.0,
            AccidentalEnum::OneAndAHalfFlat => -1.5,
            AccidentalEnum::DoubleFlat => -2.0,
            AccidentalEnum::TripleFlat => -3.0,
            AccidentalEnum::QuadrupleFlat => -4.0,
        }
    }

    fn from_alter(f: FloatType) -> Option<Self> {
        match f {
            -4.0 => Some(AccidentalEnum::QuadrupleFlat),
            -3.0 => Some(AccidentalEnum::TripleFlat),
            -2.0 => Some(AccidentalEnum::DoubleFlat),
            -1.5 => Some(AccidentalEnum::OneAndAHalfFlat),
            -1.0 => Some(AccidentalEnum::Flat),
            -0.5 => Some(AccidentalEnum::HalfFlat),
            0.0 => Some(AccidentalEnum::Natural),
            0.5 => Some(AccidentalEnum::HalfSharp),
            1.0 => Some(AccidentalEnum::Sharp),
            1.5 => Some(AccidentalEnum::OneAndAHalfSharp),
            2.0 => Some(AccidentalEnum::DoubleSharp),
            3.0 => Some(AccidentalEnum::TripleSharp),
            4.0 => Some(AccidentalEnum::QuadrupleSharp),
            _ => None,
        }
    }

    fn to_alter_str(&self) -> &'static str {
        match self {
            AccidentalEnum::Natural => "0.0",
            AccidentalEnum::HalfSharp => "0.5",
            AccidentalEnum::Sharp => "1.0",
            AccidentalEnum::OneAndAHalfSharp => "1.5",
            AccidentalEnum::DoubleSharp => "2.0",
            AccidentalEnum::TripleSharp => "3.0",
            AccidentalEnum::QuadrupleSharp => "4.0",
            AccidentalEnum::HalfFlat => "-0.5",
            AccidentalEnum::Flat => "-1.0",
            AccidentalEnum::OneAndAHalfFlat => "-1.5",
            AccidentalEnum::DoubleFlat => "-2.0",
            AccidentalEnum::TripleFlat => "-3.0",
            AccidentalEnum::QuadrupleFlat => "-4.0",
        }
    }

    fn from_alter_str(s: &str) -> Option<Self> {
        match s {
            "-4.0" => Some(AccidentalEnum::QuadrupleFlat),
            "-3.0" => Some(AccidentalEnum::TripleFlat),
            "-2.0" => Some(AccidentalEnum::DoubleFlat),
            "-1.5" => Some(AccidentalEnum::OneAndAHalfFlat),
            "-1.0" => Some(AccidentalEnum::Flat),
            "-0.5" => Some(AccidentalEnum::HalfFlat),
            "0.0" => Some(AccidentalEnum::Natural),
            "0.5" => Some(AccidentalEnum::HalfSharp),
            "1.0" => Some(AccidentalEnum::Sharp),
            "1.5" => Some(AccidentalEnum::OneAndAHalfSharp),
            "2.0" => Some(AccidentalEnum::DoubleSharp),
            "3.0" => Some(AccidentalEnum::TripleSharp),
            "4.0" => Some(AccidentalEnum::QuadrupleSharp),
            _ => None,
        }
    }

    fn to_modifier(&self) -> &'static str {
        match self {
            AccidentalEnum::Natural => "",
            AccidentalEnum::HalfSharp => "~",
            AccidentalEnum::Sharp => "#",
            AccidentalEnum::OneAndAHalfSharp => "#~",
            AccidentalEnum::DoubleSharp => "##",
            AccidentalEnum::TripleSharp => "###",
            AccidentalEnum::QuadrupleSharp => "####",
            AccidentalEnum::HalfFlat => "`",
            AccidentalEnum::Flat => "-",
            AccidentalEnum::OneAndAHalfFlat => "-`",
            AccidentalEnum::DoubleFlat => "--",
            AccidentalEnum::TripleFlat => "---",
            AccidentalEnum::QuadrupleFlat => "----",
        }
    }

    fn from_modifier(s: &str) -> Option<Self> {
        match s {
            "" => Some(AccidentalEnum::Natural),
            "~" => Some(AccidentalEnum::HalfSharp),
            "#" => Some(AccidentalEnum::Sharp),
            "#~" => Some(AccidentalEnum::OneAndAHalfSharp),
            "##" => Some(AccidentalEnum::DoubleSharp),
            "###" => Some(AccidentalEnum::TripleSharp),
            "####" => Some(AccidentalEnum::QuadrupleSharp),
            "`" => Some(AccidentalEnum::HalfFlat),
            "-" => Some(AccidentalEnum::Flat),
            "-`" => Some(AccidentalEnum::OneAndAHalfFlat),
            "--" => Some(AccidentalEnum::DoubleFlat),
            "---" => Some(AccidentalEnum::TripleFlat),
            "----" => Some(AccidentalEnum::QuadrupleFlat),
            _ => None,
        }
    }

    const fn to_unicode(&self) -> &'static str {
        match self {
            AccidentalEnum::QuadrupleSharp => "\u{1d12a}\u{1d12a}",
            AccidentalEnum::TripleSharp => "\u{266f}\u{1d12a}",
            AccidentalEnum::DoubleSharp => "\u{1d12a}",
            AccidentalEnum::OneAndAHalfSharp => "\u{266f}\u{1d132}",
            AccidentalEnum::Sharp => "\u{266f}",
            AccidentalEnum::HalfSharp => "\u{1d132}",
            AccidentalEnum::QuadrupleFlat => "\u{1d12b}\u{1d12b}",
            AccidentalEnum::TripleFlat => "\u{266d}",
            AccidentalEnum::DoubleFlat => "\u{1d12b}",
            AccidentalEnum::OneAndAHalfFlat => "\u{266d}\u{1d132}",
            AccidentalEnum::Flat => "\u{266d}",
            AccidentalEnum::HalfFlat => "\u{1d132}",
            AccidentalEnum::Natural => "\u{266e}",
        }
    }

    #[allow(unreachable_patterns)]
    fn from_unicode(s: &str) -> Option<Self> {
        match s {
            "\u{1d12a}\u{1d12a}" => Some(AccidentalEnum::QuadrupleSharp),
            "\u{266f}\u{1d12a}" => Some(AccidentalEnum::TripleSharp),
            "\u{1d12a}" => Some(AccidentalEnum::DoubleSharp),
            "\u{266f}\u{1d132}" => Some(AccidentalEnum::OneAndAHalfSharp),
            "\u{266f}" => Some(AccidentalEnum::Sharp),
            "\u{1d12b}\u{1d12b}" => Some(AccidentalEnum::QuadrupleFlat),
            "\u{266d}" => Some(AccidentalEnum::TripleFlat),
            "\u{1d12b}" => Some(AccidentalEnum::DoubleFlat),
            "\u{266d}\u{1d132}" => Some(AccidentalEnum::OneAndAHalfFlat),
            "\u{1d132}" => Some(AccidentalEnum::HalfSharp),
            "\u{1d132}" => Some(AccidentalEnum::HalfFlat),
            "\u{266e}" => Some(AccidentalEnum::Natural),
            _ => None,
        }
    }

    fn from_alternate_name(s: &str) -> Option<Self> {
        match s {
            "n" => Some(AccidentalEnum::Natural),
            "is" => Some(AccidentalEnum::Sharp),
            "isis" => Some(AccidentalEnum::DoubleSharp),
            "isisis" => Some(AccidentalEnum::TripleSharp),
            "isisisis" => Some(AccidentalEnum::QuadrupleSharp),
            "ih" | "quarter-sharp" | "semisharp" => Some(AccidentalEnum::HalfSharp),
            "isih" | "three-quarter-sharp" | "three-quarters-sharp" | "sesquisharp" => {
                Some(AccidentalEnum::OneAndAHalfSharp)
            }
            "b" | "es" => Some(AccidentalEnum::Flat),
            "eses" => Some(AccidentalEnum::DoubleFlat),
            "eseses" => Some(AccidentalEnum::TripleFlat),
            "eseseses" => Some(AccidentalEnum::QuadrupleFlat),
            "eh" | "quarter-flat" | "semiflat" => Some(AccidentalEnum::HalfFlat),
            "eseh" | "three-quarter-flat" | "three-quarters-flat" | "sesquiflat" => {
                Some(AccidentalEnum::OneAndAHalfFlat)
            }
            _ => None,
        }
    }

    fn from_int(i: i8) -> Option<Self> {
        match i {
            -4 => Some(AccidentalEnum::QuadrupleFlat),
            -3 => Some(AccidentalEnum::TripleFlat),
            -2 => Some(AccidentalEnum::DoubleFlat),
            -1 => Some(AccidentalEnum::Flat),
            0 => Some(AccidentalEnum::Natural),
            1 => Some(AccidentalEnum::Sharp),
            2 => Some(AccidentalEnum::DoubleSharp),
            3 => Some(AccidentalEnum::TripleSharp),
            4 => Some(AccidentalEnum::QuadrupleSharp),
            _ => None,
        }
    }

    fn from_float(f: FloatType) -> Option<Self> {
        match f {
            -4.0 => Some(AccidentalEnum::QuadrupleFlat),
            -3.0 => Some(AccidentalEnum::TripleFlat),
            -2.0 => Some(AccidentalEnum::DoubleFlat),
            -1.5 => Some(AccidentalEnum::OneAndAHalfFlat),
            -1.0 => Some(AccidentalEnum::Flat),
            -0.5 => Some(AccidentalEnum::HalfFlat),
            0.0 => Some(AccidentalEnum::Natural),
            0.5 => Some(AccidentalEnum::HalfSharp),
            1.0 => Some(AccidentalEnum::Sharp),
            1.5 => Some(AccidentalEnum::OneAndAHalfSharp),
            2.0 => Some(AccidentalEnum::DoubleSharp),
            3.0 => Some(AccidentalEnum::TripleSharp),
            4.0 => Some(AccidentalEnum::QuadrupleSharp),
            _ => None,
        }
    }

    fn from_string(s: &str) -> Option<Self> {
        AccidentalEnum::from_name(s)
            .or_else(|| AccidentalEnum::from_modifier(s))
            .or_else(|| AccidentalEnum::from_alter_str(s))
            .or_else(|| AccidentalEnum::from_unicode(s))
            .or_else(|| AccidentalEnum::from_alternate_name(s))
    }

    fn to_name_and_alter(&self) -> (String, FloatType) {
        (self.to_name().to_string(), self.to_alter())
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Accidental {
    proto: ProtoM21Object,
    slottedobjectmixin: SlottedObjectMixin,
    _display_type: DisplayType,
    _display_status: Option<bool>,
    display_style: DisplayStyle,
    display_size: DisplaySize,
    display_location: DisplayLocation,
    _client: Option<Arc<Pitch>>,
    _name: String,
    _modifier: String,
    pub(crate) _alter: FloatType,
}

impl PartialEq for Accidental {
    fn eq(&self, other: &Self) -> bool {
        self._name == other._name
    }
}

impl Accidental {
    pub(crate) fn new<T>(specifier: T) -> ExceptionResult<Self>
    where
        T: IntoAccidental,
    {
        let mut acci = Self {
            proto: ProtoM21Object::new(),
            slottedobjectmixin: SlottedObjectMixin::new(),
            _display_type: DisplayType::Normal,
            _display_status: None,
            display_style: DisplayStyle::Normal,
            display_size: DisplaySize::Full,
            display_location: DisplayLocation::Normal,
            _client: None,
            _name: "".to_string(),
            _modifier: "".to_string(),
            _alter: 0.0,
        };

        acci.set(specifier, false)?;
        Ok(acci)
    }

    fn set<T>(&mut self, name: T, allow_non_standard_value: bool) -> ExceptionResult<()>
    where
        T: IntoAccidental,
    {
        let acci_tuple = name.clone().accidental_args(allow_non_standard_value);

        if allow_non_standard_value {
            assert!(acci_tuple.is_some());
        }

        match acci_tuple {
            Some(acci_tuple) => {
                self._name = acci_tuple.0;
                self._alter = acci_tuple.1;
            }
            None => {
                return Err(Exception::Accidental(format!(
                    "{} is not a supported accidental type",
                    name
                )));
            }
        }

        let name: Option<AccidentalEnum> = AccidentalEnum::from_name(&self._name);

        match name {
            Some(n) => self._modifier = n.to_modifier().to_owned(),
            None => panic!("Expected name to be Some"),
        }

        if let Some(client) = &self._client {
            client.inform_client();
        }

        Ok(())
    }

    pub(crate) fn modifier(&self) -> &str {
        todo!()
    }

    pub(crate) fn natural() -> Accidental {
        let x = Accidental::new("n");
        assert!(x.is_ok());
        match x {
            Ok(val) => val,
            Err(err) => panic!("creating a natural Accidental should never fail: {}", err),
        }
    }
}

impl Default for Accidental {
    fn default() -> Self {
        Self::natural()
    }
}

impl std::fmt::Display for Accidental {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self._name)
    }
}

impl ProtoM21ObjectTrait for Accidental {}

impl SlottedObjectMixinTrait for Accidental {}

pub(crate) trait IntoAccidental: Display + Clone {
    fn accidental_args(self, allow_non_standard_values: bool) -> Option<(String, FloatType)>;
    fn is_accidental(&self) -> bool;
    fn into_accidental(self) -> ExceptionResult<Accidental>;
    fn accidental(self) -> Accidental;
}

impl IntoAccidental for i8 {
    fn accidental_args(self, allow_non_standard_values: bool) -> Option<(String, FloatType)> {
        match AccidentalEnum::from_int(self) {
            Some(acci) => Some(acci.to_name_and_alter()),
            _ if allow_non_standard_values => Some(("".to_owned(), self as FloatType)),
            _ => None,
        }
    }

    fn is_accidental(&self) -> bool {
        false
    }

    fn into_accidental(self) -> ExceptionResult<Accidental> {
        Accidental::new(self)
    }

    fn accidental(self) -> Accidental {
        panic!("call into_accidental instead")
    }
}

impl IntoAccidental for FloatType {
    fn accidental_args(self, allow_non_standard_values: bool) -> Option<(String, FloatType)> {
        match AccidentalEnum::from_float(self) {
            Some(acci) => Some(acci.to_name_and_alter()),
            _ if allow_non_standard_values => Some(("".to_owned(), self)),
            _ => None,
        }
    }

    fn is_accidental(&self) -> bool {
        false
    }

    fn into_accidental(self) -> ExceptionResult<Accidental> {
        Accidental::new(self)
    }

    fn accidental(self) -> Accidental {
        panic!("call into_accidental instead")
    }
}

impl IntoAccidental for &str {
    fn accidental_args(self, allow_non_standard_values: bool) -> Option<(String, FloatType)> {
        self.to_string().accidental_args(allow_non_standard_values)
    }

    fn is_accidental(&self) -> bool {
        false
    }

    fn into_accidental(self) -> ExceptionResult<Accidental> {
        Accidental::new(self)
    }

    fn accidental(self) -> Accidental {
        panic!("call into_accidental instead")
    }
}

impl IntoAccidental for String {
    fn accidental_args(self, allow_non_standard_values: bool) -> Option<(String, FloatType)> {
        match AccidentalEnum::from_string(self.to_lowercase().as_str()) {
            Some(acci) => Some(acci.to_name_and_alter()),
            _ if allow_non_standard_values => Some((self, 0.0)),
            _ => None,
        }
    }

    fn is_accidental(&self) -> bool {
        false
    }

    fn into_accidental(self) -> ExceptionResult<Accidental> {
        Accidental::new(self)
    }

    fn accidental(self) -> Accidental {
        panic!("call into_accidental instead")
    }
}

impl IntoAccidental for Accidental {
    fn accidental_args(self, allow_non_standard_values: bool) -> Option<(String, FloatType)> {
        Some((self._name, self._alter))
    }

    fn is_accidental(&self) -> bool {
        true
    }

    fn into_accidental(self) -> ExceptionResult<Accidental> {
        panic!("don't call into_accidental on an accidental");
    }

    fn accidental(self) -> Accidental {
        self
    }
}
