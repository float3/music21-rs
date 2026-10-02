//! Which notes MuseScore beams together.
//!
//! A MuseScore file says how a note is beamed only where that differs from
//! what the meter would do: most notes carry no beam mode at all, and the
//! program works the beams out when it lays the page out. This is that
//! working out -- the beam groups of each meter, the rules that break a
//! beam at a beat whose notes are shorter, at a gap, at a rest -- and then
//! the beams each note carries at each level.

use crate::notation::{Beam, BeamDirection, BeamType, Beams};

/// How a note says it is beamed: MuseScore's `BeamMode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BeamMode {
    /// As the meter beams it.
    Auto,
    /// A beam starts here.
    Begin,
    /// The beam carries on through here.
    Mid,
    /// The beam ends here.
    End,
    /// Not beamed.
    None,
    /// The beam carries on, the beams below the first starting afresh.
    Begin16,
    /// The beam carries on, the beams below the second starting afresh.
    Begin32,
}

impl BeamMode {
    pub(super) fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "auto" => Self::Auto,
            "begin" => Self::Begin,
            "mid" => Self::Mid,
            "end" => Self::End,
            "no" => Self::None,
            "begin16" => Self::Begin16,
            "begin32" => Self::Begin32,
            _ => return None,
        })
    }

    /// Whether the mode carries a beam on: MuseScore's `beamModeMid`.
    fn is_mid(self) -> bool {
        matches!(self, Self::Mid | Self::Begin16 | Self::Begin32)
    }
}

/// MuseScore's ticks to a whole note.
pub(super) const WHOLE: i64 = 1920;
/// MuseScore's ticks to a quarter.
const QUARTER: i64 = 480;

/// A written value as MuseScore orders them, longest first; a larger number
/// is a shorter value. Nothing written is shorter than anything.
pub(super) type Value = (u8, u32);

/// The value MuseScore compares as shorter than all the others.
const INVALID: Value = (u8::MAX, 0);

/// The number of flags on a written value: MuseScore's `hooks`.
pub(super) fn hooks(value: Value) -> u32 {
    match value.0 {
        // An eighth is 5: long, breve, whole, half and quarter come before.
        5..=13 => u32::from(value.0) - 4,
        _ => 0,
    }
}

/// The ordinal of the eighth, the sixteenth and the thirty-second.
const EIGHTH: u8 = 5;
const SIXTEENTH: u8 = 6;
const THIRTY_SECOND: u8 = 7;

/// A value from its length in ticks: MuseScore's `TDuration(Fraction)`,
/// which is only asked here of a plain power of two.
fn value_of_ticks(ticks: i64) -> Value {
    let mut length = 4 * WHOLE;
    for ordinal in 0..=13u8 {
        if length == ticks {
            return (ordinal, 0);
        }
        length /= 2;
    }
    INVALID
}

/// One node of a meter's beam groups: where it stands, in thirty-seconds,
/// and what it does to eighths, sixteenths and shorter, a nibble each.
#[derive(Clone, Copy, Debug)]
pub(super) struct Node {
    pub(super) position: i64,
    pub(super) action: u32,
}

