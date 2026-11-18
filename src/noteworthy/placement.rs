//! Where music21's NoteWorthy translator places notes, rests and marks in
//! a measure, and how it joins slurs and ties: the one implementation,
//! generic over what the measures are.
//!
//! The crate's reader places into measures of its own; the wheel's
//! translator places into Python streams a caller may hand it. Both
//! implement [`Placing`], and [`Placement`] keeps the state between lines:
//! the position in the measure, the slurs left open and the ties begun.

use crate::defaults::{FloatType, IntegerType};
use crate::notation::TieType;

/// What a measure of either kind does for the translator.
pub trait Placing {
    /// A measure, compared by identity.
    type Measure;
    /// A voice of a measure.
    type Voice;
    /// A note, chord or rest about to be placed.
    type Element;
    /// What a placed note is remembered by, for a slur to reach it.
    type Handle;
    /// What goes wrong.
    type Error;

    /// Another reference to the same measure.
    fn copy_measure(&self, measure: &Self::Measure) -> Self::Measure;
    /// Another reference to the same placed note.
    fn copy_handle(&self, handle: &Self::Handle) -> Self::Handle;
    /// Whether two measures are the same one.
    fn same_measure(&self, first: &Self::Measure, second: &Self::Measure) -> bool;
    /// Whether two placed notes are the same one.
    fn same_handle(&self, first: &Self::Handle, second: &Self::Handle) -> bool;
    /// What `element` will be remembered by.
    fn handle(&self, element: &Self::Element) -> Result<Self::Handle, Self::Error>;
    /// Where the last thing in the measure ends: music21's `highestTime`.
    fn highest_time(&self, measure: &Self::Measure) -> Result<FloatType, Self::Error>;
    /// How long an element lasts.
    fn length(&self, element: &Self::Element) -> Result<FloatType, Self::Error>;
    /// The measure's voices, in order.
    fn voices(&self, measure: &Self::Measure) -> Result<Vec<Self::Voice>, Self::Error>;
    /// Where the last thing in a voice ends.
    fn voice_end(&self, voice: &Self::Voice) -> Result<FloatType, Self::Error>;
    /// The notes, chords and rests the measure holds moved into a voice
    /// numbered nought, each where it stood, and the voice put at the
    /// start of the measure.
    fn gather_voice(&mut self, measure: &Self::Measure) -> Result<Self::Voice, Self::Error>;
    /// A new empty voice numbered `id` at the start of the measure.
    fn new_voice(&mut self, measure: &Self::Measure, id: usize)
    -> Result<Self::Voice, Self::Error>;
    /// An element put in the measure at an offset.
    fn insert(
        &mut self,
        measure: &Self::Measure,
        offset: FloatType,
        element: Self::Element,
    ) -> Result<(), Self::Error>;
    /// An element put in a voice at an offset.
    fn insert_in(
        &mut self,
        voice: &Self::Voice,
        offset: FloatType,
        element: Self::Element,
    ) -> Result<(), Self::Error>;
    /// The ties of an element's notes, in order, set where given.
    fn set_ties(
        &mut self,
        element: &mut Self::Element,
        ties: &[Option<TieType>],
    ) -> Result<(), Self::Error>;
    /// A slur from one placed note to another, kept in the measure.
    fn slur(
        &mut self,
        measure: &Self::Measure,
        start: &Self::Handle,
        end: &Self::Handle,
    ) -> Result<(), Self::Error>;
}

/// A slur begun and not yet joined: where it starts, the measure it
/// starts in, where that note ends and its place in its group.
#[derive(Clone, Debug)]
pub struct OpenSlur<M, H> {
    /// The note it starts at.
    pub start: H,
    /// The measure that note is in.
    pub measure: M,
    /// Where that note ends.
    pub end: FloatType,
    /// Its place among the notes placed with it.
    pub group: usize,
}

/// A tie begun on a staff step: the measure it was begun in, none where
/// carried over a barline, and where the tied note ends.
#[derive(Clone, Debug)]
pub struct TiedStep<M> {
    /// The staff step: a step and octave under the clef in force.
    pub step: (char, IntegerType),
    /// The measure, where not carried over a barline.
    pub measure: Option<M>,
    /// Where the tied note ends.
    pub end: FloatType,
}

