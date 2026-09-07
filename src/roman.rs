use crate::{
    chord::{Chord, root::pitch_class},
    chordsymbol::{ChordQuality, ChordSymbol},
    defaults::{FloatType, IntegerType},
    error::{Error, Result},
    interval::Interval,
    key::Key,
    pitch::{Accidental, Pitch},
};
use std::fmt;

/// How a figure on the sixth or seventh degree of a minor key decides
/// between the natural and the raised degree: music21's `Minor67Default`.
///
/// Minor is two scales at once, and a `vi` might mean either of two chords.
/// music21 lets a caller say which reading to use, and the readings differ
/// in what they do with an accidental the figure already carries.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Minor67Default {
    /// Read it from the chord the figure asks for: a minor triad on the
    /// sixth or a diminished chord on the seventh needs the raised degree,
    /// and a major one needs the natural. This is music21's default and the
    /// reading most scores assume.
    #[default]
    Quality,
    /// Always the natural degree, whatever the figure asks for. A sharp
    /// written in front still raises it.
    Flat,
    /// Always the raised degree. A flat written in front still lowers it.
    Sharp,
    /// As `Quality`, but an accidental already written in front is read as a
    /// caution rather than as a further change — so `#vi` and `vi` are the
    /// same chord, and so are `bVI` and `VI`.
    Cautionary,
}

/// A parsed Roman numeral in a key.
#[derive(Clone, Debug)]
pub struct RomanNumeral {
    figure: String,
    key: Key,
    degree: u8,
    accidental: i8,
    /// The alteration as it was written in front of the numeral, before the
    /// minor sixth and seventh were read against the chord asked for.
    ///
    /// music21 keeps the two apart: `vi` in C minor reports `#vi` as its
    /// numeral, because its root is a raised sixth degree, while the figure
    /// it was written with carries no accidental at all.
    written_accidental: i8,
    inversion: u8,
    seventh: bool,
    quality: RomanQuality,
    /// The quality the figure states, as music21 reads it off the symbol in
    /// front of the inversion digits or off the case of the numeral. It is
    /// what the notes read from the scale are respelled to.
    implied_quality: ImpliedQuality,
    /// The figured-bass column the digits stand for, with the omissions,
    /// additions and alterations written in brackets beside them.
    figures: FiguredBass,
    secondary: Option<String>,
    kind: RomanKind,
    /// How the sixth and seventh degrees of a minor key are read.
    sixth_minor: Minor67Default,
    seventh_minor: Minor67Default,
    /// The collection the figure is read over, when it is not a key at all.
    ///
    /// music21 takes a `ConcreteScale` wherever it takes a key, and reads
    /// every degree off that instead — which is how a numeral means
    /// something in a collection no key signature can write, such as the
    /// octatonic. The key is still carried, as the major of the same tonic,
    /// because everything that asks a numeral for its key expects one.
    scale: Option<crate::scale::Scale>,
    /// Whether an upper-case numeral means a major chord and a lower-case
    /// one a minor chord.
    ///
    /// music21's `caseMatters`. The older figured-bass tradition writes every
    /// numeral in upper case and lets the key say what the chord is, and a
    /// numeral read that way states no quality at all: its notes are whatever
    /// the scale spells, and the sixth and seventh of a minor key are left
    /// where the key signature put them.
    case_matters: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RomanKind {
    Diatonic,
    AugmentedSixth(AugmentedSixthKind),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AugmentedSixthKind {
    Italian,
    French,
    German,
    Swiss,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RomanQuality {
    Major,
    Minor,
    Diminished,
    HalfDiminished,
    Augmented,
}

impl AugmentedSixthKind {
    fn from_figure(figure: &str) -> Option<Self> {
        match figure.trim() {
            // music21 writes these with or without the `+6`, and with the
            // inversion figure in place of it — `Ger65` is the German sixth
            // in the position it is nearly always used in.
            "It" | "It+6" | "It6" | "It53" | "It63" | "It64" => Some(Self::Italian),
            "Fr" | "Fr+6" | "Fr6" | "Fr43" | "Fr7" | "Fr42" | "Fr65" => Some(Self::French),
            "Ger" | "Ger+6" | "Ger6" | "Ger65" | "Ger7" | "Ger43" | "Ger42" => Some(Self::German),
            "Sw" | "Sw+6" | "Sw6" | "Sw43" | "Sw7" | "Sw65" | "Sw42" => Some(Self::Swiss),
            _ => None,
        }
    }

    fn from_common_name(name: &str) -> Option<Self> {
        if name.contains("Italian augmented sixth chord") {
            Some(Self::Italian)
        } else if name.contains("French augmented sixth chord") {
            Some(Self::French)
        } else if name.contains("German augmented sixth chord") {
            Some(Self::German)
        } else if name.contains("Swiss augmented sixth chord") {
            Some(Self::Swiss)
        } else {
            None
        }
    }

    fn figure(self) -> &'static str {
        match self {
            Self::Italian => "It+6",
            Self::French => "Fr+6",
            Self::German => "Ger+6",
            Self::Swiss => "Sw+6",
        }
    }

    fn interval_names(self) -> Vec<&'static str> {
        match self {
            Self::Italian => vec!["P1", "M3", "a6"],
            Self::French => vec!["P1", "M3", "a4", "a6"],
            Self::German => vec!["P1", "M3", "P5", "a6"],
            Self::Swiss => vec!["P1", "M3", "aa4", "a6"],
        }
    }
}

impl RomanNumeral {
    /// Parses a Roman numeral figure in a key.
    ///
    /// Supports ordinary figures such as `V7/V` and augmented-sixth figures
    /// such as `It+6`, `Fr+6`, `Ger+6`, and `Sw+6`.
    pub fn new(figure: impl Into<String>, key: Key) -> Result<Self> {
        Self::with_minor_defaults(
            figure,
            key,
            Minor67Default::default(),
            Minor67Default::default(),
        )
    }

    /// The same, saying how the sixth and seventh degrees of a minor key are
    /// to be read: music21's `sixthMinor` and `seventhMinor`.
    pub fn with_minor_defaults(
        figure: impl Into<String>,
        key: Key,
        sixth_minor: Minor67Default,
        seventh_minor: Minor67Default,
    ) -> Result<Self> {
        Self::with_options(figure, key, sixth_minor, seventh_minor, true)
    }

    /// The same again, saying whether the case of the numeral states the
    /// chord's quality: music21's `caseMatters`.
    pub fn with_options(
        figure: impl Into<String>,
        key: Key,
        sixth_minor: Minor67Default,
        seventh_minor: Minor67Default,
        case_matters: bool,
    ) -> Result<Self> {
        Self::over_scale(figure, key, None, sixth_minor, seventh_minor, case_matters)
    }

    /// The same again over a scale that is not a key: music21's numerals
    /// read against a `ConcreteScale`.
    ///
    /// The key is still needed — a numeral reports one, and a secondary
    /// numeral establishes one — so pass the major key of the scale's tonic,
    /// which is what music21 falls back on.
    pub fn over_scale(
        figure: impl Into<String>,
        key: Key,
        scale: Option<crate::scale::Scale>,
        sixth_minor: Minor67Default,
        seventh_minor: Minor67Default,
        case_matters: bool,
    ) -> Result<Self> {
        let figure = figure.into();
        let written = fold_figure_symbols(figure.trim());
        let trimmed = written.as_str();
        if trimmed.is_empty() {
            return Err(Error::Chord("roman numeral cannot be empty".to_string()));
        }
        validate_figure(trimmed)?;

        if let Some(kind) = AugmentedSixthKind::from_figure(trimmed) {
            return Ok(Self {
                // The figure is kept as written, since music21 names these
                // several ways and reports back the one it was given.
                figure: written.clone(),
                key,
                degree: 6,
                accidental: -1,
                written_accidental: -1,
                inversion: 0,
                seventh: false,
                quality: RomanQuality::Augmented,
                implied_quality: ImpliedQuality::Augmented,
                figures: FiguredBass::default(),
                secondary: None,
                kind: RomanKind::AugmentedSixth(kind),
                sixth_minor,
                seventh_minor,
                case_matters,
                scale,
            });
        }

        let (primary, secondary) = split_secondary(trimmed);

        // music21 writes a few chords by name rather than by numeral: the
        // Neapolitan and the cadential six-four. Each is read as the figure
        // it stands for, while the numeral keeps the name it was given — and
        // read in the key it actually sounds in, so the `Cad64` of `Cad64/V`
        // in C minor is the major tonic of G and not the minor one of C.
        let reading = match &secondary {
            Some(secondary) => {
                secondary_key(&key, secondary, sixth_minor, seventh_minor, case_matters)?
            }
            None => key.clone(),
        };
        let mut working = named_figure(primary, &reading);

        // The brackets come off before anything reads the digits, so a
        // `[no3]` cannot be mistaken for a diminished mark.
        let omitted = take_omitted_steps(&mut working);
        let added = take_added_steps(&mut working);
        let bracketed = take_bracketed_alterations(&mut working);

        let (accidental, rest) = split_roman_accidental_prefix(&working);
        let (roman, suffix) = split_roman_prefix(rest)?;
        let degree = roman_degree(roman)?;
        let (implied_quality, mut column) =
            implied_quality_from_string(roman, suffix, case_matters);
        let inversion = parse_inversion(&column);
        let seventh = suffix_has_seventh(&column);

        let mut numeral = Self {
            figure: written.clone(),
            key,
            degree,
            accidental,
            written_accidental: accidental,
            inversion,
            seventh,
            quality: RomanQuality::from(implied_quality),
            implied_quality,
            figures: FiguredBass::default(),
            secondary,
            kind: RomanKind::Diatonic,
            sixth_minor,
            seventh_minor,
            case_matters,
            scale,
        };
        numeral.raise_minor_sixth_and_seventh(&mut column)?;
        // This crate writes an addition as `add(13)` where music21 writes a
        // bracket, and it is not part of the figured-bass column: an added
        // thirteenth is a note beside the chord, not a figure over the bass.
        let column = strip_roman_addition_groups(&column);
        numeral.figures = FiguredBass {
            omitted,
            added,
            bracketed,
            ..figured_bass_column(&expand_shorthand(&column))
        };
        Ok(numeral)
    }

    /// Raises the sixth and seventh degrees of a minor key where the figure
    /// asks for a chord that only the raised degree gives.
    ///
    /// This is music21's `_adjustMinorVIandVIIByQuality` at its default
    /// setting, and it is what makes `viio7` in A minor the diminished
    /// seventh on `G#` rather than a chord on `G` with three flats in it,
    /// and `vi` the minor triad on `F#`. The natural degrees are what `VI`
    /// and `VII` ask for, and those are left alone.
    fn raise_minor_sixth_and_seventh(&mut self, column: &mut String) -> Result<()> {
        if !matches!(self.degree, 6 | 7) {
            return Ok(());
        }
        // Against the key the figure is actually read in, so the `vi` of a
        // secondary numeral is judged in the key that numeral establishes.
        if !self.case_matters {
            return Ok(());
        }
        if self.effective_key()?.mode() != "minor" {
            return Ok(());
        }
        let reading = if self.degree == 6 {
            self.sixth_minor
        } else {
            self.seventh_minor
        };
        // A chord that only the raised degree gives.
        let wants_raised = matches!(
            self.implied_quality,
            ImpliedQuality::Minor | ImpliedQuality::Diminished | ImpliedQuality::HalfDiminished
        );
        let raise = match reading {
            Minor67Default::Flat => false,
            Minor67Default::Sharp => true,
            Minor67Default::Quality => wants_raised,
            // An accidental already written is a caution, not a further
            // change: `#vi` and `vi` are the same chord, and so are `bVI`
            // and `VI`.
            Minor67Default::Cautionary => match self.accidental {
                0 => wants_raised,
                // A sharp already written is the caution, and says nothing
                // more; a flat is a caution against the raised degree, so
                // the raise it cancels is applied and the two meet at the
                // natural one.
                sharps if sharps >= 1 => false,
                _ => true,
            },
        };
        if raise {
            self.accidental += 1;
            sharpen_figure(column);
        }
        Ok(())
    }

    /// The alterations written in square brackets, as the semitones each
    /// moves its chord step by and the step it moves.
    pub fn bracketed_alterations(&self) -> &[(i8, u8)] {
        &self.figures.bracketed
    }

    /// The chord steps the figure leaves out, as `[no3]`.
    pub fn omitted_steps(&self) -> &[u8] {
        &self.figures.omitted
    }

