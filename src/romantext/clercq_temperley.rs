//! The de Clercq-Temperley analyses of the rock corpus: music21's
//! `romanText.clercqTemperley`.
//!
//! A song is a handful of rules, each a line of its own. `Vr: I | IV | I |
//! V |` names the verse's chords a measure at a time, and the last rule,
//! `S:`, plays the song through by naming the others -- `S: [G] $In $Vr
//! $Vr $Ch`. A rule may expand another (`$VP*2`), repeat a measure of its
//! own (`|*4`), say the key or the meter in brackets (`[G]`, `[3/4]`),
//! repeat the chord before it with a dot, rest a measure with an `R`, and
//! carry a comment after a `%`. The first line is the title, also written
//! after a `%`.
//!
//! [`from_clercq_temperley`] reads one into a part of
//! [`RomanNumeral`](crate::roman::RomanNumeral)s, which is music21's
//! `CTSong.toPart`: a measure for each the rules name, its chords sharing
//! the bar equally, the figure of each written under it as a lyric and the
//! name of the rule that brought it under the first chord of its section.
//! A chord repeated across a barline is tied to the one before it rather
//! than struck again.
//!
//! The format is the rock corpus's, documented at
//! <https://rockcorpus.midside.com>. The library reads text, not files, so
//! a `.clt` file is the caller's to read.

use crate::chord::Chord;
use crate::defaults::{FloatType, IntegerType};
use crate::duration::Duration;
use crate::error::{Error, Result};
use crate::key::{Key, convert_key_string_to_music21_key_string};
use crate::makenotation::op_frac;
use crate::metadata::{Metadata, MetadataValue};
use crate::meter::TimeSignature;
use crate::notation::Tie;
use crate::rest::Rest;
use crate::roman::RomanNumeral;
use crate::stream::{Stream, StreamElement, StreamKind};

/// What is written under the chords as a song is read: music21's
/// `labelRomanNumerals` and `labelSubsectionsOnScore`, both on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CtLabels {
    /// Whether each chord carries its figure as a lyric.
    pub roman_numerals: bool,
    /// Whether the first chord of a section carries the name of the rule
    /// that brought it.
    pub subsections: bool,
}

impl Default for CtLabels {
    fn default() -> Self {
        Self {
            roman_numerals: true,
            subsections: true,
        }
    }
}

/// A song read into a part: music21's `CTSong(text).toPart()`.
///
/// ```
/// use music21_rs::romantext::clercq_temperley::from_clercq_temperley;
///
/// let song = "% Simple Gifts\nVr: I | I |\nS: [A] $Vr\n";
/// let part = from_clercq_temperley(song)?;
/// assert_eq!(part.measures().len(), 2);
/// # Ok::<(), music21_rs::Error>(())
/// ```
///
/// # Errors
///
/// Text with no barline or no `S:` rule, as music21's converter refuses
/// it; a rule naming one the song has not got, a dot with no chord before
/// it, or a key, a meter or a figure the crate cannot read.
pub fn from_clercq_temperley(text: &str) -> Result<Stream> {
    song_text(text)?;
    CtSong::parse(text).to_part()
}

/// The same, saying what to write under the chords.
///
/// # Errors
///
/// As [`from_clercq_temperley`].
pub fn from_clercq_temperley_with(text: &str, labels: CtLabels) -> Result<Stream> {
    song_text(text)?;
    CtSong::parse(text).to_part_with(labels)
}

/// Refuses text that is no song: music21's converter asks for a barline and
/// an `S:` rule both, where its `CTSong` would take anything else for the
/// name of a file to open.
fn song_text(text: &str) -> Result<()> {
    if text.contains('|') && text.contains("S:") {
        return Ok(());
    }
    Err(ct_error(
        r#"Clercq-Temperley data must contain "|" and "S:""#,
    ))
}

/// A song as its text says: music21's `CTSong`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CtSong {
    text: String,
    lines: Vec<String>,
    rules: Vec<CtRule>,
}

impl CtSong {
    /// A song read from its text: music21's `parse`, which keeps the lines
    /// that carry anything, each trimmed.
    ///
    /// music21 takes a path as readily as a text, reading the file where
    /// the string holds no `|` and no `S:`; the library reads no files, so
    /// this takes the text alone.
    pub fn parse(text: &str) -> Self {
        Self::from_lines(text.split('\n'))
    }

    /// A song read from the lines of its file: music21's `parse` once it
    /// has opened one, which keeps every line that holds anything and
    /// trims each.
    ///
    /// The library reads no files, so a caller hands the lines over; it
    /// matters that they come as the file wrote them, since music21 reads
    /// a line's emptiness *before* trimming it -- a blank line of a file
    /// still carries its newline and so survives as an empty line, where
    /// one in a text split on newlines does not.
    pub fn from_lines<I, S>(lines: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let lines: Vec<String> = lines
            .into_iter()
            .filter(|line| !line.as_ref().is_empty())
            .map(|line| line.as_ref().trim().to_string())
            .collect();
        let rules = lines
            .iter()
            .filter(|line| {
                line.split_whitespace()
                    .next()
                    .is_some_and(|word| word.ends_with(':'))
            })
            .map(|line| CtRule::new(line))
            .collect();
        Self {
            text: lines.join("\n"),
            lines,
            rules,
        }
    }

