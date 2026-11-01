/** Native controls receive the exact border boxes returned by owned layout. */
use super::flow::*;
use crate::platform::Face;

/// Chromium's `-webkit-small-control` font size for controls that do not set their own.
pub(super) const CONTROL_FONT: f64 = 13.333;
/// Default text field border box, as Chromium's macOS theme draws `size=20`.
const FIELD_SIZE: (f64, f64) = (148.0, 21.0);
/// Default textarea border box (`cols=20 rows=2`).
const TEXTAREA_SIZE: (f64, f64) = (182.0, 37.0);
/// Checkboxes and radios are fixed native glyph squares.
const CHOICE_SIZE: f64 = 13.0;
/// Horizontal padding plus border a push button adds around its measured title.
const BUTTON_EDGES: f64 = 16.0;
/// A popup adds its arrow cell and padding beside the selected option's title.
const SELECT_EDGES: f64 = 28.0;
/// Default push button and popup height.
const BUTTON_HEIGHT: f64 = 21.0;
/// Inner inset between a control's border box and its measured title.
const TITLE_INSET: (f64, f64) = (8.0, 3.0);
/// Chromium's UA padding plus border around a text field's content, each side (1px + 2px).
const FIELD_EDGE: f64 = 3.0;

/** Push buttons, including submit and button inputs, paint a measured title. */
pub(super) fn is_button(node: &Node) -> bool {
    node.tag == "button"
        || (node.tag == "input" && ["submit", "button"].contains(&node.input_type()))
}
/** A `<button>` with element children (icons, labelled spans) lays its content out like an
inline-block, as Chromium does; only a text-only button is a measured native title. Without this
every child of such a button is dropped and icon buttons paint empty. */
pub(super) fn has_content(context: &LayoutContext<'_>, node: &Node) -> bool {
    node.tag == "button" && (!context.children(node.local_id).is_empty() || restyled(node))
}
/** A button whose page set both its font and its edges (Tailwind's preflight does for every
 * button) has no native look left, so it is a CSS box drawn like any other; drawn as a native
 * title, nodejs.org's highlighted code tokens were set in the system font. */
fn restyled(node: &Node) -> bool {
    node.style.font_authored && node.style.edges_authored
}
/** A content button is an inline-block (or the block, flex or grid box it was styled as). */
pub(super) fn content_box(
    context: &mut LayoutContext<'_>,
    node: &Node,
    flow: &mut Flow,
    items: &mut Vec<DisplayItem>,
    forced: Option<BorderBox>,
) -> EngineResult<()> {
    let mut boxed = node.clone();
    boxed.style.inline_block |=
        boxed.style.inline && !boxed.style.display_flex && !boxed.style.display_grid;
    // Its display decides, not its tag: `inline_level` treats every button as inline, which laid
    // a `display: block` button out as an inline box that dropped its padding, border and
    // min-height (16px tall where Blink's block-level LayoutButton gives 32px).
    if forced.is_none() && boxed.style.inline_block {
        return super::text::inline(context, &boxed, flow, items);
    }
    // Blink never stretches a button's auto width to its container (space_utils.cc's
    // ShouldBlockContainerChildStretchAutoInlineSize): a block button still fits its content.
    let plain = !boxed.style.display_flex && !boxed.style.display_grid;
    boxed.style.width_fit |= plain && boxed.style.width.is_none();
    let start = items.len();
    block(context, &boxed, flow, items, forced)?;
    if plain {
        // Out-of-flow boxes are placed after this, from static positions recorded before the
        // shift; Blink moves the whole content box, so an `top: auto` icon in the button's
        // content stayed 8px above its text on Google's "AI Mode" pill.
        let shift = center_content(&boxed, &mut items[start..]);
        context.shift_deferred_within(boxed.local_id, shift)?;
    }
    Ok(())
}
/** Chromium centers a button's content block vertically in a taller button box; returns the
 * downward shift applied. */
fn center_content(node: &Node, items: &mut [DisplayItem]) -> f64 {
    let Some((own, content)) = items.split_first_mut() else {
        return 0.0;
    };
    let sides = super::sizing::border_sides(&node.style);
    let top = own.y + node.style.padding[0] + sides[0];
    let bottom = own.y + own.height - node.style.padding[2] - sides[2];
    let used = content
        .iter()
        .map(|item| item.y + item.height)
        .fold(top, f64::max);
    let free = bottom - used;
    if free <= 0.0 {
        return 0.0;
    }
    for item in content {
        super::line::translate(item, 0.0, free / 2.0);
    }
    free / 2.0
}
/** Space between a control's border box and its title on each side (top, right, bottom,
 * left): the page's padding and borders when it set them, else the native inset. */