    /// The notes the figure puts in beside the chord, as `[add4]`: the
    /// alteration in semitones and how far above the root each stands.
    pub fn added_steps(&self) -> &[(i8, u8)] {
        &self.figures.added
    }

    /// The scale the figure is read over, where it is not a key at all.
    ///
    /// A numeral read this way spells its chord where the scale stands, so
    /// nothing downstream has to place it.
    pub fn scale(&self) -> Option<&crate::scale::Scale> {
        self.scale.as_ref()
    }

    /// The collection the figure's degrees are read off.
    fn reading(&self) -> Result<Reading> {
        Ok(match &self.scale {
            Some(scale) => Reading::Scale(scale.clone()),
            None => Reading::Key(self.effective_key()?),
        })
    }

    /// The quality the figure states, which is what the notes read off the
    /// scale are respelled to.
    pub fn implied_quality(&self) -> ImpliedQuality {
        self.implied_quality
    }

    /// The numbers of the figured-bass column the figure's digits stand for,
    /// written high to low and expanded out of music21's shorthand.
    pub fn figure_numbers(&self) -> Vec<u8> {
        self.figures.numbers()
    }

    /// Whether the case of the numeral states the chord's quality.
    pub fn case_matters(&self) -> bool {
        self.case_matters
    }

    /// How this numeral reads the sixth degree of a minor key.
    pub fn sixth_minor(&self) -> Minor67Default {
        self.sixth_minor
    }

    /// How it reads the seventh.
    pub fn seventh_minor(&self) -> Minor67Default {
        self.seventh_minor
    }

    /// Returns the original figure.
    pub fn figure(&self) -> &str {
        &self.figure
    }

    /// Returns the one-based scale degree.
    pub fn degree(&self) -> u8 {
        self.degree
    }

    /// Returns the chromatic alteration of the scale degree in semitones.
    ///
    /// Negative values are flats and positive values are sharps, so `bII`
    /// returns `-1` and `#iv` returns `1`.
    pub fn written_accidental(&self) -> i8 {
        self.written_accidental
    }

    /// The alteration the numeral reports, once the sixth and seventh
    /// degrees of a minor key have been read against the chord asked for.
    pub fn accidental(&self) -> i8 {
        self.accidental
    }

    /// Returns the inversion number, where root position is `0`.
    pub fn inversion(&self) -> u8 {
        self.inversion
    }

    /// Returns the secondary/applied target figure, if any.
    pub fn secondary(&self) -> Option<&str> {
        self.secondary.as_deref()
    }

    /// Returns the key context.
    pub fn key(&self) -> &Key {
        &self.key
    }

    /// The numeral with its front alteration and nothing else: music21's
    /// `romanNumeral`, so `bII6` is `bII` and `V65/V` is `V`. An augmented
    /// sixth answers its nationality, `It`, `Fr`, `Ger` or `Sw`.
    pub fn roman_numeral(&self) -> String {
        let numeral = self.roman_numeral_alone();
        if matches!(self.kind, RomanKind::AugmentedSixth(_)) {
            return numeral;
        }
        let prefix = match self.accidental {
            0 => String::new(),
            sharps if sharps > 0 => "#".repeat(sharps.unsigned_abs() as usize),
            flats => "b".repeat(flats.unsigned_abs() as usize),
        };
        format!("{prefix}{numeral}")
    }

    /// The numeral with nothing written in front of it: music21's
    /// `romanNumeralAlone`, so `bVII65/V` is `VII`.
    pub fn roman_numeral_alone(&self) -> String {
        if let RomanKind::AugmentedSixth(kind) = self.kind {
            return kind.figure().trim_end_matches(['+', '6']).to_string();
        }
        let primary = self.figure.split('/').next().unwrap_or_default();
        let (_, unaltered) = split_roman_accidental_prefix(primary);
        unaltered
            .chars()
            .take_while(|c| matches!(c, 'i' | 'v' | 'x' | 'I' | 'V' | 'X'))
            .collect()
    }

    /// The figure and its key together: music21's `figureAndKey`,
    /// `bII6 in a minor`.
    pub fn figure_and_key(&self) -> String {
        format!(
            "{} in {} {}",
            self.figure,
            self.key.tonic_pitch_name_with_case(),
            self.key.mode()
        )
    }

    /// The scale degree with the alteration in front of it, as sharps
    /// (positive) or flats (negative): music21's `scaleDegreeWithAlteration`.
    pub fn scale_degree_with_alteration(&self) -> (u8, i8) {
        (self.degree, self.accidental)
    }

    /// How strongly the figure implies its function, from music21's
    /// `functionalityScores` table: `I` is 100, `V7` 80, an unknown figure 0.
    /// A secondary figure multiplies the scores of its halves.
    /// An augmented sixth is looked up by music21's spelling, `It6` rather
    /// than the `It+6` this crate writes.
    pub fn functionality_score(&self) -> u8 {
        if self.secondary.is_some() {
            let score = self.figure.split('/').fold(100.0, |score, part| {
                score * f64::from(functionality_score_of(part).unwrap_or(0)) / 100.0
            });
            return score as u8;
        }
        functionality_score_of(&self.figure.replace('+', "")).unwrap_or(0)
    }

    /// Whether this is a Neapolitan chord: a major triad on the flattened
    /// second degree, in first inversion unless `require_first_inversion` is
    /// off.
    pub fn is_neapolitan(&self, require_first_inversion: bool) -> bool {
        self.degree == 2
            && self.accidental == -1
            && self.quality == RomanQuality::Major
            && (!require_first_inversion || self.inversion == 1)
    }

    /// Whether the chord is borrowed from the parallel mode: music21's
    /// `isMixture`, so `iv` and `bVI` in a major key are mixture and `IV` in a
    /// minor key is. With `evaluate_secondary` a secondary figure is judged by
    /// the numeral after the slash.
    pub fn is_mixture(&self, evaluate_secondary: bool) -> Result<bool> {
        if evaluate_secondary && let Some(secondary) = &self.secondary {
            return RomanNumeral::new(secondary.clone(), self.key.clone())?.is_mixture(true);
        }
        if self.kind != RomanKind::Diatonic || !(1..=7).contains(&self.degree) {
            return Ok(false);
        }
        let quality = match self.quality {
            RomanQuality::Diminished => "diminished",
            RomanQuality::Minor => "minor",
            RomanQuality::Major => "major",
            RomanQuality::HalfDiminished | RomanQuality::Augmented => return Ok(false),
        };
        let front = match self.accidental {
            0 => "natural",
            1 => "sharp",
            -1 => "flat",
            _ => "other",
        };
        let entry = (self.degree, quality, front);
        Ok(match self.key.mode() {
            "major" => {
                MAJOR_KEY_MIXTURES.contains(&entry)
                    || (self.degree == 7
                        && self.seventh
                        && self.quality == RomanQuality::Diminished)
            }
            "minor" => {
                MINOR_KEY_MIXTURES.contains(&entry)
                    || (self.degree == 7
                        && self.seventh
                        && self.quality == RomanQuality::HalfDiminished)
            }
            _ => false,
        })
    }

    /// The same figure in the key transposed by `interval`.
    pub fn transpose(&self, interval: &Interval) -> Result<Self> {
        RomanNumeral::new(self.figure.clone(), self.key.transpose(interval)?)
    }

    /// The chord the numeral stands for, spelled where its key sounds.
    ///
    /// This is music21's `_updatePitches`, and it is a figured-bass reading
    /// rather than a stack of intervals: the bass is the scale degree the
    /// inversion figure puts there, every number of the column is that many
    /// scale steps above it, and only then is the result respelled to the
    /// quality the numeral's case and symbols asked for. Reading it off the
    /// scale is what lets a numeral mean something in a mode, and what makes
    /// `V7b5` alter one note rather than name a different chord.
    pub fn to_chord(&self) -> Result<Chord> {
        if let RomanKind::AugmentedSixth(kind) = self.kind {
            return self.augmented_sixth_chord(kind);
        }

        let reading = self.reading()?;
        let numbers = self.figures.numbers();
        let implies_root = FIGURES_IMPLYING_ROOT.contains(&numbers.as_slice());
        let bass_degree = self.bass_scale_degree(&numbers, implies_root)?;

        let mut pitches = vec![reading.pitch_at(bass_degree)?];
        for figure in self.figures.figures.iter().rev() {
            let degree = bass_degree + IntegerType::from(figure.number) - 1;
            let mut pitch = reading.pitch_at(degree)?;
            if let Some(alter) = figure.alter {
                pitch.set_accidental(Some(Accidental::new(modified_alter(&pitch, alter))?));
            }
            let below = pitches.last().map_or(0.0, Pitch::ps);
            if pitch.ps() < below {
                pitch.set_octave(Some(pitch.octave().unwrap_or(4) + 1));
            }
            pitches.push(pitch);
        }

        // The alteration in front of the numeral moves the chord without
        // renaming it — music21 transposes by an augmented unison — but it
        // leaves the notes above the fifth alone, since a `bVII9` is a chord
        // on a flattened seventh degree and not a flattened ninth.
        if self.accidental != 0 {
            let untouched = upper_extension_indices(&pitches)?;
            for (index, pitch) in pitches.iter_mut().enumerate() {
                if untouched.contains(&index) {
                    continue;
                }
                let alter = pitch.accidental().alter() + FloatType::from(self.accidental);
                pitch.set_accidental(Some(Accidental::new(alter)?));
            }
        }

        // A column that says nothing about a root is a stack over its bass.
        let root = if implies_root {
            None
        } else {
            Some(pitches[0].clone())
        };

        self.match_accidentals_to_quality(&mut pitches, root.as_ref())?;
        self.correct_bracketed_pitches(&mut pitches, root.as_ref())?;

        // A note left out or put in must not move the root, so the root is
        // read while the chord is still whole and recorded from there.
        let altered = !self.figures.omitted.is_empty() || !self.figures.added.is_empty();
        let recorded = match &root {
            Some(root) => Some(root.clone()),
            None if altered => Chord::new(pitches.as_slice())?.root().cloned(),
            None => None,
        };

        self.omit_steps(&mut pitches, recorded.as_ref())?;
        self.add_steps(&mut pitches, &reading)?;

        let mut chord = Chord::new(pitches.as_slice())?;
        chord.set_root(recorded);
        Ok(chord)
    }

    /// The scale degree the inversion figure puts in the bass.
    fn bass_scale_degree(&self, numbers: &[u8], implies_root: bool) -> Result<IntegerType> {
        if !implies_root {
            return Ok(IntegerType::from(self.degree));
        }
        bass_scale_degree_from_notation_in(self.degree, numbers, self.reading()?.cardinality())
            .map(IntegerType::from)
    }

    /// music21's `_matchAccidentalsToQuality`, over the chord being built:
    /// an accidental written on a figure is left where it was put, which is
    /// what keeps the flat of `V7b5`.
    fn match_accidentals_to_quality(
        &self,
        pitches: &mut [Pitch],
        root: Option<&Pitch>,
    ) -> Result<()> {
        let written: Vec<u8> = [3u8, 5, 7]
            .into_iter()
            .filter(|step| self.figures.alters(*step))
            .collect();
        match_pitches_to_quality(pitches, root, self.implied_quality, &written)
    }

    /// music21's `_correctBracketedPitches`: an alteration written in square
    /// brackets moves a chord step without changing which step it is.
    fn correct_bracketed_pitches(&self, pitches: &mut [Pitch], root: Option<&Pitch>) -> Result<()> {
        for (alter, step) in &self.figures.bracketed {
            let Some(index) = chord_step_index(pitches, root, *step)? else {
                continue;
            };
            let moved = pitches[index].accidental().alter() + FloatType::from(*alter);
            pitches[index].set_accidental(Some(Accidental::new(moved)?));
        }
        Ok(())
    }

    /// music21's omitted steps: a `[no3]` drops every note of that step.
    fn omit_steps(&self, pitches: &mut Vec<Pitch>, root: Option<&Pitch>) -> Result<()> {
        if self.figures.omitted.is_empty() {
            return Ok(());
        }
        let mut dropped = Vec::new();
        for step in &self.figures.omitted {
            if let Some(index) = chord_step_index(pitches, root, *step)? {
                dropped.push(pitches[index].name());
            }
        }
        pitches.retain(|pitch| !dropped.contains(&pitch.name()));
        Ok(())
    }

