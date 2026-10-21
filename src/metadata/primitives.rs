//! The values a score's metadata is made of: music21's
//! `metadata.primitives`.
//!
//! A metadata value is not only a string. A title has a language and may be
//! a translation; a contributor has a role; and a date may be approximate,
//! uncertain, a date before or after which something happened, a span
//! between two dates, or a choice among several. These are the types that
//! say so.
//!
//! **A date is music21's own, not a calendar date.** Any part of it may be
//! missing -- a year alone, or a year and a month -- and any part may be
//! marked approximate or uncertain, which is why music21 wrote a type of its
//! own rather than using Python's `datetime`. A date in the distant past is
//! as good as a recent one; what is not modelled is a date before the common
//! era, which music21 does not model either, nor milliseconds, which it
//! drops on purpose ("this is a tool for musicology and not for file
//! timestamps").

use std::fmt;

use crate::{
    defaults::IntegerType,
    error::{Error, Result},
};

/// How sure one part of a [`Date`] is: music21's error strings, written as
/// the symbols it writes them with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Certainty {
    /// Near enough: music21's `approximate`, written `~`.
    Approximate,
    /// Not known to be right: music21's `uncertain`, written `?`.
    Uncertain,
    /// music21's `priority`, which its own `_stripError` reads off the
    /// bracket symbols `<`, `{`, `>` and `}`. It has no symbol of its own,
    /// and nothing of music21's writes one.
    Priority,
}

impl Certainty {
    /// The name music21 gives it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Approximate => "approximate",
            Self::Uncertain => "uncertain",
            Self::Priority => "priority",
        }
    }

    /// The symbol music21 writes after the part it marks, and nothing for
    /// `priority`: music21's `errorToSymbol`.
    #[must_use]
    pub fn symbol(self) -> Option<char> {
        match self {
            Self::Approximate => Some('~'),
            Self::Uncertain => Some('?'),
            Self::Priority => None,
        }
    }

    /// What a name or a symbol stands for, read as music21's
    /// `errorToSymbol` and `_stripError` read one.
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "approximate" | "~" | "x" => Some(Self::Approximate),
            "uncertain" | "?" | "z" => Some(Self::Uncertain),
            "priority" | "<" | "{" | ">" | "}" => Some(Self::Priority),
            _ => None,
        }
    }

    /// The certainty a symbol inside a written number marks, as music21's
    /// `_stripError` reads the first one it meets.
    fn of_symbol(symbol: char) -> Option<Self> {
        match symbol {
            '~' | 'x' => Some(Self::Approximate),
            '?' | 'z' => Some(Self::Uncertain),
            '<' | '{' | '>' | '}' => Some(Self::Priority),
            _ => None,
        }
    }
}

/// One part of a [`Date`], in the order music21 writes them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DatePart {
    /// The year.
    Year,
    /// The month, one to twelve.
    Month,
    /// The day of the month.
    Day,
    /// The hour, nought to twenty-three.
    Hour,
    /// The minute.
    Minute,
    /// The second.
    Second,
}

impl DatePart {
    /// Every part, in the order music21 writes them: music21's `attrNames`.
    pub const ALL: [DatePart; 6] = [
        Self::Year,
        Self::Month,
        Self::Day,
        Self::Hour,
        Self::Minute,
        Self::Second,
    ];

    /// Where the part stands among the six.
    fn at(self) -> usize {
        match self {
            Self::Year => 0,
            Self::Month => 1,
            Self::Day => 2,
            Self::Hour => 3,
            Self::Minute => 4,
            Self::Second => 5,
        }
    }

    /// The name music21 gives it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Year => "year",
            Self::Month => "month",
            Self::Day => "day",
            Self::Hour => "hour",
            Self::Minute => "minute",
            Self::Second => "second",
        }
    }
}

/// A date as music21 keeps one: music21's `metadata.Date`. Any part may be
/// missing, and any part may say how sure it is.
///
/// ```
/// use music21_rs::metadata::primitives::{Certainty, Date, DatePart};
///
/// let read = Date::from_written("1030?/12~/?4")?;
/// assert_eq!(read.value(DatePart::Year), Some(1030));
/// assert_eq!(read.certainty(DatePart::Year), Some(Certainty::Uncertain));
/// assert_eq!(read.to_string(), "1030?/12~/04?");
/// # Ok::<(), music21_rs::Error>(())
/// ```
#[derive(Clone, Debug, Default, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct Date {
    values: [Option<IntegerType>; 6],
    certainties: [Option<Certainty>; 6],
}

