//! music21's `graph.primitives`: graphs of data already extracted, drawn
//! with matplotlib.
//!
//! Each class keeps music21's attributes and keyword configurables, and
//! draws with the matplotlib calls music21 makes, in its order, so a
//! figure here is music21's figure. The numbers a graph lays itself out by
//! -- ticks for a count, a weighted point's size, where a weighted bar
//! sits -- are the crate's `graph::primitives`.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::IntoPyObjectExt;
use pyo3::exceptions::{PyKeyError, PyValueError};
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyList, PyModule, PyTuple};

use music21::graph::primitives::{
    Channel, bar_x_ticks, integer_ticks, weighted_bar, weighted_points,
};

use super::utilities::{GraphException, accidentalLabelToUnicode, getColor, pyplot};
use super::{ClassSpec, Property, given, keywords_of, kwargs, music21_name, optional, proto};

/// The module music21 keeps these in.
pub(crate) const MODULE: &str = "music21.graph.primitives";

static BUILT: PyOnceLock<Py<PyModule>> = PyOnceLock::new();

/// What every graph takes as a keyword.
const KEYWORD_CONFIGURABLES: [&str; 21] = [
    "alpha",
    "colorBackgroundData",
    "colorBackgroundFigure",
    "colorGrid",
    "colors",
    "doneAction",
    "dpi",
    "figureSize",
    "fontFamily",
    "hideXGrid",
    "hideYGrid",
    "labelFontSize",
    "marker",
    "markersize",
    "tickColors",
    "tickFontSize",
    "title",
    "titleFontSize",
    "xTickLabelHorizontalAlignment",
    "xTickLabelRotation",
    "xTickLabelVerticalAlignment",
];

/// The colours a graph cycles through, all slate-like.
const DEFAULT_COLORS: [&str; 6] = [
    "#605c7f", "#5c7f60", "#988969", "#628297", "#ad776d", "#80a364",
];

/// What a graph does once drawn.
const DONE_ACTIONS: [&str; 2] = ["show", "write"];

/// The resolution a graph is written at when none is set.
const DEFAULT_DPI: i64 = 300;

/// The ticks `setIntegerTicksFromData` makes by default.
const DEFAULT_DATA_STEPS: i64 = 8;

/// A graph's bars of a weighted graph default to a channel this high.
const BAR_SPACE: i64 = 8;

/// The rows of a colour grid past which they touch.
const GRID_ROWS_SPACED: usize = 12;

fn figure_of<'py>(slf: &Bound<'py, PyAny>, doing: &str) -> PyResult<Bound<'py, PyAny>> {
    optional(slf, "figure")?.ok_or_else(|| {
        PyValueError::new_err(format!(
            "Cannot {doing} without a figure; call process() first"
        ))
    })
}

fn data_of<'py>(slf: &Bound<'py, PyAny>, doing: &str) -> PyResult<Bound<'py, PyAny>> {
    optional(slf, "data")?.ok_or_else(|| {
        PyValueError::new_err(format!(
            "Cannot {doing} without data; set self.data before calling process()"
        ))
    })
}

/// The smallest and largest of some values, as Python's `min` and `max`.
fn min_max<'py>(values: &Bound<'py, PyList>) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    let builtins = values.py().import("builtins")?;
    Ok((
        builtins.call_method1("min", (values,))?,
        builtins.call_method1("max", (values,))?,
    ))
}

// ---------------------------------------------------------------- Graph

/// Sets every attribute a graph starts with, then those `keywords` give
/// of the class's `keywordConfigurables`.
#[pyfunction]
#[pyo3(signature = (slf, **keywords))]
fn graph_init(slf: &Bound<'_, PyAny>, keywords: Option<&Bound<'_, PyDict>>) -> PyResult<()> {
    let py = slf.py();
    slf.setattr("plt", pyplot(py)?)?;
    slf.setattr("data", py.None())?;
    slf.setattr("figure", py.None())?;
    slf.setattr("subplot", py.None())?;

    // An empty range and no range set, for each axis.
    let axis = PyDict::new(py);
    let range_set = PyDict::new(py);
    for key in slf.getattr("axisKeys")?.try_iter()? {
        let key = key?;
        axis.set_item(&key, kwargs!(py; "range" => py.None()))?;
        range_set.set_item(&key, false)?;
    }
    slf.setattr("axis", axis)?;
    slf.setattr("grid", true)?;
    slf.setattr("axisRangeHasBeenSet", range_set)?;

    // How it looks.
    slf.setattr("alpha", 0.2)?;
    slf.setattr("dpi", py.None())?;
    slf.setattr("colorBackgroundData", "#ffffff")?;
    slf.setattr("colorBackgroundFigure", "#ffffff")?;
    slf.setattr("colorGrid", "#dddddd")?;
    slf.setattr("title", "Music21 Graph")?;
    slf.setattr("figureSize", slf.getattr("figureSizeDefault")?)?;
    slf.setattr("marker", "o")?;
    slf.setattr("markersize", 6)?;
    slf.setattr("colors", PyList::new(py, DEFAULT_COLORS)?)?;
    slf.setattr("tickFontSize", 7)?;
    slf.setattr(
        "tickColors",
        kwargs!(py; "x" => "#000000", "y" => "#000000"),
    )?;
    slf.setattr("titleFontSize", 12)?;
    slf.setattr("labelFontSize", 10)?;
    slf.setattr("fontFamily", "serif")?;
    slf.setattr("hideXGrid", false)?;
    slf.setattr("hideYGrid", false)?;
    slf.setattr("xTickLabelRotation", 0)?;
    slf.setattr("xTickLabelHorizontalAlignment", "center")?;
    slf.setattr("xTickLabelVerticalAlignment", "center")?;
    slf.setattr("hideLeftBottomSpines", false)?;
    slf.setattr("_doneAction", "write")?;
    slf.setattr("_dataColorIndex", 0)?;

    // Then what was asked for.
    let Some(keywords) = keywords else {
        return Ok(());
    };
    for name in slf.getattr("keywordConfigurables")?.try_iter()? {
        let name = name?;
        if let Some(value) = keywords.get_item(&name)? {
            slf.setattr(name.cast::<pyo3::types::PyString>()?, value)?;
        }
    }
    Ok(())
}

/// Closes the matplotlib figure, which matplotlib keeps open otherwise.
#[pyfunction]
#[pyo3(name = "__del__")]
fn graph_del(slf: &Bound<'_, PyAny>) -> PyResult<()> {
    let Some(figure) = optional(slf, "figure")? else {
        return Ok(());
    };
    if let Some(plt) = optional(slf, "plt")? {
        plt.call_method1("close", (figure,))?;
    }
    Ok(())
}

/// The graph's state without `plt`, which cannot be copied.
#[pyfunction]
#[pyo3(name = "__getstate__")]
fn graph_getstate<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    let state = slf.getattr("__dict__")?.cast::<PyDict>()?.copy()?;
    state.del_item("plt")?;
    Ok(state)
}

#[pyfunction]
#[pyo3(name = "__setstate__")]
fn graph_setstate(slf: &Bound<'_, PyAny>, state: &Bound<'_, PyDict>) -> PyResult<()> {
    slf.getattr("__dict__")?.call_method1("update", (state,))?;
    slf.setattr("plt", pyplot(slf.py())?)
}

/// What happens once the graph is drawn: `'write'` (the default) writes a
/// file, `'show'` opens it, `None` does neither.
#[pyfunction]
fn done_action_get<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    slf.getattr("_doneAction")
}

#[pyfunction]
fn done_action_set(slf: &Bound<'_, PyAny>, action: &Bound<'_, PyAny>) -> PyResult<()> {
    let known = action.is_none()
        || action
            .extract::<String>()
            .is_ok_and(|action| DONE_ACTIONS.contains(&action.as_str()));
    if !known {
        return Err(GraphException::new_err(format!(
            "no such done action: {}",
            action.str()?
        )));
    }
    slf.setattr("_doneAction", action)
}