    /// music21's added steps: an `[add4]` puts in the note that many scale
    /// steps above the *root*, at or above the bass.
    fn add_steps(&self, pitches: &mut Vec<Pitch>, reading: &Reading) -> Result<()> {
        if self.figures.added.is_empty() {
            return Ok(());
        }
        let bass = pitches.first().map_or(0.0, Pitch::ps);
        for (alter, step) in &self.figures.added {
            let degree = IntegerType::from(self.degree) + IntegerType::from(*step) - 1;
            let mut added = reading.pitch_at(degree)?;
            let moved = added.accidental().alter() + FloatType::from(*alter);
            added.set_accidental(Some(Accidental::new(moved)?));
            while added.ps() < bass {
                added.set_octave(Some(added.octave().unwrap_or(4) + 1));
            }
            // An added note spelled onto the bass belongs above it, not under
            // it: `IV[add#7]` in C would otherwise put `E#` in the bass.
            if added.ps() == bass
                && pitches
                    .first()
                    .is_some_and(|low| added.diatonic_note_number() < low.diatonic_note_number())
            {
                added.set_octave(Some(added.octave().unwrap_or(4) + 1));
            }
            if !pitches
                .iter()
                .any(|pitch| pitch.name_with_octave() == added.name_with_octave())
            {
                pitches.push(added);
            }
        }
        // Two notes may sound alike and still be written apart, and the one
        // written lower is the one written first.
        pitches.sort_by(|left, right| {
            left.ps()
                .partial_cmp(&right.ps())
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(
                    left.diatonic_note_number()
                        .cmp(&right.diatonic_note_number()),
                )
        });
        Ok(())
    }

    fn augmented_sixth_chord(&self, kind: AugmentedSixthKind) -> Result<Chord> {
        let mut lowered_sixth = self.key.pitch_from_degree(6)?;
        if self.key.mode() != "minor" {
            lowered_sixth = Interval::from_semitones(-1)?.transpose_pitch(&lowered_sixth)?;
        }
        let pitches = kind
            .interval_names()
            .into_iter()
            .map(|name| Interval::from_name(name)?.transpose_pitch(&lowered_sixth))
            .collect::<Result<Vec<_>>>()?;
        Chord::new(pitches.as_slice())
    }

    /// Performs functional Roman-numeral analysis in a key.
    pub fn analyze(chord: &Chord, key: Key) -> Result<Option<Self>> {
        let Some(root_name) = chord.root_pitch_name() else {
            return Ok(None);
        };
        let root = Pitch::from_name(normalize_pitch_name(&root_name))?;
        Self::analyze_with_root(chord, key, &root)
    }

    /// Performs Roman-numeral analysis using an explicit harmonic root.
    ///
    /// This is useful for pitch-class-set browser views where the caller has
    /// already chosen a transposition root and does not want inversion or root
    /// inference to pick a different chord member.
    pub fn analyze_with_root(chord: &Chord, key: Key, root: &Pitch) -> Result<Option<Self>> {
        if let Some(kind) = augmented_sixth_kind_for_key(chord, &key)? {
            return Self::new(kind.figure(), key).map(Some);
        }

        let root_pc = pitch_class(root);
        let intervals = intervals_above_root(chord, root_pc);
        if !intervals.contains(&0) {
            return Ok(None);
        }

        let Some((degree, accidental)) = degree_for_root(&key, root)? else {
            return Ok(None);
        };

        let symbol = chord
            .chord_symbols_with_root(root_pc)?
            .into_iter()
            .find_map(|figure| ChordSymbol::parse(figure).ok());
        let quality = symbol
            .as_ref()
            .map(symbol_quality)
            .unwrap_or_else(|| quality_from_intervals(&intervals));

        let figure = roman_figure(
            degree,
            accidental,
            quality,
            symbol.as_ref(),
            &intervals,
            roman_inversion(chord),
        );

        Self::new(figure, key).map(Some)
    }

    /// The key the figure is actually read in: the key it was given, or the
    /// one a secondary numeral establishes — the `V` of `V/V` in G major is
    /// read in D major.
    pub fn effective_key_of(&self) -> Result<Key> {
        self.effective_key()
    }

    fn effective_key(&self) -> Result<Key> {
        let Some(secondary) = &self.secondary else {
            return Ok(self.key.clone());
        };

        secondary_key(
            &self.key,
            secondary,
            self.sixth_minor,
            self.seventh_minor,
            self.case_matters,
        )
    }
}

impl fmt::Display for RomanNumeral {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.figure())
    }
}

/// music21's `functionalityScores`: how strongly a figure implies its
/// harmonic function, on a hundred-point scale, in music21's order.
pub const FUNCTIONALITY_SCORES: [(&str, u8); 48] = [
    ("I", 100),
    ("i", 90),
    ("V7", 80),
    ("V", 70),
    ("V65", 68),
    ("I6", 65),
    ("V6", 63),
    ("V43", 61),
    ("I64", 60),
    ("IV", 59),
    ("i6", 58),
    ("viio7", 57),
    ("V42", 55),
    ("viio65", 53),
    ("viio6", 52),
    ("#viio65", 51),
    ("ii", 50),
    ("#viio6", 49),
    ("ii65", 48),
    ("ii43", 47),
    ("ii42", 46),
    ("IV6", 45),
    ("ii6", 43),
    ("VI", 42),
    ("#VI", 41),
    ("vi", 40),
    ("viio", 39),
    ("#viio", 38),
    ("iio", 37),
    ("iio42", 36),
    ("bII6", 35),
    ("It6", 34),
    ("Ger65", 33),
    ("iio43", 32),
    ("iio65", 31),
    ("Fr43", 30),
    ("#vio", 28),
    ("#vio6", 27),
    ("III", 22),
    ("Sw43", 21),
    ("v", 20),
    ("VII", 19),
    ("VII7", 18),
    ("IV65", 17),
    ("IV7", 16),
    ("iii", 15),
    ("iii6", 12),
    ("vi6", 10),
];

fn functionality_score_of(figure: &str) -> Option<u8> {
    FUNCTIONALITY_SCORES
        .iter()
        .find(|(known, _)| *known == figure)
        .map(|(_, score)| *score)
}

const MAJOR_KEY_MIXTURES: [(u8, &str, &str); 7] = [
    (1, "minor", "natural"),
    (2, "diminished", "natural"),
    (3, "major", "flat"),
    (4, "minor", "natural"),
    (5, "minor", "natural"),
    (6, "major", "flat"),
    (7, "major", "flat"),
];

const MINOR_KEY_MIXTURES: [(u8, &str, &str); 5] = [
    (1, "major", "natural"),
    (2, "minor", "natural"),
    (3, "minor", "sharp"),
    (4, "major", "natural"),
    (6, "minor", "sharp"),
];

/// The figured-bass suffix a Roman numeral takes for a chord's inversion:
/// music21's `romanInversionName`, `6` and `64` for a triad, `7`, `65`, `43`
/// and `42` for a seventh chord, and nothing otherwise. The inversion is the
/// chord's own unless one is given.
pub fn roman_inversion_name(chord: &Chord, inversion: Option<u8>) -> String {
    match chord.root() {
        Some(root) => inversion_name_from_root(chord, root, inversion),
        None => String::new(),
    }
}

fn inversion_name_from_root(chord: &Chord, root: &Pitch, inversion: Option<u8>) -> String {
    let Some(inversion) = inversion.or_else(|| chord.inversion_with_root(root)) else {
        return String::new();
    };
    let has = |step: u8| chord.chord_step_from(step, root).is_some();
    let suffix = if has(7) {
        match inversion {
            0 => "7",
            1 => "65",
            2 => "43",
            3 => "42",
            _ => "",
        }
    } else if has(1) && has(3) {
        match inversion {
            1 => "6",
            2 => "64",
            _ => "",
        }
    } else {
        ""
    };
    suffix.to_string()
}

/// Reads a chord as the tonic or the dominant of a key, with its inversion:
/// music21's `identifyAsTonicOrDominant`, so `G B D F` in C major is `V7`
/// and `A C E` in A minor is `i`. A chord containing the tonic is the tonic,
/// one containing the dominant is the dominant, and anything else is judged
/// by how many of its notes the tonic and dominant sevenths share; `None`
/// when neither wins.
pub fn identify_as_tonic_or_dominant(chord: &Chord, key: &Key) -> Result<Option<String>> {
    let names = chord.pitch_names();
    let tonic = key.pitch_from_degree(1)?;
    let dominant = key.pitch_from_degree(5)?;
    let overlap = |figure: &str| -> Result<usize> {
        let members = RomanNumeral::new(figure, key.clone())?
            .to_chord()?
            .pitch_names();
        Ok(members.iter().filter(|name| names.contains(name)).count())
    };
    let is_tonic = if names.contains(&tonic.name()) {
        true
    } else if names.contains(&dominant.name()) {
        false
    } else {
        let (one, five) = (overlap("I7")?, overlap("V7")?);
        if one == five {
            return Ok(None);
        }
        one > five
    };
    let (numeral, root) = if is_tonic {
        (if key.mode() == "minor" { "i" } else { "I" }, &tonic)
    } else {
        ("V", &dominant)
    };
    Ok(Some(format!(
        "{numeral}{}",
        inversion_name_from_root(chord, root, None)
    )))
}

/// Performs functional Roman-numeral analysis in a key.
pub fn analyze_chord(chord: &Chord, key: Key) -> Result<Option<RomanNumeral>> {
    RomanNumeral::analyze(chord, key)
}

/// Performs Roman-numeral analysis in a key using an explicit harmonic root.
pub fn analyze_chord_with_root(
    chord: &Chord,
    key: Key,
    root: &Pitch,
) -> Result<Option<RomanNumeral>> {
    RomanNumeral::analyze_with_root(chord, key, root)
}

pub fn split_roman_accidental_prefix(value: &str) -> (i8, &str) {
    let mut accidental = 0;
    let mut end = 0;
    for (idx, ch) in value.char_indices() {
        match ch {
            '#' => {
                accidental += 1;
                end = idx + ch.len_utf8();
            }
            'b' | '-' => {
                accidental -= 1;
                end = idx + ch.len_utf8();
            }
            _ => break,
        }
    }
    (accidental, &value[end..])
}

/// The chords music21 writes by name, as the figures they stand for.
///
/// The Neapolitan is the flattened second degree, written `N` and — the way
/// it is nearly always used — `N6` in first inversion. The cadential
/// six-four is the tonic triad in second inversion, and takes the case of
/// the key it stands in.
fn named_figure(figure: &str, key: &Key) -> String {
    let tonic = if key.mode() == "minor" { "i" } else { "I" };
    match figure {
        "N" | "N6" => "bII6".to_string(),
        "N53" => "bII".to_string(),
        "Cad64" => format!("{tonic}64"),
        other => other.to_string(),
    }
}

/// music21's "immediate fixes" to a figure as written.
///
/// A diminished chord is written `o`, `0`, `º` or `°`, and a half-diminished
/// one `ø` or `/o`; each pair is folded onto one spelling, and the fold is
/// what the numeral reports back as its figure. The zero is left alone when a
/// digit precedes it, so `10` stays a ten.
fn fold_figure_symbols(figure: &str) -> String {
    let mut folded = String::with_capacity(figure.len());
    let mut previous: Option<char> = None;
    for letter in figure.chars() {
        let fixed = match letter {
            '0' if !previous.is_some_and(|before| before.is_ascii_digit()) => 'o',
            '\u{00ba}' | '\u{00b0}' => 'o',
            other => other,
        };
        folded.push(fixed);
        previous = Some(letter);
    }
    folded.replace("/o", "\u{00f8}")
}

/// music21's figure validation: letters, digits and the handful of symbols a
/// figure is written with, and never the letters no numeral contains.
///
/// The parentheses and spaces of this crate's own `add(...)` and `omit(...)`
/// groups come through as well, since a figure written that way is one it
/// can read.
fn validate_figure(figure: &str) -> Result<()> {
    let ok = figure.chars().all(|letter| {
        letter.is_alphanumeric()
            || matches!(
                letter,
                '#' | '\u{00b0}' | '+' | '-' | '/' | '[' | ']' | '(' | ')' | ' '
            )
    });
    if !ok
        || figure
            .chars()
            .any(|letter| matches!(letter, 'x' | 'y' | 'z'))
    {
        return Err(Error::Chord(format!("Invalid figure: {figure}")));
    }
    Ok(())
}

/// One figure of a figured-bass column: how far above the bass the note
/// stands, and the accidental written beside the number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Figure {
    /// The interval above the bass, counted inclusively, so `6` is a sixth.
    number: u8,
    /// The accidental written with the number, in semitones. `None` is no
    /// modifier at all, which is not the same as a written natural: the note
    /// then keeps whatever the scale spells it with.
    alter: Option<i8>,
}