    /// The text as it was read, its blank lines gone.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The lines the text was read as.
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// The title, which is the first line with its `%` taken off.
    ///
    /// music21 takes *every* `%` out of that line rather than the one in
    /// front of it, so a title with a per cent in it loses the sign.
    pub fn title(&self) -> String {
        self.lines
            .first()
            .map(|line| line.replace('%', "").trim().to_string())
            .unwrap_or_default()
    }

    /// Every comment of the song but the title, each as music21 lists it:
    /// a comment on a rule's line is that rule's name and the comment, and
    /// one on a line of its own is the comment alone.
    pub fn comments(&self) -> Vec<Vec<String>> {
        let mut comments = Vec::new();
        for line in self.lines.iter().skip(1) {
            let Some(at) = line.find('%') else { continue };
            let said = line[at + 1..].trim().to_string();
            match line.split_whitespace().next() {
                Some(word) if word.ends_with(':') => comments.push(vec![word.to_string(), said]),
                _ => comments.push(vec![said]),
            }
        }
        comments
    }

    /// The rules of the song, in the order the text names them, the `S:`
    /// rule among them.
    pub fn rules(&self) -> &[CtRule] {
        &self.rules
    }

    /// The rule of a name, as music21 keys its rules by the left-hand side.
    pub fn rule(&self, name: &str) -> Option<&CtRule> {
        self.rules.iter().find(|rule| rule.left_hand_side() == name)
    }

    /// The meter the song opens in: music21's `homeTimeSig`, which reads
    /// the first two atoms of the `S:` rule -- four-four where the first
    /// says no meter and no key, the meter where one says a meter, and
    /// nothing at all where both say a key.
    ///
    /// # Errors
    ///
    /// A meter the crate cannot read.
    pub fn home_time_signature(&self) -> Result<Option<TimeSignature>> {
        for atom in self.song_head() {
            if !atom.contains('[') {
                return TimeSignature::from_ratio_string("4/4").map(Some);
            }
            if atom.contains('/') {
                return TimeSignature::from_ratio_string(bracketed(atom)).map(Some);
            }
        }
        Ok(None)
    }

    /// The key the song opens in: music21's `homeKey`, read the same way --
    /// C major where the first atom says neither, the key where one says a
    /// key, and nothing where both say a meter.
    ///
    /// # Errors
    ///
    /// A key the crate cannot read.
    pub fn home_key(&self) -> Result<Option<Key>> {
        for atom in self.song_head() {
            if !atom.contains('[') {
                return Key::from_tonic("C").map(Some);
            }
            if !atom.contains('/') {
                let named = convert_key_string_to_music21_key_string(bracketed(atom));
                return Key::from_tonic(&named).map(Some);
            }
        }
        Ok(None)
    }

    /// The first two atoms of the `S:` rule, which is where the song says
    /// its key and its meter.
    fn song_head(&self) -> Vec<&str> {
        self.lines
            .iter()
            .find(|line| line.starts_with("S:"))
            .map(|line| line.split_whitespace().skip(1).take(2).collect())
            .unwrap_or_default()
    }

    /// The song as a part: music21's `toPart`, which expands the `S:` rule,
    /// numbers the measures from one and signs the part with the title.
    ///
    /// # Errors
    ///
    /// As [`from_clercq_temperley`].
    pub fn to_part(&self) -> Result<Stream> {
        self.to_part_with(CtLabels::default())
    }

    /// The same, saying what to write under the chords.
    ///
    /// # Errors
    ///
    /// As [`from_clercq_temperley`].
    pub fn to_part_with(&self, labels: CtLabels) -> Result<Stream> {
        let mut expansion = Expansion::of(self, labels)?;
        let song = self
            .rules
            .iter()
            .position(|rule| rule.left_hand_side() == "S")
            .ok_or_else(|| ct_error("a song with no S: rule to play it through"))?;
        let places = expansion.expand(song, None, None)?;
        let mut part = Stream::with_kind(StreamKind::Part);
        let mut at = 0.0;
        for (index, place) in places.iter().enumerate() {
            let measure = expansion.built[*place]
                .as_measure(IntegerType::try_from(index + 1).unwrap_or(IntegerType::MAX))?;
            let length = measure.end_offset();
            part.insert(at, measure);
            at = op_frac(at + length);
        }
        let mut metadata = Metadata::new();
        metadata.add("title", MetadataValue::new(self.title()));
        part.set_metadata(Some(metadata));
        Ok(part)
    }
}

/// One rule of a song: music21's `CTRule`, a line of the text.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CtRule {
    text: String,
    left_hand_side: String,
    music_text: String,
}

