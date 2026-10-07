//! Braille laid out in lines of so many cells: music21's `braille.text`.

use super::basic::{center, number_to_braille, python_lines, yield_dots};
use super::lookup::symbol;
use crate::error::{Error, Result};

/// One line of braille, written into cell by cell: music21's
/// `BrailleTextLine`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrailleTextLine {
    /// Whether the line is part of a heading.
    pub is_heading: bool,
    /// Whether a group of notes has been written on it.
    pub contains_note_grouping: bool,
    /// How many cells the line holds.
    pub line_length: usize,
    cells: Vec<String>,
    /// Where the next cell is written.
    pub text_location: usize,
    /// How far the line has been written.
    pub highest_used_location: usize,
}

fn refuse(message: &str) -> Error {
    Error::Notation(message.to_string())
}

impl BrailleTextLine {
    /// An empty line of so many cells.
    pub fn new(line_length: usize) -> Self {
        Self {
            is_heading: false,
            contains_note_grouping: false,
            line_length,
            cells: vec![symbol("space"); line_length],
            text_location: 0,
            highest_used_location: 0,
        }
    }

    /// Writes text at the end, after a space where asked: music21's
    /// `append`.
    ///
    /// # Errors
    ///
    /// Text that does not fit.
    pub fn append(&mut self, text: &str, add_space: bool) -> Result<()> {
        if !self.can_append(text, add_space) {
            return Err(refuse("Text does not fit at end of braille text line."));
        }
        if add_space {
            self.cells[self.text_location] = symbol("space");
            self.text_location += 1;
        }
        for cell in text.chars() {
            self.cells[self.text_location] = cell.to_string();
            self.text_location += 1;
        }
        self.highest_used_location = self.text_location;
        Ok(())
    }

    /// Writes text from a cell on, the line then ending after it: music21's
    /// `insert`.
    ///
    /// # Errors
    ///
    /// Text that does not fit there.
    pub fn insert(&mut self, location: usize, text: &str) -> Result<()> {
        if !self.can_insert(location, text) {
            return Err(refuse("Text cannot be inserted at specified location."));
        }
        for (at, cell) in (location..).zip(text.chars()) {
            self.cells[at] = cell.to_string();
        }
        self.text_location = location + text.chars().count();
        self.highest_used_location = self.highest_used_location.max(self.text_location);
        Ok(())
    }

    /// Whether text fits at the end, after a space where asked: music21's
    /// `canAppend`.
    pub fn can_append(&self, text: &str, add_space: bool) -> bool {
        let search = self.highest_used_location.max(self.text_location);
        search + text.chars().count() + usize::from(add_space) <= self.line_length
    }

    /// Whether text fits from a cell on: music21's `canInsert`.
    pub fn can_insert(&self, location: usize, text: &str) -> bool {
        location + text.chars().count() <= self.line_length
    }

    /// Takes back a music hyphen ending the line: music21's
    /// `lastHyphenToSpace`.
    pub fn last_hyphen_to_space(&mut self) {
        let Some(previous) = self.text_location.checked_sub(1) else {
            return;
        };
        if self.cells[previous] == symbol("music_hyphen") {
            self.cells[previous] = symbol("space");
            self.text_location -= 1;
        }
    }

    /// The first `text_location` cells.
    pub fn text(&self) -> String {
        self.cells[..self.text_location].concat()
    }
}

/// Braille laid out in lines, with headings, measure numbers and the signs
/// of a hand at the keyboard: music21's `BrailleText`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrailleText {
    /// How many cells a line holds.
    pub line_length: usize,
    /// Every line.
    pub all_lines: Vec<BrailleTextLine>,
    /// Whether lines of notes start with the right hand's sign.
    pub right_hand_symbol: bool,
    /// Whether lines of notes start with the left hand's sign.
    pub left_hand_symbol: bool,
    /// Where each heading's lines start and end.
    pub all_headings: Vec<(usize, usize)>,
}