/// The figured-bass column a roman numeral's digits stand for, and what the
/// brackets beside them say.
///
/// This is music21's `figuredBass.notation.Notation` reduced to what a
/// numeral needs — the numbers above the bass and their accidentals, already
/// expanded out of the shorthand a figure is usually written in — together
/// with the omissions, additions and alterations music21 parses out of the
/// figure before the column is read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct FiguredBass {
    /// The figures of the column, written high to low as music21 writes them.
    figures: Vec<Figure>,
    /// Chord steps the figure asks to be left out, as `[no3]`.
    omitted: Vec<u8>,
    /// Notes the figure asks to be added, as `[add4]`: the alteration in
    /// semitones and how far above the *root* the note stands.
    added: Vec<(i8, u8)>,
    /// Alterations written in square brackets, as `[#7]`: the alteration in
    /// semitones and the chord step it applies to.
    bracketed: Vec<(i8, u8)>,
}

impl FiguredBass {
    /// The numbers of the column, high to low.
    fn numbers(&self) -> Vec<u8> {
        self.figures.iter().map(|figure| figure.number).collect()
    }

    /// Whether an accidental was written beside a given number, which is what
    /// stops the implied quality from correcting the note back.
    fn alters(&self, number: u8) -> bool {
        self.figures
            .iter()
            .any(|figure| figure.number == number && figure.alter.is_some_and(|alter| alter != 0))
    }
}

/// The chords a figured-bass column implies a root for.
///
/// music21's `FIGURES_IMPLYING_ROOT`. Every other column — `54`, say — is
/// read as a stack over its bass, and the bass is then the root.
const FIGURES_IMPLYING_ROOT: &[&[u8]] = &[
    // triads
    &[6],
    &[6, 3],
    &[6, 4],
    // seventh chords
    &[6, 5, 3],
    &[6, 5],
    &[6, 4, 3],
    &[4, 3],
    &[6, 4, 2],
    &[4, 2],
    &[2],
    // ninth chords
    &[7, 6, 5, 3],
    &[6, 5, 4, 3],
    &[6, 4, 3, 2],
    &[7, 5, 3, 2],
    // eleventh chords
    &[9, 7, 6, 5, 3],
    &[7, 6, 5, 4, 3],
    &[9, 6, 5, 4, 3],
    &[9, 7, 6, 4, 3],
    &[7, 6, 5, 4, 2],
];

/// The shorthand a figured-bass column is written in, and what it stands for.
///
/// music21's `figuredBass.notation.shorthandNotation`. A bare `7` means a
/// seventh chord in root position — the fifth and the third are there whether
/// or not anybody wrote them.
const SHORTHAND: &[(&[u8], &[u8])] = &[
    (&[], &[5, 3]),
    (&[5], &[5, 3]),
    (&[6], &[6, 3]),
    (&[7], &[7, 5, 3]),
    (&[9], &[9, 7, 5, 3]),
    (&[11], &[11, 9, 7, 5, 3]),
    (&[13], &[13, 11, 9, 7, 5, 3]),
    (&[6, 5], &[6, 5, 3]),
    (&[4, 3], &[6, 4, 3]),
    (&[4, 2], &[6, 4, 2]),
    (&[2], &[6, 4, 2]),
];

/// The quality a figure says its chord has, whatever the scale spells.
///
/// music21's `impliedQuality`: the symbol in front of the inversion digits,
/// or — when there is none — the case the numeral was written in. What it is
/// for is respelling the third, the fifth and the seventh once they have been
/// read off the scale.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ImpliedQuality {
    /// No quality was implied, and the scale's own spelling stands.
    #[default]
    Unstated,
    Major,
    Minor,
    Diminished,
    HalfDiminished,
    Augmented,
    DominantSeventh,
}

impl From<ImpliedQuality> for RomanQuality {
    /// The quality a numeral reports, which is the stated one with the two
    /// readings that only say how a seventh is spelled folded onto major.
    fn from(quality: ImpliedQuality) -> Self {
        match quality {
            ImpliedQuality::Minor => Self::Minor,
            ImpliedQuality::Diminished => Self::Diminished,
            ImpliedQuality::HalfDiminished => Self::HalfDiminished,
            ImpliedQuality::Augmented => Self::Augmented,
            ImpliedQuality::Unstated | ImpliedQuality::Major | ImpliedQuality::DominantSeventh => {
                Self::Major
            }
        }
    }
}

impl ImpliedQuality {
    /// The quality music21 names in a string, as it writes the names.
    pub fn from_name(name: &str) -> Self {
        match name {
            "major" => Self::Major,
            "minor" => Self::Minor,
            "diminished" => Self::Diminished,
            "half-diminished" => Self::HalfDiminished,
            "augmented" => Self::Augmented,
            "minor-seventh" | "dominant-seventh" => Self::DominantSeventh,
            _ => Self::Unstated,
        }
    }

    /// The name music21 writes it under, which is the empty string for a
    /// quality nobody stated.
    pub fn name(self) -> &'static str {
        match self {
            Self::Unstated => "",
            Self::Major => "major",
            Self::Minor => "minor",
            Self::Diminished => "diminished",
            Self::HalfDiminished => "half-diminished",
            Self::Augmented => "augmented",
            Self::DominantSeventh => "dominant-seventh",
        }
    }

    /// How many semitones the third, the fifth and — where the quality says
    /// so — the seventh stand above the root.
    pub fn correct_semitones(self) -> &'static [u8] {
        match self {
            Self::Unstated => &[],
            Self::Major => &[4, 7],
            Self::Minor => &[3, 7],
            Self::Diminished => &[3, 6, 9],
            Self::HalfDiminished => &[3, 6, 10],
            Self::Augmented => &[4, 8],
            Self::DominantSeventh => &[4, 7, 10],
        }
    }
}

/// Where music21 splits a secondary numeral off a figure.
///
/// Its own regular expression asks for a letter after the slash, and
/// deliberately not `o` — so `vii/o7` is a half-diminished seventh and not a
/// numeral applied to some chord `o7` — and not a digit, so the `6/5` of
/// `Ger6/5` stays one figure.
pub fn split_secondary(figure: &str) -> (&str, Option<String>) {
    for (index, letter) in figure.char_indices() {
        if letter != '/' {
            continue;
        }
        let rest = &figure[index + 1..];
        let opens = rest.chars().next().is_some_and(|next| {
            next == '#' || (next.is_ascii_alphabetic() && next != 'o' && next != 'O')
        });
        if opens {
            return (&figure[..index], Some(rest.to_string()));
        }
    }
    (figure, None)
}

/// Takes the `[noN]` groups out of a figure, leaving the rest of it.
///
/// The steps are counted from the root and folded into one octave, so
/// `[no11]` leaves out the fourth.
pub fn take_omitted_steps(figure: &mut String) -> Vec<u8> {
    let mut steps = Vec::new();
    let mut kept = String::with_capacity(figure.len());
    let mut remaining = figure.as_str();
    while let Some(start) = remaining.find("[no") {
        let Some(end) = remaining[start..].find(']') else {
            break;
        };
        let group = &remaining[start + 1..start + end];
        for part in group.split("no") {
            if let Ok(step) = part.trim().parse::<u8>() {
                steps.push(if step % 7 == 0 { 7 } else { step % 7 });
            }
        }
        kept.push_str(&remaining[..start]);
        remaining = remaining[start + end + 1..].trim_start();
    }
    kept.push_str(remaining);
    *figure = kept;
    steps
}

/// Takes the `[addN]` groups out of a figure, with the accidental each was
/// written with.
pub fn take_added_steps(figure: &mut String) -> Vec<(i8, u8)> {
    take_bracket_groups(figure, "[add")
}

/// Takes the `[#N]` and `[bN]` groups out of a figure.
pub fn take_bracketed_alterations(figure: &mut String) -> Vec<(i8, u8)> {
    take_bracket_groups(figure, "[")
}

/// The shape the two share: a bracket, a prefix, some accidentals, a number,
/// and a closing bracket. A bracket holding anything else is left where it is.
fn take_bracket_groups(figure: &mut String, opening: &str) -> Vec<(i8, u8)> {
    let mut groups = Vec::new();
    let mut kept = String::with_capacity(figure.len());
    let mut remaining = figure.as_str();
    while let Some(start) = remaining.find(opening) {
        let Some(end) = remaining[start..].find(']') else {
            break;
        };
        let body = &remaining[start + opening.len()..start + end];
        let digits: String = body.chars().filter(char::is_ascii_digit).collect();
        let well_formed = !digits.is_empty()
            && body
                .chars()
                .all(|letter| matches!(letter, '#' | 'b' | '-') || letter.is_ascii_digit());
        if !well_formed {
            kept.push_str(&remaining[..start + end + 1]);
            remaining = &remaining[start + end + 1..];
            continue;
        }
        let alter: i8 = body
            .chars()
            .map(|letter| match letter {
                '#' => 1,
                'b' | '-' => -1,
                _ => 0,
            })
            .sum();
        if let Ok(step) = digits.parse::<u8>() {
            groups.push((alter, step));
        }
        kept.push_str(&remaining[..start]);
        remaining = remaining[start + end + 1..].trim_start();
    }
    kept.push_str(remaining);
    *figure = kept;
    groups
}

/// music21's `expandShortHand`: the figures of a column, one string each.
pub fn expand_shorthand(shorthand: &str) -> Vec<String> {
    let mut shorthand = shorthand.replace('/', "");
    // A lone flat is a flattened third.
    if shorthand == "b" || shorthand == "-" {
        shorthand.push('3');
    }
    let letters: Vec<char> = shorthand.chars().collect();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < letters.len() {
        let start = index;
        for mark in ['#', '-', 'b', 'o'] {
            while letters.get(index) == Some(&mark) {
                index += 1;
            }
        }
        // The two-digit figures are read whole, so `13` is a thirteenth and
        // not a one and a three.
        let read = match letters.get(index) {
            Some('1') if matches!(letters.get(index + 1), Some('1' | '3' | '5')) => {
                index += 2;
                true
            }
            Some(digit) if digit.is_ascii_digit() && *digit != '0' => {
                index += 1;
                true
            }
            _ => false,
        };
        if read {
            tokens.push(letters[start..index].iter().collect::<String>());
        } else {
            index = start + 1;
        }
    }
    // A column written as a third alone is a fifth and a third.
    if tokens.len() == 1 && tokens[0].ends_with('3') {
        tokens.insert(0, "5".to_string());
    }
    tokens
}

/// Reads one figured-bass column out of the figures a numeral was written
/// with, expanding music21's shorthand.
fn figured_bass_column(tokens: &[String]) -> FiguredBass {
    let mut numbers: Vec<Option<u8>> = Vec::with_capacity(tokens.len());
    let mut alters: Vec<Option<i8>> = Vec::with_capacity(tokens.len());
    for token in tokens {
        let digits: String = token.chars().filter(char::is_ascii_digit).collect();
        numbers.push(digits.parse::<u8>().ok());
        let marks: String = token
            .chars()
            .filter(|letter| !letter.is_ascii_digit())
            .collect();
        alters.push(modifier_alter(&marks));
    }

    // A figure with a modifier but no number is a third.
    let written: Vec<u8> = numbers.iter().map(|number| number.unwrap_or(3)).collect();
    let shorthand: Vec<u8> = numbers.iter().flatten().copied().collect();
    let longhand = (shorthand.len() == numbers.len())
        .then(|| {
            SHORTHAND
                .iter()
                .find(|(written, _)| *written == shorthand.as_slice())
                .map(|(_, longhand)| longhand.to_vec())
        })
        .flatten();

    let figures = match longhand {
        Some(longhand) => longhand
            .into_iter()
            .map(|number| Figure {
                number,
                alter: written
                    .iter()
                    .position(|shorthand| *shorthand == number)
                    .and_then(|index| alters[index]),
            })
            .collect(),
        None => written
            .iter()
            .zip(&alters)
            .map(|(number, alter)| Figure {
                number: *number,
                alter: *alter,
            })
            .collect(),
    };

    FiguredBass {
        figures,
        ..FiguredBass::default()
    }
}

/// The accidental a figure's modifier leaves on the note the scale spells.
///
/// A written natural replaces whatever was there; anything else is added to
/// it, so the sharp of `i#7` raises the flattened seventh of a minor key to
/// a natural rather than spelling a `B#`.
fn modified_alter(pitch: &Pitch, alter: i8) -> FloatType {
    if alter == 0 {
        return 0.0;
    }
    pitch.accidental().alter() + FloatType::from(alter)
}