/// The beam groups of a meter: MuseScore's `Groups::endings`.
pub(super) fn default_groups(numerator: i64, denominator: i64) -> Vec<Node> {
    let table: &[(i64, u32)] = match (numerator, denominator) {
        (2, 2) | (4, 4) => &[
            (4, 0x200),
            (8, 0x110),
            (12, 0x200),
            (16, 0x111),
            (20, 0x200),
            (24, 0x110),
            (28, 0x200),
        ],
        (3, 2) => &[
            (4, 0x200),
            (8, 0x110),
            (12, 0x200),
            (16, 0x111),
            (20, 0x200),
            (24, 0x110),
            (28, 0x200),
            (32, 0x111),
            (36, 0x200),
            (40, 0x110),
            (44, 0x200),
        ],
        (4, 2) => &[
            (4, 0x200),
            (8, 0x110),
            (12, 0x200),
            (16, 0x111),
            (20, 0x200),
            (24, 0x110),
            (28, 0x200),
            (32, 0x111),
            (36, 0x200),
            (40, 0x110),
            (44, 0x200),
            (48, 0x111),
            (52, 0x200),
            (56, 0x110),
            (60, 0x200),
        ],
        (2, 4) => &[(4, 0x200), (8, 0x111), (12, 0x200)],
        (3, 4) => &[
            (4, 0x200),
            (8, 0x111),
            (12, 0x200),
            (16, 0x111),
            (20, 0x200),
        ],
        (5, 4) => &[
            (4, 0x200),
            (8, 0x110),
            (12, 0x200),
            (16, 0x110),
            (20, 0x200),
            (24, 0x111),
            (28, 0x200),
            (32, 0x110),
            (36, 0x200),
        ],
        (6, 4) => &[
            (4, 0x200),
            (8, 0x110),
            (12, 0x200),
            (16, 0x110),
            (20, 0x200),
            (24, 0x111),
            (28, 0x200),
            (32, 0x110),
            (36, 0x200),
            (40, 0x110),
            (44, 0x200),
        ],
        (3, 8) => &[(4, 0x200), (8, 0x200)],
        (5, 8) => &[(4, 0x200), (8, 0x200), (12, 0x111), (16, 0x200)],
        (6, 8) => &[
            (4, 0x200),
            (8, 0x200),
            (12, 0x111),
            (16, 0x200),
            (20, 0x200),
        ],
        (7, 8) => &[
            (4, 0x200),
            (8, 0x200),
            (12, 0x111),
            (16, 0x200),
            (20, 0x111),
            (24, 0x200),
        ],
        (9, 8) => &[
            (4, 0x200),
            (8, 0x200),
            (12, 0x111),
            (16, 0x200),
            (20, 0x200),
            (24, 0x111),
            (28, 0x200),
            (32, 0x200),
        ],
        (12, 8) => &[
            (4, 0x200),
            (8, 0x200),
            (12, 0x111),
            (16, 0x200),
            (20, 0x200),
            (24, 0x111),
            (28, 0x200),
            (32, 0x200),
            (36, 0x111),
            (40, 0x200),
            (44, 0x200),
        ],
        _ => {
            // Any other meter breaks every beam on every beat.
            let step = match denominator {
                2 => 16,
                4 => 8,
                8 => 4,
                16 => 2,
                32 => 1,
                _ => 0,
            };
            return (1..numerator)
                .map(|index| Node {
                    position: step * index,
                    action: 0x111,
                })
                .collect();
        }
    };
    table
        .iter()
        .map(|&(position, action)| Node { position, action })
        .collect()
}

/// What the groups say of a value at a tick: MuseScore's `Groups::beamMode`.
fn group_mode(groups: &[Node], tick: i64, value: Value) -> BeamMode {
    let shift = match value.0 {
        EIGHTH => 0,
        SIXTEENTH => 4,
        7..=13 => 8,
        _ => return BeamMode::Auto,
    };
    let unit = QUARTER / 8;
    for node in groups {
        if node.position * unit < tick {
            continue;
        }
        if node.position * unit > tick {
            break;
        }
        return match (node.action >> shift) & 0xf {
            1 => BeamMode::Begin,
            2 => BeamMode::Begin16,
            3 => BeamMode::Begin32,
            _ => BeamMode::Auto,
        };
    }
    BeamMode::Auto
}

/// A note or rest of one voice of one measure, as the beaming sees it.
#[derive(Clone, Debug)]
pub(super) struct Member {
    /// Where it starts in the measure, in ticks.
    pub(super) tick: i64,
    /// How long it sounds, in ticks.
    pub(super) ticks: i64,
    /// Its written value.
    pub(super) value: Value,
    pub(super) is_rest: bool,
    /// The beam mode the file gives it.
    pub(super) mode: BeamMode,
    /// The innermost tuplet it is in, by any number that tells two apart.
    pub(super) tuplet: Option<usize>,
}

/// What the beaming needs of the measure.
pub(super) struct Context<'a> {
    pub(super) groups: &'a [Node],
    pub(super) denominator: i64,
    /// Ticks to add to a position in a pickup, which MuseScore beams as the
    /// end of a full bar.
    pub(super) anacrusis: i64,
    /// The voice, counting from nought.
    pub(super) voice: usize,
}