/// Two dates are equal where they are written alike, which is music21's own
/// `__eq__` -- it compares the two as strings.
impl PartialEq for Date {
    fn eq(&self, other: &Self) -> bool {
        self.to_string() == other.to_string()
    }
}

impl std::hash::Hash for Date {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.to_string().hash(state);
    }
}

impl Date {
    /// A date saying nothing at all.
    pub fn new() -> Self {
        Self::default()
    }

    /// The value of one part, where it has one.
    pub fn value(&self, part: DatePart) -> Option<IntegerType> {
        self.values[part.at()]
    }

    /// How sure one part is, where it says.
    pub fn certainty(&self, part: DatePart) -> Option<Certainty> {
        self.certainties[part.at()]
    }

    /// The year, where the date gives one.
    pub fn year(&self) -> Option<IntegerType> {
        self.value(DatePart::Year)
    }

    /// The month, where the date gives one.
    pub fn month(&self) -> Option<IntegerType> {
        self.value(DatePart::Month)
    }

    /// The day, where the date gives one.
    pub fn day(&self) -> Option<IntegerType> {
        self.value(DatePart::Day)
    }

    /// Gives one part a value, with how sure it is.
    ///
    /// # Errors
    ///
    /// A value no date can have -- a thirteenth month, the thirty-first of
    /// April, the twenty-ninth of February in a year that is not a leap
    /// year. music21 checks the same, and leaves the value it was given in
    /// place when it raises.
    pub fn set(
        &mut self,
        part: DatePart,
        value: Option<IntegerType>,
        certainty: Option<Certainty>,
    ) -> Result<()> {
        self.values[part.at()] = value;
        if certainty.is_some() {
            self.certainties[part.at()] = certainty;
        }
        self.checked()
    }

    /// The date written as music21 writes one: `year/month/day` with `--`
    /// for a part it does not give, each part followed by its symbol, and
    /// `/hour:minute:second` after it where any of those is given -- the
    /// hour after a slash and the rest after colons, which is how
    /// music21's own `__str__` joins them.
    ///
    /// This is [`fmt::Display`]; the method is here because every reading of
    /// a date goes through it.
    fn written(&self) -> String {
        let has_time = DatePart::ALL[3..]
            .iter()
            .any(|part| self.value(*part).is_some());
        let parts = if has_time { 6 } else { 3 };
        let mut written: Vec<String> = Vec::with_capacity(parts);
        for part in &DatePart::ALL[..parts] {
            match self.value(*part) {
                None => written.push("--".to_string()),
                Some(value) => {
                    let mut text = if *part == DatePart::Year {
                        format!("{value:04}")
                    } else {
                        format!("{value:02}")
                    };
                    if let Some(symbol) = self.certainty(*part).and_then(Certainty::symbol) {
                        text.push(symbol);
                    }
                    written.push(text);
                }
            }
        }
        // music21 joins the first four parts with slashes and the rest with
        // colons, so the hour goes after a slash: `1030/12/04/04:50:32`.
        let mut out = written[..written.len().min(4)].join("/");
        if written.len() > 4 {
            out.push(':');
            out.push_str(&written[4..].join(":"));
        }
        out
    }

    /// Whether any part of the time of day is given: music21's `hasTime`.
    pub fn has_time(&self) -> bool {
        DatePart::ALL[3..]
            .iter()
            .any(|part| self.value(*part).is_some())
    }

    /// Whether any part says how sure it is: music21's `hasError`.
    pub fn has_certainty(&self) -> bool {
        self.certainties.iter().any(Option::is_some)
    }

    /// A date read from the way music21 writes one: music21's `loadStr`.
    /// A part written `--` says nothing; a symbol anywhere in a part says
    /// how sure it is. Colons are read as slashes, so
    /// `1834/12/4/4:50:32` and `1834/12/4:4:50:32` are one date.
    ///
    /// # Errors
    ///
    /// A part that is not a number, or a value no date can have.
    pub fn from_written(written: &str) -> Result<Self> {
        let mut date = Self::new();
        let written = written.replace(':', "/").replace(' ', "");
        for (part, chunk) in DatePart::ALL.iter().zip(written.split('/')) {
            if chunk == "--" {
                continue;
            }
            let (value, certainty) = strip_certainty(chunk)?;
            date.values[part.at()] = Some(value);
            if certainty.is_some() {
                date.certainties[part.at()] = certainty;
            }
        }
        date.checked()?;
        Ok(date)
    }