fn title_inset(node: &Node) -> [f64; 4] {
    if node.style.edges_authored {
        let sides = super::sizing::border_sides(&node.style);
        return std::array::from_fn(|side| node.style.padding[side] + sides[side]);
    }
    let border = node.style.border;
    let (x, y) = (TITLE_INSET.0 + border, TITLE_INSET.1 + border);
    [y, x, y, x]
}
/** Checkboxes and radios keep their fixed glyph size regardless of font. */
fn is_choice(node: &Node) -> bool {
    node.tag == "input" && ["checkbox", "radio"].contains(&node.input_type())
}
/** Chromium does not inherit the page font into controls; an authored size still wins. */
pub(super) fn control_font(context: &LayoutContext<'_>, node: &Node) -> f64 {
    if node.style.font_authored {
        return node.style.font_size;
    }
    let inherited = node
        .parent
        .and_then(|parent| context.node(parent).ok())
        .map(|parent| parent.style.font_size)
        .unwrap_or(crate::constants::CSS_ROOT_FONT_SIZE);
    if node.style.font_size == inherited {
        CONTROL_FONT
    } else {
        node.style.font_size
    }
}
/** Chromium's UA margins around checkbox and radio glyphs, used when the page sets none. */
fn choice_margins(node: &Node) -> Option<Node> {
    if !is_choice(node) || node.style.margin != [0.0; 4] {
        return None;
    }
    let mut node = node.clone();
    node.style.margin = if node.input_type() == "radio" {
        [3.0, 3.0, 0.0, 5.0]
    } else {
        [3.0, 3.0, 3.0, 4.0]
    };
    Some(node)
}
/** The visible title of a button or popup; empty for every other control. */
fn title(context: &LayoutContext<'_>, node: &Node) -> String {
    if node.tag == "input" && is_button(node) {
        if node.value.is_empty() {
            "Submit".into()
        } else {
            node.value.clone()
        }
    } else if node.tag == "button" {
        node.text.clone()
    } else if node.tag == "select" {
        context
            .controls
            .options_of(node.local_id)
            .iter()
            .map(|position| &context.document.nodes[*position])
            .find(|n| n.selected)
            .map(|n| n.text.clone())
            .unwrap_or_default()
    } else {
        String::new()
    }
}
/** Intrinsic border box of a control, the size Chromium gives it with no authored width. */
pub(super) fn natural_size(
    context: &mut LayoutContext<'_>,
    node: &Node,
) -> EngineResult<(f64, f64)> {
    let font = control_font(context, node);
    if is_choice(node) {
        return Ok((CHOICE_SIZE, CHOICE_SIZE));
    }
    if node.tag == "textarea" {
        return Ok(TEXTAREA_SIZE);
    }
    let scale = font / CONTROL_FONT;
    if node.style.edges_authored && node.tag != "select" {
        return authored_size(context, node, font, scale);
    }
    if is_button(node) || node.tag == "select" {
        let text = title(context, node);
        let edges = if node.tag == "select" {
            SELECT_EDGES
        } else {
            BUTTON_EDGES
        };
        let width = context.measure(&text, Face::system(font, node.style.bold))? + edges;
        return Ok((width, (BUTTON_HEIGHT * scale).max(BUTTON_HEIGHT)));
    }
    Ok((
        FIELD_SIZE.0 * scale,
        (FIELD_SIZE.1 * scale).max(FIELD_SIZE.1),
    ))
}
/** The border box of a button or field whose page set its padding or border: those edges around
 * the title (or the default field width) and one line, as Chromium sizes a styled control; the
 * native metrics made Tailwind buttons and padded search boxes the wrong size. */
fn authored_size(
    context: &mut LayoutContext<'_>,
    node: &Node,
    font: f64,
    scale: f64,
) -> EngineResult<(f64, f64)> {
    let style = title_style(node, font);
    let content = if is_button(node) {
        context.measure(&title(context, node), super::sizing::face_for(&style, None))?
    } else {
        (FIELD_SIZE.0 - FIELD_EDGE * 2.0) * scale
    };
    let height = super::sizing::advance(&style) + vertical_edges(node);
    Ok((content + horizontal_edges(node), height))
}
/** The style a control's title is measured and drawn in: the page's font when it set one,
 * otherwise Chromium's system control face at `font`. */
fn title_style(node: &Node, font: f64) -> crate::document::ComputedStyle {
    let mut style = node.style.clone();
    style.font_size = font;
    if !node.style.font_authored {
        style.font_family = crate::document::FontFamily::SystemUi;
        style.letter_spacing = 0.0;
        style.text_transform = crate::document::TextTransform::None;
    }
    style
}
/** A textarea's value laid out in its content box as its own text (`white-space: pre-wrap` from
 * the UA sheet, or whatever the page set), so screenshots show it where Chromium draws it; the
 * desktop window draws its native editor over the box and skips these runs. */
