/** Inherited computed values, CSS-wide keywords, and authored scheme negotiation. */
use super::{ColorScheme, ComputedStyle};
use crate::document::Appearance;
/** Noninherited box fields always start fresh for each formatting subject. */
pub(super) fn inherited(parent: &ComputedStyle) -> ComputedStyle {
    ComputedStyle {
        font_size: parent.font_size,
        bold: parent.bold,
        font_weight: parent.font_weight,
        color: parent.color.clone(),
        align: parent.align.clone(),
        hidden: parent.hidden,
        interactive: parent.interactive,
        inert: parent.inert,
        line_height: parent.line_height,
        color_scheme: parent.color_scheme,
        used_scheme: parent.used_scheme,
        rtl: parent.rtl,
        letter_spacing: parent.letter_spacing,
        word_spacing: parent.word_spacing,
        spacing_percent: parent.spacing_percent,
        text_indent: parent.text_indent,
        text_transform: parent.text_transform,
        white_space: parent.white_space,
        font_family: parent.font_family,
        font_families: parent.font_families.clone(),
        font_style: parent.font_style,
        cursor_pointer: parent.cursor_pointer,
        text_decoration_none: parent.text_decoration_none,
        underline: parent.underline,
        strikethrough: parent.strikethrough,
        list_style: parent.list_style,
        border_spacing: parent.border_spacing,
        border_collapse: parent.border_collapse,
        caption_bottom: parent.caption_bottom,
        vertical_writing: parent.vertical_writing,
        ..ComputedStyle::default()
    }
}
/** Properties CSS inherits by default, so `unset` means `inherit` for them. */
fn inherits(key: &str) -> bool {
    key.starts_with("font")
        || [
            "color",
            "line-height",
            "text-align",
            "letter-spacing",
            "word-spacing",
            "text-indent",
            "text-transform",
            "white-space",
            "cursor",
            "visibility",
            "color-scheme",
            "direction",
            "writing-mode",
            "list-style",
            "list-style-type",
            "border-spacing",
            "border-collapse",
            "caption-side",
            "pointer-events",
        ]
        .contains(&key)
}
/**
 * Resolves `inherit`, `initial`, `unset` and `revert` for one property by copying the parent's
 * or the initial field; properties without a copier keep their current value.
 */
pub(super) fn keyword(style: &mut ComputedStyle, parent: &ComputedStyle, key: &str, value: &str) {
    let value = value.trim().to_ascii_lowercase();
    let from_parent = value == "inherit" || (value != "initial" && inherits(key));
    let initial = ComputedStyle::default();
    let source = if from_parent { parent } else { &initial };
    copy(style, source, key);
}
/**
 * Copies one property's computed field, so a CSS-wide keyword works for every property Eye
 * computes. Properties that compute nothing (`content`, `outline`, `font-style`) and
 * `visibility`, which never un-hides an inherited hidden subtree, keep their current value.
 */