/// The translator's placing state: music21's `positionMeasure` and
/// `measurePosition`, `openSlurs` and `tiedSteps`.
#[derive(Clone, Debug)]
pub struct Placement<M, H> {
    /// The measure placed in last, and where the next object goes in it.
    pub position: Option<(M, FloatType)>,
    /// Slurs begun and not yet joined.
    pub open_slurs: Vec<OpenSlur<M, H>>,
    /// Ties begun.
    pub tied_steps: Vec<TiedStep<M>>,
}

impl<M, H> Default for Placement<M, H> {
    fn default() -> Self {
        Self {
            position: None,
            open_slurs: Vec::new(),
            tied_steps: Vec::new(),
        }
    }
}

impl<M, H> Placement<M, H> {
    /// music21's `currentPosition`: the end of what the measure holds the
    /// first time it is asked in a measure, then where the notes placed
    /// last moved it on to.
    ///
    /// # Errors
    ///
    /// What the measure's `highest_time` raises.
    pub fn position<T>(&mut self, music: &T, measure: &M) -> Result<FloatType, T::Error>
    where
        T: Placing<Measure = M, Handle = H>,
    {
        if let Some((placed, offset)) = &self.position
            && music.same_measure(placed, measure)
        {
            return Ok(*offset);
        }
        let offset = music.highest_time(measure)?;
        self.position = Some((music.copy_measure(measure), offset));
        Ok(offset)
    }

    /// music21's `insertAtPosition`: a mark where the next note goes.
    ///
    /// # Errors
    ///
    /// What placing it raises.
    pub fn mark<T>(
        &mut self,
        music: &mut T,
        measure: &M,
        element: T::Element,
    ) -> Result<(), T::Error>
    where
        T: Placing<Measure = M, Handle = H>,
    {
        let offset = self.position(music, measure)?;
        music.insert(measure, offset, element)
    }

    /// music21's `setTieFromPitchInfo`: the ties of an element about to go
    /// at the current position, from the staff step of each of its notes
    /// and whether the note is tied on. A note is tied from one on the
    /// same step that ends where it starts.
    ///
    /// # Errors
    ///
    /// What the measure or the element raises.
    pub fn tie<T>(
        &mut self,
        music: &mut T,
        measure: &M,
        element: &mut T::Element,
        steps: &[((char, IntegerType), bool)],
    ) -> Result<(), T::Error>
    where
        T: Placing<Measure = M, Handle = H>,
    {
        let offset = self.position(music, measure)?;
        let end = offset + music.length(element)?;
        let mut ties = Vec::with_capacity(steps.len());
        for (step, tied_on) in steps {
            let mut tied_from = false;
            if let Some(at) = self.tied_steps.iter().position(|tied| tied.step == *step) {
                let tied = self.tied_steps.remove(at);
                let here = tied
                    .measure
                    .as_ref()
                    .is_none_or(|tied_measure| music.same_measure(tied_measure, measure));
                tied_from = here && tied.end == offset;
            }
            ties.push(match (tied_from, *tied_on) {
                (true, true) => Some(TieType::Continue),
                (false, true) => Some(TieType::Start),
                (true, false) => Some(TieType::Stop),
                (false, false) => None,
            });
            if *tied_on {
                self.tied_steps.push(TiedStep {
                    step: *step,
                    measure: Some(music.copy_measure(measure)),
                    end,
                });
            }
        }
        music.set_ties(element, &ties)
    }

    /// music21's `appendSimultaneous`: notes, chords or rests, each with
    /// whether it is slurred, starting together at the current position,
    /// which then moves on by the shortest of them. Where several start
    /// together, or the measure has voices, each goes in a voice free
    /// there, the longest of them first, or a new one; the notes the
    /// measure holds become its first voice where it has none.
    ///
    /// # Errors
    ///
    /// What the measure raises.
    pub fn place<T>(
        &mut self,
        music: &mut T,
        measure: &M,
        elements: Vec<(T::Element, bool)>,
    ) -> Result<(), T::Error>
    where
        T: Placing<Measure = M, Handle = H>,
    {
        let offset = self.position(music, measure)?;
        let mut voices = music.voices(measure)?;
        if voices.is_empty() && elements.len() > 1 {
            voices.push(music.gather_voice(measure)?);
        }

        // The voices free here, the one ending latest first.
        let mut free = Vec::new();
        for (index, voice) in voices.iter().enumerate() {
            let end = music.voice_end(voice)?;
            if end <= offset {
                free.push((end, index));
            }
        }
        free.sort_by(|left, right| right.0.total_cmp(&left.0));
        let mut free: std::collections::VecDeque<usize> =
            free.into_iter().map(|(_, index)| index).collect();

        let had_voices = !voices.is_empty();
        let mut group = Vec::with_capacity(elements.len());
        for (element, slurred) in elements {
            group.push((music.handle(&element)?, slurred, music.length(&element)?));
            if !had_voices {
                music.insert(measure, offset, element)?;
                continue;
            }
            let index = match free.pop_front() {
                Some(index) => index,
                None => {
                    let id = voices.len();
                    voices.push(music.new_voice(measure, id)?);
                    id
                }
            };
            music.insert_in(&voices[index], offset, element)?;
        }

        self.slurs(music, measure, &group)?;
        let shortest = group
            .iter()
            .map(|(_, _, length)| *length)
            .fold(FloatType::INFINITY, FloatType::min);
        if shortest.is_finite() {
            self.position = Some((music.copy_measure(measure), offset + shortest));
        }
        Ok(())
    }

