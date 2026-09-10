//! The chord as a pitch-class set: its place in the Forte tables, the
//! prime and normal forms, the interval vector and the constructors that
//! build a chord from those.

use super::*;

impl Chord {
    /// Builds the chord of a Forte set class from its name, `3-11` or
    /// `4-27B`: music21's `fromForteClass`. The pitches are the transposed
    /// normal form from C, without octaves, so `3-11` is `C E- G` and `3-11B`
    /// is `C E G`.
    pub fn from_forte_class(notation: &str) -> Result<Self> {
        let Some((cardinality, rest)) = notation.split_once('-') else {
            return Err(Error::Chord(format!(
                "cannot extract set-class representation from string: {notation}"
            )));
        };
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        let letters = &rest[digits.len()..];
        let inversion = match letters.to_ascii_lowercase().as_str() {
            "a" => Some(1),
            "b" => Some(-1),
            _ => None,
        };
        let cardinality = cardinality
            .parse::<u8>()
            .map_err(|_| Error::Chord(format!("cannot read a cardinality out of {notation}")))?;
        let index = digits
            .parse::<u8>()
            .map_err(|_| Error::Chord(format!("cannot read a Forte number out of {notation}")))?;
        Self::from_forte_address(cardinality, index, inversion)
    }

    /// Builds the chord of a Forte set class from its table address: the
    /// cardinality, the number within it, and `1`, `-1` or `None` for the
    /// inversion, where `None` takes the form the table lists first.
    pub fn from_forte_address(cardinality: u8, index: u8, inversion: Option<i8>) -> Result<Self> {
        let pitch_classes = tables::transposed_normal_form(cardinality, index, inversion)?;
        Self::from_pitch_class_list(&pitch_classes)
    }

    /// Builds the chord whose interval-class vector this is: music21's
    /// `fromIntervalVector`. Z-related pairs share a vector; the first of
    /// the pair is returned unless `z_relation` asks for the second. `None`
    /// when no set class has the vector.
    pub fn from_interval_vector(vector: &[u8; 6], z_relation: bool) -> Option<Self> {
        let mut seen: Vec<(u8, String)> = Vec::new();
        for entry in tables::known_chord_table_entries() {
            if entry.interval_class_vector != vector {
                continue;
            }
            let number = entry.forte_class.trim_end_matches(['A', 'B']).to_string();
            if seen
                .iter()
                .any(|(card, seen_number)| *card == entry.cardinality && *seen_number == number)
            {
                continue;
            }
            seen.push((entry.cardinality, number));
        }
        let (cardinality, number) = match (seen.len(), z_relation) {
            (1, _) | (2, false) => seen.first()?.clone(),
            (2, true) => seen.get(1)?.clone(),
            _ => return None,
        };
        let index = number.split_once('-')?.1.parse().ok()?;
        Self::from_forte_address(cardinality, index, None).ok()
    }

    /// A chord from pitch-class integers, spelled the way music21 spells a
    /// chord built from integers: each class on its own and then
    /// `simplifyEnharmonics` over the whole, so `[0, 3, 6, 8]` is `C E- G- A-`.
    pub(super) fn from_pitch_class_list(pitch_classes: &[u8]) -> Result<Self> {
        let pitches = pitch_classes
            .iter()
            .map(|&pc| Pitch::from_pitch_class(IntegerType::from(pc)))
            .collect::<Result<Vec<_>>>()?;
        Self::new(pitches.as_slice())?.simplify_enharmonics(None)
    }

    /// Returns the Forte class, such as `"3-11B"`, when available.
    ///
    /// Returns `None` when the chord's pitch-class set has no Forte-table
    /// entry, including empty or otherwise unsupported pitch-class sets.
    pub fn forte_class(&self) -> Option<String> {
        let ordered_pcs = self.ordered_pitch_classes();
        let address = tables::seek_chord_tables_address(&ordered_pcs).ok()?;
        tables::address_to_forte_name(address, "tn").ok()
    }

    /// Returns the normal form transposed to start on zero, `[0, 3, 6, 8]`
    /// for `C E G B-`. This is the Forte-table form music21 reads off the
    /// chord's table address; [`Self::normal_order`] is music21's
    /// `normalOrder`, on the chord's own pitch classes.
    ///
    /// Returns `None` when the chord's pitch-class set cannot be found in the
    /// chord tables, including empty or otherwise unsupported pitch-class sets.
    pub fn normal_form(&self) -> Option<Vec<u8>> {
        let ordered_pcs = self.ordered_pitch_classes();
        let address = tables::seek_chord_tables_address(&ordered_pcs).ok()?;
        tables::transposed_normal_form_from_address(address).ok()
    }

    /// Returns the interval-class vector when table metadata is available.
    ///
    /// Returns `None` when the chord's pitch-class set cannot be found in the
    /// chord tables, including empty or otherwise unsupported pitch-class sets.
    pub fn interval_class_vector(&self) -> Option<Vec<u8>> {
        let ordered_pcs = self.ordered_pitch_classes();
        let address = tables::seek_chord_tables_address(&ordered_pcs).ok()?;
        tables::interval_class_vector_from_address(address).ok()
    }