impl CtRule {
    /// A rule read from its line.
    pub fn new(text: &str) -> Self {
        let left_hand_side = match text.split_whitespace().next() {
            Some(word) if word.ends_with(':') => text
                .split_once(':')
                .map_or(String::new(), |(before, _)| before.trim().to_string()),
            _ => String::new(),
        };
        // music21 reads the music from one character past the name,
        // whatever that character is -- so a line with no name at all
        // loses its first letter rather than keeping it.
        let mut music_text = String::new();
        if !text.is_empty() {
            let after = text.get(left_hand_side.len() + 1..).unwrap_or_default();
            let said = after.split('%').next().unwrap_or(after);
            music_text = said.trim().to_string();
        }
        Self {
            text: text.to_string(),
            left_hand_side,
            music_text,
        }
    }

    /// The line as it was written, its name and comment included.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The name in front of the colon: music21's `LHS`.
    pub fn left_hand_side(&self) -> &str {
        &self.left_hand_side
    }

    /// The chords alone, without the name or the comment: music21's
    /// `musicText`.
    pub fn music_text(&self) -> &str {
        &self.music_text
    }

    /// What was written after the `%`, if anything.
    pub fn comment(&self) -> Option<&str> {
        self.text.find('%').map(|at| self.text[at + 1..].trim())
    }

    /// The section the name stands for, as music21 spells the six it knows:
    /// `In` is an introduction, `Br` a bridge, `Vr` a verse, `Ch` a chorus,
    /// `Tg` a tag, `S` the song and `Fadeout` itself, with whatever follows
    /// the abbreviation kept.
    pub fn section_name(&self) -> String {
        let name = &self.left_hand_side;
        let rest = |at: usize| name.get(at..).unwrap_or_default();
        for (abbreviation, said) in [
            ("In", "Introduction"),
            ("Br", "Bridge"),
            ("Vr", "Verse"),
            ("Ch", "Chorus"),
            ("Tg", "Tag"),
        ] {
            if name.contains(abbreviation) {
                return format!("{said}{}", rest(2));
            }
        }
        match name.as_str() {
            "S" => "Song".to_string(),
            "Fadeout" => "Fadeout".to_string(),
            other => other.to_string(),
        }
    }

    /// The rule's text as the groups it is read in: music21's
    /// `_measureGroups`, where each group is a measure or the expansion of
    /// another rule, with how many times it is played.
    pub fn measure_groups(&self) -> Vec<MeasureGroup> {
        // First pass: split at every barline, keeping the count a `|*4`
        // says beside it.
        let split = split_measures(&self.music_text);
        let mut barred: Vec<(String, String)> = Vec::new();
        let mut index = 0;
        while index < split.len() {
            let content = split[index].trim().to_string();
            let separator = split.get(index + 1).cloned().unwrap_or_default();
            if !content.is_empty() || !separator.is_empty() {
                barred.push((content, separator));
            }
            index += 2;
        }
        // Second pass: take the expansions out, each as a group of its own,
        // and read the counts.
        let mut groups: Vec<MeasureGroup> = Vec::new();
        for (content, separator) in barred {
            let mut kept: Vec<&str> = Vec::new();
            for atom in content.split_whitespace() {
                if let Some(named) = atom.strip_prefix('$') {
                    if !kept.is_empty() {
                        groups.push(MeasureGroup {
                            content: kept.join(" "),
                            kind: GroupKind::Continued,
                            repeats: 1,
                        });
                        kept.clear();
                    }
                    let (repeats, named) = repetition(named);
                    groups.push(MeasureGroup {
                        content: named,
                        kind: GroupKind::Expansion,
                        repeats,
                    });
                } else {
                    kept.push(atom);
                }
            }
            let (repeats, separator) = repetition(&separator);
            if !kept.is_empty() || separator == "|" {
                groups.push(MeasureGroup {
                    content: kept.join(" "),
                    kind: if separator == "|" {
                        GroupKind::Measure
                    } else {
                        GroupKind::Loose
                    },
                    repeats,
                });
            }
        }
        // Third pass: a measure saying nothing but its key or its meter
        // holds the chord before it, and content with no barline after it
        // carries on into the measure that follows.
        for group in &mut groups {
            let atoms: Vec<&str> = group.content.split_whitespace().collect();
            if group.kind == GroupKind::Measure && atoms.iter().all(|atom| atom.starts_with('[')) {
                group.content = atoms.join(" ");
                if !group.content.is_empty() {
                    group.content.push(' ');
                }
                group.content.push('.');
            } else if group.kind == GroupKind::Continued {
                group.kind = GroupKind::Measure;
            }
        }
        groups
    }

    /// The measures the rule expands to on its own, in four-four and C
    /// major unless it says otherwise: music21's `expand` on a rule with no
    /// song behind it, which writes the key and the meter into every
    /// measure it makes.
    ///
    /// # Errors
    ///
    /// A rule expanding another, which only a song can answer, a dot with
    /// no chord before it, or a key, a meter or a figure the crate cannot
    /// read.
    pub fn expand(&self) -> Result<Vec<Stream>> {
        self.expand_in(None, None)
    }

