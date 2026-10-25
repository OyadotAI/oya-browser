/** Operating-system text primitives supply shaping, never document or navigation semantics. */
#[cfg(target_os = "macos")]
mod text;
use serde::{Deserialize, Serialize};
#[cfg(target_os = "macos")]
pub use text::{
    attributed, colored, directed, edges, font, has_font, local_font_exists, measure, metrics,
    register_family, register_font, register_local, styled,
};

/// The generic face text is shaped and drawn with; System is the native interface font.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum Family {
    /// The platform interface face, kept for controls and display lists that predate families.
    #[default]
    System,
    /// Chromium's default document face (Times on macOS).
    Serif,
    /// Helvetica on macOS.
    SansSerif,
    /// Menlo on macOS.
    Monospace,
    /// A page's own font, by the registry key of its loaded file or installed `local()` name;
    /// measurement and painting both resolve it through this process's font registry.
    Web(u64),
}

/// One font selection shared by measurement and painting, so both shape identically.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Face {
    /// Font size in points.
    pub size: f64,
    /// CSS numeric weight, 1..1000.
    pub weight: u16,
    /// Generic family.
    pub family: Family,
    /// Extra advance after every character, in points; may be negative.
    pub spacing: f64,
    /// Italic or oblique posture.
    pub italic: bool,
}
impl Face {
    /// The legacy selection: system face, regular or bold, no extra spacing.
    pub fn system(size: f64, bold: bool) -> Self {
        Self {
            size,
            weight: if bold { 700 } else { 400 },
            family: Family::System,
            spacing: 0.0,
            italic: false,
        }
    }
    /// CSS treats 600 and above as bold for faces with only two weights.
    pub fn bold(&self) -> bool {
        self.weight >= 600
    }
}

/// Headless builds hold no font registry: web fonts fall back to the deterministic metrics.
#[cfg(not(target_os = "macos"))]
pub fn has_font(_key: u64) -> bool {
    false
}
/// Headless builds load no font bytes.
#[cfg(not(target_os = "macos"))]
pub fn register_font(_key: u64, _bytes: &[u8]) -> bool {
    false
}
/// Headless builds have no installed fonts to name.
#[cfg(not(target_os = "macos"))]
pub fn register_local(_key: u64, _name: &str) -> bool {
    false
}
/// Headless builds have no installed families.
#[cfg(not(target_os = "macos"))]
pub fn register_family(_key: u64, _name: &str) -> bool {
    false
}
/// Headless builds have no installed fonts to name.
#[cfg(not(target_os = "macos"))]
pub fn local_font_exists(_name: &str) -> bool {
    false
}

/// Headless non-macOS builds keep deterministic geometry without claiming GUI parity.
#[cfg(not(target_os = "macos"))]
pub fn measure(text: &str, face: Face) -> f64 {
    let count = text.chars().count() as f64;
    count * face.size * if face.bold() { 0.65 } else { 0.6 } + count * face.spacing
}

/// Headless builds use a fixed cap height and x-height.
#[cfg(not(target_os = "macos"))]
pub fn edges(face: Face) -> (f64, f64) {
    (face.size * 0.7, face.size * 0.5)
}

/// Headless builds use a fixed ascender and normal line height.
#[cfg(not(target_os = "macos"))]
pub fn metrics(face: Face) -> (f64, f64) {
    (face.size * 0.9, face.size * 1.15)
}