/// What a figure's modifier does to the note the scale spells.
///
/// music21 reads `+` as a sharp and `/` as a flat, which is how figured bass
/// has always written a raised or lowered note; `n` is a natural, and no
/// modifier at all is no instruction.
fn modifier_alter(marks: &str) -> Option<i8> {
    if marks.is_empty() {
        return None;
    }
    let mut alter = 0;
    for mark in marks.chars() {
        match mark {
            '#' | '+' => alter += 1,
            'b' | '-' | '/' => alter -= 1,
            'n' => {}
            _ => return None,
        }
    }
    Some(alter)
}

/// The key a secondary numeral establishes.
///
/// music21 reads the numeral after the slash as a chord of its own and takes
/// the key from that chord's root and quality, so the `vi` of `V/vi` in C
/// minor is the raised sixth degree the same way a plain `vi` would be.
pub fn secondary_key(
    key: &Key,
    secondary: &str,
    sixth_minor: Minor67Default,
    seventh_minor: Minor67Default,
    case_matters: bool,
) -> Result<Key> {
    let numeral = RomanNumeral::with_options(
        secondary.to_string(),
        key.clone(),
        sixth_minor,
        seventh_minor,
        case_matters,
    )?;
    let chord = numeral.to_chord()?;
    let root = chord
        .root()
        .ok_or_else(|| Error::Chord(format!("no root for secondary numeral {secondary}")))?;
    let mode = match numeral.implied_quality {
        ImpliedQuality::Minor => "minor",
        ImpliedQuality::Major => "major",
        _ if chord.semitones_from_chord_step(3) == Some(3) => "minor",
        _ => "major",
    };
    Key::from_tonic_mode(&root.name(), mode)
}

/// The numeral a figure opens with, and everything it implies.
///
/// This is what music21's `_parseRNAloneAmidstAug6` reads: usually just the
/// roman letters and the scale degree they name, but an augmented sixth is
/// written by nationality rather than by numeral and carries its own degree,
/// its own alteration and the accidentals that make it augmented — and it is
/// always read in the minor of the key it is written in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NumeralAlone {
    /// The numeral as written, or the augmented sixth's name.
    pub numeral: String,
    /// What is left of the figure once the numeral is off it.
    pub rest: String,
    /// The scale degree the numeral stands on.
    pub degree: u8,
    /// The alteration in front of it, in semitones, where the numeral itself
    /// implies one.
    pub alteration: i8,
    /// Whether the figure has to be read in the parallel minor, as every
    /// augmented sixth is.
    pub minor: bool,
    /// The alterations that make an augmented sixth augmented.
    pub bracketed: Vec<(i8, u8)>,
}

/// Reads the numeral off the front of a figure.
///
/// music21's `_parseRNAloneAmidstAug6`, which is where the augmented sixths
/// stop being numerals: `Ger` alone means `Ger65`, since that is the position
/// the chord is nearly always written in, and `Fr6` means `Fr43` for the same
/// reason.
pub fn parse_numeral_alone(figure: &str) -> Result<NumeralAlone> {
    if let Some(kind) = augmented_sixth_prefix(figure) {
        let (name, default) = match kind {
            AugmentedSixthKind::Italian => ("It", "6"),
            AugmentedSixthKind::French => ("Fr", "43"),
            AugmentedSixthKind::German => ("Ger", "65"),
            AugmentedSixthKind::Swiss => ("Sw", "43"),
        };
        let (degree, alteration) = match kind {
            AugmentedSixthKind::Italian | AugmentedSixthKind::German => (4, 1),
            AugmentedSixthKind::French => (2, 0),
            AugmentedSixthKind::Swiss => (2, 1),
        };
        let rest = figure[name.len()..].trim_start_matches('+');
        // A figure written `6/5` is the same as `65`.
        let rest = unslash_inversion(rest);
        let first = rest.chars().next();
        let rest = if !first.is_some_and(|digit| digit.is_ascii_digit()) {
            format!("{default}{rest}")
        } else if first == Some('6')
            && kind != AugmentedSixthKind::Italian
            && !rest
                .chars()
                .nth(1)
                .is_some_and(|next| next.is_ascii_digit())
        {
            format!("{default}{}", &rest[1..])
        } else {
            rest
        };
        let mut bracketed = Vec::new();
        if kind != AugmentedSixthKind::French {
            bracketed.push((1, 1));
        }
        if matches!(kind, AugmentedSixthKind::French | AugmentedSixthKind::Swiss) {
            bracketed.push((1, 3));
        }
        return Ok(NumeralAlone {
            numeral: name.to_string(),
            rest,
            degree,
            alteration,
            minor: true,
            bracketed,
        });
    }

    let (numeral, rest) = split_roman_prefix(figure)?;
    Ok(NumeralAlone {
        numeral: numeral.to_string(),
        rest: rest.to_string(),
        degree: roman_degree(numeral)?,
        alteration: 0,
        minor: false,
        bracketed: Vec::new(),
    })
}

/// The augmented sixth a figure opens with, if it opens with one.
fn augmented_sixth_prefix(figure: &str) -> Option<AugmentedSixthKind> {
    for (name, kind) in [
        ("It", AugmentedSixthKind::Italian),
        ("Ger", AugmentedSixthKind::German),
        ("Fr", AugmentedSixthKind::French),
        ("Sw", AugmentedSixthKind::Swiss),
    ] {
        if figure.starts_with(name) {
            return Some(kind);
        }
    }
    None
}

/// `6/5` written as `65`, which is the same figure with a slash in it.
fn unslash_inversion(figure: &str) -> String {
    let letters: Vec<char> = figure.chars().collect();
    let mut out = String::with_capacity(figure.len());
    let mut index = 0;
    while index < letters.len() {
        if letters[index] == '/'
            && index > 0
            && letters[index - 1].is_ascii_digit()
            && letters.get(index + 1).is_some_and(char::is_ascii_digit)
        {
            index += 1;
            continue;
        }
        out.push(letters[index]);
        index += 1;
    }
    out
}

/// Which scale degree a figured-bass column puts in the bass, for a chord
/// whose root stands on `degree`.
///
/// This is music21's `bassScaleDegreeFromNotation`. It works the answer out
/// rather than looking it up: a chord of naturals spaced by the column's own
/// numbers is spelled, its root found, and the bass is that many steps below
/// the root. A column that implies no root at all — `54`, say — leaves the
/// root in the bass, which is what the degree already says.
pub fn bass_scale_degree_from_notation(degree: u8, numbers: &[u8]) -> Result<u8> {
    bass_scale_degree_from_notation_in(degree, numbers, 7)
}

/// The same in a collection of a given size, which for anything but a key is
/// not seven.
fn bass_scale_degree_from_notation_in(degree: u8, numbers: &[u8], cardinality: u8) -> Result<u8> {
    if !FIGURES_IMPLYING_ROOT.contains(&numbers) {
        return Ok(degree);
    }
    let middle_c = 22;
    let mut pitches = vec![natural_at_diatonic_number(middle_c)?];
    for number in numbers {
        pitches.push(natural_at_diatonic_number(
            middle_c + IntegerType::from(*number) - 1,
        )?);
    }
    let spelled = Chord::new(pitches.as_slice())?;
    let root = spelled
        .root()
        .ok_or_else(|| Error::Chord("figured bass column has no root".to_string()))?;
    let distance = root.diatonic_note_number() - middle_c;
    let count = IntegerType::from(cardinality);
    let bass = (IntegerType::from(degree) - distance).rem_euclid(count);
    Ok(if bass == 0 { cardinality } else { bass as u8 })
}

/// Respells the third, fifth and seventh of a chord to the quality asked for.
///
/// This is music21's `_matchAccidentalsToQuality`. The letters come from
/// wherever the notes came from — a scale, usually — and the quality decides
/// only the accidentals, so a minor reading of `C E G` gives `C E- G` and
/// keeps the letters it was handed. Chord steps listed in `written` are left
/// alone, which is how an accidental somebody wrote survives the correction.
pub fn match_pitches_to_quality(
    pitches: &mut [Pitch],
    root: Option<&Pitch>,
    quality: ImpliedQuality,
    written: &[u8],
) -> Result<()> {
    let correct = quality.correct_semitones();
    for (step, want) in [3u8, 5, 7].into_iter().zip(correct.iter().copied()) {
        if written.contains(&step) {
            continue;
        }
        let Some(index) = chord_step_index(pitches, root, step)? else {
            continue;
        };
        let have = step_semitones(pitches, root, index)?;
        if have == IntegerType::from(want) {
            continue;
        }
        correct_faulty_pitch(&mut pitches[index], IntegerType::from(want) - have)?;
    }

    // A seventh does not have to match the scale: an `i7` read in a major key
    // would otherwise take the major seventh the scale spells.
    if correct.len() == 2
        && quality == ImpliedQuality::Minor
        && !written.contains(&7)
        && let Some(index) = chord_step_index(pitches, root, 7)?
        && step_semitones(pitches, root, index)? == 11
    {
        correct_faulty_pitch(&mut pitches[index], -1)?;
    }
    Ok(())
}

/// What a roman numeral counts its degrees against.
///
/// Usually a key, which has seven of them; but music21 reads a numeral over
/// any concrete scale, and an octatonic one has eight.
enum Reading {
    Key(Key),
    Scale(crate::scale::Scale),
}

impl Reading {
    /// How many degrees there are before the collection repeats.
    fn cardinality(&self) -> u8 {
        match self {
            Self::Key(_) => 7,
            Self::Scale(scale) => scale.degree_count() as u8,
        }
    }

    /// The pitch a degree spells, folded into the octave the collection's
    /// tonic stands in.
    fn pitch_at(&self, degree: IntegerType) -> Result<Pitch> {
        match self {
            Self::Key(key) => degree_pitch(key, degree),
            Self::Scale(scale) => {
                let count = IntegerType::from(self.cardinality());
                let wrapped = (degree - 1).rem_euclid(count) + 1;
                scale.pitch_at_degree(wrapped as usize)
            }
        }
    }
}

/// The pitch a scale degree spells, folded into the octave the scale's tonic
/// stands in — which is what music21's `pitchFromDegree` does, so a ninth
/// comes back as the second and the caller lifts it.
fn degree_pitch(key: &Key, degree: IntegerType) -> Result<Pitch> {
    let wrapped = (degree - 1).rem_euclid(7) + 1;
    key.pitch_from_degree(wrapped as usize)
}

/// The natural note at a diatonic note number, where 22 is middle C.
fn natural_at_diatonic_number(number: IntegerType) -> Result<Pitch> {
    const LETTERS: [char; 7] = ['C', 'D', 'E', 'F', 'G', 'A', 'B'];
    let letter = LETTERS[((number - 1).rem_euclid(7)) as usize];
    Pitch::builder()
        .step(letter)
        .octave((number - 1).div_euclid(7))
        .build()
}

/// Which of a chord's notes are the seventh and the extensions above it.
///
/// music21 leaves these alone when it moves a chord by the alteration in
/// front of its numeral, since the alteration is written against the root.
fn upper_extension_indices(pitches: &[Pitch]) -> Result<Vec<usize>> {
    let chord = Chord::new(pitches)?;
    let Some(root) = chord.root().cloned() else {
        return Ok(Vec::new());
    };
    let mut indices = Vec::new();
    for step in [7u8, 2, 4, 6] {
        if let Some(index) = chord_step_index(pitches, Some(&root), step)? {
            indices.push(index);
        }
    }
    Ok(indices)
}

/// Where a chord step stands among a set of pitches, counting from the root
/// the chord infers when none was recorded.
fn chord_step_index(pitches: &[Pitch], root: Option<&Pitch>, step: u8) -> Result<Option<usize>> {
    let inferred;
    let root = match root {
        Some(root) => root,
        None => {
            let chord = Chord::new(pitches)?;
            let Some(found) = chord.root().cloned() else {
                return Ok(None);
            };
            inferred = found;
            &inferred
        }
    };
    let wanted = IntegerType::from(step);
    Ok(pitches.iter().position(|pitch| {
        (pitch.diatonic_note_number() - root.diatonic_note_number()).rem_euclid(7) + 1 == wanted
    }))
}

/// How many semitones a pitch stands above the root, within the octave.
fn step_semitones(pitches: &[Pitch], root: Option<&Pitch>, index: usize) -> Result<IntegerType> {
    let inferred;
    let root = match root {
        Some(root) => root,
        None => {
            let chord = Chord::new(pitches)?;
            let Some(found) = chord.root().cloned() else {
                return Ok(0);
            };
            inferred = found;
            &inferred
        }
    };
    let distance = (pitches[index].ps() - root.ps()).round() as IntegerType;
    Ok(distance.rem_euclid(12))
}