    /// The same, in a key and a meter of the caller's.
    ///
    /// # Errors
    ///
    /// As [`CtRule::expand`].
    pub fn expand_in(&self, meter: Option<TimeSignature>, key: Option<Key>) -> Result<Vec<Stream>> {
        let alone = [self.clone()];
        let mut expansion = Expansion {
            rules: &alone,
            labels: CtLabels::default(),
            has_song: false,
            states: vec![RuleState {
                meter: match meter {
                    Some(meter) => meter,
                    None => TimeSignature::from_ratio_string("4/4")?,
                },
                key: match key {
                    Some(key) => key,
                    None => Key::from_tonic("C")?,
                },
                last_regular_atom: String::new(),
                last_chord: None,
            }],
            built: Vec::new(),
            meters_said: Vec::new(),
            keys_said: Vec::new(),
        };
        let places = expansion.expand(0, None, None)?;
        places
            .iter()
            .enumerate()
            .map(|(index, place)| {
                expansion.built[*place]
                    .as_measure(IntegerType::try_from(index + 1).unwrap_or(IntegerType::MAX))
            })
            .collect()
    }
}

/// What a group of a rule's text stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GroupKind {
    /// A measure, closed by a barline.
    Measure,
    /// Another rule, played through as many times as the `*` says.
    Expansion,
    /// Chords with no barline after them, which music21 warns about and
    /// leaves out.
    Loose,
    /// Chords an expansion interrupted, which carry on into the measure
    /// that follows: music21's `?`, which its second pass writes and its
    /// third turns into a measure.
    Continued,
}

/// One group of a rule's text: music21's `(content, sep, numReps)`.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MeasureGroup {
    /// The atoms of the group, as they were written.
    pub content: String,
    /// What the group stands for.
    pub kind: GroupKind,
    /// How many times it is played.
    pub repeats: usize,
}

/// A figure as music21 reads one: music21's `fixupChordAtom`, which reads
/// the lab's `x` as a diminished chord, its `h` as a half-diminished one
/// and, on a minor degree, its `a` as an augmented one.
///
/// ```
/// use music21_rs::romantext::clercq_temperley::fixup_chord_atom;
///
/// assert_eq!(fixup_chord_atom("iix"), "iio");
/// assert_eq!(fixup_chord_atom("viih7"), "vii/o7");
/// assert_eq!(fixup_chord_atom("iia"), "ii+");
/// ```
pub fn fixup_chord_atom(atom: &str) -> String {
    let mut atom = atom.to_string();
    if atom.contains('x') {
        atom = atom.replace('x', "o");
    }
    if atom.contains('h') {
        atom = atom.replace('h', "/o");
    }
    if atom.chars().next().is_some_and(char::is_lowercase) && atom.contains('a') {
        atom = atom.replace('a', "+");
    }
    atom
}

/// Whether two chords sound the same notes, which is what decides a tie:
/// music21's `isSame`, comparing the pitches by name and octave.
pub fn is_same(chord: &Chord, last: Option<&Chord>) -> bool {
    let Some(last) = last else { return false };
    let names = |chord: &Chord| -> Vec<String> {
        chord
            .pitches()
            .iter()
            .map(crate::pitch::Pitch::name_with_octave)
            .collect()
    };
    names(chord) == names(last)
}

/// Whether two keys are the one key, as music21's `==` reads them: the
/// same tonic, spelled the same, in the same mode. The crate's [`Key`] is
/// not `PartialEq`, and what music21 compares here is the key music21
/// writes into a measure.
fn same_key(one: &Key, other: &Key) -> bool {
    one.tonic_pitch_name_with_case() == other.tonic_pitch_name_with_case()
        && one.mode() == other.mode()
        && one.sharps() == other.sharps()
}

/// What is inside a bracketed atom, `[3/4]` being `3/4`.
fn bracketed(atom: &str) -> &str {
    atom.trim_start_matches('[').trim_end_matches(']')
}

/// An error in a song, which music21 raises as a `CTSongException` or a
/// `CTRuleException`.
fn ct_error(said: impl Into<String>) -> Error {
    Error::RomanText(said.into())
}

/// The rule's text split at every barline, the barlines kept: music21's
/// `SPLITMEASURES`, a `|` with the `*4` that may follow it.
fn split_measures(text: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find('|') {
        parts.push(rest[..at].to_string());
        let after = &rest[at + 1..];
        let bytes = after.as_bytes();
        let mut walked = 0;
        let mut separator = String::from("|");
        if bytes.first() == Some(&b'*') {
            separator.push('*');
            walked += 1;
        }
        while bytes.get(walked).is_some_and(u8::is_ascii_digit) {
            separator.push(char::from(bytes[walked]));
            walked += 1;
        }
        parts.push(separator);
        rest = &after[walked..];
    }
    parts.push(rest.to_string());
    parts
}

