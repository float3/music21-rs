//! How alike two sequences are, as Python's `difflib.SequenceMatcher`
//! measures it, which music21's approximate searches rank by.

use std::collections::HashMap;

use crate::defaults::FloatType;

/// Python's `SequenceMatcher(None, a, b).ratio()`: twice the number of
/// items in the matching blocks over the items of both, one for two empty
/// sequences. The blocks are found as Python finds them -- the longest
/// match first, earliest where several are as long, then the same either
/// side of it -- and an item of `b` coming in more than one percent of a
/// `b` of 200 items or more is junk, as Python's `autojunk` makes it.
pub(crate) fn ratio<T: Copy + Eq + std::hash::Hash>(a: &[T], b: &[T]) -> FloatType {
    let matches: usize = matching_blocks(a, b).iter().map(|(_, _, size)| size).sum();
    let length = a.len() + b.len();
    if length == 0 {
        1.0
    } else {
        2.0 * matches as FloatType / length as FloatType
    }
}

/// Where each item of `b` comes, its popular items taken out.
fn positions<T: Copy + Eq + std::hash::Hash>(b: &[T]) -> HashMap<T, Vec<usize>> {
    let mut b2j: HashMap<T, Vec<usize>> = HashMap::new();
    for (index, item) in b.iter().enumerate() {
        b2j.entry(*item).or_default().push(index);
    }
    if b.len() >= 200 {
        let most = b.len() / 100 + 1;
        b2j.retain(|_, indices| indices.len() <= most);
    }
    b2j
}

/// Python's `find_longest_match` within `a[alo..ahi]` and `b[blo..bhi]`,
/// with no `isjunk`.
fn longest_match<T: Copy + Eq + std::hash::Hash>(
    a: &[T],
    b: &[T],
    b2j: &HashMap<T, Vec<usize>>,
    (alo, ahi, blo, bhi): (usize, usize, usize, usize),
) -> (usize, usize, usize) {
    let (mut best_i, mut best_j, mut best_size) = (alo, blo, 0);
    let mut j2len: HashMap<usize, usize> = HashMap::new();
    for (i, item) in a.iter().enumerate().take(ahi).skip(alo) {
        let mut next: HashMap<usize, usize> = HashMap::new();
        if let Some(indices) = b2j.get(item) {
            for &j in indices {
                if j < blo {
                    continue;
                }
                if j >= bhi {
                    break;
                }
                let k = j
                    .checked_sub(1)
                    .and_then(|before| j2len.get(&before))
                    .copied()
                    .unwrap_or(0)
                    + 1;
                next.insert(j, k);
                if k > best_size {
                    (best_i, best_j, best_size) = (i + 1 - k, j + 1 - k, k);
                }
            }
        }
        j2len = next;
    }
    // Without `isjunk` nothing is junk -- the popular items are only left
    // out of `b2j` -- so the match grows over whatever is equal on either
    // side.
    while best_i > alo && best_j > blo && a[best_i - 1] == b[best_j - 1] {
        (best_i, best_j, best_size) = (best_i - 1, best_j - 1, best_size + 1);
    }
    while best_i + best_size < ahi
        && best_j + best_size < bhi
        && a[best_i + best_size] == b[best_j + best_size]
    {
        best_size += 1;
    }
    (best_i, best_j, best_size)
}

/// What one stretch of an [`Opcode`] does to turn the first sequence into
/// the second: Python's opcode tags.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum OpcodeTag {
    /// The first sequence's stretch is replaced by the second's: `replace`.
    Replace,
    /// The first sequence's stretch is left out: `delete`.
    Delete,
    /// The second sequence's stretch is put in: `insert`.
    Insert,
    /// The two stretches are the same: `equal`.
    Equal,
}

impl OpcodeTag {
    /// Python's name for the tag.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Replace => "replace",
            Self::Delete => "delete",
            Self::Insert => "insert",
            Self::Equal => "equal",
        }
    }
}

/// One step of turning one sequence into another: Python's opcode
/// `(tag, i1, i2, j1, j2)`, the first sequence's items `i1..i2` and the
/// second's `j1..j2`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Opcode {
    /// What the step does.
    pub tag: OpcodeTag,
    /// Where the stretch of the first sequence starts.
    pub i1: usize,
    /// Where it ends.
    pub i2: usize,
    /// Where the stretch of the second sequence starts.
    pub j1: usize,
    /// Where it ends.
    pub j2: usize,
}

