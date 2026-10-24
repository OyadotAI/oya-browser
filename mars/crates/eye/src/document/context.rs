/** Host presentation facts are explicit data, never a page-granted capability. */
use crate::{
    constants::{DEFAULT_VIEWPORT_HEIGHT, DEFAULT_VIEWPORT_WIDTH, MIN_VIEWPORT_HEIGHT},
    protocol::{EngineError, EngineResult},
};
use serde::{Deserialize, Serialize};

/// CSS chooses among supported schemes using the actual native window preference.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Appearance {
    /// Legacy headless callers start in a deterministic light environment.
    #[default]
    Light,
    /// A native dark preference does not override an author's single supported light scheme.
    Dark,
}

/// The same viewport and preference drive computed values, layout and the visible snapshot.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RenderContext {
    /// Logical width excludes the native Mars shell (toolbar, tabs, status).
    pub width: f64,
    /// Viewport-relative units use visible page height, never the document's scroll extent.
    pub height: f64,
    /// Effective native appearance is supplied by the shell rather than inferred from CSS.
    pub appearance: Appearance,
}
impl Default for RenderContext {
    /** Existing public facades preserve their deterministic initial viewport. */
    fn default() -> Self {
        Self {
            width: DEFAULT_VIEWPORT_WIDTH,
            height: DEFAULT_VIEWPORT_HEIGHT,
            appearance: Appearance::Light,
        }
    }
}
impl RenderContext {
    /// Reject malformed private-wire context before it can influence native geometry.
    pub fn validate(self) -> EngineResult<()> {
        if !self.width.is_finite()
            || !self.height.is_finite()
            || !(super::render::MIN_VIEWPORT..=super::render::MAX_VIEWPORT).contains(&self.width)
            || !(MIN_VIEWPORT_HEIGHT..=super::render::MAX_VIEWPORT).contains(&self.height)
        {
            return Err(EngineError::invalid("Invalid native render context."));
        }
        Ok(())
    }
}