    /// music21's `setSlurs`, for notes just placed together at the current
    /// position, each with whether it is slurred and how long it lasts: a
    /// slurred note is slurred to the next note starting once it has
    /// ended, the one in the same place in its group if there is one, else
    /// the first slurred one, else the first. Slurred notes in a row make
    /// one slur.
    ///
    /// # Errors
    ///
    /// What the measure raises.
    pub fn slurs<T>(
        &mut self,
        music: &mut T,
        measure: &M,
        group: &[(H, bool, FloatType)],
    ) -> Result<(), T::Error>
    where
        T: Placing<Measure = M, Handle = H>,
    {
        let offset = self.position(music, measure)?;
        let ended: Vec<OpenSlur<M, H>> = self
            .open_slurs
            .iter()
            .filter(|open| !music.same_measure(&open.measure, measure) || open.end <= offset)
            .map(|open| OpenSlur {
                start: music.copy_handle(&open.start),
                measure: music.copy_measure(&open.measure),
                end: open.end,
                group: open.group,
            })
            .collect();
        let first_slurred = group
            .iter()
            .position(|(_, slurred, _)| *slurred)
            .unwrap_or(0);
        for (index, (handle, slurred, length)) in group.iter().enumerate() {
            let mut arriving: Vec<&OpenSlur<M, H>> = ended
                .iter()
                .filter(|open| {
                    let lands = if open.group < group.len() {
                        open.group
                    } else {
                        first_slurred
                    };
                    lands == index
                })
                .collect();
            let end = offset + length;
            if *slurred && !arriving.is_empty() {
                let start = music.copy_handle(&arriving.remove(0).start);
                self.forget(music, &start);
                self.open_slurs.push(OpenSlur {
                    start,
                    measure: music.copy_measure(measure),
                    end,
                    group: index,
                });
            } else if *slurred {
                self.open_slurs.push(OpenSlur {
                    start: music.copy_handle(handle),
                    measure: music.copy_measure(measure),
                    end,
                    group: index,
                });
            }
            for open in arriving {
                music.slur(measure, &open.start, handle)?;
                self.forget(music, &open.start);
            }
        }
        Ok(())
    }

    /// Every open slur begun at `start` dropped.
    fn forget<T>(&mut self, music: &T, start: &H)
    where
        T: Placing<Measure = M, Handle = H>,
    {
        self.open_slurs
            .retain(|open| !music.same_handle(&open.start, start));
    }

    /// music21's `createBarlines` for ties: a tie running to the end of
    /// the measure goes on into the next; any other is dropped.
    ///
    /// # Errors
    ///
    /// What the measure's `highest_time` raises.
    pub fn barline<T>(&mut self, music: &T, measure: &M) -> Result<(), T::Error>
    where
        T: Placing<Measure = M, Handle = H>,
    {
        let end = music.highest_time(measure)?;
        self.tied_steps.retain(|tied| {
            tied.measure
                .as_ref()
                .is_some_and(|tied_measure| music.same_measure(tied_measure, measure))
                && tied.end == end
        });
        for tied in &mut self.tied_steps {
            tied.measure = None;
            tied.end = 0.0;
        }
        Ok(())
    }

    /// music21's `createStaff`: nothing left open from the staff before.
    pub fn new_staff(&mut self) {
        self.open_slurs.clear();
        self.tied_steps.clear();
    }
}