impl BrailleText {
    /// Text of lines this long, with a hand's sign where asked: `right` or
    /// `left`.
    ///
    /// # Errors
    ///
    /// A hand music21 does not know.
    pub fn new(line_length: usize, show_hand: Option<&str>) -> Result<Self> {
        let mut text = Self {
            line_length,
            all_lines: Vec::new(),
            right_hand_symbol: false,
            left_hand_symbol: false,
            all_headings: Vec::new(),
        };
        text.make_new_line();
        text.set_show_hand(show_hand)?;
        Ok(text)
    }

    /// Which hand's sign the notes' lines start with: music21's `showHand`
    /// setter. music21's getter never answers what was set.
    ///
    /// # Errors
    ///
    /// A hand music21 does not know.
    pub fn set_show_hand(&mut self, hand: Option<&str>) -> Result<()> {
        match hand {
            Some("right") => self.right_hand_symbol = true,
            Some("left") => self.left_hand_symbol = true,
            None => {}
            Some(_) => return Err(refuse("Illegal hand sign request.")),
        }
        Ok(())
    }

    /// The line being written.
    pub fn current_line(&mut self) -> &mut BrailleTextLine {
        self.all_lines.last_mut().expect("a text always has a line")
    }

    /// Starts a new line: music21's `makeNewLine`.
    pub fn make_new_line(&mut self) {
        self.all_lines.push(BrailleTextLine::new(self.line_length));
    }

    /// A heading, a line each, on lines of its own: music21's `addHeading`.
    ///
    /// # Errors
    ///
    /// A heading line wider than a line.
    pub fn add_heading(&mut self, heading: &str) -> Result<()> {
        if self.current_line().text_location != 0 {
            self.make_new_line();
        }
        let start = self.all_lines.len() - 1;
        let mut end = start;
        for line in python_lines(heading) {
            let current = self.current_line();
            current.is_heading = true;
            current.append(&line, false)?;
            self.make_new_line();
            end += 1;
        }
        self.all_headings.push((start, end));
        Ok(())
    }

    /// Words of a long text expression, each where it fits: music21's
    /// `addLongExpression`.
    ///
    /// # Errors
    ///
    /// A word wider than a line.
    pub fn add_long_expression(&mut self, expression: &str) -> Result<()> {
        for word in expression.split(&symbol("space")) {
            self.append_or_insert_current(word, true)?;
        }
        Ok(())
    }

    /// A group of notes on a new line, indented two cells or after the
    /// hand's sign: music21's `addToNewLine`.
    ///
    /// # Errors
    ///
    /// A group wider than a line.
    pub fn add_to_new_line(&mut self, grouping: &str) -> Result<()> {
        self.make_new_line();
        if self.right_hand_symbol || self.left_hand_symbol {
            self.optional_add_keyboard_symbols_and_dots(Some(grouping))?;
            self.current_line().append(grouping, false)
        } else {
            self.current_line().insert(2, grouping)
        }
    }

    /// Text at the end of the line, or on a new one indented two cells:
    /// music21's `appendOrInsertCurrent`.
    ///
    /// # Errors
    ///
    /// Text wider than a line.
    pub fn append_or_insert_current(&mut self, text: &str, add_space: bool) -> Result<()> {
        if self.current_line().can_append(text, add_space) {
            self.current_line().append(text, add_space)
        } else {
            self.make_new_line();
            self.current_line().insert(2, text)
        }
    }

    /// Voices written in accord, on the line or a new one: music21's
    /// `addInaccord`.
    ///
    /// # Errors
    ///
    /// Voices wider than a line.
    pub fn add_inaccord(&mut self, inaccord: &str) -> Result<()> {
        let add_space = self.optional_add_keyboard_symbols_and_dots(Some(inaccord))?;
        if self.current_line().append(inaccord, add_space).is_err() {
            self.make_new_line();
            if self.right_hand_symbol || self.left_hand_symbol {
                let sign = symbol(if self.right_hand_symbol {
                    "rh_keyboard"
                } else {
                    "lh_keyboard"
                });
                self.current_line().insert(2, &sign)?;
                if let Some(first) = inaccord.chars().next() {
                    for dot in yield_dots(first) {
                        self.current_line().append(&dot, false)?;
                    }
                }
                self.current_line().append(inaccord, false)?;
            } else {
                self.current_line().insert(2, inaccord)?;
            }
        }
        self.current_line().contains_note_grouping = true;
        Ok(())
    }

