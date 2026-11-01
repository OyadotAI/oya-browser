/** Inherited computed values, CSS-wide keywords, and authored scheme negotiation. */
use super::{ColorScheme, ComputedStyle};
use crate::document::Appearance;
/** Noninherited box fields always start fresh for each formatting subject. */
pub(crate) fn inherited(parent: &ComputedStyle) -> ComputedStyle {
    ComputedStyle {
        font_size: parent.font_size,
        font_keyword: parent.font_keyword,
        font_size_absolute: parent.font_size_absolute,
        bold: parent.bold,
        font_weight: parent.font_weight,
        color: parent.color.clone(),
        align: parent.align.clone(),
        align_last: parent.align_last.clone(),
        text_justify: parent.text_justify.clone(),
        text_autospace: parent.text_autospace,
        lang: parent.lang.clone(),
        hyphens_none: parent.hyphens_none,
        hyphenate_character: parent.hyphenate_character.clone(),
        parent_rtl: Some(parent.rtl),
        hidden: parent.hidden,
        invisible: parent.invisible,
        interactive: parent.interactive,
        inert: parent.inert,
        line_height: parent.line_height,
        color_scheme: parent.color_scheme,
        used_scheme: parent.used_scheme,
        rtl: parent.rtl,
        letter_spacing: parent.letter_spacing,
        text_shadows: parent.text_shadows.clone(),
        word_spacing: parent.word_spacing,
        spacing_percent: parent.spacing_percent,
        text_indent: parent.text_indent,
        text_transform: parent.text_transform,
        text_transform_full_width: parent.text_transform_full_width,
        text_transform_full_size_kana: parent.text_transform_full_size_kana,
        white_space: parent.white_space,
        break_spaces: parent.break_spaces,
        tab_size: parent.tab_size,
        font_family: parent.font_family,
        font_families: parent.font_families.clone(),
        font_style: parent.font_style,
        cursor_pointer: parent.cursor_pointer,
        text_decoration_none: parent.text_decoration_none,
        underline: parent.underline,
        strikethrough: parent.strikethrough,
        list_style: parent.list_style,
        list_marker: parent.list_marker.clone(),
        border_spacing: parent.border_spacing,
        border_spacing_rows: parent.border_spacing_rows,
        border_collapse: parent.border_collapse,
        block_ellipsis: parent.block_ellipsis.clone(),
        caption_bottom: parent.caption_bottom,
        vertical_writing: parent.vertical_writing,
        writing_lr: parent.writing_lr,
        writing_sideways: parent.writing_sideways,
        text_orientation: parent.text_orientation,
        text_box_edge: parent.text_box_edge,
        kept_values: super::kept_values::inherited(parent),
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
            "text-align-last",
            "text-justify",
            "text-autospace",
            "hyphens",
            "hyphenate-character",
            "letter-spacing",
            "word-spacing",
            "text-indent",
            "text-transform",
            "text-shadow",
            "white-space",
            "cursor",
            "visibility",
            "color-scheme",
            "direction",
            "writing-mode",
            "text-orientation",
            "list-style",
            "list-style-type",
            "border-spacing",
            "border-collapse",
            "caption-side",
            "pointer-events",
            "text-box-edge",
        ]
        .contains(&key)
        || super::kept_values::inherits(key)
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
    if super::transition::copy(&mut style.transition, &source.transition, key)
        || super::kept_values::copy(style, source, key)
    {
        return;
    }
    if let Some(index) = super::calc_size::KEYS.iter().position(|name| *name == key) {
        style.calc_size[index] = source.calc_size[index];
    }
    if sided(style, source, key)
        || corner(style, source, key)
        || copy_box(style, source, key)
        || super::multicol::copy(style, source, key)
    {
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
    style.radius_y[corner] = source.radius_y[corner];
    style.radius_px[corner] = source.radius_px[corner];
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
        "height" => {
            style.height = source.height;
            style.height_fit = source.height_fit;
            style.height_stretch[0] = source.height_stretch[0];
        }
        "min-height" => {
            style.min_height = source.min_height;
            style.height_bounds_fit[0] = source.height_bounds_fit[0];
            style.height_stretch[1] = source.height_stretch[1];
        }
        "max-height" => {
            style.max_height = source.max_height;
            style.height_bounds_fit[1] = source.height_bounds_fit[1];
            style.height_stretch[2] = source.height_stretch[2];
        }
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
            style.align_stretch[0] = source.align_stretch[0];
            style.align_safe[0] = source.align_safe[0];
            style.align_side[0] = source.align_side[0];
            style.justify_items = source.justify_items;
            style.align_stretch[2] = source.align_stretch[2];
            style.align_safe[2] = source.align_safe[2];
            style.align_side[2] = source.align_side[2];
        }
        "place-self" => {
            style.align_self = source.align_self;
            style.align_self_safe = source.align_self_safe;
            style.align_stretch[1] = source.align_stretch[1];
            style.align_safe[1] = source.align_safe[1];
            style.align_side[1] = source.align_side[1];
            style.justify_self = source.justify_self;
            style.align_stretch[3] = source.align_stretch[3];
            style.align_safe[3] = source.align_safe[3];
            style.align_side[3] = source.align_side[3];
        }
        "place-content" => style.justify_content = source.justify_content,
        "flex-basis" => style.flex_basis = source.flex_basis,
        "flex-grow" => style.flex_grow = source.flex_grow,
        "flex-shrink" => style.flex_shrink = source.flex_shrink,
        "justify-content" => {
            style.justify_content = source.justify_content;
            style.content_safe[1] = source.content_safe[1];
        }
        "align-items" => {
            style.align_items = source.align_items;
            style.align_stretch[0] = source.align_stretch[0];
            style.align_safe[0] = source.align_safe[0];
            style.align_side[0] = source.align_side[0];
        }
        "align-self" => {
            style.align_self = source.align_self;
            style.align_self_safe = source.align_self_safe;
            style.align_stretch[1] = source.align_stretch[1];
            style.align_safe[1] = source.align_safe[1];
            style.align_side[1] = source.align_side[1];
        }
        "justify-items" => {
            style.justify_items = source.justify_items;
            style.align_stretch[2] = source.align_stretch[2];
            style.align_safe[2] = source.align_safe[2];
            style.align_side[2] = source.align_side[2];
        }
        "justify-self" => {
            style.justify_self = source.justify_self;
            style.align_stretch[3] = source.align_stretch[3];
            style.align_safe[3] = source.align_safe[3];
            style.align_side[3] = source.align_side[3];
        }
        "order" => style.order = source.order,
        "row-gap" | "grid-row-gap" => {
            style.row_gap = source.row_gap;
            style.gap_set[1] = source.gap_set[1];
        }
        "column-gap" | "grid-column-gap" => {
            style.column_gap = source.column_gap;
            style.gap_set[0] = source.gap_set[0];
        }
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
        "contain-intrinsic-width" => style.contain_intrinsic[0] = source.contain_intrinsic[0],
        "contain-intrinsic-height" => style.contain_intrinsic[1] = source.contain_intrinsic[1],
        _ => return false,
    }
    true
}
/** Font, color and text fields. */
fn copy_text(style: &mut ComputedStyle, source: &ComputedStyle, key: &str) -> bool {
    match key {
        "color" => style.color = source.color.clone(),
        "font-size" => {
            style.font_size = source.font_size;
            style.font_keyword = source.font_keyword;
            style.font_size_absolute = source.font_size_absolute;
        }
        "direction" => style.rtl = source.rtl,
        "writing-mode" => {
            style.vertical_writing = source.vertical_writing;
            style.writing_lr = source.writing_lr;
            style.writing_sideways = source.writing_sideways;
        }
        "text-orientation" => style.text_orientation = source.text_orientation,
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
        "text-align-last" => style.align_last = source.align_last.clone(),
        "text-justify" => style.text_justify = source.text_justify.clone(),
        "text-autospace" => style.text_autospace = source.text_autospace,
        "hyphens" => style.hyphens_none = source.hyphens_none,
        "hyphenate-character" => {
            style.hyphenate_character = source.hyphenate_character.clone();
        }
        "letter-spacing" => {
            style.letter_spacing = source.letter_spacing;
            style.spacing_percent[0] = source.spacing_percent[0];
        }
        "word-spacing" => {
            style.word_spacing = source.word_spacing;
            style.spacing_percent[1] = source.spacing_percent[1];
        }
        "text-indent" => style.text_indent = source.text_indent,
        "text-transform" => {
            style.text_transform = source.text_transform;
            style.text_transform_full_width = source.text_transform_full_width;
            style.text_transform_full_size_kana = source.text_transform_full_size_kana;
        }
        "white-space" => {
            style.white_space = source.white_space;
            style.break_spaces = source.break_spaces;
        }
        "tab-size" => style.tab_size = source.tab_size,
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
        "list-style" | "list-style-type" => {
            style.list_style = source.list_style;
            style.list_marker = source.list_marker.clone();
        }
        "border-spacing" => {
            style.border_spacing = source.border_spacing;
            style.border_spacing_rows = source.border_spacing_rows;
        }
        "border-collapse" => style.border_collapse = source.border_collapse,
        "caption-side" => style.caption_bottom = source.caption_bottom,
        _ => return false,
    }
    true
}
/** Display, positioning, paint and background fields. */
fn copy_layout(style: &mut ComputedStyle, source: &ComputedStyle, key: &str) {
    if super::border_image::copy(style, source, key) {
        return;
    }
    match key {
        "display" => {
            style.inline = source.inline;
            style.inline_block = source.inline_block;
            style.display_flex = source.display_flex;
            style.display_grid = source.display_grid;
            style.grid_lanes = source.grid_lanes;
            style.display_contents = source.display_contents;
            style.display_none = source.display_none;
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
        "text-shadow" => style.text_shadows = source.text_shadows.clone(),
        "transform" => {
            style.transform = source.transform;
            style.transform_4 = source.transform_4;
            style.translate_percent = source.translate_percent;
            style.translate_percent_cross = source.translate_percent_cross;
            style.transform_animation_excluded = source.transform_animation_excluded;
            style.rotation_radians = source.rotation_radians;
            style.scale_factors = source.scale_factors;
        }
        "transform-origin" => {
            style.transform_origin = source.transform_origin;
            style.transform_origin_z = source.transform_origin_z;
        }
        "perspective" => style.perspective = source.perspective,
        "perspective-origin" => style.perspective_origin = source.perspective_origin,
        "transform-style" => style.preserve_3d = source.preserve_3d,
        "backface-visibility" => style.backface_hidden = source.backface_hidden,
        "object-fit" => style.object_fit = source.object_fit,
        "object-position" => style.object_position = source.object_position,
        "clip" => style.clip_rect = source.clip_rect,
        "clip-path" => style.clip_path = source.clip_path,
        "overflow-clip-margin" => {
            style.overflow_clip_margin = source.overflow_clip_margin;
            style.overflow_clip_box = source.overflow_clip_box;
        }
        "background-color" => style.background = source.background.clone(),
        "background-clip" | "-webkit-background-clip" => {
            style.background_clip = source.background_clip;
        }
        "background-origin" | "-webkit-background-origin" => {
            style.background_origin = source.background_origin;
        }
        "background-image" => {
            style.backgrounds = source.backgrounds.clone();
            style.background_geometry.slots = source.background_geometry.slots.clone();
        }
        "background-size" => {
            style.background_size = source.background_size;
            style.background_geometry.sizes = source.background_geometry.sizes.clone();
        }
        "background-position" => {
            style.background_position = source.background_position;
            style.background_geometry.positions = source.background_geometry.positions.clone();
        }
        "background-repeat" => {
            style.background_repeat = source.background_repeat;
            style.background_geometry.repeats = source.background_geometry.repeats.clone();
        }
        "background-attachment" => {
            style.background_fixed = source.background_fixed;
            style.background_local = source.background_local;
        }
        "initial-letter" => style.initial_letter = source.initial_letter,
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