    /// A date of a year, and of a month and a day where they are given.
    ///
    /// # Errors
    ///
    /// A value no date can have.
    pub fn from_parts(
        year: Option<IntegerType>,
        month: Option<IntegerType>,
        day: Option<IntegerType>,
    ) -> Result<Self> {
        let mut date = Self::new();
        date.values[0] = year;
        date.values[1] = month;
        date.values[2] = day;
        date.checked()?;
        Ok(date)
    }

    /// The parts of another date that it gives, written over this one's,
    /// with their certainties: music21's `loadOther`.
    ///
    /// # Errors
    ///
    /// A value no date can have.
    pub fn take_from(&mut self, other: &Self) -> Result<()> {
        for part in DatePart::ALL {
            if let Some(value) = other.value(part) {
                self.values[part.at()] = Some(value);
                if let Some(certainty) = other.certainty(part) {
                    self.certainties[part.at()] = Some(certainty);
                }
            }
        }
        self.checked()
    }

    /// music21's `_sanityCheck`. It does not check whether a Gregorian leap
    /// year really was one, "as it changes historically", and neither does
    /// this.
    fn checked(&self) -> Result<()> {
        let (year, month, day) = (self.year(), self.month(), self.day());
        if let Some(month) = month
            && !(1..=12).contains(&month)
        {
            return Err(Error::Value(format!(
                "Month must be between 1 and 12, not {month}."
            )));
        }
        if let Some(day) = day {
            let impossible = match month {
                Some(4 | 6 | 9 | 11) => day == 31,
                Some(2) => day > 29 || (day == 29 && year.is_some_and(|year| year % 4 != 0)),
                _ => false,
            };
            if !(1..=31).contains(&day) || impossible {
                let month = month.map_or_else(|| "None".to_string(), |month| month.to_string());
                return Err(Error::Value(format!(
                    "Day {day} is not possible with month {month}."
                )));
            }
        }
        if self.value(DatePart::Hour).is_some_and(|hour| hour > 23)
            || self.value(DatePart::Hour).is_some_and(|hour| hour < 0)
        {
            return Err(Error::Value("Hour must be between 0 and 23".to_string()));
        }
        if self
            .value(DatePart::Minute)
            .is_some_and(|minute| !(0..=59).contains(&minute))
        {
            return Err(Error::Value("Minute must be between 0 and 59".to_string()));
        }
        if self
            .value(DatePart::Second)
            .is_some_and(|second| !(0..=59).contains(&second))
        {
            return Err(Error::Value("Second must be between 0 and 59".to_string()));
        }
        Ok(())
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.written())
    }
}

/// A written number with its certainty taken off it: music21's
/// `_stripError`. Only one symbol is expected, and the first one met is the
/// one read. A fraction is read as the whole number before the point --
/// music21 drops milliseconds on purpose.
///
/// # Errors
///
/// Something that is not a number at all.
fn strip_certainty(written: &str) -> Result<(IntegerType, Option<Certainty>)> {
    let mut certainty = None;
    let mut cleaned = written.to_string();
    for character in written.chars() {
        if let Some(found) = Certainty::of_symbol(character) {
            certainty = Some(found);
            cleaned = written.replace(character, "");
            break;
        }
    }
    let read: f64 = cleaned
        .parse()
        .map_err(|_| Error::Value(format!("{written} is not a date")))?;
    Ok((read as IntegerType, certainty))
}

/// What a [`DatePrimitive`] says about the dates it holds: music21's
/// `relevance`, which is what tells its four date classes apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Relevance {
    /// The date is the date: music21's `DateSingle('...')`.
    Certain,
    /// Near enough.
    Approximate,
    /// Not known to be right.
    Uncertain,
    /// Something happened before this date: music21's
    /// `DateRelative(..., 'prior')`, written `prior to ...`.
    Prior,
    /// After this date, written `after ...`.
    After,
    /// On this date or before it, written `... or earlier`.
    OnOrBefore,
    /// On this date or after it, written `... or later`.
    OnOrAfter,
    /// Between two dates: music21's `DateBetween`, written `... to ...`.
    Between,
    /// One of these dates: music21's `DateSelection`, written
    /// `... or ...`.
    Or,
    /// All of these dates, written `... and ...`.
    And,
}

