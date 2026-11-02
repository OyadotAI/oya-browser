/**
 * `::first-line` fonts in line layout: text measured on a block's first formatted line takes
 * the pseudo-element's font size, weight, style, family, spacing and line height (where the text
 * inherits them from the block), so the line breaks and grows as Blink lays it out. The paint
 * pass (`first_line.rs`) gives the runs on that line the same fonts.
 */
use super::flow::*;
use crate::document::ComputedStyle;

/// The block's style and its `::first-line` style, while that block's first line is open.
pub(super) type Pending = (Box<ComputedStyle>, Box<ComputedStyle>);

/// The pending first line a block found when it opened, and whether the block is a new
/// formatting context (a float, an inline-block), which leaves the outer first line open.
pub(super) struct Scope {
    /// The pending first line before this block.
    saved: Option<Pending>,
    /// The block is laid out apart from the outer line (forced bounds).
    fresh: bool,
}

/** Opens a block's first-line scope: its own `::first-line` fonts when they differ from its
 * own, else, for an in-flow block met before the outer block's first line has any content
 * (`outer_open`), the outer block's, whose first formatted line is this block's first line. */
pub(super) fn open(
    context: &mut LayoutContext<'_>,
    node: &Node,
    (fresh, outer_open): (bool, bool),
) -> Scope {
    let saved = context.first_line.take();
    let own = node
        .style
        .first_line
        .as_ref()
        .filter(|line| fonts_differ(&node.style, line))
        .map(|line| (Box::new((*node.style).clone()), line.clone()));
    let inherited = saved.clone().filter(|_| !fresh && outer_open);
    context.first_line = own.or(inherited);
    Scope { saved, fresh }
}

/** Closes a block's scope, restoring the outer pending line only past a new formatting context. */
pub(super) fn close(context: &mut LayoutContext<'_>, scope: Scope) {
    context.first_line = if scope.fresh { scope.saved } else { None };
}

/** `node` restyled for the first line when its text lands there: None off that line, or when no
 * first line with its own fonts is open. */
pub(super) fn styled(context: &LayoutContext<'_>, node: &Node, flow: &Flow) -> Option<Node> {
    let (block, line) = context.first_line.as_ref()?;
    if !flow.finished.is_empty() {
        return None;
    }
    let mut styled = node.clone();
    styled.style = merged(block, line, &node.style, inherits(node)).into();
    Some(styled)
}

/** Whether a node takes first-line properties only where it inherits them: an inline element
 * may set its own; the block's text and a block on the line take them all. */
pub(super) fn inherits(node: &Node) -> bool {
    node.style.inline && node.tag != "#text"
}

/** `own` with the block's `::first-line` font properties: with `inherits` (an inline element)
 * each one `own` inherits from the block (equal to the block's), else each one the first-line
 * style sets (different from the block's). */
pub(super) fn merged(
    block: &ComputedStyle,
    line: &ComputedStyle,
    own: &ComputedStyle,
    inherits: bool,
) -> ComputedStyle {
    let mut out = own.clone();
    let takes = |own_value: bool, line_value: bool| if inherits { own_value } else { !line_value };
    if takes(
        own.font_size == block.font_size,
        line.font_size == block.font_size,
    ) {
        out.font_size = line.font_size;
    }
    let weight = |s: &ComputedStyle| (s.bold, s.font_weight);
    if takes(weight(own) == weight(block), weight(line) == weight(block)) {
        (out.bold, out.font_weight) = weight(line);
    }
    if takes(
        own.font_style == block.font_style,
        line.font_style == block.font_style,
    ) {
        out.font_style = line.font_style;
    }
    let families = |s: &ComputedStyle| s.font_families == block.font_families;
    if takes(families(own), families(line)) {
        out.font_family = line.font_family;
        out.font_families.clone_from(&line.font_families);
    }
    let spacing = |s: &ComputedStyle| s.letter_spacing == block.letter_spacing;
    if takes(spacing(own), spacing(line)) {
        out.letter_spacing = line.letter_spacing;
    }
    let height = |s: &ComputedStyle| s.line_height == block.line_height;
    if takes(height(own), height(line)) {
        out.line_height = line.line_height;
    }
    out
}

/** Whether the first-line style changes any font property of the block's. */
fn fonts_differ(block: &ComputedStyle, line: &ComputedStyle) -> bool {
    merged(block, line, block, false) != *block
}

/// Line tops this close count as one line box (runs in different fonts start apart).
const SAME_LINE: f64 = 0.5;

/**
 * The style a laid-out run of `items[item]` at `line_y` was measured in: its first-line style
 * when the run sits on the first formatted line of its block (the topmost line of the block's
 * items so far), else None for its own. Line clamping re-measures runs in it.
 */
pub(super) fn run_style(
    context: &LayoutContext<'_>,
    items: &[DisplayItem],
    item: usize,
    line_y: f64,
) -> Option<ComputedStyle> {
    let node = context.node(items.get(item)?.local_id).ok()?;
    // The nearest block with a `::first-line` of its own: a first child block's first line is
    // its ancestor's first formatted line too.
    let mut block = node;
    while block.style.inline || block.tag == "#text" || block.style.first_line.is_none() {
        block = context.node(block.parent?).ok()?;
    }
    let line = block.style.first_line.as_ref()?;
    let start = items[..=item]
        .iter()
        .rposition(|i| i.local_id == block.local_id)?;
    let top = items[start..=item]
        .iter()
        .flat_map(|i| i.lines.iter().map(|line| line.y))
        .fold(f64::INFINITY, f64::min);
    (line_y <= top + SAME_LINE).then(|| merged(&block.style, line, &node.style, inherits(node)))
}