    /// A measure number at the start of a line: music21's
    /// `addMeasureNumber`.
    ///
    /// # Errors
    ///
    /// A number wider than a line.
    pub fn add_measure_number(&mut self, number: &str) -> Result<()> {
        if self.current_line().text_location != 0 {
            self.make_new_line();
        }
        self.current_line().append(number, false)
    }

    /// A measure number given as a number.
    ///
    /// # Errors
    ///
    /// A number wider than a line.
    pub fn add_measure_number_value(&mut self, number: i64) -> Result<()> {
        let braille = number_to_braille(&number.to_string(), true, false)?;
        self.add_measure_number(&braille)
    }

    /// The hand's sign and the dots of a group's first cell, before the
    /// first group of a line, and whether what follows is spaced from it:
    /// music21's `optionalAddKeyboardSymbolsAndDots`.
    ///
    /// # Errors
    ///
    /// Signs wider than a line.
    pub fn optional_add_keyboard_symbols_and_dots(
        &mut self,
        grouping: Option<&str>,
    ) -> Result<bool> {
        let mut add_space = true;
        let (right, left) = (self.right_hand_symbol, self.left_hand_symbol);
        let line = self.current_line();
        if !line.contains_note_grouping && (right || left) {
            if line.text_location == 0 {
                add_space = false;
            }
            let sign = symbol(if right { "rh_keyboard" } else { "lh_keyboard" });
            line.append(&sign, add_space)?;
            if let Some(first) = grouping.and_then(|grouping| grouping.chars().next()) {
                for dot in yield_dots(first) {
                    line.append(&dot, false)?;
                }
            }
            add_space = false;
        }
        if line.text_location == 0 {
            add_space = false;
        }
        Ok(add_space)
    }

    /// Key and time signatures, after a space unless they start the line:
    /// music21's `addSignatures`.
    ///
    /// # Errors
    ///
    /// Signatures wider than a line.
    pub fn add_signatures(&mut self, signatures: &str) -> Result<()> {
        let add_space = self.current_line().text_location != 0;
        self.append_or_insert_current(signatures, add_space)
    }

    /// Each heading line centred over the widest line of music below it,
    /// up to the next heading: music21's `recenterHeadings`.
    pub fn recenter_headings(&mut self) {
        let space = symbol("space");
        for (start, end) in self.all_headings.clone() {
            let mut widest = 0;
            for line in &self.all_lines[end..] {
                if line.is_heading {
                    break;
                }
                widest = widest.max(line.text_location);
            }
            for line in &mut self.all_lines[start..end] {
                let text = line.text();
                let trimmed = text.trim_matches(|cell: char| cell.to_string() == space);
                if widest > trimmed.chars().count() {
                    let centred = center(trimmed, widest, &space);
                    // A centred line fits where its widest line does.
                    let _ = line.insert(0, &centred);
                    line.text_location = widest;
                }
            }
        }
    }

    /// The text, headings centred: music21's `__str__`.
    pub fn render(&mut self) -> String {
        self.recenter_headings();
        self.all_lines
            .iter()
            .map(BrailleTextLine::text)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Braille for the two hands at a keyboard, side by side in lines of each:
/// music21's `BrailleKeyboard`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrailleKeyboard {
    /// The text written so far.
    pub text: BrailleText,
    /// Which of the text's lines the right hand is on.
    pub right_hand_line: Option<usize>,
    /// Which of the text's lines the left hand is on.
    pub left_hand_line: Option<usize>,
    /// How many digits the highest measure number has, so the numbers line
    /// up.
    pub highest_measure_number_length: usize,
}

impl BrailleKeyboard {
    /// A keyboard's text of lines this long.
    pub fn new(line_length: usize) -> Self {
        Self {
            text: BrailleText::new(line_length, None).expect("no hand is always allowed"),
            right_hand_line: None,
            left_hand_line: None,
            highest_measure_number_length: 0,
        }
    }