fn textarea_lines(
    context: &mut LayoutContext<'_>,
    node: &Node,
    font: f64,
    local: &mut Flow,
) -> EngineResult<Vec<PaintLine>> {
    let value = if node.value.is_empty() {
        node.text.clone()
    } else {
        node.value.clone()
    };
    let mut sized = node.clone();
    sized.style = title_style(node, font);
    let (left, width) = (local.x, local.width);
    let mut lines = super::text::text(context, &value, &sized, local, usize::MAX)?;
    super::textarea_align::align(&mut lines, &sized.style, (left, width));
    Ok(lines)
}
/** A control's baseline sits this share of its font below its vertical middle, times two. */
const CONTROL_BASELINE: f64 = 0.7;
/** Native controls are inline-blocks: they share line boxes with text and wrap like words. */
pub(super) fn control(
    context: &mut LayoutContext<'_>,
    node: &Node,
    flow: &mut Flow,
    items: &mut Vec<DisplayItem>,
    forced: Option<BorderBox>,
) -> EngineResult<()> {
    let margins = choice_margins(node);
    let style = &margins.as_ref().unwrap_or(node).style;
    let font = control_font(context, node);
    let (natural, natural_box) = natural_size(context, node)?;
    let available = (flow.width - style.margin[1] - style.margin[3]).max(1.0);
    // Authored widths and min/max apply; a wide choice box centers its fixed glyph when painted.
    // A natural width is shrink-to-fit: capped at the containing block, as Chromium caps it, so
    // a long label wraps inside the button instead of running off the page.
    let fits = natural <= available;
    let width = forced.map(|bounds| bounds.width).unwrap_or_else(|| {
        if style.width.is_some() || style.min_width.is_some() || style.max_width.is_some() {
            border_width(node, flow.width, available, Some(natural))
        } else {
            natural.min(available)
        }
    });
    let outer = width + style.margin[1] + style.margin[3];
    let (x, y) = match forced {
        Some(bounds) => (bounds.x, bounds.y),
        None => {
            if flow.cursor > 0.0 && !flow.fits(outer) {
                flow.wrap();
            }
            flow.fit(outer);
            (
                flow.left() + flow.cursor + style.margin[3],
                flow.y + style.margin[0],
            )
        }
    };
    let inset = title_inset(node);
    let mut local = Flow::new(
        x + inset[3],
        y + inset[0],
        (width - inset[1] - inset[3]).max(1.0),
    );
    let lines = if is_button(node) {
        // Controls draw their titles in the system face, as Chromium's macOS theme does, unless
        // the page set their font.
        let mut sized = node.clone();
        sized.style = title_style(node, font);
        // A naturally sized button fits its title measured whole; word-by-word sums can exceed
        // that by kerning, so it never wraps.
        if style.width.is_none() && style.max_width.is_none() && fits {
            sized.style.white_space = crate::document::WhiteSpace::Nowrap;
        }
        super::text::text(
            context,
            &title(context, node),
            &sized,
            &mut local,
            usize::MAX,
        )?
    } else if node.tag == "textarea" && !node.style.display_contents {
        // `display: contents` on a control renders nothing of it, its value included.
        textarea_lines(context, node, font, &mut local)?
    } else {
        Vec::new()
    };
    let height = natural_box.max(local.bottom - y + inset[2]);
    let mut validation = Vec::new();
    let mut validation_extent = 0.0;
    if let Some(error) = &node.validation {
        let mut error_flow = Flow::new(x, y + height + 2.0, width.max(FIELD_SIZE.0));
        validation = super::text::text(context, error, node, &mut error_flow, usize::MAX)?;
        validation_extent = error_flow.bottom - (y + height);
    }
    // An authored height sizes the control's border box, then min- and max-height clamp it, as
    // for any inline-block.
    let authored = style
        .height
        .and_then(|extent| extent.resolve_size(flow.definite))
        .map(|content| super::sizing::border_height(node, content));
    let total = forced.and_then(|bounds| bounds.height).unwrap_or_else(|| {
        clamp_height(node, authored.unwrap_or(height), flow.definite) + validation_extent
    });
    let control_height = (total - validation_extent).max(0.0);
    // A taller authored box centers its title, as native buttons do.
    let mut lines = lines;
    // A textarea's text starts at its top, like any block's; buttons center their title.
    let spare = (control_height - height) / 2.0;
    if spare > 0.0 && node.tag != "textarea" {
        for line in &mut lines {
            line.y += spare;
        }
    }
    for line in &mut validation {
        line.y += control_height - height;
    }
    let mut item = box_item(node, x, y, width, total, lines);
    item.font_size = font;
    item.control_height = Some(control_height);
    item.validation = validation;
    let start = items.len();
    items.push(item);
    if forced.is_none() {
        // The text baseline sits a little below the control's middle, as native controls draw it.
        let ascent = style.margin[0] + (control_height + font * CONTROL_BASELINE) / 2.0;
        super::text::place_atomic(
            flow,
            (start, start + 1),
            outer,
            (ascent, total + style.margin[0] + style.margin[2]),
            (style, Some(super::text::central_offset(context, node))),
            (style.margin[3], style.margin[1]),
        );
    }
    Ok(())
}