/// music21's `correctFaultyPitch`: moves a note by the semitones it is out
/// by, reading a correction of half an octave or more the short way round.
fn correct_faulty_pitch(pitch: &mut Pitch, correction: IntegerType) -> Result<()> {
    let folded = fold_correction(correction) + pitch.accidental().alter() as IntegerType;
    let alter = fold_correction(folded);
    pitch.set_accidental(Some(Accidental::new(FloatType::from(alter))?));
    Ok(())
}

/// Half an octave or more in either direction is the same note the other way.
fn fold_correction(semitones: IntegerType) -> IntegerType {
    if semitones >= 6 {
        semitones - 12
    } else if semitones <= -6 {
        semitones + 12
    } else {
        semitones
    }
}

/// music21's `_setImpliedQualityFromString`: the quality symbol in front of
/// the inversion digits, and the digits left after it.
///
/// The crate's own `dim`, `aug` and `m7b5` spellings are read here too, since
/// a figure written that way is one it has always accepted.
fn implied_quality_from_string(
    roman: &str,
    suffix: &str,
    case_matters: bool,
) -> (ImpliedQuality, String) {
    if let Some(rest) = suffix.strip_prefix('o') {
        return (ImpliedQuality::Diminished, rest.to_string());
    }
    if let Some(rest) = suffix.strip_prefix('\u{00f8}') {
        return (ImpliedQuality::HalfDiminished, rest.to_string());
    }
    if let Some(rest) = suffix.strip_prefix('+') {
        return (ImpliedQuality::Augmented, rest.to_string());
    }
    let lower = suffix.to_ascii_lowercase();
    if lower.contains("m7b5") {
        return (ImpliedQuality::HalfDiminished, suffix.to_string());
    }
    if lower.contains("dim") {
        return (ImpliedQuality::Diminished, suffix.to_string());
    }
    if lower.contains("aug") {
        return (ImpliedQuality::Augmented, suffix.to_string());
    }
    // A `d` before the inversion figure is music21's dominant seventh: `Vd7`
    // and `IVd65` are dominant sevenths whatever the scale spells.
    if let Some((leading, figure)) = suffix.rsplit_once('d')
        && matches!(
            figure,
            "7" | "65" | "6/5" | "43" | "4/3" | "42" | "4/2" | "2"
        )
    {
        return (
            ImpliedQuality::DominantSeventh,
            format!("{leading}{figure}"),
        );
    }
    if !case_matters {
        return (ImpliedQuality::Unstated, suffix.to_string());
    }
    let quality = if roman.chars().next().is_some_and(char::is_uppercase) {
        ImpliedQuality::Major
    } else {
        ImpliedQuality::Minor
    };
    (quality, suffix.to_string())
}

/// music21's `sharpen`: raises the alteration in front of a numeral, taking
/// a sharp already written on the root out of the figure so the note is not
/// raised twice.
fn sharpen_figure(figure: &mut String) {
    if figure.contains("##") {
        *figure = figure.replace("##8", "#8");
    } else if figure.contains("#2") {
        *figure = figure.replace("#2", "2");
    } else if figure.contains("#4") {
        *figure = figure.replace("#4", "4");
    } else if figure.contains("#6") {
        *figure = figure.replace("#6", "6");
    } else {
        *figure = figure.replace("#8", "");
    }
}

pub fn split_roman_prefix(value: &str) -> Result<(&str, &str)> {
    let end = value
        .char_indices()
        .find_map(|(idx, ch)| (!matches!(ch, 'I' | 'V' | 'X' | 'i' | 'v' | 'x')).then_some(idx))
        .unwrap_or(value.len());

    if end == 0 {
        return Err(Error::Chord(format!("No roman numeral found in '{value}'")));
    }

    Ok((&value[..end], &value[end..]))
}

fn roman_degree(roman: &str) -> Result<u8> {
    match roman.to_ascii_uppercase().as_str() {
        "I" => Ok(1),
        "II" => Ok(2),
        "III" => Ok(3),
        "IV" => Ok(4),
        "V" => Ok(5),
        "VI" => Ok(6),
        "VII" => Ok(7),
        _ => Err(Error::Chord(format!("unsupported roman numeral {roman:?}"))),
    }
}

fn suffix_has_seventh(suffix: &str) -> bool {
    let suffix = strip_roman_addition_groups(suffix);
    suffix.contains('7')
        || suffix.contains('9')
        || suffix.contains("11")
        || suffix.contains("13")
        || suffix.contains("65")
        || suffix.contains("43")
        || suffix.contains("42")
}

/// music21's `roman.figureShorthands`, mapping a full figured-bass string to
/// the abbreviation musicians actually write.
///
/// This is the table the inversion logic used to approximate with
/// `suffix.contains("64")`-style probes, which read `642` as a second inversion
/// because `64` matched before `42` was ever tested. Normalizing through the
/// real table removes that ordering hazard rather than reshuffling the probes.
const FIGURE_SHORTHANDS: [(&str, &str); 20] = [
    ("53", ""),
    ("3", ""),
    ("63", "6"),
    ("753", "7"),
    ("75", "7"),
    ("73", "7"),
    ("9753", "9"),
    ("975", "9"),
    ("953", "9"),
    ("97", "9"),
    ("95", "9"),
    ("93", "9"),
    ("653", "65"),
    ("6b53", "6b5"),
    ("643", "43"),
    ("642", "42"),
    ("bb7b5b3", "o7"),
    ("b7b5b3", "\u{00f8}7"),
    ("bb7b53", "o7"),
    ("b7b53", "\u{00f8}7"),
];

/// Returns the shorthand for a figure, or the figure itself when it has none.
fn normalize_figure(figure: &str) -> &str {
    FIGURE_SHORTHANDS
        .iter()
        .find(|(full, _)| *full == figure)
        .map_or(figure, |(_, short)| *short)
}

fn parse_inversion(suffix: &str) -> u8 {
    let suffix = strip_roman_addition_groups(suffix);
    // Only the figured-bass digits decide the inversion; quality marks such as
    // `o`, `+` and the half-diminished sign ride along in the suffix.
    let digits: String = suffix.chars().filter(char::is_ascii_digit).collect();

    match normalize_figure(&digits) {
        "42" => 3,
        "43" | "64" => 2,
        "65" | "6" => 1,
        _ => 0,
    }
}

fn strip_roman_addition_groups(suffix: &str) -> String {
    let mut stripped = String::with_capacity(suffix.len());
    let mut rest = suffix;
    while let Some(index) = rest.find("add(") {
        stripped.push_str(&rest[..index]);
        let addition = &rest[index + 4..];
        let Some(end) = addition.find(')') else {
            rest = addition;
            continue;
        };
        rest = &addition[end + 1..];
    }
    stripped.push_str(rest);
    stripped
}

fn degree_for_root(key: &Key, root: &Pitch) -> Result<Option<(u8, i8)>> {
    let root_pc = pitch_class(root);
    let root_step = root.step();
    let mut best: Option<(u8, i8, bool)> = None;

    for degree in 1..=7 {
        let degree_pitch = key.pitch_from_degree(degree)?;
        let diff = ((root_pc as i16 - pitch_class(&degree_pitch) as i16).rem_euclid(12)) as u8;
        let Some(accidental) = chromatic_diff_to_accidental(diff) else {
            continue;
        };
        let same_step = degree_pitch.step() == root_step;

        let replace = match best {
            None => true,
            Some((_, best_accidental, best_same_step)) => {
                (same_step && !best_same_step)
                    || (same_step == best_same_step && accidental.abs() < best_accidental.abs())
            }
        };
        if replace {
            best = Some((degree as u8, accidental, same_step));
        }
    }

    Ok(best.map(|(degree, accidental, _)| (degree, accidental)))
}

fn chromatic_diff_to_accidental(diff: u8) -> Option<i8> {
    match diff {
        0 => Some(0),
        1 => Some(1),
        2 => Some(2),
        10 => Some(-2),
        11 => Some(-1),
        _ => None,
    }
}

fn intervals_above_root(chord: &Chord, root_pc: u8) -> Vec<u8> {
    let mut intervals = chord
        .pitch_classes()
        .into_iter()
        .map(|pc| (pc + 12 - root_pc) % 12)
        .collect::<Vec<_>>();
    intervals.sort_unstable();
    intervals.dedup();
    intervals
}

fn augmented_sixth_kind_for_key(chord: &Chord, key: &Key) -> Result<Option<AugmentedSixthKind>> {
    let kind = std::iter::once(chord.common_name())
        .chain(chord.common_names())
        .find_map(|name| AugmentedSixthKind::from_common_name(&name));
    let Some(kind) = kind else {
        return Ok(None);
    };

    let pitch_classes = chord.pitch_classes();
    let tonic = key_degree_pitch_class(key, 1, 0)?;
    let lowered_sixth_adjust = if key.mode() == "minor" { 0 } else { -1 };
    let lowered_sixth = key_degree_pitch_class(key, 6, lowered_sixth_adjust)?;
    let raised_fourth = key_degree_pitch_class(key, 4, 1)?;

    if !pitch_classes.contains(&tonic)
        || !pitch_classes.contains(&lowered_sixth)
        || !pitch_classes.contains(&raised_fourth)
    {
        return Ok(None);
    }

    let required_extra = match kind {
        AugmentedSixthKind::Italian => None,
        AugmentedSixthKind::French => Some(key_degree_pitch_class(key, 2, 0)?),
        AugmentedSixthKind::German => {
            let lowered_third_adjust = if key.mode() == "minor" { 0 } else { -1 };
            Some(key_degree_pitch_class(key, 3, lowered_third_adjust)?)
        }
        AugmentedSixthKind::Swiss => Some(key_degree_pitch_class(key, 2, 1)?),
    };

    if required_extra.is_some_and(|pitch_class| !pitch_classes.contains(&pitch_class)) {
        return Ok(None);
    }

    Ok(Some(kind))
}

fn key_degree_pitch_class(key: &Key, degree: usize, semitones: IntegerType) -> Result<u8> {
    let mut pitch = key.pitch_from_degree(degree)?;
    if semitones != 0 {
        pitch = Interval::from_semitones(semitones)?.transpose_pitch(&pitch)?;
    }
    Ok(pitch_class(&pitch))
}

fn roman_inversion(chord: &Chord) -> u8 {
    if chord.pitches().iter().any(|pitch| pitch.octave().is_some()) {
        chord.inversion().unwrap_or(0)
    } else {
        0
    }
}

fn symbol_quality(symbol: &ChordSymbol) -> RomanQuality {
    match symbol.quality() {
        ChordQuality::Major
        | ChordQuality::Dominant
        | ChordQuality::Suspended2
        | ChordQuality::Suspended4
        | ChordQuality::Power
        | ChordQuality::Pedal => RomanQuality::Major,
        ChordQuality::Minor => RomanQuality::Minor,
        ChordQuality::Diminished => RomanQuality::Diminished,
        ChordQuality::HalfDiminished => RomanQuality::HalfDiminished,
        ChordQuality::Augmented => RomanQuality::Augmented,
    }
}

fn quality_from_intervals(intervals: &[u8]) -> RomanQuality {
    if intervals.contains(&3) && intervals.contains(&6) {
        if intervals.contains(&10) {
            RomanQuality::HalfDiminished
        } else {
            RomanQuality::Diminished
        }
    } else if intervals.contains(&4) && intervals.contains(&8) {
        RomanQuality::Augmented
    } else if intervals.contains(&3) && intervals.contains(&7) {
        RomanQuality::Minor
    } else {
        RomanQuality::Major
    }
}

fn roman_figure(
    degree: u8,
    accidental: i8,
    quality: RomanQuality,
    symbol: Option<&ChordSymbol>,
    intervals: &[u8],
    inversion: u8,
) -> String {
    let base = degree_to_roman(degree);
    let prefix = roman_accidental_prefix(accidental);
    let body = roman_body_for_quality(base, quality);
    let suffix = functional_suffix(symbol, intervals, inversion, quality);
    format!("{prefix}{body}{suffix}")
}

fn roman_accidental_prefix(accidental: i8) -> String {
    match accidental.cmp(&0) {
        std::cmp::Ordering::Less => "b".repeat(accidental.unsigned_abs() as usize),
        std::cmp::Ordering::Equal => String::new(),
        std::cmp::Ordering::Greater => "#".repeat(accidental as usize),
    }
}

