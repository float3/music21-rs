//! Which plot a format and some quantities ask for: music21's
//! `graph.findPlot`.
//!
//! The plots and axes chosen among are described by the caller, each by
//! its graph type and the quantities its axes plot, so that a plot class
//! of the caller's own is chosen as music21 would choose it.

use crate::error::{Error, Result};

/// Each format and the other names a user may give it, the format first.
pub const FORMAT_SYNONYMS: [&[&str]; 7] = [
    &["horizontalbar", "bar", "horizontal", "pianoroll", "piano"],
    &["histogram", "histo", "count"],
    &["scatter", "point"],
    &["scatterweighted", "weightedscatter", "weighted"],
    &["3dbars", "3d"],
    &["colorgrid", "grid", "window", "windowed"],
    &["horizontalbarweighted", "barweighted", "weightedbar"],
];

/// Names that ask for one plot class outright, and the class each names.
pub const PLOT_SHORTCUTS: [(&str, &str); 5] = [
    ("ambitus", "WindowedAmbitus"),
    ("dolan", "Dolan"),
    ("instruments", "Dolan"),
    ("key", "WindowedKey"),
    ("pianoroll", "HorizontalBarPitchSpaceOffset"),
];

/// The format asked for when nothing is.
const DEFAULT_FORMAT: &str = "pianoroll";

/// The axis letters, in order.
const AXIS_LETTERS: [char; 3] = ['x', 'y', 'z'];

/// A format as its first name: music21's `userFormatsToFormat`. An unknown
/// format is lowercased with its spaces taken out.
///
/// ```
/// use music21::graph::find::user_format;
///
/// assert_eq!(user_format("Weighted Scatter"), "scatterweighted");
/// assert_eq!(user_format("3D"), "3dbars");
/// assert_eq!(user_format("4D super chart"), "4dsuperchart");
/// ```
pub fn user_format(format: &str) -> String {
    let format = format.to_lowercase().replace(' ', "");
    for names in FORMAT_SYNONYMS {
        if names.contains(&format.as_str()) {
            return names[0].to_string();
        }
    }
    format
}

/// A plot class chosen among: its graph type and the quantities each of
/// its axes plots, by axis letter.
#[derive(Clone, Debug, PartialEq)]
pub struct PlotCandidate {
    /// music21's `graphType`.
    pub graph_type: String,
    /// Each axis's letter and its class's `quantities`.
    pub axes: Vec<(char, Vec<String>)>,
}

impl PlotCandidate {
    fn axis(&self, letter: char) -> Option<&[String]> {
        self.axes
            .iter()
            .find(|(found, _)| *found == letter)
            .map(|(_, quantities)| quantities.as_slice())
    }
}

/// Whether a value names one of an axis's quantities, ignoring case:
/// music21's `axisMatchesValue`.
///
/// ```
/// use music21::graph::find::axis_matches;
///
/// let quantities = ["count".to_string(), "counting".to_string()];
/// assert!(axis_matches(&quantities, "Counting"));
/// assert!(!axis_matches(&quantities, "offset"));
/// ```
pub fn axis_matches(quantities: &[String], value: &str) -> bool {
    let value = value.to_lowercase();
    quantities
        .iter()
        .any(|quantity| quantity.to_lowercase() == value)
}

/// The plots of a format among `plots`, by index: music21's
/// `getPlotClassesFromFormat`.
pub fn plots_of_format(format: &str, plots: &[PlotCandidate], among: &[usize]) -> Vec<usize> {
    let format = user_format(format).to_lowercase();
    among
        .iter()
        .copied()
        .filter(|index| plots[*index].graph_type.to_lowercase() == format)
        .collect()
}

/// What `plots_to_make` chose.
#[derive(Clone, Debug, PartialEq)]
pub enum PlotChoice {
    /// Plot classes that plot what was asked, by index.
    Plots(Vec<usize>),
    /// The plot class nearest what was asked, and the axis class, by index
    /// in the axes given, to put on each of its axes.
    WithAxes {
        /// The plot class.
        plot: usize,
        /// Each axis letter and its axis class.
        axes: Vec<(char, usize)>,
    },
}