    /// Returns Robert Morris's eight-entry invariance vector, when available.
    ///
    /// The values are taken from the same music21 Forte table as
    /// [`Self::forte_class`] and [`Self::interval_class_vector`].
    pub fn invariance_vector(&self) -> Option<Vec<u8>> {
        let ordered_pcs = self.ordered_pitch_classes();
        let address = tables::seek_chord_tables_address(&ordered_pcs).ok()?;
        tables::invariance_vector_from_address(address).ok()
    }

    /// Returns this chord's Z-related Forte class, when music21 records one.
    pub fn z_relation(&self) -> Option<String> {
        let ordered_pcs = self.ordered_pitch_classes();
        let address = tables::seek_chord_tables_address(&ordered_pcs).ok()?;
        tables::z_relation_from_address(address).ok().flatten()
    }

    pub(super) fn forte_address(&self) -> Option<(u8, u8, i8)> {
        tables::seek_chord_tables_address(&self.ordered_pitch_classes())
            .ok()
            .map(|(card, index, inversion, _)| (card, index, inversion))
    }

    /// Returns the Forte prime form of the pitch-class set, such as
    /// `[0, 3, 7]` for any major or minor triad. Empty when the chord has no
    /// table entry.
    pub fn prime_form(&self) -> Vec<u8> {
        tables::seek_chord_tables_address(&self.ordered_pitch_classes())
            .and_then(tables::prime_form_from_address)
            .unwrap_or_default()
    }

    /// Returns the prime form in music21's angle-bracket notation, such as
    /// `"<037>"`, with `A` and `B` standing for ten and eleven.
    pub fn prime_form_string(&self) -> String {
        format_pitch_classes(&self.prime_form())
    }

    /// Returns the Forte class without the `A`/`B` inversion suffix, such as
    /// `"3-11"` for both major and minor triads.
    pub fn forte_class_tni(&self) -> Option<String> {
        let address = tables::seek_chord_tables_address(&self.ordered_pitch_classes()).ok()?;
        tables::address_to_forte_name(address, "tni").ok()
    }

    /// Returns the number of distinct pitch classes.
    pub fn pitch_class_cardinality(&self) -> usize {
        self.pitch_class_set().len()
    }

    /// Returns the distinct pitch classes in ascending order in music21's
    /// angle-bracket notation, such as `"<047>"`.
    pub fn ordered_pitch_classes_string(&self) -> String {
        format_pitch_classes(&self.ordered_pitch_classes())
    }

    pub(super) fn ordered_pitch_classes(&self) -> Vec<u8> {
        let mut pcs = self
            .notes
            .iter()
            .map(|note| root::pitch_class(&note.pitch))
            .collect::<Vec<_>>();
        pcs.sort_unstable();
        pcs.dedup();
        pcs
    }

    pub(super) fn pitch_class_set(&self) -> std::collections::BTreeSet<u8> {
        self.ordered_pitch_classes().into_iter().collect()
    }

    pub(super) fn pitch_class_mask(&self) -> u16 {
        self.ordered_pitch_classes()
            .into_iter()
            .fold(0_u16, |mask, pc| mask | (1_u16 << pc))
    }

    /// Where this chord's set class sits in the Forte tables.
    ///
    /// An empty chord answers all zeros rather than failing, which is the
    /// one place music21's `Chord.chordTablesAddress` differs from the
    /// `seekChordTablesAddress` underneath it.
    pub fn chord_tables_address_entry(&self) -> ChordTableAddress {
        match self.chord_tables_address() {
            Some((cardinality, forte_class, inversion, original)) => ChordTableAddress {
                cardinality,
                forte_class,
                inversion,
                pitch_class_original: original.unwrap_or(0),
            },
            None => ChordTableAddress {
                cardinality: 0,
                forte_class: 0,
                inversion: 0,
                pitch_class_original: 0,
            },
        }
    }

    pub(super) fn chord_tables_address(&self) -> Option<tables::RawAddress> {
        tables::seek_chord_tables_address(&self.ordered_pitch_classes()).ok()
    }

    /// Returns music21's `geometricNormalForm`: the distinct pitch classes
    /// rotated so the intervals between neighbours read smallest first, then
    /// written from zero, so both `C E G` and `E G C` are `[0, 3, 8]`. Empty
    /// for an empty chord.
    pub fn geometric_normal_form(&self) -> Vec<u8> {
        let pitch_classes = self.ordered_pitch_classes();
        if pitch_classes.is_empty() {
            return Vec::new();
        }
        let intervals: Vec<u8> = pitch_classes
            .iter()
            .zip(pitch_classes.iter().cycle().skip(1))
            .map(|(&low, &high)| (high + 12 - low) % 12)
            .collect();
        let best = (0..intervals.len())
            .map(|rotation| {
                let mut rotated = intervals[rotation + 1..].to_vec();
                rotated.extend_from_slice(&intervals[..=rotation]);
                rotated
            })
            .min()
            .unwrap_or_default();
        let mut sum = 0;
        best.iter()
            .map(|interval| {
                let pitch_class = sum;
                sum += interval;
                pitch_class
            })
            .collect()
    }