fn roman_body_for_quality(base: &str, quality: RomanQuality) -> String {
    match quality {
        RomanQuality::Major => base.to_string(),
        RomanQuality::Minor => base.to_ascii_lowercase(),
        RomanQuality::Diminished => format!("{}o", base.to_ascii_lowercase()),
        RomanQuality::HalfDiminished => format!("{}\u{00f8}", base.to_ascii_lowercase()),
        RomanQuality::Augmented => format!("{base}+"),
    }
}

fn functional_suffix(
    symbol: Option<&ChordSymbol>,
    intervals: &[u8],
    inversion: u8,
    quality: RomanQuality,
) -> String {
    if let Some(symbol) = symbol
        && needs_chord_symbol_suffix(symbol)
    {
        return chord_symbol_suffix_for_roman(symbol, quality);
    }
    figured_bass_suffix(intervals, inversion, quality)
}

fn needs_chord_symbol_suffix(symbol: &ChordSymbol) -> bool {
    matches!(
        symbol.quality(),
        ChordQuality::Suspended2 | ChordQuality::Suspended4 | ChordQuality::Power
    ) || !symbol.additions().is_empty()
        || symbol.alterations().iter().any(|alteration| {
            !(matches!(symbol.quality(), ChordQuality::HalfDiminished)
                && alteration.degree() == 5
                && alteration.semitones() == -1)
        })
        || symbol.extensions().iter().any(|degree| *degree != 7)
        || chord_symbol_suffix(symbol).contains("maj7")
}

fn chord_symbol_suffix_for_roman(symbol: &ChordSymbol, quality: RomanQuality) -> String {
    let suffix = chord_symbol_suffix(symbol);
    let converted = match quality {
        RomanQuality::Major => suffix.to_string(),
        RomanQuality::Minor => suffix
            .strip_prefix('m')
            .filter(|rest| !rest.starts_with("aj"))
            .unwrap_or(suffix)
            .to_string(),
        RomanQuality::Diminished => suffix.strip_prefix("dim").unwrap_or(suffix).to_string(),
        RomanQuality::HalfDiminished => {
            suffix.strip_prefix('m').unwrap_or(suffix).replace("b5", "")
        }
        RomanQuality::Augmented => suffix
            .strip_prefix("aug")
            .or_else(|| suffix.strip_prefix('+'))
            .unwrap_or(suffix)
            .to_string(),
    };

    if converted == "6" {
        " add(13)".to_string()
    } else {
        converted
    }
}

fn chord_symbol_suffix(symbol: &ChordSymbol) -> &str {
    let body = symbol
        .figure()
        .split_once('/')
        .map_or(symbol.figure(), |(body, _)| body);
    let root_name = normalize_symbol_root_name(&symbol.root().name());
    body.strip_prefix(&root_name).unwrap_or(body)
}

fn normalize_symbol_root_name(name: &str) -> String {
    name.replace('-', "b")
}

fn figured_bass_suffix(intervals: &[u8], inversion: u8, quality: RomanQuality) -> String {
    if has_seventh(intervals) {
        let suffix = match inversion {
            1 => "65",
            2 => "43",
            3 => "42",
            _ => "7",
        };
        if matches!(quality, RomanQuality::Major) && intervals.contains(&11) {
            format!("maj{suffix}")
        } else {
            suffix.to_string()
        }
    } else if has_triad_shape(intervals) {
        match inversion {
            1 => "6".to_string(),
            2 => "64".to_string(),
            _ => String::new(),
        }
    } else {
        String::new()
    }
}

fn has_seventh(intervals: &[u8]) -> bool {
    intervals.contains(&10) || intervals.contains(&11) || intervals.contains(&9)
}

fn has_triad_shape(intervals: &[u8]) -> bool {
    (intervals.contains(&3) || intervals.contains(&4))
        && intervals.iter().any(|interval| matches!(interval, 6..=8))
}

fn degree_to_roman(degree: u8) -> &'static str {
    match degree {
        1 => "I",
        2 => "II",
        3 => "III",
        4 => "IV",
        5 => "V",
        6 => "VI",
        7 => "VII",
        _ => "I",
    }
}

