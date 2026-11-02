/** The `all` shorthand (CSS Cascade 4): a CSS-wide keyword for every property Eye computes. */
use super::shorthand::SIDES;

/** The longhands `all` sets: every property Eye computes except `direction` and
 * `unicode-bidi`, which `all` leaves alone, and custom properties. */
pub(super) fn longhands() -> Vec<String> {
    let mut names: Vec<String> = ALL.iter().map(|name| name.to_string()).collect();
    names.extend(super::transition::LONGHANDS.map(str::to_string));
    for side in SIDES {
        names.extend([
            format!("margin-{side}"),
            format!("padding-{side}"),
            side.to_string(),
        ]);
        names.extend(PARTS.iter().map(|part| format!("border-{side}-{part}")));
    }
    names
}

/// The parts of each border side.
const PARTS: [&str; 3] = ["width", "style", "color"];

/// Properties `all` resets besides the per-side ones `longhands` adds.
const ALL: [&str; 79] = [
    "align-items",
    "align-self",
    "aspect-ratio",
    "backface-visibility",
    "background-attachment",
    "background-color",
    "background-image",
    "background-position",
    "background-repeat",
    "background-size",
    "background-origin",
    "background-clip",
    "border-bottom-left-radius",
    "border-bottom-right-radius",
    "border-collapse",
    "border-spacing",
    "border-top-left-radius",
    "border-top-right-radius",
    "box-shadow",
    "box-sizing",
    "caption-side",
    "clear",
    "clip",
    "clip-path",
    "color",
    "color-scheme",
    "column-gap",
    "cursor",
    "display",
    "flex-basis",
    "flex-direction",
    "flex-grow",
    "flex-shrink",
    "flex-wrap",
    "float",
    "font-family",
    "font-size",
    "font-style",
    "font-weight",
    "grid-column-end",
    "grid-column-start",
    "grid-row-end",
    "grid-row-start",
    "grid-template-areas",
    "grid-template-columns",
    "grid-template-rows",
    "height",
    "justify-content",
    "justify-items",
    "justify-self",
    "letter-spacing",
    "line-height",
    "list-style-type",
    "max-height",
    "max-width",
    "min-height",
    "min-width",
    "object-fit",
    "object-position",
    "opacity",
    "order",
    "perspective",
    "perspective-origin",
    "overflow-x",
    "overflow-y",
    "position",
    "row-gap",
    "tab-size",
    "text-align",
    "text-decoration-line",
    "text-indent",
    "text-transform",
    "transform",
    "transform-origin",
    "transform-style",
    "vertical-align",
    "white-space",
    "width",
    "z-index",
];