/// The count a `*4` says and the text without it: music21's `REPETITION`,
/// which reads the first `*` with digits after it.
fn repetition(text: &str) -> (usize, String) {
    let Some(at) = text.find('*') else {
        return (1, text.to_string());
    };
    let digits: String = text[at + 1..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    if digits.is_empty() {
        return (1, text.to_string());
    }
    let mut kept = text[..at].to_string();
    kept.push_str(&text[at + 1 + digits.len()..]);
    (digits.parse().unwrap_or(1), kept)
}

/// A measure being built: the key and the meter it states, if it states
/// them, and the chords and rests in the order they were read.
#[derive(Clone, Debug, Default)]
struct Built {
    meter: Option<TimeSignature>,
    key: Option<Key>,
    items: Vec<StreamElement>,
}

impl Built {
    /// The measure as a stream: the key and the meter at its start, and
    /// each chord after the one before it.
    fn as_measure(&self, number: IntegerType) -> Result<Stream> {
        let mut measure = Stream::with_kind(StreamKind::Measure);
        measure.set_number(number);
        if let Some(key) = &self.key {
            measure.insert(0.0, key.clone());
        }
        if let Some(meter) = &self.meter {
            measure.insert(0.0, meter.clone());
        }
        let mut at = 0.0;
        for item in &self.items {
            let length = item.quarter_length();
            measure.insert(at, item.clone());
            at = op_frac(at + length);
        }
        Ok(measure)
    }

    /// The chord standing at a place in the measure, where one does.
    fn chord(&self, item: usize) -> Option<&Chord> {
        match self.items.get(item) {
            Some(StreamElement::Chord(chord)) => Some(chord),
            _ => None,
        }
    }
}

/// Where a chord stands while a song is being read.
#[derive(Clone, Copy, Debug)]
struct Place {
    measure: usize,
    item: usize,
}

/// What a rule keeps while it is expanded, which music21 keeps on the rule
/// itself -- so the chord before is the one that rule last read, however
/// many measures of other rules stand between.
#[derive(Clone, Debug)]
struct RuleState {
    meter: TimeSignature,
    key: Key,
    last_regular_atom: String,
    last_chord: Option<Place>,
}

/// A song being expanded: every measure made so far, in the order they were
/// made, and what each rule is keeping.
struct Expansion<'a> {
    rules: &'a [CtRule],
    labels: CtLabels,
    /// Whether a song stands behind the rules, which decides whether a
    /// measure restating the key or the meter says so: music21 writes both
    /// into every measure of a rule it expands on its own, and otherwise
    /// only where they have changed.
    has_song: bool,
    states: Vec<RuleState>,
    built: Vec<Built>,
    meters_said: Vec<TimeSignature>,
    keys_said: Vec<Key>,
}

impl<'a> Expansion<'a> {
    /// A song's rules, each starting in the key and the meter the song
    /// opens in.
    fn of(song: &'a CtSong, labels: CtLabels) -> Result<Self> {
        let meter = match song.home_time_signature()? {
            Some(meter) => meter,
            None => TimeSignature::from_ratio_string("4/4")?,
        };
        let key = match song.home_key()? {
            Some(key) => key,
            None => Key::from_tonic("C")?,
        };
        let state = RuleState {
            meter,
            key,
            last_regular_atom: String::new(),
            last_chord: None,
        };
        Ok(Self {
            rules: song.rules(),
            labels,
            has_song: true,
            states: vec![state; song.rules().len()],
            built: Vec::new(),
            meters_said: Vec::new(),
            keys_said: Vec::new(),
        })
    }

    /// One rule expanded whole: music21's `expand`, which reads its groups
    /// in the key and the meter it is given and puts its own back
    /// afterwards.
    fn expand(
        &mut self,
        rule: usize,
        meter: Option<TimeSignature>,
        key: Option<Key>,
    ) -> Result<Vec<usize>> {
        let kept = (
            self.states[rule].meter.clone(),
            self.states[rule].key.clone(),
        );
        if let Some(meter) = meter {
            self.states[rule].meter = meter;
        }
        if let Some(key) = key {
            self.states[rule].key = key;
        }
        let mut places = Vec::new();
        for group in self.rules[rule].measure_groups() {
            match group.kind {
                GroupKind::Expansion => {
                    self.expand_expansion(rule, &group.content, group.repeats, &mut places)?;
                }
                GroupKind::Measure => {
                    self.expand_simple(rule, &group.content, group.repeats, &mut places)?;
                }
                // music21 warns and reads nothing where a group has
                // neither a barline nor a name.
                GroupKind::Loose | GroupKind::Continued => {}
            }
        }
        self.label_section(rule, &places)?;
        self.states[rule].meter = kept.0;
        self.states[rule].key = kept.1;
        Ok(places)
    }