impl Relevance {
    /// The name music21 gives it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Certain => "certain",
            Self::Approximate => "approximate",
            Self::Uncertain => "uncertain",
            Self::Prior => "prior",
            Self::After => "after",
            Self::OnOrBefore => "onorbefore",
            Self::OnOrAfter => "onorafter",
            Self::Between => "between",
            Self::Or => "or",
            Self::And => "and",
        }
    }

    /// What a name stands for, read without regard to case, with
    /// music21's `DateRelative` reading of `before` as `prior`.
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "certain" => Some(Self::Certain),
            "approximate" => Some(Self::Approximate),
            "uncertain" => Some(Self::Uncertain),
            "prior" | "before" => Some(Self::Prior),
            "after" => Some(Self::After),
            "onorbefore" => Some(Self::OnOrBefore),
            "onorafter" => Some(Self::OnOrAfter),
            "between" => Some(Self::Between),
            "or" => Some(Self::Or),
            "and" => Some(Self::And),
            _ => None,
        }
    }

    /// Which of music21's four date classes keeps a date of this
    /// relevance.
    #[must_use]
    pub fn kind(self) -> DateKind {
        match self {
            Self::Certain | Self::Approximate | Self::Uncertain => DateKind::Single,
            Self::Prior | Self::After | Self::OnOrBefore | Self::OnOrAfter => DateKind::Relative,
            Self::Between => DateKind::Between,
            Self::Or | Self::And => DateKind::Selection,
        }
    }
}

/// Which of music21's four date classes a [`DatePrimitive`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DateKind {
    /// music21's `DateSingle`: one date, certain, approximate or uncertain.
    Single,
    /// music21's `DateRelative`: a date something happened before or after.
    Relative,
    /// music21's `DateBetween`: a span between two dates.
    Between,
    /// music21's `DateSelection`: a choice among several dates.
    Selection,
}

impl DateKind {
    /// The name music21 gives the class.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Single => "DateSingle",
            Self::Relative => "DateRelative",
            Self::Between => "DateBetween",
            Self::Selection => "DateSelection",
        }
    }

    /// The relevance music21's class starts out with.
    #[must_use]
    pub fn default_relevance(self) -> Relevance {
        match self {
            Self::Single => Relevance::Certain,
            Self::Relative => Relevance::After,
            Self::Between => Relevance::Between,
            Self::Selection => Relevance::Or,
        }
    }
}

/// One or more dates and what they say: music21's `DatePrimitive` and the
/// four classes under it, which differ only in the relevances they take and
/// in how they are written.
///
/// ```
/// use music21_rs::metadata::primitives::{DateKind, DatePrimitive, Relevance};
///
/// let span = DatePrimitive::between(["2009/12/31", "2010/1/28"])?;
/// assert_eq!(span.to_string(), "2009/12/31 to 2010/01/28");
/// assert_eq!(span.kind(), DateKind::Between);
///
/// let prior = DatePrimitive::relative("2009/12/31", Relevance::Prior)?;
/// assert_eq!(prior.to_string(), "prior to 2009/12/31");
/// # Ok::<(), music21_rs::Error>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct DatePrimitive {
    dates: Vec<Date>,
    relevance: Relevance,
}

impl DatePrimitive {
    /// The dates it holds.
    pub fn dates(&self) -> &[Date] {
        &self.dates
    }

    /// What they say.
    pub fn relevance(&self) -> Relevance {
        self.relevance
    }

    /// Which of music21's four classes it is.
    pub fn kind(&self) -> DateKind {
        self.relevance.kind()
    }

    /// Says something else about the dates it holds.
    ///
    /// # Errors
    ///
    /// A relevance one of music21's other date classes keeps: its own
    /// setters refuse any that is not theirs, so `DateSingle` cannot be
    /// made to say `between`.
    pub fn set_relevance(&mut self, relevance: Relevance) -> Result<()> {
        if relevance.kind() != self.kind() {
            return Err(Error::Metadata(format!(
                "Relevance value is not supported by this object: '{}'",
                relevance.name()
            )));
        }
        self.relevance = relevance;
        Ok(())
    }

    /// One date: music21's `DateSingle`.
    ///
    /// # Errors
    ///
    /// A date that cannot be read, or a relevance `DateSingle` does not
    /// take.
    pub fn single(written: &str, relevance: Relevance) -> Result<Self> {
        Self::of(
            vec![Date::from_written(written)?],
            relevance,
            DateKind::Single,
        )
    }

