//! Bach's chorales by their numbers in each catalogue: music21's
//! `corpus.chorales.ChoraleList` and `ChoraleListRKBWV`.
//!
//! Two lists, as music21 has them. The first, from Wikipedia's list of
//! chorale harmonisations (CC BY-SA), numbers BWV 250-438 in the Kalmus,
//! Bärenreiter, Budapest and Riemenschneider catalogues. The second, from
//! Margaret Greentree's jsbchorales.net, covers every chorale in the corpus
//! but only by Riemenschneider, BWV and Kalmus number. Both are kept in
//! `src/corpus/chorales/` as music21 writes them.

use std::sync::LazyLock;

/// The Wikipedia list, one `|title||bwv||kalmus||baerenreiter||budapest||riemenschneider||notes` per line.
const WIKIPEDIA: &str = include_str!("chorales/wikipedia.txt");

/// The Greentree list, one `riemenschneider---||---bwv---||---title---||---kalmus` per line.
const GREENTREE: &str = include_str!("chorales/greentree.txt");

/// What the Wikipedia list writes for a chorale with no notes.
const NO_NOTES: &str = "&nbsp;";

/// A chorale of BWV 250-438, numbered in every catalogue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chorale {
    /// The chorale's German title.
    pub title: String,
    /// Its number in the Bach-Werke-Verzeichnis.
    pub bwv: u32,
    /// Its number in the Kalmus edition.
    pub kalmus: u32,
    /// Its number in the Bärenreiter (Kirnberger) edition.
    pub baerenreiter: u32,
    /// Its number in the Budapest edition.
    pub budapest: u32,
    /// Its number in Riemenschneider's edition.
    pub riemenschneider: u32,
    /// What the list notes about it, where it notes anything.
    pub notes: Option<String>,
}

/// A chorale of the corpus by its Riemenschneider, BWV and Kalmus numbers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CorpusChorale {
    /// Its number in Riemenschneider's edition.
    pub riemenschneider: u32,
    /// Its BWV number with the movement, as the corpus names its file:
    /// `153.1`, `248.12-2`.
    pub bwv: String,
    /// The chorale's German title.
    pub title: String,
    /// Its number in the Kalmus edition; 0 where it has none.
    pub kalmus: u32,
}

/// The chorales of BWV 250-438, in the list's order.
///
/// ```
/// let first = &music21::corpus::chorales::chorales()[0];
/// assert_eq!(first.title, "Was Gott tut, das ist wohlgetan");
/// assert_eq!((first.bwv, first.riemenschneider), (250, 347));
/// ```
pub fn chorales() -> &'static [Chorale] {
    static READ: LazyLock<Vec<Chorale>> =
        LazyLock::new(|| WIKIPEDIA.lines().filter_map(wikipedia_line).collect());
    &READ
}

/// The corpus's chorales, by Riemenschneider number in the list's order.
///
/// ```
/// let second = &music21::corpus::chorales::corpus_chorales()[1];
/// assert_eq!(second.bwv, "347");
/// assert_eq!(second.kalmus, 176);
/// ```
pub fn corpus_chorales() -> &'static [CorpusChorale] {
    static READ: LazyLock<Vec<CorpusChorale>> =
        LazyLock::new(|| GREENTREE.lines().filter_map(greentree_line).collect());
    &READ
}

fn wikipedia_line(line: &str) -> Option<Chorale> {
    let line = line.trim().strip_prefix('|')?;
    let fields: Vec<&str> = line.split("||").collect();
    let [
        title,
        bwv,
        kalmus,
        baerenreiter,
        budapest,
        riemenschneider,
        notes,
    ] = fields[..]
    else {
        return None;
    };
    Some(Chorale {
        title: title.to_string(),
        bwv: bwv.parse().ok()?,
        kalmus: kalmus.parse().ok()?,
        baerenreiter: baerenreiter.parse().ok()?,
        budapest: budapest.parse().ok()?,
        riemenschneider: riemenschneider.parse().ok()?,
        notes: (notes != NO_NOTES).then(|| notes.to_string()),
    })
}

fn greentree_line(line: &str) -> Option<CorpusChorale> {
    let fields: Vec<&str> = line.trim().split("---||---").collect();
    let [riemenschneider, bwv, title, kalmus] = fields[..] else {
        return None;
    };
    Some(CorpusChorale {
        riemenschneider: riemenschneider.parse().ok()?,
        bwv: bwv.to_string(),
        title: title.to_string(),
        kalmus: kalmus.parse().ok()?,
    })
}

/// A BWV number as a number to sort by: music21's `Iterator._bwvSort`.
/// A hyphen becomes a decimal point or a zero, and a letter its place
/// counted from `a` as 10, so `69.6-a` sorts as `69.6010`.
///
/// ```
/// use music21::corpus::chorales::bwv_sort_key;
///
/// assert_eq!(bwv_sort_key("69.6-a"), 69.601);
/// assert_eq!(bwv_sort_key("145-a"), 145.01);
/// assert!(bwv_sort_key("18.5-w") < bwv_sort_key("20.7"));
/// ```
pub fn bwv_sort_key(bwv: &str) -> f64 {
    let mut out = String::new();
    for character in bwv.chars() {
        if character.is_ascii_digit() || character == '.' {
            out.push(character);
        } else if character == '-' {
            out.push_str(if out.contains('.') { "0" } else { ".0" });
        } else if character.is_alphabetic() {
            // Shifted so that 'a' is 10.
            out.push_str(&(character as i64 - 87).to_string());
        }
    }
    out.parse().unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_line_of_both_lists_is_read() {
        assert_eq!(chorales().len(), WIKIPEDIA.lines().count());
        assert_eq!(corpus_chorales().len(), GREENTREE.lines().count());
    }

    #[test]
    fn a_chorale_without_notes_has_none() {
        let found = chorales()
            .iter()
            .find(|chorale| chorale.bwv == 431)
            .unwrap();
        assert_eq!(found.notes, None);
        assert_eq!(found.budapest, 358);
        assert_eq!(
            chorales()[2].notes.as_deref(),
            Some("3rd Wedding Chorale in this group")
        );
    }
}