    /// Returns the interval-class vector in music21's angle-bracket
    /// notation, `<001110>`; an empty chord reads `<000000>`.
    pub fn interval_vector_string(&self) -> String {
        format_pitch_classes(&self.interval_class_vector().unwrap_or_else(|| vec![0; 6]))
    }

    /// Returns music21's `normalOrder`: the most compact rotation of the
    /// pitch classes, on the chord's own pitch classes rather than
    /// transposed to zero, so `C E G B-` is `[4, 7, 10, 0]` where
    /// [`Self::normal_form`] is `[0, 3, 6, 8]`. Empty for an empty chord.
    pub fn normal_order(&self) -> Vec<u8> {
        let Some(transposed) = self.normal_form() else {
            return Vec::new();
        };
        let ordered = self.ordered_pitch_classes();
        ordered
            .iter()
            .map(|&transposition| {
                transposed
                    .iter()
                    .map(|&pc| (pc + transposition) % 12)
                    .collect::<Vec<u8>>()
            })
            .find(|candidate| {
                let mut sorted = candidate.clone();
                sorted.sort_unstable();
                sorted == ordered
            })
            .unwrap_or_default()
    }

    /// Returns [`Self::normal_order`] in music21's angle-bracket notation,
    /// `<47A0>`.
    pub fn normal_order_string(&self) -> String {
        format_pitch_classes(&self.normal_order())
    }

    /// Returns the Forte class number within the cardinality, `11` for a
    /// major or minor triad. `None` for an empty chord.
    pub fn forte_class_number(&self) -> Option<u8> {
        self.chord_tables_address().map(|address| address.1)
    }

    /// Returns the Forte class under transposition equivalence, with the
    /// `A`/`B` inversion suffix: music21's `forteClassTn`, the same as
    /// [`Self::forte_class`].
    pub fn forte_class_tn(&self) -> Option<String> {
        self.forte_class()
    }

    /// Returns the number of notes, counting repeated pitch classes: music21's
    /// `multisetCardinality`.
    pub fn multiset_cardinality(&self) -> usize {
        self.notes.len()
    }

    /// Returns whether the pitch-class set is the inversion of its prime
    /// form, so its Forte class carries a `B` suffix.
    pub fn is_prime_form_inversion(&self) -> bool {
        self.chord_tables_address()
            .is_some_and(|address| address.2 == -1)
    }

    /// Returns whether music21 records a Z-related set class for this chord.
    pub fn has_z_relation(&self) -> bool {
        self.z_relation().is_some()
    }

    /// Returns whether `other` belongs to the set class Z-related to this
    /// chord's, so the two share an interval vector without being related by
    /// transposition or inversion.
    pub fn are_z_relations(&self, other: &Chord) -> bool {
        let Some(z_relation) = self.z_relation() else {
            return false;
        };
        other
            .chord_tables_address()
            .is_some_and(|address| format!("{}-{}", address.0, address.1) == z_relation)
    }

    /// Returns whether the pitch classes form a fully diminished seventh
    /// however it is spelled: music21's `isFalseDiminishedSeventh`, true for
    /// `C E- G- A` where [`Self::is_diminished_seventh`] is not.
    pub fn is_false_diminished_seventh(&self) -> bool {
        self.chord_tables_address()
            .is_some_and(|address| (address.0, address.1, address.2) == (4, 28, 0))
    }
}

/// Writes a list of pitch classes the way music21's
/// `Chord.formatVectorString` does, with ten and eleven as `A` and `B`:
/// `[0, 11]` is `<0B>`.
pub fn format_vector_string(values: &[u8]) -> String {
    let digits: String = values
        .iter()
        .map(|value| crate::pitch::convert_pitch_class_to_str(*value as IntegerType))
        .collect();
    format!("<{digits}>")
}

pub(super) fn format_pitch_classes(pitch_classes: &[u8]) -> String {
    let mut out = String::with_capacity(pitch_classes.len() + 2);
    out.push('<');
    for pitch_class in pitch_classes {
        out.push_str(&crate::pitch::pitchclass::convert_pitch_class_to_str(
            IntegerType::from(*pitch_class),
        ));
    }
    out.push('>');
    out
}

/// Where a chord's set class sits in the Forte tables: music21's
/// `ChordTableAddress`.
///
/// The cardinality and the class number index the table; the inversion says
/// which of an inversionally related pair this is, `0` when the class is its
/// own inversion; and the original pitch class is the one the prime form was
/// transposed away from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct ChordTableAddress {
    /// How many distinct pitch classes the chord has.
    pub cardinality: u8,
    /// The Forte class number within that cardinality.
    pub forte_class: u8,
    /// `1`, `-1`, or `0` for a class that is its own inversion.
    pub inversion: i8,
    /// The pitch class the prime form was transposed away from.
    pub pitch_class_original: u8,
}