    /// A date something happened before or after: music21's
    /// `DateRelative`.
    ///
    /// # Errors
    ///
    /// A date that cannot be read, or a relevance `DateRelative` does not
    /// take.
    pub fn relative(written: &str, relevance: Relevance) -> Result<Self> {
        Self::of(
            vec![Date::from_written(written)?],
            relevance,
            DateKind::Relative,
        )
    }

    /// A span between dates: music21's `DateBetween`.
    ///
    /// # Errors
    ///
    /// A date that cannot be read.
    pub fn between<I, S>(written: I) -> Result<Self>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self::of(read_dates(written)?, Relevance::Between, DateKind::Between)
    }

    /// A choice among dates: music21's `DateSelection`.
    ///
    /// # Errors
    ///
    /// A date that cannot be read, or a relevance `DateSelection` does not
    /// take.
    pub fn selection<I, S>(written: I, relevance: Relevance) -> Result<Self>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self::of(read_dates(written)?, relevance, DateKind::Selection)
    }

    /// Dates already read, said to be of a kind.
    ///
    /// # Errors
    ///
    /// A relevance that kind does not take.
    pub fn of_dates(dates: Vec<Date>, relevance: Relevance, kind: DateKind) -> Result<Self> {
        Self::of(dates, relevance, kind)
    }

    fn of(dates: Vec<Date>, relevance: Relevance, kind: DateKind) -> Result<Self> {
        if relevance.kind() != kind {
            return Err(Error::Metadata(format!(
                "Relevance value is not supported by this object: '{}'",
                relevance.name()
            )));
        }
        Ok(Self { dates, relevance })
    }
}

/// Every date of a list read as music21 reads one.
fn read_dates<I, S>(written: I) -> Result<Vec<Date>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    written
        .into_iter()
        .map(|one| Date::from_written(one.as_ref()))
        .collect()
}

impl fmt::Display for DatePrimitive {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let written: Vec<String> = self.dates.iter().map(Date::to_string).collect();
        match self.relevance {
            Relevance::Certain | Relevance::Approximate | Relevance::Uncertain => {
                f.write_str(written.first().map_or("", String::as_str))
            }
            Relevance::Prior => write!(f, "prior to {}", written.join(" ")),
            Relevance::After => write!(f, "after {}", written.join(" ")),
            Relevance::OnOrBefore => write!(f, "{} or earlier", written.join(" ")),
            Relevance::OnOrAfter => write!(f, "{} or later", written.join(" ")),
            Relevance::Between => f.write_str(&written.join(" to ")),
            Relevance::Or => f.write_str(&written.join(" or ")),
            Relevance::And => f.write_str(&written.join(" and ")),
        }
    }
}

/// One piece of text a score says about itself: music21's
/// `metadata.Text`. It keeps the language it is written in, whether it is a
/// translation, and the scheme it was encoded by.
///
/// ```
/// use music21_rs::metadata::primitives::Text;
///
/// let title = Text::new("concerto in d").in_language("en");
/// assert_eq!(title.to_string(), "concerto in d");
/// assert_eq!(title.language(), Some("en"));
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct Text {
    data: String,
    language: Option<String>,
    is_translated: Option<bool>,
    encoding_scheme: Option<String>,
    /// What music21's `Copyright` adds to a text: a role.
    role: Option<String>,
}

impl Text {
    /// Some text, saying nothing else about itself.
    pub fn new(data: impl Into<String>) -> Self {
        Self {
            data: data.into(),
            ..Self::default()
        }
    }

    /// The text itself.
    pub fn data(&self) -> &str {
        &self.data
    }

    /// The language it is written in, where it says.
    pub fn language(&self) -> Option<&str> {
        self.language.as_deref()
    }

    /// Whether it is a translation, where it says. Nothing at all means
    /// nobody has said.
    pub fn is_translated(&self) -> Option<bool> {
        self.is_translated
    }

    /// The scheme it was encoded by, where it says.
    pub fn encoding_scheme(&self) -> Option<&str> {
        self.encoding_scheme.as_deref()
    }

    /// The role it was given, which only music21's `Copyright` has.
    pub fn role(&self) -> Option<&str> {
        self.role.as_deref()
    }