/// The plots to make for a format and up to three quantities: music21's
/// `getPlotsToMake`.
///
/// `plots` is every plot class, `shortcut` the index among them of the
/// class a shortcut format names, where it is one, and `axes` the
/// quantities of every axis class. Where no plot plots the quantities
/// asked, the nearest plot is chosen with an axis class for each.
///
/// ```
/// use music21::graph::find::{PlotCandidate, PlotChoice, plots_to_make};
///
/// let quantities = |names: &[&str]| names.iter().map(|n| n.to_string()).collect::<Vec<_>>();
/// let plots = [PlotCandidate {
///     graph_type: "histogram".to_string(),
///     axes: vec![('x', quantities(&["pitchSpace", "pitch"])), ('y', quantities(&["count"]))],
/// }];
/// let choice = plots_to_make(Some("pitch"), [None, None, None], None, &plots, &[])?;
/// assert_eq!(choice, PlotChoice::Plots(vec![0]));
/// # Ok::<(), music21::Error>(())
/// ```
///
/// # Errors
///
/// Quantities given with no format, or no plot classes to choose from.
pub fn plots_to_make(
    format: Option<&str>,
    values: [Option<&str>; 3],
    shortcut: Option<usize>,
    plots: &[PlotCandidate],
    axes: &[Vec<String>],
) -> Result<PlotChoice> {
    let mut format = format.map(str::to_string);
    let mut values = values.map(|value| value.map(str::to_string));
    if format.is_none() && values.iter().all(Option::is_none) {
        format = Some(DEFAULT_FORMAT.to_string());
    }
    let all: Vec<usize> = (0..plots.len()).collect();

    // The classes the format names; where none, the format is taken as
    // the first quantity and every class is a candidate.
    let mut classes = match (&format, shortcut) {
        (Some(_), Some(index)) => vec![index],
        (Some(named), None) => plots_of_format(named, plots, &all),
        (None, _) => {
            return Err(Error::Value(
                "'NoneType' object has no attribute 'lower'".to_string(),
            ));
        }
    };
    if classes.is_empty() {
        values = [format.take(), values[0].take(), values[1].take()];
        classes = all;
    }
    let format_given = format.is_some();
    let counted = values.iter().filter(|value| value.is_some()).count();
    let best = |among: Vec<usize>| -> Vec<usize> {
        let format = match counted {
            3 => "scatterweighted",
            2 => "scatter",
            1 => "histogram",
            _ => "",
        };
        let inner = plots_of_format(format, plots, &among);
        if inner.is_empty() { among } else { inner }
    };
    let finish = |kept: Vec<usize>| -> Option<PlotChoice> {
        if kept.is_empty() {
            return None;
        }
        if format_given || kept.len() == 1 {
            return Some(PlotChoice::Plots(kept));
        }
        Some(PlotChoice::Plots(best(kept)))
    };

    // Classes whose axis of each letter plots that letter's quantity.
    let kept: Vec<usize> = classes
        .iter()
        .copied()
        .filter(|index| {
            AXIS_LETTERS.iter().zip(&values).all(|(letter, value)| {
                let Some(value) = value else {
                    return true;
                };
                plots[*index]
                    .axis(*letter)
                    .is_some_and(|quantities| axis_matches(quantities, value))
            })
        })
        .collect();
    if let Some(choice) = finish(kept) {
        return Ok(choice);
    }

    // Then classes with some axis plotting each quantity.
    let kept: Vec<usize> = classes
        .iter()
        .copied()
        .filter(|index| {
            values.iter().flatten().all(|value| {
                plots[*index]
                    .axes
                    .iter()
                    .any(|(_, quantities)| axis_matches(quantities, value))
            })
        })
        .collect();
    if let Some(choice) = finish(kept) {
        return Ok(choice);
    }

    // Else the nearest class, with an axis class for each quantity.
    let mut chosen_axes = Vec::new();
    for (letter, value) in AXIS_LETTERS.iter().zip(&values) {
        let Some(value) = value else {
            continue;
        };
        if let Some(axis) = axes
            .iter()
            .position(|quantities| axis_matches(quantities, value))
        {
            chosen_axes.push((*letter, axis));
        }
    }
    let Some(first) = classes.first().copied() else {
        return Err(Error::Value("list index out of range".to_string()));
    };
    let plot = if classes.len() == 1 {
        first
    } else {
        best(classes).first().copied().unwrap_or(first)
    };
    Ok(PlotChoice::WithAxes {
        plot,
        axes: chosen_axes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quantities(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| name.to_string()).collect()
    }

    fn scatter(y: &[&str]) -> PlotCandidate {
        PlotCandidate {
            graph_type: "scatter".to_string(),
            axes: vec![('x', quantities(&["offset"])), ('y', quantities(y))],
        }
    }

    #[test]
    fn quantities_in_the_wrong_order_still_find_a_plot() {
        let plots = [scatter(&["pitchClass", "pc"]), scatter(&["dynamic"])];
        let choice = plots_to_make(
            Some("scatter"),
            [Some("pc"), Some("offset"), None],
            None,
            &plots,
            &[],
        )
        .unwrap();
        assert_eq!(choice, PlotChoice::Plots(vec![0]));
    }

    #[test]
    fn an_unknown_pairing_gets_axes_of_its_own() {
        let plots = [scatter(&["pitchClass"])];
        let axes = [quantities(&["offset"]), quantities(&["count"])];
        let choice = plots_to_make(
            Some("scatter"),
            [Some("offset"), Some("count"), None],
            None,
            &plots,
            &axes,
        )
        .unwrap();
        assert_eq!(
            choice,
            PlotChoice::WithAxes {
                plot: 0,
                axes: vec![('x', 0), ('y', 1)]
            }
        );
    }
}
