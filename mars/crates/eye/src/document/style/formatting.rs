/** Non-target root/body formatting and inherited CSS values retain semantic distinctions until layout. */
use super::ComputedStyle;
use super::constants::NORMAL_LINE_HEIGHT;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Unitless factors inherit differently from lengths computed against a declaring font.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq)]
pub enum LineHeight {
    /// Chromium's normal advance for its default face.
    #[default]
    Normal,
    /// An inherited multiplier follows each descendant's own computed font size.
    Number(f64),
    /// Authored lengths and resolved percentages inherit as fixed logical points.
    Length(f64),
}
impl LineHeight {
    /// Measurement and painting consume one used line advance for the current font.
    pub fn used(self, font_size: f64) -> f64 {
        match self {
            Self::Normal => font_size * NORMAL_LINE_HEIGHT,
            Self::Number(value) => font_size * value,
            Self::Length(value) => value,
        }
    }
}

/// The bounded supported set remains distinct from the native preferred appearance.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ColorScheme {
    /// Unstyled native documents preserve semantic OS colors.
    #[default]
    Normal,
    /// Only the light authored scheme is supported.
    Light,
    /// Only the dark authored scheme is supported.
    Dark,
    /// Both schemes are offered with light first.
    LightDark,
    /// Both schemes are offered with dark first.
    DarkLight,
}

/// Wrapper attributes and computed styles do not create DOM identities or actionable targets.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct FormattingElement {
    /// Parsed attributes use the existing bounded attribute decoder.
    pub attributes: BTreeMap<String, String>,
    /// The owned cascade resolves each wrapper before its real descendants.
    pub style: ComputedStyle,
}

/// Document-wide formatting is separate from the author-visible native node tree.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct DocumentFormatting {
    /// Root inheritance and background propagation begin here.
    pub html: FormattingElement,
    /// One body box positions all of its actual children.
    pub body: FormattingElement,
    /// Winning authored box declarations replace the legacy native reading inset once.
    pub body_box_authored: bool,
    /// Some hiding rule could not be kept within the CSS bounds, so no node is agent-actionable.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub agent_untrusted: bool,
    /// The sheets' `@font-face` rules in source order, set by the cascade; `font-family` lists
    /// and the app's font loader both read them, so the files a page needs are decided once.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub font_faces: Vec<super::FontFace>,
    /// Font files the app unpacked for those rules. They travel with the document so the
    /// service that measures and the window that paints load the same bytes.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fonts: Vec<super::LoadedFont>,
    /// The page has no standards doctype, so Chromium lays it out in quirks mode (CERN's first
    /// page and Hacker News among them).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub quirks: bool,
}