    /// The same text, said to be in a language.
    pub fn in_language(mut self, language: impl Into<String>) -> Self {
        self.language = Some(language.into());
        self
    }

    /// The same text, said to be a translation or not.
    pub fn translated(mut self, is_translated: bool) -> Self {
        self.is_translated = Some(is_translated);
        self
    }

    /// The same text, said to be encoded by a scheme.
    pub fn encoded_by(mut self, scheme: impl Into<String>) -> Self {
        self.encoding_scheme = Some(scheme.into());
        self
    }

    /// The same text, given a role: music21's `Copyright`.
    pub fn in_role(mut self, role: impl Into<String>) -> Self {
        self.role = Some(role.into());
        self
    }

    /// Says what language it is in.
    pub fn set_language(&mut self, language: Option<String>) {
        self.language = language;
    }

    /// Says whether it is a translation.
    pub fn set_translated(&mut self, is_translated: Option<bool>) {
        self.is_translated = is_translated;
    }

    /// Says what scheme it was encoded by.
    pub fn set_encoding_scheme(&mut self, scheme: Option<String>) {
        self.encoding_scheme = scheme;
    }

    /// Gives it a role.
    pub fn set_role(&mut self, role: Option<String>) {
        self.role = role;
    }

    /// The text with the article put back at the front, where the language
    /// is one whose articles are moved: music21's `getNormalizedArticle`,
    /// which is `text::prepend_article`.
    ///
    /// # Errors
    ///
    /// A language whose articles are not known.
    pub fn normalized_article(&self) -> Result<String> {
        crate::text::prepend_article(&self.data, self.language.as_deref())
    }
}

impl fmt::Display for Text {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.data)
    }
}

/// Somebody who had a hand in a work: music21's `metadata.Contributor`. A
/// composer, a lyricist, an arranger, or anybody else, under one or more
/// names.
///
/// ```
/// use music21_rs::metadata::primitives::{Contributor, Text};
///
/// let composer = Contributor::new("composer", [Text::new("Beethoven, Ludwig van")]);
/// assert_eq!(composer.role(), Some("composer"));
/// assert_eq!(composer.to_string(), "composer:Beethoven, Ludwig van");
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct Contributor {
    role: Option<String>,
    names: Vec<Text>,
    birth: Option<DatePrimitive>,
    death: Option<DatePrimitive>,
}

/// The ten roles Humdrum writes an abbreviation for: music21's
/// `Contributor.roleAbbreviationsDict`.
pub const ROLE_ABBREVIATIONS: [(&str, &str); 10] = [
    ("com", "composer"),
    ("coa", "attributedComposer"),
    ("cos", "suspectedComposer"),
    ("col", "composerAlias"),
    ("coc", "composerCorporate"),
    ("lyr", "lyricist"),
    ("lib", "librettist"),
    ("lar", "arranger"),
    ("lor", "orchestrator"),
    ("trn", "translator"),
];