/// The next of `colors`, round again after the last.
#[pyfunction]
fn nextColor(slf: &Bound<'_, PyAny>) -> PyResult<String> {
    let colors = slf.getattr("colors")?;
    let index: usize = slf.getattr("_dataColorIndex")?.extract()?;
    let chosen = getColor(&colors.get_item(index % colors.len()?)?)?;
    slf.setattr("_dataColorIndex", index + 1)?;
    Ok(chosen)
}

/// Sets an axis's ticks from pairs of position and label; `None` leaves
/// them as they are.
#[pyfunction]
fn setTicks(
    slf: &Bound<'_, PyAny>,
    axisKey: &Bound<'_, PyAny>,
    pairs: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let py = slf.py();
    if pairs.is_none() {
        return Ok(());
    }
    let axis = slf.getattr("axis")?;
    if !axis.contains(axisKey)? {
        return Err(GraphException::new_err(format!(
            "Cannot find key '{}' in self.axis",
            axisKey.str()?
        )));
    }
    let positions = PyList::empty(py);
    let labels = PyList::empty(py);
    for pair in pairs.try_iter()? {
        let (value, label) = super::pair(&pair?)?;
        positions.append(value)?;
        labels.append(label)?;
    }
    axis.get_item(axisKey)?
        .set_item("ticks", PyTuple::new(py, [positions, labels])?)
}

/// Ticks for an axis (`'y'` by default) counting from nought to the most
/// of the data, about `dataSteps` of them.
#[pyfunction]
#[pyo3(signature = (slf, unsortedData, axisKey="y", dataSteps=DEFAULT_DATA_STEPS))]
fn setIntegerTicksFromData(
    slf: &Bound<'_, PyAny>,
    unsortedData: Vec<f64>,
    axisKey: &str,
    dataSteps: i64,
) -> PyResult<()> {
    let py = slf.py();
    let pairs = PyList::empty(py);
    for tick in integer_ticks(&unsortedData, dataSteps) {
        pairs.append((tick, tick.to_string()))?;
    }
    slf.call_method1("setTicks", (axisKey, pairs)).map(|_| ())
}

fn no_axis(slf: &Bound<'_, PyAny>, axisKey: &Bound<'_, PyAny>) -> PyResult<()> {
    if slf.getattr("axisKeys")?.contains(axisKey)? {
        return Ok(());
    }
    Err(GraphException::new_err(format!(
        "No such axis exists: {}",
        axisKey.str()?
    )))
}

/// Sets an axis's range from its lowest to its highest value, padded by
/// `paddingFraction` of the range each way.
#[pyfunction]
#[pyo3(signature = (slf, axisKey, valueRange, paddingFraction=None))]
fn setAxisRange<'py>(
    slf: &Bound<'py, PyAny>,
    axisKey: &Bound<'py, PyAny>,
    valueRange: &Bound<'py, PyAny>,
    paddingFraction: Option<&Bound<'py, PyAny>>,
) -> PyResult<()> {
    let py = slf.py();
    no_axis(slf, axisKey)?;
    let low = valueRange.get_item(0)?;
    let high = valueRange.get_item(1)?;
    let padding = match paddingFraction {
        Some(padding) => padding.clone(),
        None => 0.1_f64.into_bound_py_any(py)?,
    };
    let shift = if padding.eq(0)? {
        0_i64.into_bound_py_any(py)?
    } else {
        high.sub(&low)?.mul(&padding)?
    };
    let range = PyTuple::new(py, [low.sub(&shift)?, high.add(&shift)?])?;
    slf.getattr("axis")?
        .get_item(axisKey)?
        .set_item("range", range)?;
    slf.getattr("axisRangeHasBeenSet")?.set_item(axisKey, true)
}

/// Sets an axis's label; `conditional` keeps a label already set.
#[pyfunction]
#[pyo3(signature = (slf, axisKey, label, conditional=false))]
fn setAxisLabel(
    slf: &Bound<'_, PyAny>,
    axisKey: &Bound<'_, PyAny>,
    label: &Bound<'_, PyAny>,
    conditional: bool,
) -> PyResult<()> {
    no_axis(slf, axisKey)?;
    let axis = slf.getattr("axis")?.get_item(axisKey)?;
    if conditional
        && let Some(set) = axis.cast::<PyDict>()?.get_item("label")?
        && set.is_truthy()?
    {
        return Ok(());
    }
    axis.set_item("label", label)
}

/// Takes the right and top spines off a subplot, and the left and bottom
/// too where `leftBottom` is true, by colouring them `'none'`, and hides
/// their tick lines.
#[pyfunction]
#[pyo3(signature = (subplot, leftBottom=false))]
fn hideAxisSpines(subplot: &Bound<'_, PyAny>, leftBottom: bool) -> PyResult<()> {
    for item in subplot
        .getattr("spines")?
        .call_method0("items")?
        .try_iter()?
    {
        let (location, spine): (String, Bound<'_, PyAny>) = item?.extract()?;
        match location.as_str() {
            "left" | "bottom" => {
                if leftBottom {
                    spine.call_method1("set_color", ("none",))?;
                }
            }
            "right" | "top" => {
                spine.call_method1("set_color", ("none",))?;
            }
            _ => {
                return Err(PyValueError::new_err(format!(
                    "unknown spine location: {location}"
                )));
            }
        }
    }

    // The top and right tick lines are the odd ones.
    let lines = subplot
        .call_method0("get_xticklines")?
        .add(subplot.call_method0("get_yticklines")?)?;
    for (index, line) in lines.try_iter()?.enumerate() {
        if leftBottom || index % 2 == 1 {
            line?.call_method1("set_visible", (false,))?;
        }
    }
    Ok(())
}

/// Colours, ranges, labels, ticks, title, grid and size, applied to the
/// subplot and the figure.
#[pyfunction]
fn applyFormatting(slf: &Bound<'_, PyAny>, subplot: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = slf.py();
    let background = getColor(&slf.getattr("colorBackgroundData")?)?;
    subplot
        .getattr("patch")?
        .call_method1("set_facecolor", (background,))?;
    for axis in slf.getattr("axisKeys")?.try_iter()? {
        slf.call_method1("applyFormattingToOneAxis", (subplot, axis?))?;
    }
    let title = slf.getattr("title")?;
    if title.is_truthy()? {
        let keywords = kwargs!(py;
            "fontsize" => slf.getattr("titleFontSize")?,
            "family" => slf.getattr("fontFamily")?,
        );
        subplot.call_method("set_title", (title,), Some(&keywords))?;
    }
    let tick_colors = slf.getattr("tickColors")?;
    for axis in slf.getattr("axisKeys")?.try_iter()? {
        let axis = axis?;
        if !tick_colors.contains(&axis)? {
            continue;
        }
        let keywords = kwargs!(py; "axis" => &axis, "colors" => tick_colors.get_item(&axis)?);
        subplot.call_method("tick_params", (), Some(&keywords))?;
    }
    slf.call_method1("applyGrid", (subplot,))?;

    let figure = optional(slf, "figure")?.ok_or_else(|| {
        PyValueError::new_err("Cannot apply formatting without a figure; call process() first")
    })?;
    let size = slf.getattr("figureSize")?;
    figure.call_method1("set_figwidth", (size.get_item(0)?,))?;
    figure.call_method1("set_figheight", (size.get_item(1)?,))?;
    Ok(())
}

/// The grid, drawn below the data, unless `grid` is off or `colorGrid` is
/// `None`; `hideXGrid` and `hideYGrid` take one direction off.
#[pyfunction]
fn applyGrid(slf: &Bound<'_, PyAny>, subplot: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = slf.py();
    let color = slf.getattr("colorGrid")?;
    if slf.getattr("grid")?.is_truthy()? && !color.is_none() {
        subplot.call_method1("set_axisbelow", (true,))?;
        let keywords = kwargs!(py; "which" => "major", "color" => getColor(&color)?);
        subplot.call_method("grid", (true,), Some(&keywords))?;
    }
    if slf.getattr("hideYGrid")?.is_truthy()? {
        subplot.getattr("yaxis")?.call_method1("grid", (false,))?;
    }
    if slf.getattr("hideXGrid")?.is_truthy()? {
        subplot.getattr("xaxis")?.call_method1("grid", (false,))?;
    }
    Ok(())
}

/// One axis of the subplot set from `self.axis`: its range, label, scale,
/// ticks and tick labels. Answers that axis's settings.
#[pyfunction]
fn applyFormattingToOneAxis<'py>(
    slf: &Bound<'py, PyAny>,
    subplot: &Bound<'py, PyAny>,
    axis: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let this_axis = slf.getattr("axis")?.get_item(axis)?;
    if !["x", "y", "z"].contains(&axis) {
        return Ok(py.None().into_bound(py));
    }
    let settings = this_axis.cast::<PyDict>()?;
    let font = || -> PyResult<Bound<'py, PyDict>> {
        Ok(kwargs!(py;
            "fontsize" => slf.getattr("tickFontSize")?,
            "family" => slf.getattr("fontFamily")?,
        ))
    };

    // Range, label and scale.
    if let Some(range) = settings.get_item("range")?.filter(|range| !range.is_none()) {
        let mut name = format!("set_{axis}lim");
        if slf.getattr("axisKeys")?.len()? == 3 {
            name.push_str("3d");
        }
        let bounds = PyTuple::new(py, range.try_iter()?.collect::<PyResult<Vec<_>>>()?)?;
        subplot.getattr(name.as_str())?.call1(bounds)?;
    }
    if let Some(label) = settings.get_item("label")?.filter(|label| !label.is_none()) {
        let keywords = kwargs!(py;
            "fontsize" => slf.getattr("labelFontSize")?,
            "family" => slf.getattr("fontFamily")?,
        );
        subplot.call_method(
            format!("set_{axis}label").as_str(),
            (label,),
            Some(&keywords),
        )?;
    }
    if let Some(scale) = settings.get_item("scale")?.filter(|scale| !scale.is_none()) {
        subplot.call_method1(format!("set_{axis}scale").as_str(), (scale,))?;
    }

    // Ticks: matplotlib's own, formatted, where none are set.
    let get_ticks = optional(subplot, &format!("get_{axis}ticks"))?;
    let set_ticks = optional(subplot, &format!("set_{axis}ticks"))?;
    let set_labels = optional(subplot, &format!("set_{axis}ticklabels"))?;
    let (Some(get_ticks), Some(set_ticks), Some(set_labels)) = (get_ticks, set_ticks, set_labels)
    else {
        let (values, labels): (Bound<'py, PyAny>, Bound<'py, PyAny>) = settings
            .get_item("ticks")?
            .ok_or_else(|| PyKeyError::new_err("ticks"))?
            .extract()?;
        let _ = (values, labels);
        return Ok(this_axis);
    };
    let Some(ticks) = settings.get_item("ticks")? else {
        let ticks = get_ticks.call0()?;
        set_ticks.call1((&ticks,))?;
        set_labels.call((ticks,), Some(&font()?))?;
        return Ok(this_axis);
    };
    let (values, labels) = super::pair(&ticks)?;
    set_ticks.call1((values,))?;
    match axis {
        "x" => {
            let keywords = font()?;
            keywords.set_item(
                "horizontalalignment",
                slf.getattr("xTickLabelHorizontalAlignment")?,
            )?;
            keywords.set_item(
                "verticalalignment",
                slf.getattr("xTickLabelVerticalAlignment")?,
            )?;
            keywords.set_item("rotation", slf.getattr("xTickLabelRotation")?)?;
            keywords.set_item("y", -0.01)?;
            subplot.call_method("set_xticklabels", (labels,), Some(&keywords))?;
        }
        "y" => {
            let keywords = font()?;
            keywords.set_item("horizontalalignment", "right")?;
            keywords.set_item("verticalalignment", "center")?;
            subplot.call_method("set_yticklabels", (labels,), Some(&keywords))?;
        }
        _ => {
            set_labels.call((labels,), Some(&font()?))?;
        }
    }
    Ok(this_axis)
}