    /// Another rule played through: music21's `expandExpansionContent`.
    fn expand_expansion(
        &mut self,
        rule: usize,
        content: &str,
        repeats: usize,
        places: &mut Vec<usize>,
    ) -> Result<()> {
        let named = self
            .rules
            .iter()
            .position(|known| known.left_hand_side() == content)
            .filter(|_| self.has_song)
            .ok_or_else(|| {
                ct_error(format!(
                    "cannot expand rule {content} in {}",
                    self.rules[rule].text()
                ))
            })?;
        for _ in 0..repeats {
            let meter = self.states[rule].meter.clone();
            let key = self.states[rule].key.clone();
            let returned = self.expand(named, Some(meter.clone()), Some(key.clone()))?;
            let Some(first) = returned.first() else {
                return Err(ct_error(format!(
                    "rule {content} expands to no measure at all"
                )));
            };
            self.insert_key_and_meter(*first, &meter, &key);
            places.extend(returned);
        }
        Ok(())
    }

    /// A measure of the rule's own chords: music21's `expandSimpleContent`,
    /// which shares the bar out equally among them.
    fn expand_simple(
        &mut self,
        rule: usize,
        content: &str,
        repeats: usize,
        places: &mut Vec<usize>,
    ) -> Result<()> {
        // The key and the meter first, since they say how long the chords
        // of this very measure are.
        let mut regular: Vec<String> = Vec::new();
        for atom in content.split_whitespace() {
            if atom.starts_with('[') {
                let inside = bracketed(atom);
                if inside == "0" {
                    // A meter music21 cannot write; it reads the measure in
                    // four-four instead.
                    self.states[rule].meter = TimeSignature::from_ratio_string("4/4")?;
                } else if inside.contains('/') {
                    self.states[rule].meter = TimeSignature::from_ratio_string(inside)?;
                } else {
                    let named = convert_key_string_to_music21_key_string(inside);
                    self.states[rule].key = Key::from_tonic(&named)?;
                }
            } else if atom == "." {
                if self.states[rule].last_regular_atom.is_empty() {
                    return Err(ct_error(format!(
                        ". with no chord before it: {}",
                        self.rules[rule].text()
                    )));
                }
                regular.push(self.states[rule].last_regular_atom.clone());
            } else {
                regular.push(atom.to_string());
                self.states[rule].last_regular_atom = atom.to_string();
            }
        }
        if regular.is_empty() {
            // The group said nothing but a key or a meter.
            return Ok(());
        }
        let measure = self.built.len();
        self.built.push(Built::default());
        let meter = self.states[rule].meter.clone();
        let key = self.states[rule].key.clone();
        self.insert_key_and_meter(measure, &meter, &key);
        let length = op_frac(meter.bar_duration().quarter_length() / regular.len() as FloatType);
        let mut last_is_in_this_measure = false;
        for atom in regular {
            if atom == "R" {
                self.built[measure]
                    .items
                    .push(Rest::new(Duration::new(length)?).into());
                self.states[rule].last_chord = None;
                last_is_in_this_measure = false;
                continue;
            }
            let numeral = RomanNumeral::new(fixup_chord_atom(&atom), key.clone())?;
            let mut chord = numeral.to_chord()?.with_numeral(numeral);
            let last = self.states[rule]
                .last_chord
                .and_then(|place| self.built[place.measure].chord(place.item))
                .cloned();
            if is_same(&chord, last.as_ref()) && last_is_in_this_measure {
                // The same chord twice in one measure is one chord as long
                // as the two.
                if let Some(place) = self.states[rule].last_chord
                    && let Some(StreamElement::Chord(held)) =
                        self.built[place.measure].items.get_mut(place.item)
                {
                    let held_length = held
                        .duration()
                        .map_or(0.0, crate::duration::Duration::quarter_length);
                    held.set_duration(Duration::new(op_frac(held_length + length))?);
                }
                continue;
            }
            chord.set_duration(Duration::new(length)?);
            self.tie_and_label(rule, &mut chord, last.as_ref())?;
            let item = self.built[measure].items.len();
            self.built[measure].items.push(chord.into());
            self.states[rule].last_chord = Some(Place { measure, item });
            last_is_in_this_measure = true;
        }
        places.push(measure);
        // A measure played more than once is copied, the copies saying
        // nothing about the key or the meter.
        for _ in 1..repeats {
            let mut copy = self.built[measure].clone();
            copy.meter = None;
            copy.key = None;
            places.push(self.built.len());
            self.built.push(copy);
        }
        Ok(())
    }