impl Contributor {
    /// The role an abbreviation stands for, read without regard to case:
    /// music21's `abbreviationToRole`.
    ///
    /// # Errors
    ///
    /// An abbreviation no role is written with.
    pub fn role_of_abbreviation(abbreviation: &str) -> Result<&'static str> {
        let abbreviation = abbreviation.to_lowercase();
        ROLE_ABBREVIATIONS
            .iter()
            .find(|(written, _)| *written == abbreviation)
            .map(|(_, role)| *role)
            .ok_or_else(|| Error::Metadata(format!("no such role: '{abbreviation}'")))
    }

    /// The abbreviation a role is written with, read without regard to
    /// case: music21's `roleToAbbreviation`.
    ///
    /// # Errors
    ///
    /// A role no abbreviation is written for.
    pub fn abbreviation_of_role(role: &str) -> Result<&'static str> {
        ROLE_ABBREVIATIONS
            .iter()
            .find(|(_, known)| known.eq_ignore_ascii_case(role))
            .map(|(written, _)| *written)
            .ok_or_else(|| Error::Metadata(format!("No such role: {role}")))
    }

    /// Somebody in a role, under these names. An abbreviation is read as
    /// the role it stands for.
    pub fn new<I, T>(role: impl Into<String>, names: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<Text>,
    {
        let mut made = Self {
            role: None,
            names: names.into_iter().map(Into::into).collect(),
            birth: None,
            death: None,
        };
        made.set_role(Some(role.into()));
        made
    }

    /// Somebody in no role at all, which is what music21 starts a
    /// contributor out as.
    pub fn unnamed() -> Self {
        Self::default()
    }

    /// What they did, where it is known.
    pub fn role(&self) -> Option<&str> {
        self.role.as_deref()
    }

    /// Every name they go by, the first being the one music21 answers for
    /// `name`.
    pub fn names(&self) -> &[Text] {
        &self.names
    }

    /// The name they go by.
    pub fn name(&self) -> Option<&Text> {
        self.names.first()
    }

    /// When they were born, where it is known.
    pub fn birth(&self) -> Option<&DatePrimitive> {
        self.birth.as_ref()
    }

    /// When they died, where it is known.
    pub fn death(&self) -> Option<&DatePrimitive> {
        self.death.as_ref()
    }

    /// Says what they did. An abbreviation is read as the role it stands
    /// for, as music21's own setter reads one, so a contributor given
    /// `com` is a composer.
    pub fn set_role(&mut self, role: Option<String>) {
        self.role = role
            .map(|role| Self::role_of_abbreviation(&role).map_or(role, |found| found.to_string()));
    }

    /// Gives them one name, in place of any they had: music21's `name`
    /// setter, which replaces the whole list.
    pub fn set_name(&mut self, name: Text) {
        self.names = vec![name];
    }

    /// Gives them every name they go by.
    pub fn set_names(&mut self, names: Vec<Text>) {
        self.names = names;
    }

    /// Says when they were born.
    pub fn set_birth(&mut self, birth: Option<DatePrimitive>) {
        self.birth = birth;
    }

    /// Says when they died.
    pub fn set_death(&mut self, death: Option<DatePrimitive>) {
        self.death = death;
    }
}

/// Written as music21's `_reprInternal` writes one: the role, a colon, and
/// every name they go by.
impl fmt::Display for Contributor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<String> = self.names.iter().map(Text::to_string).collect();
        write!(
            f,
            "{}:{}",
            self.role.as_deref().unwrap_or(""),
            names.join(",")
        )
    }
}

impl From<&str> for Text {
    fn from(data: &str) -> Self {
        Self::new(data)
    }
}