fn copy(style: &mut ComputedStyle, source: &ComputedStyle, key: &str) {
    if sided(style, source, key) || corner(style, source, key) || copy_box(style, source, key) {
        return;
    }
    if !copy_text(style, source, key) {
        copy_layout(style, source, key);
    }
}
/** Margins, paddings and offsets, one side each. */
fn sided(style: &mut ComputedStyle, source: &ComputedStyle, key: &str) -> bool {
    let side = |prefix: &str| {
        super::shorthand::SIDES
            .iter()
            .position(|s| key.strip_prefix(prefix) == Some(s))
    };
    if let Some(side) = side("margin-") {
        style.margin[side] = source.margin[side];
        style.margin_percent[side] = source.margin_percent[side];
        style.margin_auto[side] = source.margin_auto[side];
    } else if let Some(side) = side("padding-") {
        style.padding[side] = source.padding[side];
        style.padding_percent[side] = source.padding_percent[side];
    } else if let Some(side) = side("") {
        style.inset[side] = source.inset[side];
        style.inset_percent[side] = source.inset_percent[side];
    } else if let Some((side, part)) = super::border::longhand(key) {
        super::border::copy(style, source, side, part);
    } else {
        return false;
    }
    true
}
/** One corner radius. */
fn corner(style: &mut ComputedStyle, source: &ComputedStyle, key: &str) -> bool {
    let corners = [
        "border-top-left-radius",
        "border-top-right-radius",
        "border-bottom-right-radius",
        "border-bottom-left-radius",
    ];
    let Some(corner) = corners.iter().position(|corner| *corner == key) else {
        return false;
    };
    style.radius[corner] = source.radius[corner];
    style.radius_percent[corner] = source.radius_percent[corner];
    true
}
/** Box model, flex and grid fields. */
fn copy_box(style: &mut ComputedStyle, source: &ComputedStyle, key: &str) -> bool {
    match key {
        "width" => {
            style.width = source.width;
            style.percent_width = source.percent_width;
            style.width_offset = source.width_offset;
            style.width_stretch = source.width_stretch;
            style.width_fit = source.width_fit;
            style.size_keywords[0] = source.size_keywords[0];
        }
        "max-width" => {
            style.max_width = source.max_width;
            style.percent_max_width = source.percent_max_width;
            style.size_keywords[2] = source.size_keywords[2];
        }
        "min-width" => {
            style.min_width = source.min_width;
            style.min_width_percent = source.min_width_percent;
            style.size_keywords[1] = source.size_keywords[1];
        }
        "height" => style.height = source.height,
        "min-height" => style.min_height = source.min_height,
        "max-height" => style.max_height = source.max_height,
        "box-sizing" => style.box_sizing = source.box_sizing,
        "aspect-ratio" => style.aspect_ratio = source.aspect_ratio,
        "flex-direction" => {
            style.flex_direction = source.flex_direction;
            style.flex_reverse = source.flex_reverse;
        }
        "flex-wrap" => {
            style.flex_wrap = source.flex_wrap;
            style.wrap_reverse = source.wrap_reverse;
        }
        "flex-flow" => {
            style.flex_direction = source.flex_direction;
            style.flex_reverse = source.flex_reverse;
            style.flex_wrap = source.flex_wrap;
            style.wrap_reverse = source.wrap_reverse;
        }
        "place-items" => {
            style.align_items = source.align_items;
            style.justify_items = source.justify_items;
        }
        "place-self" => {
            style.align_self = source.align_self;
            style.justify_self = source.justify_self;
        }
        "place-content" => style.justify_content = source.justify_content,
        "flex-basis" => style.flex_basis = source.flex_basis,
        "flex-grow" => style.flex_grow = source.flex_grow,
        "flex-shrink" => style.flex_shrink = source.flex_shrink,
        "justify-content" => style.justify_content = source.justify_content,
        "align-items" => style.align_items = source.align_items,
        "align-self" => style.align_self = source.align_self,
        "justify-items" => style.justify_items = source.justify_items,
        "justify-self" => style.justify_self = source.justify_self,
        "order" => style.order = source.order,
        "row-gap" | "grid-row-gap" => style.row_gap = source.row_gap,
        "column-gap" | "grid-column-gap" => style.column_gap = source.column_gap,
        "grid-template-columns" => style.grid_columns = source.grid_columns.clone(),
        "grid-template-rows" => style.grid_rows = source.grid_rows.clone(),
        "grid-template-areas" => style.grid_areas = source.grid_areas.clone(),
        "grid-row" | "grid-row-start" | "grid-row-end" => {
            style.grid_row = source.grid_row;
            style.grid_area_names[0] = source.grid_area_names[0].clone();
            style.grid_line_names[0] = source.grid_line_names[0].clone();
        }
        "grid-column" | "grid-column-start" | "grid-column-end" => {
            style.grid_column = source.grid_column;
            style.grid_area_names[1] = source.grid_area_names[1].clone();
            style.grid_line_names[1] = source.grid_line_names[1].clone();
        }
        "container-type" => style.container_type = source.container_type,
        "container-name" => style.container_names = source.container_names.clone(),
        _ => return false,
    }
    true
}
/** Font, color and text fields. */
fn copy_text(style: &mut ComputedStyle, source: &ComputedStyle, key: &str) -> bool {
    match key {
        "color" => style.color = source.color.clone(),
        "font-size" => style.font_size = source.font_size,
        "direction" => style.rtl = source.rtl,
        "writing-mode" => style.vertical_writing = source.vertical_writing,
        "font-weight" => {
            style.font_weight = source.font_weight;
            style.bold = source.bold;
        }
        "font-family" => {
            style.font_family = source.font_family;
            style.font_families = source.font_families.clone();
        }
        "font-style" => style.font_style = source.font_style,
        "line-height" => style.line_height = source.line_height,
        "text-align" => style.align = source.align.clone(),
        "letter-spacing" => {
            style.letter_spacing = source.letter_spacing;
            style.spacing_percent[0] = source.spacing_percent[0];
        }
        "word-spacing" => {
            style.word_spacing = source.word_spacing;
            style.spacing_percent[1] = source.spacing_percent[1];
        }
        "text-indent" => style.text_indent = source.text_indent,
        "text-transform" => style.text_transform = source.text_transform,
        "white-space" => style.white_space = source.white_space,
        "cursor" => style.cursor_pointer = source.cursor_pointer,
        "color-scheme" => {
            style.color_scheme = source.color_scheme;
            style.used_scheme = source.used_scheme;
        }
        "text-decoration" | "text-decoration-line" => {
            style.text_decoration_none = source.text_decoration_none;
            style.underline = source.underline;
            style.strikethrough = source.strikethrough;
        }
        "list-style" | "list-style-type" => style.list_style = source.list_style,
        "border-spacing" => style.border_spacing = source.border_spacing,
        "border-collapse" => style.border_collapse = source.border_collapse,
        "caption-side" => style.caption_bottom = source.caption_bottom,
        _ => return false,
    }
    true
}
/** Display, positioning, paint and background fields. */
fn copy_layout(style: &mut ComputedStyle, source: &ComputedStyle, key: &str) {
    match key {
        "display" => {
            style.inline = source.inline;
            style.inline_block = source.inline_block;
            style.display_flex = source.display_flex;
            style.display_grid = source.display_grid;
            style.grid_lanes = source.grid_lanes;
            style.display_contents = source.display_contents;
            style.list_item = source.list_item;
            style.table_box = source.table_box;
            style.flow_root = source.flow_root;
        }
        "pointer-events" => style.interactive = source.interactive,
        "float" => style.float = source.float,
        "clear" => style.clear = source.clear,
        "vertical-align" => style.vertical_align = source.vertical_align,
        "opacity" => style.opacity = source.opacity,
        "position" => style.position = source.position,
        "z-index" => style.z_index = source.z_index,
        "overflow-x" | "overflow-y" => {
            style.overflow = source.overflow;
            style.overflow_x = source.overflow_x;
            style.overflow_y = source.overflow_y;
        }
        "box-shadow" => style.shadows = source.shadows.clone(),
        "transform" => {
            style.transform = source.transform;
            style.translate_percent = source.translate_percent;
        }
        "object-fit" => style.object_fit = source.object_fit,
        "object-position" => style.object_position = source.object_position,
        "clip" => style.clip_rect = source.clip_rect,
        "clip-path" => style.clip_path = source.clip_path,
        "overflow-clip-margin" => {
            style.overflow_clip_margin = source.overflow_clip_margin;
            style.overflow_clip_box = source.overflow_clip_box;
        }
        "background-color" => style.background = source.background.clone(),
        "background-image" => style.backgrounds = source.backgrounds.clone(),
        "background-size" => style.background_size = source.background_size,
        "background-position" => style.background_position = source.background_position,
        "background-repeat" => style.background_repeat = source.background_repeat,
        "mask-image" => {
            style.masked = source.masked;
            style.mask = source.mask.clone();
        }
        "mask-size" | "mask-position" | "mask-repeat" => {
            if let (Some(mask), Some(from)) = (style.mask.as_mut(), source.mask.as_ref()) {
                match key {
                    "mask-size" => mask.size = from.size,
                    "mask-position" => mask.position = from.position,
                    _ => mask.repeat = from.repeat,
                }
            }
        }
        _ => {}
    }
}
/** Single offered schemes win; either ordered pair admits the effective native preference. */
pub(super) fn used_scheme(scheme: ColorScheme, preference: Appearance) -> Appearance {
    match scheme {
        ColorScheme::Normal | ColorScheme::Light => Appearance::Light,
        ColorScheme::Dark => Appearance::Dark,
        ColorScheme::LightDark | ColorScheme::DarkLight => preference,
    }
}
/** UA link color outranks inherited canvas text; an author's own link declaration still wins. */
pub(super) fn link_foreground(style: &ComputedStyle) -> Option<String> {
    (style.color_scheme != ColorScheme::Normal || style.color.is_some()).then(|| {
        match style.used_scheme {
            Appearance::Light => "#0000ee".into(),
            Appearance::Dark => "#9e9eff".into(),
        }
    })
}