/// Makes the figure and its subplot, has `renderSubplot` draw the data,
/// takes off the spines, applies the formatting and does the done action.
#[pyfunction]
fn process(slf: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = slf.py();
    let plt = pyplot(py)?;
    let keywords = kwargs!(py; "facecolor" => slf.getattr("colorBackgroundFigure")?);
    let figure = plt.call_method("figure", (), Some(&keywords))?;
    slf.setattr("figure", &figure)?;
    let subplot = figure.call_method1("add_subplot", (1, 1, 1))?;
    slf.setattr("subplot", &subplot)?;

    // From the first colour, so a graph drawn twice draws the same.
    slf.setattr("_dataColorIndex", 0)?;
    slf.call_method1("renderSubplot", (&subplot,))?;

    let keywords = kwargs!(py; "leftBottom" => slf.getattr("hideLeftBottomSpines")?);
    slf.call_method("hideAxisSpines", (&subplot,), Some(&keywords))?;
    slf.call_method1("applyFormatting", (&subplot,))?;
    slf.call_method0("callDoneAction")?;
    Ok(())
}

/// Draws the data; each kind of graph draws its own.
#[pyfunction]
fn renderSubplot(slf: &Bound<'_, PyAny>, subplot: &Bound<'_, PyAny>) {
    let _ = (slf, subplot);
}

/// Writes or shows the graph, as `doneAction` says.
#[pyfunction]
#[pyo3(signature = (slf, fp=None))]
fn callDoneAction(slf: &Bound<'_, PyAny>, fp: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
    let action = slf.getattr("doneAction")?;
    if action.is_none() {
        return Ok(());
    }
    match action.extract::<String>()?.as_str() {
        "show" => {
            slf.call_method0("show")?;
        }
        "write" => {
            slf.call_method1("write", (fp,))?;
        }
        _ => {}
    }
    Ok(())
}

/// Opens the figure in matplotlib's window.
#[pyfunction]
fn show(slf: &Bound<'_, PyAny>) -> PyResult<()> {
    figure_of(slf, "show graph")?.call_method0("show")?;
    Ok(())
}

/// Writes the figure to `fp`, or a temporary file, as a PNG at `dpi`
/// (300 where unset), and opens it; in a notebook, answers the figure.
#[pyfunction]
#[pyo3(signature = (slf, fp=None))]
fn write<'py>(
    slf: &Bound<'py, PyAny>,
    fp: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let figure = figure_of(slf, "write graph")?;
    let fp = match fp.filter(|fp| !fp.is_none()) {
        Some(fp) => fp.clone(),
        None => music21_name(py, "music21.environment", "Environment")?
            .call1(("graph.primitives",))?
            .call_method1("getTempFile", (".png",))?,
    };
    let dpi = match optional(slf, "dpi")? {
        Some(dpi) => dpi,
        None => DEFAULT_DPI.into_bound_py_any(py)?,
    };
    figure.call_method("savefig", (&fp,), Some(&kwargs!(py; "dpi" => dpi)))?;

    let notebook = music21_name(py, "music21.common", "runningInNotebook")?.call0()?;
    if notebook.is_truthy()? {
        return Ok(figure);
    }
    let path = py.import("pathlib")?.getattr("Path")?.call1((fp,))?;
    let converter = music21_name(py, "music21.converter.subConverters", "SubConverter")?.call0()?;
    converter.call_method("launch", (path,), Some(&kwargs!(py; "fmt" => "png")))?;
    Ok(py.None().into_bound(py))
}

// ---------------------------------------------------------------- GraphColorGrid

#[pyfunction]
#[pyo3(signature = (slf, **keywords))]
fn color_grid_init(slf: &Bound<'_, PyAny>, keywords: Option<&Bound<'_, PyDict>>) -> PyResult<()> {
    slf.setattr("hideLeftBottomSpines", true)?;
    graph_init(slf, keywords)
}