    /// The tie a chord repeated across a barline carries, and the figure
    /// written under a chord that is not: music21's
    /// `addOptionalTieAndLyrics`, which writes the tie onto the chord
    /// before as well.
    fn tie_and_label(
        &mut self,
        rule: usize,
        chord: &mut Chord,
        last: Option<&Chord>,
    ) -> Result<()> {
        let same = is_same(chord, last);
        let before = self.states[rule].last_chord;
        let before_tie = last.and_then(|chord| chord.tie().cloned());
        if !same {
            if let Some(mut tie) = before_tie {
                tie.set_tie_type(crate::notation::TieType::Stop);
                self.write_tie(before, Some(tie));
            }
            if self.labels.roman_numerals {
                let figure = chord
                    .numeral()
                    .map(|numeral| numeral.figure().to_string())
                    .unwrap_or_default();
                chord.add_lyric(&figure, Some(1), false)?;
            }
            return Ok(());
        }
        match before_tie {
            None => {
                self.write_tie(before, Some(Tie::new(crate::notation::TieType::Start)));
            }
            Some(mut tie) => {
                tie.set_tie_type(crate::notation::TieType::Continue);
                self.write_tie(before, Some(tie));
            }
        }
        chord.set_tie(Some(Tie::new(crate::notation::TieType::Stop)));
        Ok(())
    }

    /// Writes a tie onto a chord already built.
    fn write_tie(&mut self, place: Option<Place>, tie: Option<Tie>) {
        if let Some(place) = place
            && let Some(StreamElement::Chord(held)) =
                self.built[place.measure].items.get_mut(place.item)
        {
            held.set_tie(tie);
        }
    }

    /// The key and the meter written into a measure where they have not
    /// been said yet: music21's `insertKsTs`.
    fn insert_key_and_meter(&mut self, measure: usize, meter: &TimeSignature, key: &Key) {
        if !self.has_song {
            // A rule expanded on its own says both in every measure.
            self.built[measure].meter = Some(meter.clone());
            self.built[measure].key = Some(key.clone());
            return;
        }
        if !self.meters_said.contains(meter) {
            self.built[measure].meter = Some(meter.clone());
            self.meters_said.push(meter.clone());
        }
        if !self.keys_said.iter().any(|said| same_key(said, key)) {
            self.built[measure].key = Some(key.clone());
            self.keys_said.push(key.clone());
        }
    }

