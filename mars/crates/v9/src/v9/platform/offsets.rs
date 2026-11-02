/** CSSOM View `offsetParent`, `offsetTop` and `offsetLeft`: offsets are measured from the offset
parent (the nearest positioned ancestor, or a `td`, `th` or `table` around a static element,
else the body), not from the document, so a script that copies them onto an absolutely
positioned sibling lands on the element (ref-rectangle.js in the sticky tests).
ponytail: the offset parent's border is not subtracted (the app's boxes carry no border widths). */
use super::dom::{client_box, computed_value};
use super::{BODY, HTML};
use crate::v9::vm::Vm;

/** Tags that are offset parents of a static element inside them. */
const TABLE_PARENTS: [&str; 3] = ["td", "th", "table"];

/** Whether `node`'s computed `position` is anything but `static`. */
fn positioned(vm: &mut Vm, node: usize) -> bool {
    computed_value(vm, node, "position") != "static"
}

/** The offset parent of a connected element; None for the root, the body and fixed boxes. */
pub(super) fn offset_parent(vm: &mut Vm, node: usize) -> Option<usize> {
    if node == HTML || node == BODY || !vm.page.connected(node) {
        return None;
    }
    if computed_value(vm, node, "position") == "fixed" {
        return None;
    }
    let static_node = !positioned(vm, node);
    let mut current = vm.page.parent(node);
    while let Some(ancestor) = current.filter(|id| *id != BODY && *id != HTML) {
        if positioned(vm, ancestor)
            || (static_node && TABLE_PARENTS.contains(&vm.page.tag(ancestor).as_str()))
        {
            return Some(ancestor);
        }
        current = vm.page.parent(ancestor);
    }
    Some(BODY)
}

/** `offsetLeft` (axis 0) or `offsetTop` (axis 1): the border box's position from the offset
parent's box, unscrolled by the parent's own scroll offset; from the document for the body. */
pub(super) fn offset(vm: &mut Vm, node: usize, axis: usize) -> f64 {
    let own = layout_position(vm, node);
    match offset_parent(vm, node).filter(|parent| *parent != BODY) {
        Some(parent) => {
            let base = layout_position(vm, parent);
            own[axis] - base[axis] + super::scrolling::offset(vm, parent)[axis]
        }
        None => own[axis],
    }
}

/** Visual translation never moves CSSOM layout offsets; legacy hosts keep their former fallback. */
fn layout_position(vm: &mut Vm, node: usize) -> [f64; 2] {
    if let Some(position) = vm
        .page
        .environment
        .geometry
        .iter()
        .find(|bounds| bounds.node == node)
        .and_then(|bounds| bounds.layout_position)
        .filter(|position| position.iter().all(|value| value.is_finite()))
    {
        return position;
    }
    let (x, y, ..) = client_box(vm, node);
    [
        x + vm.page.environment.scroll_x,
        y + vm.page.environment.scroll_y,
    ]
}
