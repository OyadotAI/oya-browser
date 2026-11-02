/** Immutable computed values travel with document snapshots without copying every CSS field. */
use super::ComputedStyle;
use serde::{Deserialize, Serialize};
use std::{
    ops::{Deref, DerefMut},
    sync::{Arc, OnceLock},
};

/** A node keeps one pointer to its computed values; editing a candidate detaches only its style. */
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(transparent)]
pub struct SharedStyle(Arc<ComputedStyle>);

impl Default for SharedStyle {
    fn default() -> Self {
        static DEFAULT: OnceLock<Arc<ComputedStyle>> = OnceLock::new();
        Self(
            DEFAULT
                .get_or_init(|| Arc::new(ComputedStyle::default()))
                .clone(),
        )
    }
}

impl From<ComputedStyle> for SharedStyle {
    fn from(style: ComputedStyle) -> Self {
        Self(Arc::new(style))
    }
}

impl Deref for SharedStyle {
    type Target = ComputedStyle;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for SharedStyle {
    fn deref_mut(&mut self) -> &mut Self::Target {
        Arc::make_mut(&mut self.0)
    }
}

impl PartialEq for SharedStyle {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0) || self.0 == other.0
    }
}