    /// The name of the rule written under the first chord it brought: the
    /// end of music21's `expand`, which writes it on the first measure of
    /// the rule holding a note at all and on no other.
    fn label_section(&mut self, rule: usize, places: &[usize]) -> Result<()> {
        if !self.labels.subsections || self.rules[rule].left_hand_side() == "S" {
            return Ok(());
        }
        let name = self.rules[rule].left_hand_side().to_string();
        for place in places {
            let Some(item) = self.built[*place]
                .items
                .iter()
                .position(|item| matches!(item, StreamElement::Chord(_)))
            else {
                // A measure of rests alone carries no name.
                continue;
            };
            if let Some(StreamElement::Chord(chord)) = self.built[*place].items.get_mut(item) {
                let number = IntegerType::try_from(chord.lyrics().len() + 1).unwrap_or(1);
                chord.add_lyric(&name, Some(number), false)?;
            }
            break;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// music21's own `_measureGroups` docstring, group for group: a rule
    /// expanding others, measures of its own, an empty measure repeating
    /// the chord before it, and the counts a `|*4` says.
    #[test]
    fn a_rule_is_read_as_the_groups_music21_reads() {
        let rule = CtRule::new(
            "In: [A] [4/4] $Vr $BP*3 I IV | I | $BP*3 I IV | I | | R |*4 I |*4 % A comment",
        );
        let said: Vec<(String, GroupKind, usize)> = rule
            .measure_groups()
            .into_iter()
            .map(|group| (group.content, group.kind, group.repeats))
            .collect();
        let expected: Vec<(String, GroupKind, usize)> = vec![
            ("[A] [4/4]", GroupKind::Measure, 1),
            ("Vr", GroupKind::Expansion, 1),
            ("BP", GroupKind::Expansion, 3),
            ("I IV", GroupKind::Measure, 1),
            ("I", GroupKind::Measure, 1),
            ("BP", GroupKind::Expansion, 3),
            ("I IV", GroupKind::Measure, 1),
            ("I", GroupKind::Measure, 1),
            (".", GroupKind::Measure, 1),
            ("R", GroupKind::Measure, 4),
            ("I", GroupKind::Measure, 4),
        ]
        .into_iter()
        .map(|(content, kind, repeats)| (content.to_string(), kind, repeats))
        .collect();
        assert_eq!(said, expected);
        assert_eq!(rule.left_hand_side(), "In");
        assert_eq!(rule.comment(), Some("A comment"));
        assert_eq!(
            rule.music_text(),
            "[A] [4/4] $Vr $BP*3 I IV | I | $BP*3 I IV | I | | R |*4 I |*4"
        );
    }

    /// An empty measure holds the chord before it and a measure saying a
    /// meter holds it too, which is music21's `_measureGroups` docstring
    /// again -- the third measure is a four-four `IV` and the fourth the
    /// same chord in two-four, lasting half as long.
    #[test]
    fn an_empty_measure_holds_the_chord_before_it() {
        let rule = CtRule::new("Vr: [4/4] bVII | IV | | [2/4] |");
        let measures = rule.expand().expect("the rule expands");
        assert_eq!(measures.len(), 4);
        let described = |measure: &Stream| -> Vec<String> {
            measure
                .leaves()
                .iter()
                .map(|(offset, element)| match element {
                    StreamElement::Chord(chord) => format!(
                        "{offset} {} {}",
                        chord.numeral().map_or("", RomanNumeral::figure),
                        element.quarter_length()
                    ),
                    StreamElement::TimeSignature(meter) => meter.ratio_string(),
                    StreamElement::Key(key) => key.tonic_pitch_name_with_case(),
                    other => format!("{other:?}"),
                })
                .collect()
        };
        assert_eq!(described(&measures[2]), ["C", "4/4", "0 IV 4"]);
        assert_eq!(described(&measures[3]), ["C", "2/4", "0 IV 2"]);
    }

    /// The song music21 measures in its own `toPart` docstring.
    /// music21's converter refuses text with no barline or no `S:` rule
    /// rather than reading it as a song of nothing (b30248a6a).
    #[test]
    fn text_with_no_barline_or_no_song_is_refused() {
        for text in ["S: [C] I IV V I\n", "Vr: I | IV |\n"] {
            let refused = from_clercq_temperley(text).unwrap_err();
            assert!(
                refused.to_string().contains(r#"must contain "|" and "S:""#),
                "{refused}"
            );
        }
    }

    #[test]
    fn blitzkrieg_bop_is_as_long_as_music21_makes_it() {
        let song = "% Blitzkrieg Bop\n\
                    BP: I | IV V | %THIS IS A COMMENT\n\
                    In: $BP*3 I IV | I | $BP*3 I IV | I | R |*4 I |*4\n\
                    Vr: $BP*3 I IV | I |\n\
                    Br: IV | | I | IV I | IV | | ii | IV V |\n\
                    Co: R |*4 I |*4\n\
                    S: [A] $In $Vr $Vr $Br $Vr $Vr $Br $Vr $Vr $Co\n";
        let part = from_clercq_temperley(song).expect("the song reads");
        assert_eq!(part.measures().len(), 96);
        assert_eq!(
            part.measures().last().map(|measure| measure.number()),
            Some(96)
        );
        // music21's `partObj.highestOffset`, which is where the last
        // measure stands.
        assert_eq!(part.end_offset() - 4.0, 380.0);
    }

    /// The title and the comments of music21's smallest example, as its own
    /// docstrings read them.
    #[test]
    fn a_song_says_its_title_and_its_comments() {
        let song = CtSong::parse(
            "\n% Simple Gifts\n% A wonderful shaker melody\nVr: I | I | %incomplete verse\n\
             S: [A] $Vr % Not quite finished!",
        );
        assert_eq!(song.title(), "Simple Gifts");
        assert_eq!(
            song.comments(),
            vec![
                vec!["A wonderful shaker melody".to_string()],
                vec!["Vr:".to_string(), "incomplete verse".to_string()],
                vec!["S:".to_string(), "Not quite finished!".to_string()],
            ]
        );
        assert_eq!(song.rules().len(), 2);
        assert_eq!(
            song.home_key()
                .expect("a key")
                .map(|key| key.tonic_pitch_name_with_case()),
            Some("A".to_string())
        );
        assert_eq!(
            song.home_time_signature()
                .expect("a meter")
                .map(|meter| meter.ratio_string()),
            Some("4/4".to_string())
        );
    }

    /// What music21 reads off a line with no name on it, and a title with a
    /// per cent in it: both are what its own slicing gives rather than what
    /// a reader would guess, and both were read off music21.
    #[test]
    fn a_line_is_read_as_music21_slices_it() {
        // One character past the name, whatever that character is.
        assert_eq!(CtRule::new("I | IV |").music_text(), "| IV |");
        assert_eq!(CtRule::new("I | IV |").left_hand_side(), "");
        assert_eq!(CtRule::new("In: $BP*3 I | % c").music_text(), "$BP*3 I |");
        // Every per cent comes out of the title, not only the one in front.
        let song = CtSong::parse("% 50% More\nVr: I |\nS: [C] $Vr\n");
        assert_eq!(song.title(), "50 More");
    }

    /// The sections music21 spells out, its own docstring's among them.
    #[test]
    fn a_rule_names_the_section_it_stands_for() {
        let named = |text: &str| CtRule::new(text).section_name();
        assert_eq!(named("Vr2: I |"), "Verse2");
        assert_eq!(named("In: I |"), "Introduction");
        assert_eq!(named("Br: I |"), "Bridge");
        assert_eq!(named("Ch2: I |"), "Chorus2");
        assert_eq!(named("Tg: I |"), "Tag");
        assert_eq!(named("S: $Vr"), "Song");
        assert_eq!(named("Fadeout: I |"), "Fadeout");
        assert_eq!(named("Xx: I |"), "Xx");
    }
}