/// MuseScore's `Groups::baseBeamMode`.
fn base_mode(
    context: &Context<'_>,
    member: &Member,
    value: Value,
    prev: Option<&Member>,
) -> BeamMode {
    if member.mode != BeamMode::Auto {
        return member.mode;
    }
    let longest = member.ticks.max(prev.map_or(0, |prev| prev.ticks));
    let mut smallest = WHOLE / 8;
    let limit = WHOLE / 32;
    while smallest > longest || member.tick % smallest != 0 {
        smallest /= 2;
        if smallest < limit {
            smallest = member.ticks;
            break;
        }
    }
    let big_beat = value_of_ticks(smallest);
    let tick = member.tick + context.anacrusis;
    let by_type = group_mode(context.groups, tick, value);
    let by_position = group_mode(context.groups, tick, big_beat);
    let mut mode = if by_type == BeamMode::Auto {
        by_position
    } else {
        by_type
    };
    if mode == BeamMode::Auto && tick != 0 {
        if let Some(prev) = prev
            && member.tuplet != prev.tuplet
            && value == prev.value
        {
            let shorter = if value.0 <= EIGHTH {
                SIXTEENTH
            } else if value.0 == SIXTEENTH {
                THIRTY_SECOND
            } else {
                8
            };
            mode = group_mode(context.groups, tick, (shorter, 0));
        }
        if context.voice > 0
            && let Some(prev) = prev
            && prev.tuplet.is_none()
            && prev.tick + prev.ticks < member.tick
        {
            mode = BeamMode::Begin;
        }
    }
    mode
}

/// MuseScore's `Groups::actualBeamMode`.
fn actual_mode(
    context: &Context<'_>,
    member: &Member,
    prev: Option<&Member>,
    subdivision: &[(i64, Value)],
) -> BeamMode {
    if member.is_rest && member.mode == BeamMode::Auto {
        return BeamMode::None;
    }
    if !member.is_rest && hooks(member.value) == 0 {
        return BeamMode::None;
    }
    let mut mode = base_mode(context, member, member.value, prev);
    if mode == BeamMode::Auto {
        if member.tick == 0 {
            return BeamMode::Begin;
        }
        if context.denominator == 4 && member.tick % QUARTER == 0 {
            let beat = member.tick / QUARTER;
            let find = |wanted: i64| {
                subdivision
                    .iter()
                    .find(|(held, _)| *held == wanted)
                    .map_or(INVALID, |(_, value)| *value)
            };
            let shortest = find(beat).max(find(beat - 1));
            mode = base_mode(context, member, shortest, prev);
        }
    }
    if mode == BeamMode::Auto {
        BeamMode::Mid
    } else {
        mode
    }
}

/// The beam groups of one voice of one measure: MuseScore's `createBeams`.
/// Each group is the indices of its members, two or more of them, not all
/// rests.
pub(super) fn beam_groups(context: &Context<'_>, members: &[Member]) -> Vec<Vec<usize>> {
    // The shortest value on each beat, which a beat on a quarter-note meter
    // is beamed by.
    let mut subdivision: Vec<(i64, Value)> = Vec::new();
    if context.denominator == 4 {
        for member in members {
            let beat = member.tick / QUARTER;
            match subdivision.iter_mut().find(|(held, _)| *held == beat) {
                Some((_, value)) => *value = (*value).max(member.value),
                None => subdivision.push((beat, member.value)),
            }
        }
    }
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut open: Option<Vec<usize>> = None;
    let mut first: Option<usize> = None;
    let mut prev: Option<usize> = None;
    let close = |open: &mut Option<Vec<usize>>, groups: &mut Vec<Vec<usize>>| {
        if let Some(group) = open.take()
            && group.len() > 1
            && !group.iter().all(|index| members[*index].is_rest)
        {
            groups.push(group);
        }
    };
    for (index, member) in members.iter().enumerate() {
        let mode = actual_mode(
            context,
            member,
            prev.map(|prev| &members[prev]),
            &subdivision,
        );
        prev = Some(index);
        let can_be_beamed = member.value.0 > 4 || member.is_rest;
        if !can_be_beamed || mode == BeamMode::None {
            close(&mut open, &mut groups);
            first = None;
            continue;
        }
        if let Some(group) = &mut open {
            if mode == BeamMode::Begin {
                close(&mut open, &mut groups);
            } else {
                group.push(index);
                if mode == BeamMode::End {
                    close(&mut open, &mut groups);
                }
                continue;
            }
        }
        match first {
            None => first = Some(index),
            Some(start) => {
                let gap = members[start].tick + members[start].ticks < member.tick;
                if !mode.is_mid() && (mode == BeamMode::Begin || gap) {
                    first = Some(index);
                } else {
                    // A beam just made is not ended by the mode of its second
                    // note, only by what comes after.
                    open = Some(vec![start, index]);
                    first = None;
                }
            }
        }
    }
    close(&mut open, &mut groups);
    groups
}

/// The longest plain value no longer than so many ticks: MuseScore's
/// `TDuration(Fraction)`, of which only the flags are asked.
fn value_within(ticks: i64) -> Value {
    let mut length = 4 * WHOLE;
    for ordinal in 0..=13u8 {
        if length <= ticks {
            return (ordinal, 0);
        }
        length /= 2;
    }
    INVALID
}

