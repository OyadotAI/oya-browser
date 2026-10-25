/**
 * Owned best-effort CSS cascade. Unsupported properties, values, selectors and at-rules are
 * dropped with a value-free diagnostic, as CSS requires; only inputs that break engine bounds
 * refuse the page. Dropped rules that could have hidden content withdraw agent actionability.
 */
mod animation;
mod apply;
mod border;
mod bucket;
mod calc;
mod cascade;
mod clip;
mod color_spaces;
mod colors;
mod compute;
mod constants;
mod container;
mod content;
mod declaration;
mod flex;
mod flex_model;
mod font;
mod font_face;
mod font_match;
mod formatting;
mod hiding;
mod hints;
mod layer;
mod layout_props;
mod logical;
mod media;
mod model;
mod paint;
mod parsed;
mod resolve;
mod selector_parse;
mod selectors;
mod sheet;
mod shorthand;
mod subjects;
mod syntax;
mod ua;
mod urls;
mod validation;
mod values;
mod vars;
mod visual;
mod visual_model;
use crate::constants::DIAGNOSTICS;
use crate::protocol::EngineError;
pub use border::BorderStyle;
pub use cascade::{compute_styles, compute_styles_with_context};
pub use colors::painter_rgba;
pub use flex_model::{AlignItems, BoxSizing, FlexBasis, FlexDirection, FlexWrap, JustifyContent};
pub use font_face::{
    FONT_FACES, FONT_FILE_BYTES, FONT_FILES, FONT_NAME_BYTES, FONT_TOTAL_BYTES, FontDisplay,
    FontFace, FontSource, FontStyle, LoadedFont, content_key,
};
pub(crate) use font_match::select as select_font;
pub use font_match::{FontRequest, font_requests, install_fonts};
pub use formatting::{ColorScheme, DocumentFormatting, FormattingElement, LineHeight};
pub use model::{ComputedStyle, Generated, SizeKeyword, StyleSource};
pub use urls::{BACKGROUND_IMAGES, background_urls, rebase_urls};
pub use visual_model::{
    BackgroundLayer, BackgroundRepeat, BackgroundSize, BoxShadow, Clear, ColorStop, ContainerType,
    Extent, Float, FontFamily, GridArea, GridLine, GridTemplate, ListStyle, MaskLayer, ObjectFit,
    Overflow, Position, RadialShape, RadialSize, RepeatStyle, TableBox, TextTransform, Track,
    TrackSize, VerticalAlign, WhiteSpace,
};
pub use visual_model::{ContentKeyword, TrackContent};
/** Internal value failures; never include arbitrary declarations or selectors in an error. */
fn unsupported() -> EngineError {
    EngineError::unsupported(
        "This page uses a CSS selector, condition, value or layout property Eye does not support yet.",
    )
}
/** Records one value-free diagnostic once, within the document's bound. */
fn note(diagnostics: &mut Vec<String>, message: String) {
    if diagnostics.len() < DIAGNOSTICS && !diagnostics.contains(&message) {
        diagnostics.push(message);
    }
}
/** Author-chosen names appear in diagnostics only when they look like plain CSS identifiers. */
fn diagnostic_name(name: &str) -> &str {
    if !name.is_empty()
        && name.len() <= constants::DIAGNOSTIC_NAME
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        name
    } else {
        "(unnamed)"
    }
}
/** SVG paint shares the CSS color parser and its painter literal output. */
pub(crate) use values::color as literal_color;
/** A link's `media` list that holds by media type alone (e.g. `not print`); feature queries stay unevaluated. */
pub(crate) fn linked_media_applies(media: &str) -> bool {
    !media.contains('(')
        && media::media(media, crate::document::RenderContext::default()) == media::Truth::True
}
