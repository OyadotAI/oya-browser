/** Explicit computed flex values distinguish automatic sizes from definite pixels and percentages. */
use serde::{Deserialize, Serialize};
/// CSS widths name either the content box or the complete border box.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BoxSizing {
    ContentBox,
    BorderBox,
}
/// Supported main axes retain source order and do not include reverse flow.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FlexDirection {
    Row,
    Column,
}
/// Wrapped rows form independently sized lines; wrapped columns remain explicitly unsupported.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FlexWrap {
    NoWrap,
    Wrap,
}
/// A zero percentage stays distinct from zero pixels when a column's main extent is indefinite.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FlexBasis {
    Auto,
    /// `content`: sized from the item's content even when it has an authored main size, and
    /// never definite, so a column item's percentage children stay unresolved.
    Content,
    Pixels(f64),
    Percent(f64),
}
/// Content distribution (`justify-content`, `align-content`), as Blink's
/// `StyleContentAlignmentData`: a position or a distribution with its fallback.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum JustifyContent {
    /// `normal` and `stretch`: flex lines and `auto` grid tracks take the free space; items in
    /// a flex line pack at the start.
    Stretch,
    /// `flex-start`: the main-start edge, the far end on a reversed axis.
    Start,
    Center,
    /// `flex-end`.
    End,
    /// `start`: the container's own start edge whatever the flex direction (Blink's `kStart`),
    /// so `row-reverse` with `start` packs items at the left.
    LogicalStart,
    /// `end`: the container's own end edge (Blink's `kEnd`).
    LogicalEnd,
    /// `left`: the start edge in a left-to-right row, the end in a right-to-left one, and the
    /// start in a column (Blink's `ResolvedJustifyContent`).
    Left,
    /// `right`: the mirror of `left`.
    Right,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}
/// Supported cross-axis alignment excludes baseline and per-item overrides.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AlignItems {
    Start,
    Center,
    End,
    #[default]
    Stretch,
    /// First-baseline alignment in a flex line; grid areas treat it as `start`.
    Baseline,
}
impl JustifyContent {
    /** The initial value, `normal`. */
    pub fn normal() -> Self {
        Self::Stretch
    }
}
