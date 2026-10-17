//! Counting what a stream's elements say: music21's `analysis.elements`.

/// How often each value of some property of a stream's elements comes, in
/// the order each value is first met: music21's `attributeCount`, with the
/// property a function of the element rather than the name of one, and
/// `None` where the element has no such property -- which is music21's
/// `hasattr` test.
///
/// The elements are whatever reading the caller counts over: a stream's own
/// [`events`](crate::Stream::events), a recursive one
/// ([`recurse`](crate::Stream::recurse), [`leaves`](crate::Stream::leaves)),
/// or either of those filtered, as music21 takes a stream or a stream
/// iterator.
///
/// The counts are in counting order, as Python's `Counter` keeps them, so
/// `most_common` is a sort by count that leaves ties as they stand.
///
/// ```
/// use music21_rs::analysis::elements::attribute_count;
/// use music21_rs::stream::StreamElement;
/// use music21_rs::tinynotation::from_tiny_notation;
///
/// let line = from_tiny_notation("4/4 c4 d8 e8 f2")?;
/// let lengths = attribute_count(line.recurse(), |(_, element)| match element {
///     StreamElement::Note(_) => Some(element.quarter_length()),
///     // A clef or a meter is not a note, and is not counted -- as
///     // music21 passes over an element the attribute is not on.
///     _ => None,
/// });
/// assert_eq!(lengths, [(1.0, 1), (0.5, 2), (2.0, 1)]);
/// # Ok::<(), music21_rs::Error>(())
/// ```
pub fn attribute_count<T, K: PartialEq>(
    elements: impl IntoIterator<Item = T>,
    attribute: impl Fn(T) -> Option<K>,
) -> Vec<(K, usize)> {
    let mut counts: Vec<(K, usize)> = Vec::new();
    for element in elements {
        let Some(value) = attribute(element) else {
            continue;
        };
        match counts.iter_mut().find(|(known, _)| *known == value) {
            Some((_, count)) => *count += 1,
            None => counts.push((value, 1)),
        }
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::attribute_count;
    use crate::stream::StreamElement;
    use crate::tinynotation::from_tiny_notation;

    #[test]
    fn a_value_is_counted_where_it_was_first_met() {
        let line = from_tiny_notation("4/4 e4 c4 c4 e4").expect("the line is read");
        let names = attribute_count(line.recurse(), |(_, element)| match element {
            StreamElement::Note(note) => Some(note.pitch().name()),
            _ => None,
        });
        assert_eq!(names, [("E".to_string(), 2), ("C".to_string(), 2)]);
    }

    #[test]
    fn an_element_the_property_is_not_on_is_passed_over() {
        let line = from_tiny_notation("4/4 c4 r4 d2").expect("the line is read");
        let counted = attribute_count(line.recurse(), |(_, element)| match element {
            StreamElement::Note(_) => Some(element.quarter_length()),
            _ => None,
        });
        assert_eq!(counted, [(1.0, 1), (2.0, 1)]);
    }
}