/// A row of coloured blocks for each list of colours, the first at the
/// bottom.
#[pyfunction]
#[pyo3(name = "renderSubplot")]
fn color_grid_render(slf: &Bound<'_, PyAny>, subplot: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = slf.py();
    let _ = subplot;
    let figure = figure_of(slf, "render subplot")?;
    let data = data_of(slf, "render subplot")?;
    figure.call_method("subplots_adjust", (), Some(&kwargs!(py; "left" => 0.15)))?;

    let row_count = data.len()?;
    for i in 0..row_count {
        let row = data.get_item(i)?;
        let positions = PyList::empty(py);
        let heights = PyList::empty(py);
        let colors = PyList::empty(py);
        for (j, color) in row.try_iter()?.enumerate() {
            positions.append(j as f64 + 0.5)?;
            colors.append(color?)?;
            heights.append(1)?;
        }

        // A subplot per row; thin black edges, no antialiasing.
        let ax = figure.call_method1("add_subplot", (row_count, 1, row_count - i))?;
        let keywords = kwargs!(py;
            "color" => colors,
            "linewidth" => 0.3,
            "edgecolor" => "#000000",
            "antialiased" => false,
        );
        ax.call_method("bar", (positions, heights, 1), Some(&keywords))?;
        for spine in ax.getattr("spines")?.call_method0("values")?.try_iter()? {
            let spine = spine?;
            spine.call_method1("set_linewidth", (0.3,))?;
            spine.call_method1("set_color", ("#000000",))?;
            spine.call_method1("set_alpha", (1,))?;
        }

        // No ticks on any row.
        let lines = ax
            .call_method0("get_xticklines")?
            .add(ax.call_method0("get_yticklines")?)?;
        for line in lines.try_iter()? {
            line?.call_method1("set_visible", (false,))?;
        }
        ax.call_method1("set_xticks", (PyList::empty(py),))?;
        ax.call_method1("set_yticks", (PyList::empty(py),))?;
        let blank_y = vec![""; ax.call_method0("get_yticklabels")?.len()?];
        ax.call_method1("set_yticklabels", (blank_y,))?;
        let blank_x = vec![""; ax.call_method0("get_xticklabels")?.len()?];
        ax.call_method1("set_xticklabels", (blank_x,))?;
        ax.call_method1("set_xlim", ((0, row.len()?),))?;
        ax.call_method0("get_xaxis")?
            .call_method1("set_visible", (false,))?;
        ax.call_method0("get_yaxis")?
            .call_method1("set_visible", (false,))?;
    }

    // Rows touch where there are many.
    let space = if row_count > GRID_ROWS_SPACED {
        0.0
    } else {
        0.1
    };
    figure.call_method("subplots_adjust", (), Some(&kwargs!(py; "hspace" => space)))?;
    setAxisRange(
        slf,
        &"x".into_bound_py_any(py)?,
        &(0, 1).into_bound_py_any(py)?,
        Some(&0.into_bound_py_any(py)?),
    )?;
    slf.setattr("grid", false)
}

// ---------------------------------------------------------------- GraphColorGridLegend

#[pyfunction]
#[pyo3(signature = (slf, **keywords))]
fn legend_init(slf: &Bound<'_, PyAny>, keywords: Option<&Bound<'_, PyDict>>) -> PyResult<()> {
    slf.setattr("hideLeftBottomSpines", true)?;
    graph_init(slf, keywords)?;
    if !given(keywords, "title")? {
        slf.setattr("title", "Legend")?;
    }
    Ok(())
}

/// A row of labelled blocks for each label and its colours.
#[pyfunction]
#[pyo3(name = "renderSubplot")]
fn legend_render(slf: &Bound<'_, PyAny>, subplot: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = slf.py();
    let figure = figure_of(slf, "render subplot")?;
    let data = data_of(slf, "render subplot")?;
    for (i, row) in data.try_iter()?.enumerate() {
        let row = row?;
        slf.call_method1(
            "makeOneRowOfGraph",
            (&figure, i, row.get_item(0)?, row.get_item(1)?),
        )?;
    }
    let x = "x".into_bound_py_any(py)?;
    let y = "y".into_bound_py_any(py)?;
    setAxisRange(
        slf,
        &x,
        &(0, 1).into_bound_py_any(py)?,
        Some(&0.into_bound_py_any(py)?),
    )?;

    let lines = subplot
        .call_method0("get_xticklines")?
        .add(subplot.call_method0("get_yticklines")?)?;
    for line in lines.try_iter()? {
        line?.call_method1("set_visible", (false,))?;
    }
    let keywords = kwargs!(py; "hspace" => 1.5, "top" => 0.75, "bottom" => 0.2);
    figure.call_method("subplots_adjust", (), Some(&keywords))?;
    let empty = "".into_bound_py_any(py)?;
    setAxisLabel(slf, &y, &empty, false)?;
    setAxisLabel(slf, &x, &empty, false)?;
    setTicks(slf, &y, &PyList::empty(py))?;
    setTicks(slf, &x, &PyList::empty(py))
}

/// One row of the legend, labelled on the left, each block labelled
/// below: answers the subplot made for it.
#[pyfunction]
fn makeOneRowOfGraph<'py>(
    slf: &Bound<'py, PyAny>,
    figure: &Bound<'py, PyAny>,
    rowIndex: usize,
    rowLabel: &Bound<'py, PyAny>,
    rowData: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let data = optional(slf, "data")?.ok_or_else(|| {
        PyValueError::new_err(
            "Cannot create row of graph without data; set self.data before calling process()",
        )
    })?;
    let positions = PyList::empty(py);
    let heights = PyList::empty(py);
    let colors = PyList::empty(py);
    let labels = PyList::empty(py);
    for (j, mapping) in rowData.try_iter()?.enumerate() {
        let mapping = mapping?;
        positions.append(1.0 + j as f64)?;
        colors.append(mapping.get_item(1)?)?;
        heights.append(1)?;
        labels.append(accidentalLabelToUnicode(&mapping.get_item(0)?)?)?;
    }
    let count = labels.len();

    let ax = figure.call_method1("add_subplot", (data.len()?, 1, rowIndex + 1))?;
    let keywords = kwargs!(py; "color" => colors, "linewidth" => 0.3, "edgecolor" => "#000000");
    ax.call_method("bar", (positions, heights, 1), Some(&keywords))?;
    for spine in ax.getattr("spines")?.call_method0("values")?.try_iter()? {
        let spine = spine?;
        spine.call_method1("set_linewidth", (0.3,))?;
        spine.call_method1("set_color", ("#000000",))?;
    }
    let lines = ax
        .call_method0("get_xticklines")?
        .add(ax.call_method0("get_yticklines")?)?;
    for line in lines.try_iter()? {
        line?.call_method1("set_visible", (false,))?;
    }

    // The row's label in the middle of its left side; each block's below
    // it, pushed down for lower case.
    ax.call_method1("set_yticks", (vec![0.5],))?;
    let keywords = kwargs!(py;
        "fontsize" => slf.getattr("tickFontSize")?,
        "family" => slf.getattr("fontFamily")?,
        "horizontalalignment" => "right",
        "verticalalignment" => "center",
    );
    ax.call_method("set_yticklabels", (vec![rowLabel],), Some(&keywords))?;
    let ticks: Vec<usize> = (1..=count).collect();
    ax.call_method1("set_xticks", (ticks,))?;
    let keywords = kwargs!(py;
        "fontsize" => slf.getattr("tickFontSize")?,
        "family" => slf.getattr("fontFamily")?,
        "horizontalalignment" => "center",
        "verticalalignment" => "center",
        "y" => -0.4,
    );
    ax.call_method("set_xticklabels", (labels,), Some(&keywords))?;
    ax.call_method1("set_xlim", ((0.5, count as f64 + 0.5),))?;
    Ok(ax)
}

// ---------------------------------------------------------------- GraphHorizontalBar

#[pyfunction]
#[pyo3(signature = (slf, **keywords))]
fn horizontal_init(slf: &Bound<'_, PyAny>, keywords: Option<&Bound<'_, PyDict>>) -> PyResult<()> {
    slf.setattr("barSpace", BAR_SPACE)?;
    slf.setattr("margin", 2)?;
    graph_init(slf, keywords)?;
    if !given(keywords, "alpha")? {
        slf.setattr("alpha", 0.6)?;
    }
    Ok(())
}

/// The height of a bar: its channel less the margin above and below.
#[pyfunction]
fn bar_height<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let margin = slf.getattr("margin")?.mul(2)?;
    slf.getattr("barSpace")?.sub(margin)
}