    /// A line for each hand, the right one on the current line if it is
    /// empty: music21's `makeNewLines`.
    pub fn make_new_lines(&mut self) {
        let length = self.text.line_length;
        if self.text.current_line().text_location == 0 {
            self.right_hand_line = Some(self.text.all_lines.len() - 1);
        } else {
            self.text.all_lines.push(BrailleTextLine::new(length));
            self.right_hand_line = Some(self.text.all_lines.len() - 1);
        }
        self.text.all_lines.push(BrailleTextLine::new(length));
        self.left_hand_line = Some(self.text.all_lines.len() - 1);
    }

    fn lines(&mut self) -> (&mut BrailleTextLine, &mut BrailleTextLine) {
        let right = self.right_hand_line.expect("the hands have lines");
        let left = self.left_hand_line.expect("the hands have lines");
        assert!(right < left, "the right hand's line is above the left's");
        let (above, below) = self.text.all_lines.split_at_mut(left);
        (&mut above[right], &mut below[0])
    }

    /// A measure's groups for each hand, on their lines or new ones, each
    /// line starting with the measure number and the hand's sign: music21's
    /// `addNoteGroupings`.
    ///
    /// # Errors
    ///
    /// Groups wider than a line.
    pub fn add_note_groupings(
        &mut self,
        measure_number: &str,
        right: &str,
        left: &str,
    ) -> Result<()> {
        if self.right_hand_line.is_none() && self.left_hand_line.is_none() {
            self.make_new_lines();
        }
        let indent = self
            .highest_measure_number_length
            .saturating_sub(measure_number.chars().count());
        let (right_line, left_line) = self.lines();
        if right_line.text_location == 0 {
            right_line.insert(indent, measure_number)?;
            left_line.text_location = right_line.text_location;
        }
        let mut add_space = true;
        if !right_line.contains_note_grouping {
            add_space = false;
            right_line.append(&symbol("rh_keyboard"), true)?;
            left_line.append(&symbol("lh_keyboard"), true)?;
            if let Some(first) = right.chars().next() {
                for dot in yield_dots(first) {
                    right_line.append(&dot, false)?;
                }
            }
            if let Some(first) = left.chars().next() {
                for dot in yield_dots(first) {
                    left_line.append(&dot, false)?;
                }
            }
        }
        if right_line.can_append(right, add_space) && left_line.can_append(left, add_space) {
            if !left.is_empty() {
                left_line.append(left, add_space)?;
            }
            if !right.is_empty() {
                right_line.append(right, add_space)?;
            }
        } else {
            self.make_new_lines();
            let (right_line, left_line) = self.lines();
            right_line.insert(indent, measure_number)?;
            left_line.text_location = right_line.text_location;
            right_line.append(&symbol("rh_keyboard"), true)?;
            left_line.append(&symbol("lh_keyboard"), true)?;
            if let Some(first) = right.chars().next() {
                for dot in yield_dots(first) {
                    right_line.append(&dot, false)?;
                }
                right_line.append(right, false)?;
            }
            if let Some(first) = left.chars().next() {
                for dot in yield_dots(first) {
                    left_line.append(&dot, false)?;
                }
                left_line.append(left, false)?;
            }
        }
        let (right_line, left_line) = self.lines();
        if right_line.text_location > left_line.text_location {
            left_line.text_location = right_line.text_location;
        } else {
            right_line.text_location = left_line.text_location;
        }
        right_line.contains_note_grouping = true;
        left_line.contains_note_grouping = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_take_what_fits_and_headings_are_centred() -> Result<()> {
        let mut text = BrailleText::new(10, None)?;
        text.add_heading("⠁⠃")?;
        text.add_measure_number("⠼⠁")?;
        text.append_or_insert_current("⠉⠙⠑⠋", true)?;
        text.append_or_insert_current("⠛⠓⠊⠚", true)?;
        assert_eq!(text.render(), "⠀⠀⠀⠁⠃⠀⠀\n⠼⠁⠀⠉⠙⠑⠋\n⠀⠀⠛⠓⠊⠚");
        Ok(())
    }
}
