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
    Pixels(f64),
    Percent(f64),
}
/// Positive free main-axis space can be placed or distributed between existing items.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum JustifyContent {
    Start,
    Center,
    End,
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
}