/// Whether the beams below the first break at a note: MuseScore's
/// `Beam::calcBeamBreaks`, answering for the sixteenth beam and for the
/// thirty-second one.
fn beam_breaks(
    context: &Context<'_>,
    members: &[Member],
    at: usize,
    prev: Option<usize>,
    level: u32,
    tuplet_ticks: &dyn Fn(usize) -> i64,
) -> (bool, bool) {
    let member = &members[at];
    if member.is_rest && member.mode.is_mid() {
        return match member.mode {
            BeamMode::Begin16 => (level > 0, false),
            BeamMode::Begin32 => (false, level > 1),
            _ => (false, false),
        };
    }
    let prev = prev.map(|prev| &members[prev]);
    let default = base_mode(context, member, member.value, prev);
    let said =
        |mode: BeamMode| member.mode == mode || (member.mode == BeamMode::Auto && default == mode);
    let mut broken16 = level >= 1 && said(BeamMode::Begin16);
    let mut broken32 = level >= 2 && said(BeamMode::Begin32);
    // A tuplet inside a beam breaks the beams below as its whole length
    // would.
    if level > 0
        && member.mode == BeamMode::Auto
        && let Some(prev) = prev
    {
        let starting = member.tuplet.filter(|_| member.tuplet != prev.tuplet);
        let ended = prev.tuplet.filter(|_| member.tuplet.is_none());
        if let Some(tuplet) = starting.or(ended) {
            let flags = hooks(value_within(tuplet_ticks(tuplet))).max(1);
            if flags <= level {
                broken16 = level == 1;
                broken32 = level >= 2;
            }
        }
    }
    (broken16, broken32)
}

/// A tuplet as the bracket rule sees it.
pub(super) struct TupletShape<'a> {
    /// Every note and rest under it, those of the tuplets inside it too.
    pub(super) notes: &'a [usize],
    /// The notes and rests it holds itself.
    pub(super) own: &'a [usize],
    /// Whether it holds another tuplet.
    pub(super) nests: bool,
}

/// Whether a tuplet that leaves its bracket to MuseScore is drawn with one:
/// MuseScore's `Tuplet::calcHasBracket`. A tuplet whose notes one beam
/// joins, and which that beam sets apart from its neighbours, needs none.
pub(super) fn tuplet_has_bracket(
    context: &Context<'_>,
    members: &[Member],
    groups: &[Vec<usize>],
    shape: &TupletShape<'_>,
    tuplet_ticks: &dyn Fn(usize) -> i64,
) -> bool {
    let (Some(&first), Some(&last)) = (shape.notes.first(), shape.notes.last()) else {
        return true;
    };
    if first == last {
        return false;
    }
    if members[first].is_rest || members[last].is_rest {
        return true;
    }
    let Some(group) = groups.iter().find(|group| group.contains(&first)) else {
        return true;
    };
    if !group.contains(&last) {
        return true;
    }
    let starts = group.first() == Some(&first);
    let ends = group.last() == Some(&last);
    if starts && ends {
        return false;
    }
    if shape.nests {
        return true;
    }
    let flags = hooks(members[first].value);
    for &own in shape.own {
        let member = &members[own];
        if member.is_rest || !group.contains(&own) || hooks(member.value) != flags {
            return true;
        }
    }
    if flags < 1 {
        return true;
    }
    let level = flags - 1;
    let before = first.checked_sub(1).filter(|at| !members[*at].is_rest);
    let (start16, start32) = match before {
        Some(before) => beam_breaks(context, members, first, Some(before), level, tuplet_ticks),
        None => (false, false),
    };
    let start_defines =
        start16 || start32 || starts || before.is_some_and(|at| hooks(members[at].value) < flags);
    let after = Some(last + 1).filter(|at| members.get(*at).is_some_and(|next| !next.is_rest));
    let (end16, end32) = match after {
        Some(after) => beam_breaks(context, members, after, Some(last), level, tuplet_ticks),
        None => (false, false),
    };
    let end_defines =
        end16 || end32 || ends || after.is_some_and(|at| hooks(members[at].value) < flags);
    !(start_defines && end_defines)
}

