/**
 * Out-of-flow boxes inside a multicol container. One whose containing block lies inside the
 * columns is placed before the content is cut, so it fragments with that block as Blink's
 * OutOfFlowLayoutPart does inside a fragmentation context; the rest keep a static position
 * that the cut moves into its column, and are placed later as usual.
 */
use super::flow::LayoutContext;
use crate::document::{DisplayItem, Node, Position};
use crate::protocol::EngineResult;

/** The viewport size `place` never uses: these boxes' containing blocks are all inside. */
const NO_VIEWPORT: (f64, f64) = (0.0, 0.0);

/** An out-of-flow box and its static position. */
pub(super) type Static = (usize, (f64, f64));

/** Out-of-flow descendants recorded while the container's content laid out, by their static
position, split into those placed now and those placed later. */
pub(super) struct Pending {
    /// Boxes whose containing block lies inside the container.
    pub inside: Vec<Static>,
    /// The rest, whose static positions the cut moves.
    pub outside: Vec<Static>,
}

/** Takes the container's out-of-flow descendants out of the deferred list. */
pub(super) fn take(context: &mut LayoutContext<'_>, container: &Node) -> EngineResult<Pending> {
    let mut pending = Pending {
        inside: Vec::new(),
        outside: Vec::new(),
    };
    for (local, position) in context.take_deferred() {
        let node = context.node(local)?;
        match containing_block(context, node, container.local_id) {
            Placement::Elsewhere => context.defer(local, position),
            Placement::Inside => pending.inside.push((local, position)),
            Placement::Outside => pending.outside.push((local, position)),
        }
    }
    Ok(pending)
}

/** Places the boxes whose containing block is inside the columns, in document order. */
pub(super) fn place_inside(
    context: &mut LayoutContext<'_>,
    pending: &Pending,
    items: &mut Vec<DisplayItem>,
) -> EngineResult<()> {
    for (local, position) in &pending.inside {
        let node = context.node(*local)?;
        super::positioned::one(context, node, *position, items, NO_VIEWPORT)?;
    }
    Ok(())
}

/** Records the remaining boxes again at their moved static positions. */
pub(super) fn restore(context: &mut LayoutContext<'_>, pending: Pending) {
    for (local, position) in pending.outside {
        context.defer(local, position);
    }
}

/** Where an out-of-flow box sits relative to the container. */
enum Placement {
    /// Not inside the container at all.
    Elsewhere,
    /// Inside, with its containing block inside too.
    Inside,
    /// Inside, with the container or a box above it as its containing block.
    Outside,
}

/** Whether `node` is inside the container, and whether its containing block is. */
fn containing_block(context: &LayoutContext<'_>, node: &Node, container: usize) -> Placement {
    let fixed = node.style.position == Position::Fixed;
    let mut block_inside = None;
    let mut current = node.parent.and_then(|parent| context.node(parent).ok());
    while let Some(ancestor) = current {
        if ancestor.local_id == container {
            return if block_inside == Some(true) {
                Placement::Inside
            } else {
                Placement::Outside
            };
        }
        let contains = super::positioned::contains_fixed(&ancestor.style)
            || (!fixed && ancestor.style.position != Position::Static);
        if contains && block_inside.is_none() {
            block_inside = Some(true);
        }
        current = ancestor.parent.and_then(|parent| context.node(parent).ok());
    }
    Placement::Elsewhere
}