impl From<String> for Text {
    fn from(data: String) -> Self {
        Self::new(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// music21's own printed answers for `Date`.
    #[test]
    fn a_date_is_written_as_music21_writes_one() {
        let mut date = Date::from_written("1030?/12~/?4").expect("a date");
        assert_eq!(date.to_string(), "1030?/12~/04?");
        assert_eq!(date.value(DatePart::Month), Some(12));
        assert_eq!(
            date.certainty(DatePart::Month),
            Some(Certainty::Approximate)
        );
        assert_eq!(date.value(DatePart::Day), Some(4));
        assert_eq!(date.certainty(DatePart::Day), Some(Certainty::Uncertain));

        let timed = Date::from_written("1834/12/4/4:50:32").expect("a date");
        assert_eq!(timed.to_string(), "1834/12/04/04:50:32");
        assert_eq!(timed.value(DatePart::Minute), Some(50));
        assert_eq!(timed.value(DatePart::Second), Some(32));
        assert!(timed.has_time());
        assert!(!timed.has_certainty());

        let mut approximate = Date::from_parts(Some(1805), Some(3), None).expect("a date");
        approximate
            .set(DatePart::Month, Some(3), Some(Certainty::Approximate))
            .expect("a month");
        assert_eq!(approximate.to_string(), "1805/03~/--");

        assert_eq!(
            Date::from_parts(Some(1843), Some(3), Some(3))
                .expect("a date")
                .to_string(),
            "1843/03/03"
        );
        date.take_from(&Date::from_parts(Some(1843), None, None).expect("a date"))
            .expect("taking a year");
        assert_eq!(date.year(), Some(1843));
    }

    /// music21 drops milliseconds, and reads the first symbol it meets.
    #[test]
    fn a_written_number_gives_up_its_certainty() {
        assert_eq!(
            strip_certainty("1247~").expect("a number"),
            (1247, Some(Certainty::Approximate))
        );
        assert_eq!(
            strip_certainty("63?").expect("a number"),
            (63, Some(Certainty::Uncertain))
        );
        assert_eq!(strip_certainty("4.43").expect("a number"), (4, None));
    }

    /// music21's `_sanityCheck`, message for message.
    #[test]
    fn a_date_no_calendar_has_is_refused() {
        assert_eq!(
            Date::from_written("1999/14/32")
                .expect_err("a fourteenth month")
                .to_string(),
            "Value error: Month must be between 1 and 12, not 14."
        );
        assert!(Date::from_parts(Some(1999), Some(4), Some(31)).is_err());
        assert!(Date::from_parts(Some(1999), Some(2), Some(29)).is_err());
        assert!(Date::from_parts(Some(2000), Some(2), Some(29)).is_ok());
    }

    /// music21's own printed answers for the four date classes.
    #[test]
    fn the_four_date_classes_are_written_as_music21_writes_them() {
        let single = DatePrimitive::single("2009/12/31", Relevance::Approximate).expect("a date");
        assert_eq!(single.to_string(), "2009/12/31");
        assert_eq!(single.relevance(), Relevance::Approximate);
        assert_eq!(single.kind(), DateKind::Single);
        assert_eq!(
            DatePrimitive::single("1805/3/12", Relevance::Uncertain)
                .expect("a date")
                .to_string(),
            "1805/03/12"
        );

        let mut relative = DatePrimitive::relative("2009/12/31", Relevance::Prior).expect("a date");
        assert_eq!(relative.to_string(), "prior to 2009/12/31");
        relative
            .set_relevance(Relevance::After)
            .expect("after a date");
        assert_eq!(relative.to_string(), "after 2009/12/31");
        assert!(DatePrimitive::relative("2009/12/31", Relevance::Certain).is_err());

        assert_eq!(
            DatePrimitive::between(["2009/12/31", "2010/1/28"])
                .expect("a span")
                .to_string(),
            "2009/12/31 to 2010/01/28"
        );
        assert_eq!(
            DatePrimitive::of_dates(
                vec![
                    Date::from_parts(Some(1605), None, None).expect("a year"),
                    Date::from_written("1608/11?").expect("a date"),
                ],
                Relevance::Between,
                DateKind::Between,
            )
            .expect("a span")
            .to_string(),
            "1605/--/-- to 1608/11?/--"
        );

        assert_eq!(
            DatePrimitive::selection(["2009/12/31", "2010/1/28", "1894/1/28"], Relevance::Or)
                .expect("a choice")
                .to_string(),
            "2009/12/31 or 2010/01/28 or 1894/01/28"
        );
        assert_eq!(
            DatePrimitive::selection(["1750", "1775/03?"], Relevance::And)
                .expect("a choice")
                .to_string(),
            "1750/--/-- and 1775/03?/--"
        );
        assert!(DatePrimitive::selection(["2009/12/31"], Relevance::Certain).is_err());
    }

    /// music21's own printed answers for `Text` and `Contributor`.
    #[test]
    fn a_text_keeps_what_it_says_of_itself() {
        let title = Text::new("concerto in d").in_language("en");
        assert_eq!(title.to_string(), "concerto in d");
        assert_eq!(title.language(), Some("en"));
        assert_ne!(title, Text::new("concerto in d"));
        assert_eq!(Text::new("some text"), Text::new("some text"));
        assert_ne!(
            Text::new("some text"),
            Text::new("some text").translated(true)
        );

        let article = Text::new("Ale is Dear, The").in_language("en");
        assert_eq!(
            article.normalized_article().expect("an article"),
            "The Ale is Dear"
        );
        let german = Text::new("Ale is Dear, The").in_language("de");
        assert_eq!(
            german.normalized_article().expect("an article"),
            "Ale is Dear, The"
        );

        let composer = Contributor::new("composer", [Text::new("Beethoven, Ludwig van")]);
        assert_eq!(composer.to_string(), "composer:Beethoven, Ludwig van");
        assert_eq!(
            composer.name().map(Text::data),
            Some("Beethoven, Ludwig van")
        );
        assert_eq!(Contributor::unnamed().role(), None);
        // An abbreviation is read as the role it stands for.
        assert_eq!(
            Contributor::new("com", [Text::new("Chopin")]).role(),
            Some("composer")
        );
        assert_eq!(
            Contributor::role_of_abbreviation("lib").expect("a role"),
            "librettist"
        );
        assert_eq!(
            Contributor::abbreviation_of_role("composer").expect("an abbreviation"),
            "com"
        );
        assert!(Contributor::role_of_abbreviation("zzz").is_err());
        assert!(Contributor::abbreviation_of_role("conductor").is_err());
    }
}