/// A channel for each label, its bars from each start for each length.
#[pyfunction]
#[pyo3(name = "renderSubplot")]
fn horizontal_render(slf: &Bound<'_, PyAny>, subplot: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = slf.py();
    let figure = figure_of(slf, "render subplot")?;
    let data = data_of(slf, "render subplot")?;
    figure.call_method("subplots_adjust", (), Some(&kwargs!(py; "left" => 0.15)))?;

    let bar_space: f64 = slf.getattr("barSpace")?.extract()?;
    let margin: f64 = slf.getattr("margin")?.extract()?;
    let height = slf.getattr("barHeight")?;
    let mut y_pos = 0.0;
    let mut x_points: Vec<f64> = Vec::new();
    let y_ticks = PyList::empty(py);
    let mut keys = 0;
    for info in data.try_iter()? {
        let info = info?;
        let key = info.get_item(0)?;
        let points = info.get_item(1)?;
        let row_format = if info.len()? == 2 {
            PyDict::new(py).into_any()
        } else {
            info.get_item(2)?
        };
        keys += 1;

        // The row's colour, the next one unless the row gives one.
        let next = nextColor(slf)?;
        let face = row_format.call_method1("get", ("color", next))?;
        if points.is_truthy()? {
            let uniform = points.get_item(0)?.len()? == 2;
            let (faces, positions) = if uniform {
                (face.clone(), points.clone())
            } else {
                let faces = PyList::empty(py);
                let positions = PyList::empty(py);
                for point in points.try_iter()? {
                    let point = point?;
                    faces.append(point.get_item(2)?.call_method1("get", ("color", &face))?)?;
                    positions.append(point.get_item(pyo3::types::PySlice::new(py, 0, 2, 1))?)?;
                }
                (faces.into_any(), positions.into_any())
            };
            let y_range = (y_pos + margin, &height);
            let keywords = kwargs!(py; "facecolors" => faces, "alpha" => slf.getattr("alpha")?);
            subplot.call_method("broken_barh", (positions, y_range), Some(&keywords))?;
            for point in points.try_iter()? {
                let point = point?;
                if point.len()? < 2 {
                    return Err(PyValueError::new_err(format!(
                        "Points must be length 2 or 3, not {}: {}",
                        point.len()?,
                        point.str()?
                    )));
                }
                let start: f64 = point.get_item(0)?.extract()?;
                let length: f64 = point.get_item(1)?.extract()?;
                for x in [start, start + length] {
                    if !x_points.contains(&x) {
                        x_points.push(x);
                    }
                }
            }
        }
        y_ticks.append(PyList::new(
            py,
            [(y_pos + bar_space * 0.5).into_bound_py_any(py)?, key],
        )?)?;
        y_pos += bar_space;
    }

    let x_min = x_points.iter().copied().fold(f64::INFINITY, f64::min);
    let x_max = x_points.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let (x_min, x_max) = if x_points.is_empty() {
        (0.0, 0.0)
    } else {
        (x_min, x_max)
    };
    let x = "x".into_bound_py_any(py)?;
    let y = "y".into_bound_py_any(py)?;
    let y_span = (0, keys as f64 * bar_space).into_bound_py_any(py)?;
    setAxisRange(slf, &y, &y_span, None)?;
    setAxisRange(slf, &x, &(x_min, x_max).into_bound_py_any(py)?, None)?;
    setTicks(slf, &y, &y_ticks)?;

    // Ticks of its own only where the x ticks were emptied.
    let x_axis = slf.getattr("axis")?.get_item("x")?.cast_into::<PyDict>()?;
    if let Some(ticks) = x_axis.get_item("ticks")?
        && !ticks.is_truthy()?
    {
        let pairs = PyList::empty(py);
        for tick in bar_x_ticks(x_min, x_max) {
            pairs.append(PyList::new(
                py,
                [
                    tick.into_bound_py_any(py)?,
                    tick.to_string().into_bound_py_any(py)?,
                ],
            )?)?;
        }
        setTicks(slf, &x, &pairs)?;
    }
    Ok(())
}

// ---------------------------------------------------------------- GraphHorizontalBarWeighted

#[pyfunction]
#[pyo3(signature = (slf, **keywords))]
fn weighted_bar_init(slf: &Bound<'_, PyAny>, keywords: Option<&Bound<'_, PyDict>>) -> PyResult<()> {
    slf.setattr("barSpace", BAR_SPACE)?;
    slf.setattr("margin", 0.25)?;
    graph_init(slf, keywords)?;
    if !given(keywords, "alpha")? {
        slf.setattr("alpha", 1)?;
    }
    Ok(())
}

/// A channel for each label, each bar its own height, colour and
/// opacity, the first label at the top.
#[pyfunction]
#[pyo3(name = "renderSubplot")]
fn weighted_bar_render(slf: &Bound<'_, PyAny>, subplot: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = slf.py();
    let figure = figure_of(slf, "render subplot")?;
    let data = data_of(slf, "render subplot")?;
    figure.call_method("subplots_adjust", (), Some(&kwargs!(py; "left" => 0.15)))?;

    let bar_space: f64 = slf.getattr("barSpace")?.extract()?;
    let margin: f64 = slf.getattr("margin")?.extract()?;
    let bar_height: f64 = slf.getattr("barHeight")?.extract()?;

    // Drawn from the bottom up, so the list is reversed in place.
    let reversed = PyList::new(py, data.try_iter()?.collect::<PyResult<Vec<_>>>()?)?;
    reversed.reverse()?;
    slf.setattr("data", &reversed)?;

    let mut y_pos = 0.0;
    let mut x_points: Vec<f64> = Vec::new();
    let y_ticks = PyList::empty(py);
    for row in reversed.iter() {
        let (key, points) = super::pair(&row)?;
        let channel = Channel {
            start: y_pos,
            margin,
            bar_height,
        };
        for point in points.try_iter()? {
            let point = point?;
            let size = point.len()?;
            let mut color = nextColor(slf)?.into_bound_py_any(py)?;
            let mut alpha = slf.getattr("alpha")?;
            let mut scale = 1.0;
            let mut shift = 0.0;
            if size < 3 {
                return Err(PyValueError::new_err(format!(
                    "Data point must have at least 3 elements (x, span, heightScalar, ...); got: {}",
                    point.str()?
                )));
            }
            let x: f64 = point.get_item(0)?.extract()?;
            let span: f64 = point.get_item(1)?.extract()?;
            if size <= 6 {
                scale = point.get_item(2)?.extract()?;
            }
            if (4..=6).contains(&size) {
                color = point.get_item(3)?;
            }
            if (5..=6).contains(&size) {
                alpha = point.get_item(4)?;
            }
            if size == 6 {
                shift = point.get_item(5)?.extract()?;
            }
            let color = getColor(&color)?;
            for value in [x, x + span] {
                if !x_points.contains(&value) {
                    x_points.push(value);
                }
            }
            let (start, height) = weighted_bar(channel, scale, shift);
            let keywords =
                kwargs!(py; "facecolors" => &color, "alpha" => alpha, "edgecolor" => &color);
            subplot.call_method(
                "broken_barh",
                (vec![(x, span)], (start, height)),
                Some(&keywords),
            )?;
        }
        y_ticks.append(PyList::new(
            py,
            [(y_pos + bar_space * 0.5).into_bound_py_any(py)?, key],
        )?)?;
        y_pos += bar_space;
    }

    let builtins = py.import("builtins")?;
    let x_min = builtins.call_method1("min", (x_points.clone(),))?;
    let x_max = builtins.call_method1("max", (x_points,))?;
    let x = "x".into_bound_py_any(py)?;
    let y = "y".into_bound_py_any(py)?;
    let y_span = (0, reversed.len() as f64 * bar_space).into_bound_py_any(py)?;
    setAxisRange(slf, &y, &y_span, Some(&0.05.into_bound_py_any(py)?))?;
    let x_span = PyTuple::new(py, [x_min, x_max])?.into_any();
    setAxisRange(slf, &x, &x_span, Some(&0.01.into_bound_py_any(py)?))?;
    setTicks(slf, &y, &y_ticks)
}

// ---------------------------------------------------------------- GraphScatterWeighted

