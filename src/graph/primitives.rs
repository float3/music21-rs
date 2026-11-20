//! The layout numbers music21's graph primitives draw with: where the
//! ticks of a count go, how big a weighted point is, and where a bar of a
//! horizontal bar graph sits in its channel.

use crate::defaults::FloatType;

use super::py_range;

/// The step a count's ticks take at the least.
const MIN_COUNT_STEP: i64 = 2;

/// The bar-graph ticks run about a tenth of the span apart.
const BAR_TICK_DIVISIONS: FloatType = 10.0;

/// How far right of its point a weighted point's count is written, in
/// units of the axis.
const LABEL_SHIFT_X: FloatType = 0.05;

/// How far above its point a weighted point's count is written.
const LABEL_SHIFT_Y: FloatType = 0.10;

/// Ticks for a count from nought to the most of `data`, about `steps`
/// apart: music21's `Graph.setIntegerTicksFromData`.
///
/// ```
/// use music21::graph::primitives::integer_ticks;
///
/// assert_eq!(integer_ticks(&[10.0, 5.0, 3.0, 8.0, 20.0, 11.0], 4), vec![0, 5, 10, 15, 20]);
/// ```
pub fn integer_ticks(data: &[FloatType], steps: i64) -> Vec<i64> {
    let most = data
        .iter()
        .copied()
        .fold(FloatType::NEG_INFINITY, FloatType::max) as i64;
    let mut step = (most as FloatType / steps as FloatType).round_ties_even() as i64;
    if step <= 1 {
        step = MIN_COUNT_STEP;
    }
    py_range(0, most + 1, step)
}

/// The x ticks a horizontal bar graph makes for itself when it is given
/// none: music21's `GraphHorizontalBar.renderSubplot`.
///
/// ```
/// use music21::graph::primitives::bar_x_ticks;
///
/// assert_eq!(bar_x_ticks(0.0, 20.0), vec![0, 2, 4, 6, 8, 10, 12, 14, 16, 18]);
/// ```
pub fn bar_x_ticks(min: FloatType, max: FloatType) -> Vec<i64> {
    let span = max - min;
    let mut step = (min + (span / BAR_TICK_DIVISIONS).round_ties_even()).trunc() as i64;
    if step == 0 {
        step = 1;
    }
    py_range(
        min.floor() as i64,
        max.ceil().round_ties_even() as i64,
        step,
    )
}

/// A weighted point as drawn: an ellipse, and where its count is written
/// where it is written at all.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeightedPoint {
    /// The ellipse's width, in units of the x axis.
    pub width: FloatType,
    /// Its height, in units of the y axis.
    pub height: FloatType,
    /// Where the count is written, for a count over one.
    pub label_at: Option<(FloatType, FloatType)>,
}

/// Each point of a weighted scatter graph, sized by its count between the
/// two diameters and stretched so it draws round on unequal axes: music21's
/// `GraphScatterWeighted.renderSubplot`.
///
/// ```
/// use music21::graph::primitives::weighted_points;
///
/// let drawn = weighted_points(&[(0.0, 0.0, 1.0), (10.0, 10.0, 3.0)], 0.25, 1.25);
/// assert_eq!(drawn[0].width, 0.25);
/// assert_eq!(drawn[1].width, 1.25);
/// assert!(drawn[0].label_at.is_none());
/// ```
pub fn weighted_points(
    points: &[(FloatType, FloatType, FloatType)],
    min_diameter: FloatType,
    max_diameter: FloatType,
) -> Vec<WeightedPoint> {
    let span = |values: Vec<FloatType>| -> (FloatType, FloatType) {
        let low = values
            .iter()
            .copied()
            .fold(FloatType::INFINITY, FloatType::min);
        let high = values
            .iter()
            .copied()
            .fold(FloatType::NEG_INFINITY, FloatType::max);
        (low, high - low)
    };
    let (_, x_range) = span(points.iter().map(|p| p.0).collect());
    let (_, y_range) = span(points.iter().map(|p| p.1).collect());
    let (z_min, z_range) = span(points.iter().map(|p| p.2).collect());

    // A circle on axes of unequal span is drawn narrower along the longer.
    let mut x_distort = 1.0;
    let mut y_distort = 1.0;
    if x_range > y_range {
        y_distort = y_range / x_range;
    } else if y_range > x_range {
        x_distort = x_range / y_range;
    }

    let range_diameter = max_diameter - min_diameter;
    points
        .iter()
        .map(|&(x, y, z)| {
            let diameter = if z == 0.0 {
                0.0
            } else if z_range != 0.0 {
                min_diameter + range_diameter * (z - z_min) / z_range
            } else {
                min_diameter + range_diameter * 0.5
            };
            let width = diameter * x_distort;
            let height = diameter * y_distort;
            let label = (
                x + (width * 0.5 + LABEL_SHIFT_X * x_distort),
                y + LABEL_SHIFT_Y,
            );
            let label_at = (z > 1.0).then_some(label);
            WeightedPoint {
                width,
                height,
                label_at,
            }
        })
        .collect()
}

/// A weighted horizontal bar's channel and how it sits in it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Channel {
    /// Where the channel starts.
    pub start: FloatType,
    /// The space above and below its bars.
    pub margin: FloatType,
    /// The height of a full bar.
    pub bar_height: FloatType,
}

/// Where a bar of a weighted horizontal bar graph starts and how tall it
/// is, `scale` of a full bar, shifted up or down by `shift` from -1 to 1:
/// music21's `GraphHorizontalBarWeighted.renderSubplot`.
///
/// ```
/// use music21::graph::primitives::{Channel, weighted_bar};
///
/// let channel = Channel { start: 0.0, margin: 0.25, bar_height: 7.5 };
/// assert_eq!(weighted_bar(channel, 1.0, 0.0), (0.25, 7.5));
/// assert_eq!(weighted_bar(channel, 0.5, 0.0), (2.125, 3.75));
/// ```
pub fn weighted_bar(
    channel: Channel,
    scale: FloatType,
    shift: FloatType,
) -> (FloatType, FloatType) {
    let height = channel.bar_height * scale;
    let centring = (channel.bar_height - height) * 0.5;
    let shift_unit = channel.bar_height * (1.0 - scale) * 0.5;
    let start = channel.start + channel.margin + centring + shift_unit * shift;
    (start, height)
}
