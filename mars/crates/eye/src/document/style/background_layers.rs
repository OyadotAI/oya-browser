/** Per-layer `background-size`, `-position` and `-repeat` lists, after CSS Backgrounds: the
 * Nth comma-separated value belongs to the Nth `background-image` layer, a short list repeating.
 * The single first-layer fields stay as image layers use them; gradient layers read their own
 * entry here, so stacked gradient squares (`linear-gradient(...) 1ch 0 / 1ch 1ch no-repeat,
 * ...`) each land where their layer puts them. */
use super::{BackgroundRepeat, BackgroundSize, ComputedStyle, Extent, syntax::split_top};
use crate::{document::RenderContext, protocol::EngineResult};
use serde::{Deserialize, Serialize};

/// The comma-separated geometry lists of a box's background layers, and which `background-image`
/// entry each kept layer came from (unsupported and `none` entries are not kept as layers).
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct LayerGeometry {
    pub sizes: Vec<BackgroundSize>,
    pub positions: Vec<[Extent; 2]>,
    pub repeats: Vec<BackgroundRepeat>,
    /// For each kept background layer, its index in the `background-image` list.
    pub slots: Vec<usize>,
}

impl LayerGeometry {
    /** Whether no list was set: serialization leaves the field out. */
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/** The list entry for source layer `slot`, cycling a short list; None for an empty list. */
fn pick<T: Copy>(list: &[T], slot: usize) -> Option<T> {
    (!list.is_empty()).then(|| list[slot % list.len()])
}

/** Size, position and repeat of kept layer `layer` of `style`, falling back to the first-layer
 * fields where a list is missing. */
pub(crate) fn geometry(
    style: &ComputedStyle,
    layer: usize,
) -> (BackgroundSize, [Extent; 2], BackgroundRepeat) {
    let lists = &style.background_geometry;
    let slot = lists.slots.get(layer).copied().unwrap_or(layer);
    (
        pick(&lists.sizes, slot).unwrap_or(style.background_size),
        pick(&lists.positions, slot).unwrap_or(style.background_position),
        pick(&lists.repeats, slot).unwrap_or(style.background_repeat),
    )
}

/** Each comma-separated entry of `value` parsed by `one`; an invalid entry fails the list. */
pub(super) fn list<T>(
    value: &str,
    mut one: impl FnMut(&str) -> EngineResult<T>,
) -> EngineResult<Vec<T>> {
    split_top(value, b',')
        .into_iter()
        .map(|entry| one(entry.trim()))
        .collect()
}

/** `background-position`'s entries, each a position as `paint::position` reads one. */
pub(super) fn positions(
    value: &str,
    font: f64,
    context: RenderContext,
) -> EngineResult<Vec<[Extent; 2]>> {
    list(value, |entry| super::paint::position(entry, font, context))
}