#[pyfunction]
#[pyo3(signature = (slf, **keywords))]
fn scatter_weighted_init(
    slf: &Bound<'_, PyAny>,
    keywords: Option<&Bound<'_, PyDict>>,
) -> PyResult<()> {
    slf.setattr("maxDiameter", 1.25)?;
    slf.setattr("minDiameter", 0.25)?;
    graph_init(slf, keywords)?;
    if !given(keywords, "alpha")? {
        slf.setattr("alpha", 0.6)?;
    }
    Ok(())
}

/// The span of a point's diameter, from the smallest to the largest.
#[pyfunction]
fn range_diameter<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    slf.getattr("maxDiameter")?.sub(slf.getattr("minDiameter")?)
}

/// An ellipse at each point, sized by its count, which is written beside
/// it where it is over one.
#[pyfunction]
#[pyo3(name = "renderSubplot")]
fn scatter_weighted_render(slf: &Bound<'_, PyAny>, subplot: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = slf.py();
    let figure = figure_of(slf, "render subplot")?;
    let data = data_of(slf, "render subplot")?;
    let patches = super::utilities::getExtendedModules(py)?.getattr("patches")?;
    let keywords = kwargs!(py; "left" => 0.15, "bottom" => 0.15);
    figure.call_method("subplots_adjust", (), Some(&keywords))?;

    let xs = PyList::empty(py);
    let ys = PyList::empty(py);
    let mut points = Vec::new();
    let mut counts = Vec::new();
    let mut formats = Vec::new();
    for point in data.try_iter()? {
        let point = point?;
        let (x, y, z) = (point.get_item(0)?, point.get_item(1)?, point.get_item(2)?);
        points.push((
            x.extract::<f64>()?,
            y.extract::<f64>()?,
            z.extract::<f64>()?,
        ));
        xs.append(x)?;
        ys.append(y)?;
        counts.push(z);
        formats.push(if point.len()? > 3 {
            point.get_item(3)?
        } else {
            PyDict::new(py).into_any()
        });
    }
    let drawn = weighted_points(
        &points,
        slf.getattr("minDiameter")?.extract()?,
        slf.getattr("maxDiameter")?.extract()?,
    );

    for (index, shape) in drawn.iter().enumerate() {
        let (x, y, _) = points[index];
        let keywords = formats[index].cast::<PyDict>()?.copy()?;
        keywords.set_item("xy", (x, y))?;
        keywords.set_item("width", shape.width)?;
        keywords.set_item("height", shape.height)?;
        let ellipse = patches.getattr("Ellipse")?.call((), Some(&keywords))?;
        subplot.call_method1("add_artist", (&ellipse,))?;
        ellipse.call_method1("set_clip_box", (subplot.getattr("bbox")?,))?;
        ellipse.call_method1("set_alpha", (slf.getattr("alpha")?,))?;
        ellipse.call_method1("set_facecolor", (nextColor(slf)?,))?;
        if let Some((label_x, label_y)) = shape.label_at {
            let keywords = kwargs!(py;
                "size" => 6, "va" => "baseline", "ha" => "left", "multialignment" => "left",
            );
            let text = counts[index].str()?;
            subplot.call_method("text", (label_x, label_y, text), Some(&keywords))?;
        }
    }

    let (y_min, y_max) = min_max(&ys)?;
    let (x_min, x_max) = min_max(&xs)?;
    let y = "y".into_bound_py_any(py)?;
    let x = "x".into_bound_py_any(py)?;
    setAxisRange(slf, &y, &PyTuple::new(py, [y_min, y_max])?.into_any(), None)?;
    setAxisRange(slf, &x, &PyTuple::new(py, [x_min, x_max])?.into_any(), None)
}

// ---------------------------------------------------------------- GraphScatter

/// A marker at each point, each in the next colour unless its own format
/// says otherwise.
#[pyfunction]
#[pyo3(name = "renderSubplot")]
fn scatter_render(slf: &Bound<'_, PyAny>, subplot: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = slf.py();
    let figure = figure_of(slf, "render subplot")?;
    let data = data_of(slf, "render subplot")?;
    figure.call_method("subplots_adjust", (), Some(&kwargs!(py; "left" => 0.15)))?;

    let xs = PyList::empty(py);
    let ys = PyList::empty(py);
    for row in data.try_iter()? {
        let row = row?;
        if row.len()? < 2 {
            return Err(GraphException::new_err(
                "Need at least two points for a graph data object!",
            ));
        }
        let (x, y) = (row.get_item(0)?, row.get_item(1)?);
        xs.append(&x)?;
        ys.append(&y)?;
        let keywords = kwargs!(py;
            "marker" => slf.getattr("marker")?,
            "color" => nextColor(slf)?,
            "alpha" => slf.getattr("alpha")?,
            "markersize" => slf.getattr("markersize")?,
        );
        if row.len()? >= 3 {
            let display = row.get_item(2)?;
            for name in ["color", "marker", "alpha", "markersize"] {
                if display.contains(name)? {
                    keywords.set_item(name, display.get_item(name)?)?;
                }
            }
        }
        subplot.call_method("plot", (x, y), Some(&keywords))?;
    }

    let range_set = slf.getattr("axisRangeHasBeenSet")?;
    if !range_set.get_item("y")?.is_truthy()? {
        let (low, high) = min_max(&ys)?;
        setAxisRange(
            slf,
            &"y".into_bound_py_any(py)?,
            &PyTuple::new(py, [low, high])?.into_any(),
            None,
        )?;
    }
    if !range_set.get_item("x")?.is_truthy()? {
        let (low, high) = min_max(&xs)?;
        setAxisRange(
            slf,
            &"x".into_bound_py_any(py)?,
            &PyTuple::new(py, [low, high])?.into_any(),
            None,
        )?;
    }
    Ok(())
}

// ---------------------------------------------------------------- GraphHistogram

#[pyfunction]
#[pyo3(signature = (slf, *, binWidth=None, alpha=None, **keywords))]
fn histogram_init(
    slf: &Bound<'_, PyAny>,
    binWidth: Option<&Bound<'_, PyAny>>,
    alpha: Option<&Bound<'_, PyAny>>,
    keywords: Option<&Bound<'_, PyDict>>,
) -> PyResult<()> {
    let py = slf.py();
    match binWidth {
        Some(width) => slf.setattr("binWidth", width)?,
        None => slf.setattr("binWidth", 0.8)?,
    }
    let keywords = keywords_of(py, keywords)?;
    match alpha {
        Some(alpha) => keywords.set_item("alpha", alpha)?,
        None => keywords.set_item("alpha", 0.8)?,
    }
    graph_init(slf, Some(&keywords))
}

/// A bar for each value, as tall as its count.
#[pyfunction]
#[pyo3(name = "renderSubplot")]
fn histogram_render(slf: &Bound<'_, PyAny>, subplot: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = slf.py();
    let figure = figure_of(slf, "render subplot")?;
    let data = data_of(slf, "render subplot")?;
    figure.call_method("subplots_adjust", (), Some(&kwargs!(py; "left" => 0.15)))?;
    let xs = PyList::empty(py);
    let ys = PyList::empty(py);
    for point in data.try_iter()? {
        let point = point?;
        xs.append(point.get_item(0)?)?;
        ys.append(point.get_item(1)?)?;
    }
    let color = getColor(&slf.getattr("colors")?.get_item(0)?)?;
    let keywords = kwargs!(py;
        "width" => slf.getattr("binWidth")?,
        "alpha" => slf.getattr("alpha")?,
        "color" => color,
    );
    subplot.call_method("bar", (xs, ys), Some(&keywords))?;
    Ok(())
}

// ---------------------------------------------------------------- GraphGroupedVerticalBar

#[pyfunction]
#[pyo3(signature = (slf, **keywords))]
fn grouped_init(slf: &Bound<'_, PyAny>, keywords: Option<&Bound<'_, PyDict>>) -> PyResult<()> {
    slf.setattr("binWidth", 1)?;
    slf.setattr("roundDigits", 1)?;
    slf.setattr("groupLabelHeight", 0.0)?;
    graph_init(slf, keywords)
}