fn normalize_pitch_name(name: &str) -> String {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut normalized = first.to_string();
    for ch in chars {
        if ch == 'b' {
            normalized.push('-');
        } else {
            normalized.push(ch);
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_minor_key_raises_its_sixth_and_seventh_where_the_figure_asks() {
        let minor = Key::from_tonic_mode("a", Some("minor")).unwrap();
        // The diminished seventh on the leading note is the raised seventh
        // degree: G#, not G with three flats hung off it.
        let leading = RomanNumeral::new("viio7", minor.clone()).unwrap();
        assert_eq!(
            leading.to_chord().unwrap().pitch_names(),
            ["G#", "B", "D", "F"]
        );
        assert_eq!(leading.roman_numeral(), "#vii");
        assert_eq!(leading.roman_numeral_alone(), "vii");
        assert_eq!(leading.scale_degree_with_alteration(), (7, 1));

        // A minor triad on the sixth degree is the raised one too.
        assert_eq!(
            RomanNumeral::new("vi", minor.clone())
                .unwrap()
                .to_chord()
                .unwrap()
                .pitch_names(),
            ["F#", "A", "C#"]
        );
        // The natural degrees are what the upper-case figures ask for.
        assert_eq!(
            RomanNumeral::new("VI", minor.clone())
                .unwrap()
                .to_chord()
                .unwrap()
                .pitch_names(),
            ["F", "A", "C"]
        );
        // And a major key is left alone entirely.
        let major = Key::from_tonic_mode("C", Some("major")).unwrap();
        assert_eq!(
            RomanNumeral::new("vi", major)
                .unwrap()
                .to_chord()
                .unwrap()
                .pitch_names(),
            ["A", "C", "E"]
        );
    }

    #[test]
    fn the_four_readings_of_a_minor_sixth_and_seventh() {
        let minor = Key::from_tonic_mode("c", Some("minor")).unwrap();
        let read = |figure: &str, sixth: Minor67Default| {
            RomanNumeral::with_minor_defaults(figure, minor.clone(), sixth, sixth)
                .unwrap()
                .to_chord()
                .unwrap()
                .pitch_names()
                .join(" ")
        };

        // By quality, which is the default: the chord the figure asks for
        // says which sixth it is built on.
        assert_eq!(read("vi", Minor67Default::Quality), "A C E");
        assert_eq!(read("VI", Minor67Default::Quality), "A- C E-");

        // Flat is always the natural degree, whatever the figure asks for,
        // and a sharp written in front still raises it.
        assert_eq!(read("vi", Minor67Default::Flat), "A- C- E-");
        assert_eq!(read("#vi", Minor67Default::Flat), "A C E");

        // Sharp is always the raised one, and a flat still lowers it.
        assert_eq!(read("VI", Minor67Default::Sharp), "A C# E");
        assert_eq!(read("bVI", Minor67Default::Sharp), "A- C E-");

        // Cautionary reads the quality, but an accidental already written is
        // a caution rather than a further change, so `#vi` is `vi` and
        // `bVI` is `VI`.
        assert_eq!(read("#vi", Minor67Default::Cautionary), "A C E");
        assert_eq!(read("vi", Minor67Default::Cautionary), "A C E");
        assert_eq!(read("bVI", Minor67Default::Cautionary), "A- C E-");
        assert_eq!(read("VI", Minor67Default::Cautionary), "A- C E-");
    }

    #[test]
    fn the_neapolitan_can_be_written_by_name() {
        // music21 writes the flattened second degree as `N`, and in first
        // inversion — the way it is nearly always used — as `N6`.
        let key = Key::from_tonic_mode("c#", Some("minor")).unwrap();
        let named = RomanNumeral::new("N6", key.clone()).unwrap();
        assert_eq!(named.figure(), "N6");
        assert_eq!(named.degree(), 2);
        assert_eq!(named.accidental(), -1);
        assert_eq!(named.inversion(), 1);
        assert!(named.is_neapolitan(true));

        let spelled = RomanNumeral::new("bII6", key.clone()).unwrap();
        assert_eq!(
            named.to_chord().unwrap().pitch_names(),
            spelled.to_chord().unwrap().pitch_names()
        );

        // `N53` is the same chord in root position.
        let root_position = RomanNumeral::new("N53", key).unwrap();
        assert_eq!(root_position.inversion(), 0);
    }

    #[test]
    fn inversion_names_and_tonic_dominant_reading_match_music21() {
        let chord = |notes: &str| Chord::new(notes).unwrap();
        assert_eq!(roman_inversion_name(&chord("C4 E4 G4"), None), "");
        assert_eq!(roman_inversion_name(&chord("E4 G4 C5"), None), "6");
        assert_eq!(roman_inversion_name(&chord("G3 C4 E4"), None), "64");
        assert_eq!(roman_inversion_name(&chord("C4 E4 G4 B-4"), None), "7");
        assert_eq!(roman_inversion_name(&chord("E4 G4 B-4 C5"), None), "65");
        assert_eq!(roman_inversion_name(&chord("G3 B-3 C4 E4"), None), "43");
        assert_eq!(roman_inversion_name(&chord("B-3 C4 E4 G4"), None), "42");
        assert_eq!(roman_inversion_name(&chord("C4 E4 G4"), Some(2)), "64");
        assert_eq!(roman_inversion_name(&chord("C4 E4 G4"), Some(3)), "");
        assert_eq!(roman_inversion_name(&Chord::empty(), None), "");

        let cases = [
            ("C4 E4 G4", "C", Some("I")),
            ("E4 G4 C5", "C", Some("I6")),
            ("G3 B3 D4 F4", "C", Some("V7")),
            ("D4 F4 A4", "C", Some("V43")),
            ("A3 C4 E4", "a", Some("i")),
            ("E3 G#3 B3", "a", Some("V")),
            ("C4 E4", "C", Some("I")),
            ("G4 D5", "C", Some("V")),
            ("B3 D4", "C", Some("V")),
            ("F#4 A4", "C", None),
        ];
        for (notes, key, expected) in cases {
            let reading =
                identify_as_tonic_or_dominant(&chord(notes), &Key::from_tonic(key).unwrap())
                    .unwrap();
            assert_eq!(reading.as_deref(), expected, "{notes} in {key}");
        }
    }

    #[test]
    #[allow(clippy::type_complexity)]
    fn numeral_names_scores_and_mixture_match_music21() {
        let cases: [(&str, &str, &str, &str, (u8, i8), u8, bool, bool, bool); 18] = [
            (
                "V7",
                "C",
                "V",
                "V7 in C major",
                (5, 0),
                80,
                false,
                false,
                false,
            ),
            (
                "bII6",
                "C",
                "bII",
                "bII6 in C major",
                (2, -1),
                35,
                true,
                true,
                false,
            ),
            (
                "bII",
                "C",
                "bII",
                "bII in C major",
                (2, -1),
                0,
                false,
                true,
                false,
            ),
            (
                "bII6",
                "a",
                "bII",
                "bII6 in a minor",
                (2, -1),
                35,
                true,
                true,
                false,
            ),
            (
                "viio7",
                "C",
                "vii",
                "viio7 in C major",
                (7, 0),
                57,
                false,
                false,
                true,
            ),
            (
                "I",
                "C",
                "I",
                "I in C major",
                (1, 0),
                100,
                false,
                false,
                false,
            ),
            (
                "i",
                "a",
                "i",
                "i in a minor",
                (1, 0),
                90,
                false,
                false,
                false,
            ),
            (
                "V65/V",
                "C",
                "V",
                "V65/V in C major",
                (5, 0),
                47,
                false,
                false,
                false,
            ),
            (
                "iv",
                "C",
                "iv",
                "iv in C major",
                (4, 0),
                0,
                false,
                false,
                true,
            ),
            (
                "bVI",
                "C",
                "bVI",
                "bVI in C major",
                (6, -1),
                0,
                false,
                false,
                true,
            ),
            (
                "I",
                "a",
                "I",
                "I in a minor",
                (1, 0),
                100,
                false,
                false,
                true,
            ),
            (
                "III",
                "a",
                "III",
                "III in a minor",
                (3, 0),
                22,
                false,
                false,
                false,
            ),
            (
                "#iii",
                "a",
                "#iii",
                "#iii in a minor",
                (3, 1),
                0,
                false,
                false,
                true,
            ),
            (
                "ii",
                "C",
                "ii",
                "ii in C major",
                (2, 0),
                50,
                false,
                false,
                false,
            ),
            (
                "iio",
                "C",
                "ii",
                "iio in C major",
                (2, 0),
                37,
                false,
                false,
                true,
            ),
            (
                "IV",
                "a",
                "IV",
                "IV in a minor",
                (4, 0),
                59,
                false,
                false,
                true,
            ),
            (
                "bVII",
                "C",
                "bVII",
                "bVII in C major",
                (7, -1),
                0,
                false,
                false,
                true,
            ),
            (
                "v",
                "C",
                "v",
                "v in C major",
                (5, 0),
                20,
                false,
                false,
                true,
            ),
        ];
        for (figure, key, numeral, figure_and_key, degree, score, neapolitan, loose, mixture) in
            cases
        {
            let rn = RomanNumeral::new(figure, Key::from_tonic(key).unwrap()).unwrap();
            assert_eq!(rn.roman_numeral(), numeral, "{figure} in {key}");
            assert_eq!(rn.figure_and_key(), figure_and_key, "{figure} in {key}");
            assert_eq!(
                rn.scale_degree_with_alteration(),
                degree,
                "{figure} in {key}"
            );
            assert_eq!(rn.functionality_score(), score, "{figure} in {key}");
            assert_eq!(rn.is_neapolitan(true), neapolitan, "{figure} in {key}");
            assert_eq!(rn.is_neapolitan(false), loose, "{figure} in {key}");
            assert_eq!(rn.is_mixture(false).unwrap(), mixture, "{figure} in {key}");
            assert_eq!(rn.is_mixture(true).unwrap(), mixture, "{figure} in {key}");
        }
        let italian = RomanNumeral::new("It6", Key::from_tonic("C").unwrap()).unwrap();
        assert_eq!(italian.roman_numeral(), "It");
        assert_eq!(italian.functionality_score(), 34);
        assert!(!italian.is_mixture(false).unwrap());
        let transposed = RomanNumeral::new("bII6", Key::from_tonic("a").unwrap())
            .unwrap()
            .transpose(&Interval::from_name("M2").unwrap())
            .unwrap();
        assert_eq!(transposed.figure_and_key(), "bII6 in b minor");
    }

    #[test]
    fn functionality_scores_table_has_no_duplicates() {
        let mut figures: Vec<&str> = FUNCTIONALITY_SCORES.iter().map(|(f, _)| *f).collect();
        figures.sort_unstable();
        figures.dedup();
        assert_eq!(figures.len(), FUNCTIONALITY_SCORES.len());
    }
    use super::*;

    #[test]
    fn secondary_dominant_resolves_to_chord() {
        let key = Key::from_tonic_mode("C", "major").unwrap();
        let rn = RomanNumeral::new("V7/V", key).unwrap();
        assert_eq!(rn.degree(), 5);
        assert_eq!(rn.secondary(), Some("V"));
        assert_eq!(
            rn.to_chord().unwrap().pitched_common_name(),
            "D-dominant seventh chord"
        );
    }

    #[test]
    fn analyzes_chord_in_key() {
        let key = Key::from_tonic_mode("C", "major").unwrap();
        let chord = Chord::new("G B D F").unwrap();
        let rn = RomanNumeral::analyze(&chord, key).unwrap().unwrap();
        assert_eq!(rn.figure(), "V7");
    }

    #[test]
    fn analyzes_accidentals_inversions_and_half_diminished_quality() {
        let key = Key::from_tonic_mode("C", "major").unwrap();

        let neapolitan = Chord::new("D- F A-").unwrap();
        let rn = RomanNumeral::analyze(&neapolitan, key.clone())
            .unwrap()
            .unwrap();
        assert_eq!(rn.figure(), "bII");
        assert_eq!(rn.degree(), 2);
        assert_eq!(rn.accidental(), -1);

        let first_inversion = Chord::new("E4 G4 C5").unwrap();
        let rn = RomanNumeral::analyze(&first_inversion, key.clone())
            .unwrap()
            .unwrap();
        assert_eq!(rn.figure(), "I6");

        let leading_tone = Chord::new("B D F A").unwrap();
        let rn = RomanNumeral::analyze(&leading_tone, key).unwrap().unwrap();
        assert_eq!(rn.figure(), "vii\u{00f8}7");
    }

    #[test]
    fn analyzes_with_explicit_root_for_browser_style_sets() {
        let key = Key::from_tonic_mode("C", "major").unwrap();
        let root = Pitch::from_name("C").unwrap();
        let chord = Chord::new("C E G").unwrap();
        let rn = RomanNumeral::analyze_with_root(&chord, key.clone(), &root)
            .unwrap()
            .unwrap();
        assert_eq!(rn.figure(), "I");

        let seventh = Chord::new("C E G B-").unwrap();
        let rn = RomanNumeral::analyze_with_root(&seventh, key, &root)
            .unwrap()
            .unwrap();
        assert_eq!(rn.figure(), "I7");
    }

    #[test]
    fn analyzes_augmented_sixth_chords_functionally() {
        let key = Key::from_tonic_mode("C", "minor").unwrap();
        let root = Pitch::from_name("C").unwrap();
        let french = Chord::new("C D F# A-").unwrap();
        let rn = RomanNumeral::analyze_with_root(&french, key.clone(), &root)
            .unwrap()
            .unwrap();
        assert_eq!(rn.figure(), "Fr+6");

        let german = Chord::new("A- C E- F#").unwrap();
        let rn = RomanNumeral::analyze(&german, key).unwrap().unwrap();
        assert_eq!(rn.figure(), "Ger+6");
    }

    #[test]
    fn figured_bass_shorthands_give_music21_inversions() {
        // Captured from music21's RomanNumeral(...).inversion(). `V642` is the
        // case the old `contains("64")` probe got wrong: it matched `64` and
        // reported a second inversion where music21 reports a third.
        let key = Key::from_tonic_mode("C", "major").unwrap();
        for (figure, expected) in [
            ("V", 0),
            ("V6", 1),
            ("V64", 2),
            ("V7", 0),
            ("V65", 1),
            ("V43", 2),
            ("V42", 3),
            ("V642", 3),
            ("V653", 1),
            ("V643", 2),
            ("V63", 1),
            ("V53", 0),
            ("V9", 0),
        ] {
            let numeral = RomanNumeral::new(figure, key.clone())
                .unwrap_or_else(|err| panic!("{figure} should parse: {err}"));
            assert_eq!(numeral.inversion(), expected, "{figure}");
        }
    }

    #[test]
    fn a_figure_expands_into_a_figured_bass_column() {
        // music21's shorthand: a bare `7` is a seventh chord in root
        // position, and `43` is the same chord over its fifth.
        let numbers = |figure: &str| {
            RomanNumeral::new(figure, Key::from_tonic("C").unwrap())
                .unwrap()
                .figure_numbers()
        };
        assert_eq!(numbers("V"), vec![5, 3]);
        assert_eq!(numbers("V7"), vec![7, 5, 3]);
        assert_eq!(numbers("V65"), vec![6, 5, 3]);
        assert_eq!(numbers("V43"), vec![6, 4, 3]);
        assert_eq!(numbers("V9"), vec![9, 7, 5, 3]);
        // A column written out is left as written, alterations and all.
        assert_eq!(numbers("V7#5b3"), vec![7, 5, 3]);
    }

    #[test]
    fn a_column_says_which_degree_is_in_the_bass() {
        // A sixth and a third over the bass is a triad in first inversion, so
        // the bass of a `V6` is the seventh degree.
        assert_eq!(bass_scale_degree_from_notation(5, &[6, 3]).unwrap(), 7);
        assert_eq!(bass_scale_degree_from_notation(1, &[5, 3]).unwrap(), 1);
        assert_eq!(bass_scale_degree_from_notation(2, &[6, 5, 3]).unwrap(), 4);
        // A column that implies no root leaves the degree where it was.
        assert_eq!(bass_scale_degree_from_notation(5, &[5, 4]).unwrap(), 5);
    }

    #[test]
    fn a_figure_alters_one_note_rather_than_naming_another_chord() {
        let names = |figure: &str, key: &str| {
            RomanNumeral::new(figure, Key::from_tonic(key).unwrap())
                .unwrap()
                .to_chord()
                .unwrap()
                .pitch_names()
        };
        // Writing the fifth out says the column is `7,b5` and nothing else,
        // so the third music21 would have implied is gone with it.
        assert_eq!(names("V7b5", "C"), ["G", "D-", "F"]);
        assert_eq!(names("V[no3]", "F"), ["C", "G"]);
        assert_eq!(names("I[add4][no3]", "C"), ["C", "F", "G"]);
        // The sharp of `i#7` raises the flattened seventh of a minor key to a
        // natural, rather than spelling a `B#`.
        assert_eq!(names("i#7", "c"), ["C", "E-", "G", "B"]);
        assert_eq!(names("i#7", "C"), ["C", "E-", "G", "B#"]);
    }

    #[test]
    fn a_numeral_can_be_read_over_a_scale_that_is_not_a_key() {
        // music21 reads a numeral against any concrete scale, and an
        // octatonic one has eight degrees rather than seven.
        let scale = crate::scale::Scale::new(
            crate::scale::ScaleType::Octatonic,
            Pitch::from_name("C2").unwrap(),
        );
        let numeral = RomanNumeral::over_scale(
            "I9",
            Key::from_tonic("C").unwrap(),
            Some(scale),
            Minor67Default::Quality,
            Minor67Default::Quality,
            false,
        )
        .unwrap();
        let pitches: Vec<String> = numeral
            .to_chord()
            .unwrap()
            .pitches()
            .iter()
            .map(Pitch::name_with_octave)
            .collect();
        assert_eq!(pitches, ["C2", "E-2", "G-2", "A2", "C3"]);
    }

    #[test]
    fn roman_numerals_parse_inversions_and_qualities() {
        let key = Key::from_tonic_mode("C", "major").unwrap();
        let first_inversion = RomanNumeral::new("I6", key.clone()).unwrap();
        assert_eq!(first_inversion.inversion(), 1);
        assert_eq!(
            first_inversion
                .to_chord()
                .unwrap()
                .pitches()
                .into_iter()
                .map(|pitch| pitch.name())
                .collect::<Vec<_>>(),
            vec!["E", "G", "C"]
        );

        let diminished = RomanNumeral::new("viio7", key.clone()).unwrap();
        assert_eq!(diminished.degree(), 7);
        assert!(
            diminished
                .to_chord()
                .unwrap()
                .common_name()
                .contains("diminished")
        );

        let half_diminished = RomanNumeral::new("vii\u{00f8}7", key.clone()).unwrap();
        assert_eq!(half_diminished.degree(), 7);
        assert_eq!(half_diminished.accidental(), 0);
        assert!(
            half_diminished
                .to_chord()
                .unwrap()
                .common_name()
                .contains("half-diminished")
        );

        let borrowed = RomanNumeral::new("bII", key.clone()).unwrap();
        assert_eq!(borrowed.degree(), 2);
        assert_eq!(borrowed.accidental(), -1);

        let added_thirteenth = RomanNumeral::new("I add(13)", key.clone()).unwrap();
        assert_eq!(added_thirteenth.inversion(), 0);
        assert_eq!(
            added_thirteenth.to_chord().unwrap().common_name(),
            "major triad"
        );

        let augmented = RomanNumeral::new("III+", key).unwrap();
        assert_eq!(
            augmented
                .to_chord()
                .unwrap()
                .pitches()
                .into_iter()
                .map(|pitch| pitch.name())
                .collect::<Vec<_>>(),
            vec!["E", "G#", "B#"]
        );
    }

    #[test]
    fn roman_numerals_parse_augmented_sixth_figures() {
        let key = Key::from_tonic_mode("C", "minor").unwrap();
        let french = RomanNumeral::new("Fr+6", key).unwrap();
        assert_eq!(french.degree(), 6);
        assert_eq!(french.accidental(), -1);
        assert_eq!(
            french
                .to_chord()
                .unwrap()
                .pitches()
                .into_iter()
                .map(|pitch| pitch.name())
                .collect::<Vec<_>>(),
            vec!["A-", "C", "D", "F#"]
        );
    }

    #[test]
    fn roman_numerals_report_invalid_figures_and_empty_analysis() {
        let key = Key::from_tonic_mode("C", "major").unwrap();

        assert!(RomanNumeral::new("", key.clone()).is_err());
        assert!(RomanNumeral::new("Q", key.clone()).is_err());
        assert!(analyze_chord(&Chord::empty(), key).unwrap().is_none());
    }
}