/// Python's `SequenceMatcher(None, a, b).get_opcodes()`: the steps that turn
/// `a` into `b`, between the matching blocks.
pub(crate) fn opcodes<T: Copy + Eq + std::hash::Hash>(a: &[T], b: &[T]) -> Vec<Opcode> {
    let mut blocks = matching_blocks(a, b);
    blocks.push((a.len(), b.len(), 0));
    let (mut i, mut j) = (0, 0);
    let mut steps = Vec::new();
    for (ai, bj, size) in blocks {
        let tag = if i < ai && j < bj {
            Some(OpcodeTag::Replace)
        } else if i < ai {
            Some(OpcodeTag::Delete)
        } else if j < bj {
            Some(OpcodeTag::Insert)
        } else {
            None
        };
        if let Some(tag) = tag {
            steps.push(Opcode {
                tag,
                i1: i,
                i2: ai,
                j1: j,
                j2: bj,
            });
        }
        (i, j) = (ai + size, bj + size);
        if size > 0 {
            steps.push(Opcode {
                tag: OpcodeTag::Equal,
                i1: ai,
                i2: i,
                j1: bj,
                j2: j,
            });
        }
    }
    steps
}

/// Python's `get_matching_blocks`, without its closing sentinel: each
/// `(i, j, size)` with `a[i..i + size] == b[j..j + size]`, adjacent blocks
/// joined.
fn matching_blocks<T: Copy + Eq + std::hash::Hash>(a: &[T], b: &[T]) -> Vec<(usize, usize, usize)> {
    let b2j = positions(b);
    let mut queue = vec![(0, a.len(), 0, b.len())];
    let mut blocks = Vec::new();
    while let Some(range) = queue.pop() {
        let (alo, ahi, blo, bhi) = range;
        let (i, j, k) = longest_match(a, b, &b2j, range);
        if k > 0 {
            blocks.push((i, j, k));
            if alo < i && blo < j {
                queue.push((alo, i, blo, j));
            }
            if i + k < ahi && j + k < bhi {
                queue.push((i + k, ahi, j + k, bhi));
            }
        }
    }
    blocks.sort_unstable();
    let mut joined: Vec<(usize, usize, usize)> = Vec::new();
    for (i, j, k) in blocks {
        match joined.last_mut() {
            Some((i1, j1, k1)) if *i1 + *k1 == i && *j1 + *k1 == j => *k1 += k,
            _ => joined.push((i, j, k)),
        }
    }
    joined
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(text: &str) -> Vec<char> {
        text.chars().collect()
    }

    #[test]
    fn opcodes_are_pythons() {
        // Read off Python's SequenceMatcher(None, 'qabxcd', 'abycdf').get_opcodes().
        let steps: Vec<(&str, usize, usize, usize, usize)> =
            opcodes(&chars("qabxcd"), &chars("abycdf"))
                .iter()
                .map(|step| (step.tag.as_str(), step.i1, step.i2, step.j1, step.j2))
                .collect();
        assert_eq!(
            steps,
            [
                ("delete", 0, 1, 0, 0),
                ("equal", 1, 3, 0, 2),
                ("replace", 3, 4, 2, 3),
                ("equal", 4, 6, 3, 5),
                ("insert", 6, 6, 5, 6),
            ]
        );
        assert!(opcodes::<char>(&[], &[]).is_empty());
    }

    #[test]
    fn a_ratio_is_pythons() {
        // Each read off Python's difflib.SequenceMatcher(None, a, b).ratio().
        assert_eq!(ratio(&chars("abcd"), &chars("bcde")), 0.75);
        assert_eq!(ratio(&chars(""), &chars("")), 1.0);
        assert_eq!(ratio(&chars("abc"), &chars("")), 0.0);
        assert_eq!(
            ratio(
                &chars("private Thread currentThread;"),
                &chars("private volatile Thread currentThread;")
            ),
            0.865_671_641_791_044_7
        );
        assert_eq!(
            ratio(&chars("hello world"), &chars("world hello")),
            0.454_545_454_545_454_53
        );
        // A long b, where Python's autojunk leaves its common letters out
        // of the search, though a match still grows over them.
        let long_b = chars(&"abc".repeat(100));
        assert_eq!(
            ratio(&chars(&"ab".repeat(150)), &long_b),
            0.006_666_666_666_666_667
        );
        assert_eq!(
            ratio(&chars(&format!("{}q", "xab".repeat(80))), &long_b),
            0.0
        );
    }
}