/// Writes each bar's height above it.
#[pyfunction]
fn labelBars(
    slf: &Bound<'_, PyAny>,
    subplot: &Bound<'_, PyAny>,
    rects: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let py = slf.py();
    let round = py.import("builtins")?.getattr("round")?;
    for rect in rects.try_iter()? {
        let rect = rect?;
        let width: f64 = rect.call_method0("get_width")?.extract()?;
        let x: f64 = rect.call_method0("get_x")?.extract()?;
        let height = rect.call_method0("get_height")?;
        let text = round.call1((&height, slf.getattr("roundDigits")?))?.str()?;
        let keywords = kwargs!(py;
            "ha" => "center",
            "va" => "bottom",
            "fontsize" => slf.getattr("tickFontSize")?,
            "family" => slf.getattr("fontFamily")?,
        );
        subplot.call_method("text", (x + width / 2.0, height, text), Some(&keywords))?;
    }
    Ok(())
}

/// A group of bars for each label, one for each key of its counts, with a
/// legend of the keys.
#[pyfunction]
#[pyo3(name = "renderSubplot")]
fn grouped_render(slf: &Bound<'_, PyAny>, subplot: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = slf.py();
    let data = data_of(slf, "render subplot")?;
    let matplotlib = super::utilities::getExtendedModules(py)?.getattr("matplotlib")?;
    let sorted = py.import("builtins")?.getattr("sorted")?;

    // The keys of the first group name the bars of every group.
    let mut per_group = 1;
    let mut sub_labels = PyList::empty(py).into_any();
    if let Some(first) = data.try_iter()?.next() {
        let counts = first?.get_item(1)?;
        per_group = counts.len()?;
        sub_labels = sorted.call1((counts.call_method0("keys")?,))?;
    }
    let width: f64 = slf.getattr("binWidth")?.extract()?;
    let shift = width / per_group as f64;
    let mut bundles: Vec<Vec<Bound<'_, PyAny>>> = Vec::new();
    for group in data.try_iter()? {
        let counts = group?.get_item(1)?;
        let mut bundle = Vec::new();
        for key in sorted.call1((counts.call_method0("keys")?,))?.try_iter()? {
            bundle.push(counts.get_item(key?)?);
        }
        bundles.push(bundle);
    }

    let mut rects = Vec::new();
    for i in 0..per_group {
        let ys: Vec<&Bound<'_, PyAny>> = bundles.iter().map(|bundle| &bundle[i]).collect();
        let xs: Vec<f64> = (0..bundles.len())
            .map(|x| x as f64 + shift * i as f64)
            .collect();
        let keywords = kwargs!(py; "width" => shift, "alpha" => 0.8, "color" => nextColor(slf)?);
        rects.push(subplot.call_method("bar", (xs, ys), Some(&keywords))?);
    }
    let colors = PyList::empty(py);
    for rect in &rects {
        labelBars(slf, subplot, rect)?;
        colors.append(rect.get_item(0)?)?;
    }
    let keywords =
        kwargs!(py; "size" => slf.getattr("tickFontSize")?, "family" => slf.getattr("fontFamily")?);
    let font = matplotlib
        .getattr("font_manager")?
        .getattr("FontProperties")?
        .call((), Some(&keywords))?;
    subplot.call_method(
        "legend",
        (colors, sub_labels),
        Some(&kwargs!(py; "prop" => font)),
    )?;
    Ok(())
}

// ---------------------------------------------------------------- Graph3DBars

#[pyfunction]
#[pyo3(signature = (slf, *, alpha=None, barWidth=None, barDepth=None, **keywords))]
fn bars3d_init(
    slf: &Bound<'_, PyAny>,
    alpha: Option<&Bound<'_, PyAny>>,
    barWidth: Option<&Bound<'_, PyAny>>,
    barDepth: Option<&Bound<'_, PyAny>>,
    keywords: Option<&Bound<'_, PyDict>>,
) -> PyResult<()> {
    let py = slf.py();
    let merged = keywords_of(py, keywords)?;
    match alpha {
        Some(alpha) => merged.set_item("alpha", alpha)?,
        None => merged.set_item("alpha", 0.8)?,
    }
    graph_init(slf, Some(&merged))?;
    slf.setattr("barWidth", barWidth)?;
    slf.setattr("barDepth", barDepth)?;
    if !given(keywords, "colors")? {
        slf.setattr(
            "colors",
            PyList::new(py, ["#ff0000", "#00ff00", "#6666ff"])?,
        )?;
    }
    Ok(())
}

/// A 3D figure, drawn, formatted, and its done action done.
#[pyfunction]
#[pyo3(name = "process")]
fn bars3d_process(slf: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = slf.py();
    let figure = pyplot(py)?.call_method0("figure")?;
    slf.setattr("figure", &figure)?;
    let keywords = kwargs!(py; "projection" => "3d");
    let subplot = figure.call_method("add_subplot", (1, 1, 1), Some(&keywords))?;
    slf.setattr("subplot", &subplot)?;
    slf.call_method1("renderSubplot", (&subplot,))?;
    slf.call_method1("applyFormatting", (&subplot,))?;
    slf.call_method0("callDoneAction")?;
    Ok(())
}

/// A bar at each x and y, as tall as its z.
#[pyfunction]
#[pyo3(name = "renderSubplot")]
fn bars3d_render(slf: &Bound<'_, PyAny>, subplot: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = slf.py();
    let data = data_of(slf, "render subplot")?;
    let mut columns: [Vec<f64>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    for point in data.try_iter()? {
        let point = point?;
        for (index, column) in columns.iter_mut().enumerate() {
            column.push(point.get_item(index)?.extract()?);
        }
    }
    let span = |values: &[f64]| -> (f64, f64) {
        (
            values.iter().copied().fold(f64::INFINITY, f64::min),
            values.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        )
    };
    let spans = [span(&columns[0]), span(&columns[1]), span(&columns[2])];

    // Ranges from the data where none is set.
    let axis = slf.getattr("axis")?;
    for (letter, index) in [("x", 0), ("z", 2), ("y", 1)] {
        let settings = axis.get_item(letter)?;
        if settings.get_item("range")?.is_none() {
            settings.set_item("range", spans[index])?;
        }
    }
    let mut width: f64 = match optional(slf, "barWidth")? {
        Some(width) => width.extract()?,
        None => (spans[0].1 - spans[0].0) / 20.0,
    };
    let mut depth: f64 = match optional(slf, "barDepth")? {
        Some(depth) => depth.extract()?,
        None => (spans[1].1 - spans[1].0) / 20.0,
    };

    for point in data.try_iter()? {
        let point = point?;
        let format = match point.len()? {
            3 => PyDict::new(py).into_any(),
            4 => point.get_item(3)?,
            _ => {
                return Err(GraphException::new_err(
                    "Cannot plot a point unless it has 3 or 4 values",
                ));
            }
        };
        let (x, y, z): (f64, f64, Bound<'_, PyAny>) = (
            point.get_item(0)?.extract()?,
            point.get_item(1)?.extract()?,
            point.get_item(2)?,
        );
        let color = if format.contains("color")? {
            format.get_item("color")?
        } else {
            nextColor(slf)?.into_bound_py_any(py)?
        };
        if format.contains("barWidth")? {
            width = format.get_item("barWidth")?.extract()?;
        }
        if format.contains("barDepth")? {
            depth = format.get_item("barDepth")?.extract()?;
        }
        let keywords = kwargs!(py; "color" => color, "alpha" => slf.getattr("alpha")?);
        subplot.call_method(
            "bar3d",
            (x - width / 2.0, y - depth / 2.0, 0, width, depth, z),
            Some(&keywords),
        )?;
    }
    for letter in ["x", "y", "z"] {
        let key = letter.into_bound_py_any(py)?;
        setAxisLabel(slf, &key, &key, true)?;
    }
    Ok(())
}

// ---------------------------------------------------------------- the module

/// `Graph.keywordConfigurables` and the names a subclass adds.
fn configurables<'py>(py: Python<'py>, more: &[&str]) -> PyResult<Bound<'py, PyAny>> {
    let all: Vec<&str> = KEYWORD_CONFIGURABLES.iter().chain(more).copied().collect();
    super::strings(py, &all)
}