/// The grace notes before or after one note, beamed among themselves:
/// MuseScore's `beamGraceNotes`.
pub(super) fn grace_groups(members: &[Member]) -> Vec<Vec<usize>> {
    let mut groups = Vec::new();
    let mut run: Vec<usize> = Vec::new();
    for (index, member) in members.iter().enumerate() {
        let mode = member.mode;
        if hooks(member.value) == 0 || mode == BeamMode::None {
            if run.len() > 1 {
                groups.push(std::mem::take(&mut run));
            }
            run.clear();
            continue;
        }
        if mode == BeamMode::Begin && !run.is_empty() {
            if run.len() > 1 {
                groups.push(std::mem::take(&mut run));
            }
            run.clear();
        }
        run.push(index);
        if mode == BeamMode::End {
            if run.len() > 1 {
                groups.push(std::mem::take(&mut run));
            }
            run.clear();
        }
    }
    if run.len() > 1 {
        groups.push(run);
    }
    groups
}

/// The beams each member of a group carries, level by level: what
/// MuseScore's MusicXML export writes of a beam.
pub(super) fn beams_of_group(members: &[&Member]) -> Vec<Beams> {
    let levels: Vec<u32> = members.iter().map(|member| hooks(member.value)).collect();
    let mut out = Vec::new();
    for (index, member) in members.iter().enumerate() {
        let before = index.checked_sub(1).map_or(-1, |at| levels[at] as i64);
        let current = levels[index] as i64;
        let (after, after_mode) = members.get(index + 1).map_or((-1, BeamMode::Auto), |next| {
            (i64::from(hooks(next.value)), next.mode)
        });
        let mode = member.mode;
        let mut beams = Beams::new();
        for level in 1..=current {
            let (beam_type, direction) = if (before < level && after >= level)
                || (mode == BeamMode::Begin16 && level > 1)
                || (mode == BeamMode::Begin32 && level > 2)
            {
                (BeamType::Start, None)
            } else if before < level && after < level {
                if after > 0 {
                    (BeamType::PartialBeam, Some(BeamDirection::Right))
                } else if before > 0 {
                    (BeamType::PartialBeam, Some(BeamDirection::Left))
                } else {
                    continue;
                }
            } else if (before >= level && after < level)
                || (after_mode == BeamMode::Begin16 && level > 1)
                || (after_mode == BeamMode::Begin32 && level > 2)
            {
                (BeamType::Stop, None)
            } else {
                (BeamType::Continue, None)
            };
            let mut beam = Beam::new(beam_type, direction);
            beam.set_number(Some(beams.len() as u32 + 1));
            beams.beams_mut().push(beam);
        }
        out.push(beams);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(tick: i64, ticks: i64, value: u8) -> Member {
        Member {
            tick,
            ticks,
            value: (value, 0),
            is_rest: false,
            mode: BeamMode::Auto,
            tuplet: None,
        }
    }

    #[test]
    fn four_four_beams_eighths_by_the_half_bar() {
        let groups = default_groups(4, 4);
        let context = Context {
            groups: &groups,
            denominator: 4,
            anacrusis: 0,
            voice: 0,
        };
        let members: Vec<Member> = (0..8).map(|index| note(index * 240, 240, EIGHTH)).collect();
        assert_eq!(
            beam_groups(&context, &members),
            [vec![0, 1, 2, 3], vec![4, 5, 6, 7]]
        );
    }

    #[test]
    fn a_rest_breaks_a_beam_and_a_quarter_is_never_beamed() {
        let groups = default_groups(3, 4);
        let context = Context {
            groups: &groups,
            denominator: 4,
            anacrusis: 0,
            voice: 0,
        };
        let mut members = vec![note(0, 240, EIGHTH), note(240, 240, EIGHTH)];
        members.push(Member {
            is_rest: true,
            ..note(480, 240, EIGHTH)
        });
        members.push(note(720, 240, EIGHTH));
        members.push(note(960, 480, 4));
        assert_eq!(beam_groups(&context, &members), [vec![0, 1]]);
    }

    #[test]
    fn a_sixteenth_after_a_dotted_eighth_takes_a_backward_hook() {
        let dotted = Member {
            value: (EIGHTH, 1),
            ..note(0, 360, EIGHTH)
        };
        let short = note(360, 120, SIXTEENTH);
        let beams = beams_of_group(&[&dotted, &short]);
        assert_eq!(beams[0].types(), [Some(BeamType::Start)]);
        assert_eq!(
            beams[1].types(),
            [Some(BeamType::Stop), Some(BeamType::PartialBeam)]
        );
        assert_eq!(
            beams[1].by_number(2).and_then(Beam::direction),
            Some(BeamDirection::Left)
        );
    }
}