/// A subclass of `base` with its graph type and figure size, built in
/// `m`.
fn graph_class<'py>(
    m: &Bound<'py, PyModule>,
    base: &Bound<'py, PyAny>,
    name: &'static str,
    doc: &'static str,
    graph_type: &str,
    more: &[(&'static str, Bound<'py, PyAny>)],
    methods: Vec<(&'static str, Bound<'py, PyAny>)>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = m.py();
    let mut spec = ClassSpec::new(name, doc, vec![base.clone()]);
    spec.attributes
        .push(("graphType", graph_type.into_bound_py_any(py)?));
    spec.attributes.extend(more.iter().cloned());
    spec.methods = methods;
    let class = spec.build(MODULE)?;
    m.add(name, &class)?;
    Ok(class)
}

/// A Rust function, to be a method.
macro_rules! f {
    ($m:expr, $function:path) => {
        wrap_pyfunction!($function, $m)?.into_any()
    };
}

/// music21's `graph.primitives`, built.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, MODULE)?;
        let m = &m;

        // Graph itself.
        let mut spec = ClassSpec::new(
            "Graph",
            "A graph or plot drawn with matplotlib from data already extracted. \
             Keywords set its `keywordConfigurables`; `doneAction` says whether it \
             is written (`'write'`, the default), shown (`'show'`) or neither \
             (`None`) once drawn.",
            vec![proto(py)],
        );
        spec.attributes = vec![
            ("graphType", "genericGraph".into_bound_py_any(py)?),
            ("axisKeys", super::strings(py, &["x", "y"])?),
            ("figureSizeDefault", (6, 6).into_bound_py_any(py)?),
            ("keywordConfigurables", configurables(py, &[])?),
        ];
        spec.methods = vec![
            ("__init__", f!(m, graph_init)),
            ("__del__", f!(m, graph_del)),
            ("__getstate__", f!(m, graph_getstate)),
            ("__setstate__", f!(m, graph_setstate)),
            ("nextColor", f!(m, nextColor)),
            ("setTicks", f!(m, setTicks)),
            ("setIntegerTicksFromData", f!(m, setIntegerTicksFromData)),
            ("setAxisRange", f!(m, setAxisRange)),
            ("setAxisLabel", f!(m, setAxisLabel)),
            ("applyFormatting", f!(m, applyFormatting)),
            ("applyGrid", f!(m, applyGrid)),
            ("applyFormattingToOneAxis", f!(m, applyFormattingToOneAxis)),
            ("process", f!(m, process)),
            ("renderSubplot", f!(m, renderSubplot)),
            ("callDoneAction", f!(m, callDoneAction)),
            ("show", f!(m, show)),
            ("write", f!(m, write)),
        ];
        spec.statics = vec![("hideAxisSpines", f!(m, hideAxisSpines))];
        spec.properties = vec![Property {
            name: "doneAction",
            getter: f!(m, done_action_get),
            setter: Some(f!(m, done_action_set)),
        }];
        let graph = spec.build(MODULE)?;
        m.add("Graph", &graph)?;

        let bars = configurables(py, &["barSpace", "margin"])?;
        graph_class(
            m,
            &graph,
            "GraphColorGrid",
            "A grid of coloured blocks, a row for each list of colours: the \
             results of a windowed analysis.",
            "colorGrid",
            &[
                ("figureSizeDefault", (9, 6).into_bound_py_any(py)?),
                (
                    "keywordConfigurables",
                    configurables(py, &["hideLeftBottomSpines"])?,
                ),
            ],
            vec![
                ("__init__", f!(m, color_grid_init)),
                ("renderSubplot", f!(m, color_grid_render)),
            ],
        )?;
        graph_class(
            m,
            &graph,
            "GraphColorGridLegend",
            "A legend of coloured blocks, a labelled row for each label and its \
             pairs of block label and colour.",
            "colorGridLegend",
            &[
                ("figureSizeDefault", (5, 1.5).into_bound_py_any(py)?),
                (
                    "keywordConfigurables",
                    configurables(py, &["hideLeftBottomSpines"])?,
                ),
            ],
            vec![
                ("__init__", f!(m, legend_init)),
                ("renderSubplot", f!(m, legend_render)),
                ("makeOneRowOfGraph", f!(m, makeOneRowOfGraph)),
            ],
        )?;
        let horizontal = graph_class(
            m,
            &graph,
            "GraphHorizontalBar",
            "Horizontal bars in a channel for each label, from each start for \
             each length; bars may overlap.",
            "horizontalBar",
            &[
                ("figureSizeDefault", (10, 4).into_bound_py_any(py)?),
                ("keywordConfigurables", bars.clone()),
            ],
            vec![
                ("__init__", f!(m, horizontal_init)),
                ("renderSubplot", f!(m, horizontal_render)),
            ],
        )?;
        let weighted = graph_class(
            m,
            &graph,
            "GraphHorizontalBarWeighted",
            "Horizontal bars in a channel for each label, each of its own height, \
             colour and opacity.",
            "horizontalBarWeighted",
            &[
                ("figureSizeDefault", (10, 4).into_bound_py_any(py)?),
                ("keywordConfigurables", bars),
            ],
            vec![
                ("__init__", f!(m, weighted_bar_init)),
                ("renderSubplot", f!(m, weighted_bar_render)),
            ],
        )?;
        let property = py.import("builtins")?.getattr("property")?;
        for class in [&horizontal, &weighted] {
            let getter = crate::plumbing::built::method(
                f!(m, bar_height),
                MODULE,
                "GraphHorizontalBar",
                "barHeight",
            )?;
            class.setattr("barHeight", property.call1((getter,))?)?;
        }
        let scatter_weighted = graph_class(
            m,
            &graph,
            "GraphScatterWeighted",
            "A scatter plot whose points are sized by how many values each holds.",
            "scatterWeighted",
            &[
                ("figureSizeDefault", (5, 5).into_bound_py_any(py)?),
                (
                    "keywordConfigurables",
                    configurables(py, &["maxDiameter", "minDiameter"])?,
                ),
            ],
            vec![
                ("__init__", f!(m, scatter_weighted_init)),
                ("renderSubplot", f!(m, scatter_weighted_render)),
            ],
        )?;
        let getter = crate::plumbing::built::method(
            f!(m, range_diameter),
            MODULE,
            "GraphScatterWeighted",
            "rangeDiameter",
        )?;
        scatter_weighted.setattr("rangeDiameter", property.call1((getter,))?)?;
        graph_class(
            m,
            &graph,
            "GraphScatter",
            "Two values against each other in a scatter plot.",
            "scatter",
            &[],
            vec![("renderSubplot", f!(m, scatter_render))],
        )?;
        graph_class(
            m,
            &graph,
            "GraphHistogram",
            "The count of each value, as bars.",
            "histogram",
            &[("keywordConfigurables", configurables(py, &["binWidth"])?)],
            vec![
                ("__init__", f!(m, histogram_init)),
                ("renderSubplot", f!(m, histogram_render)),
            ],
        )?;
        graph_class(
            m,
            &graph,
            "GraphGroupedVerticalBar",
            "A group of vertical bars for each label, a bar for each key of its \
             counts.",
            "groupedVerticalBar",
            &[(
                "keywordConfigurables",
                configurables(py, &["binWidth", "roundDigits", "groupLabelHeight"])?,
            )],
            vec![
                ("__init__", f!(m, grouped_init)),
                ("labelBars", f!(m, labelBars)),
                ("renderSubplot", f!(m, grouped_render)),
            ],
        )?;
        graph_class(
            m,
            &graph,
            "Graph3DBars",
            "Bars in 3D, one at each x and y, as tall as its z. Takes `alpha`, \
             `barWidth` and `barDepth`, which a point's format may override.",
            "3DBars",
            &[("axisKeys", super::strings(py, &["x", "y", "z"])?)],
            vec![
                ("__init__", f!(m, bars3d_init)),
                ("process", f!(m, bars3d_process)),
                ("renderSubplot", f!(m, bars3d_render)),
            ],
        )?;
        Ok::<_, PyErr>(m.clone().unbind())
    })?;
    Ok(held.bind(py).clone())
}
