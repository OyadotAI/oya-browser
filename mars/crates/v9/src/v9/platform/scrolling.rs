/** Script scrolling: `scrollTop`/`scrollLeft` and `scroll`/`scrollTo`/`scrollBy` on elements and
the window. V9 does no layout, so it records each offset for the app (`HostEffects::element_scroll`,
`HostEffects::scroll_x` and `HostEffects::scroll_y`), which paints scroll containers and the
viewport at them; the app clamps them to each scroll range. Reads answer the last offset set, as
the app supplied or a script wrote. The viewport's horizontal offset is negative in a document
whose content overflows to the left (vertical-rl, right-to-left), as CSSOM View says. */
use super::{BODY, HTML};
use crate::v9::value::JsValue;
use crate::v9::vm::heap::Key;
use crate::v9::vm::{Call, Res, Vm};

/** The axis a scroll offset index names: 0 for x (`scrollLeft`), 1 for y (`scrollTop`). */
const X: usize = 0;
/** The vertical axis index. */
const Y: usize = 1;

/** The root element and the body scroll the viewport, as `document.scrollingElement` does. */
fn is_viewport(node: usize) -> bool {
    node == HTML || node == BODY
}

/** `node`'s current scroll offset `[x, y]`. */
pub(super) fn offset(vm: &Vm, node: usize) -> [f64; 2] {
    let environment = &vm.page.environment;
    if is_viewport(node) {
        return [environment.scroll_x, environment.scroll_y];
    }
    environment
        .element_scroll
        .get(&node)
        .copied()
        .unwrap_or_default()
}

/** Move `node` to `target` within its known range, recording it for the app; Eye clamps again
against the layout it paints. A non-finite value leaves that axis. */
pub(super) fn set(vm: &mut Vm, node: usize, target: [Option<f64>; 2]) {
    let mut next = offset(vm, node);
    let bounds = range(vm, node);
    for axis in [X, Y] {
        if let Some(value) = target[axis].filter(|value| value.is_finite()) {
            next[axis] = value.clamp(bounds[axis][0], bounds[axis][1]);
        }
    }
    let page = &mut vm.page;
    if is_viewport(node) {
        [page.environment.scroll_x, page.environment.scroll_y] = next;
        page.effects.scroll_x = Some(next[X]);
        page.effects.scroll_y = Some(next[Y]);
    } else {
        page.environment.element_scroll.insert(node, next);
        page.effects.element_scroll.insert(node, next);
    }
}

/** The `[min, max]` offsets `node` can scroll to along each axis: the last layout's range for a
node it saw (none when it did not overflow) and unbounded past zero for a node laid out since;
for the viewport, the app's horizontal range when it gave one, and anything from zero down. */
fn range(vm: &Vm, node: usize) -> [[f64; 2]; 2] {
    let environment = &vm.page.environment;
    let open = [0.0, f64::INFINITY];
    if is_viewport(node) {
        let x = environment
            .scroll_x_range
            .unwrap_or([f64::NEG_INFINITY, f64::INFINITY]);
        return [x, open];
    }
    let laid_out = environment
        .laid_out_before
        .is_some_and(|first_new| node < first_new);
    if !laid_out {
        return [open; 2];
    }
    let max = environment
        .scroll_max
        .get(&node)
        .copied()
        .unwrap_or_default();
    [[0.0, max[X]], [0.0, max[Y]]]
}

/** The root's `scrollWidth`: the viewport plus its horizontal scroll range, once the app gave
one; None to measure the root's box instead. */
pub(super) fn document_width(vm: &Vm) -> Option<f64> {
    let environment = &vm.page.environment;
    let [min, max] = environment.scroll_x_range?;
    Some(environment.viewport_width + max - min)
}

/** The `scrollTop`/`scrollLeft` setters. */
pub(super) fn set_property(vm: &mut Vm, node: usize, name: &str, value: &JsValue) -> Res<()> {
    let number = vm.to_number(value)?;
    let target = if name == "scrollLeft" {
        [Some(number), None]
    } else {
        [None, Some(number)]
    };
    set(vm, node, target);
    Ok(())
}

/** `scroll`, `scrollTo` and `scrollBy` on `node` (HTML for the window): either `(x, y)` or a
`ScrollToOptions` dictionary whose missing `left`/`top` keep that axis. `scrollBy` adds. */
pub(super) fn scroll_method(vm: &mut Vm, node: usize, call: &Call, by: bool) -> Res<()> {
    let mut target = arguments(vm, call)?;
    if by {
        let current = offset(vm, node);
        for axis in [X, Y] {
            target[axis] = Some(current[axis] + target[axis].unwrap_or(0.0));
        }
    }
    set(vm, node, target);
    Ok(())
}

/** The requested `[left, top]` of a scroll call, None where it names no value for an axis. */
fn arguments(vm: &mut Vm, call: &Call) -> Res<[Option<f64>; 2]> {
    let first = call.arg(0);
    if let JsValue::Object(_) = first {
        let mut target = [None, None];
        for (axis, name) in [(X, "left"), (Y, "top")] {
            let value = vm.get(&first, &Key::from(name))?;
            if !matches!(value, JsValue::Undefined) {
                target[axis] = Some(vm.to_number(&value)?);
            }
        }
        return Ok(target);
    }
    if call.args.len() < 2 {
        return Ok([None, None]);
    }
    Ok([
        Some(vm.to_number(&first)?),
        Some(vm.to_number(&call.arg(1))?),
    ])
}

/** Window `scroll`, `scrollTo` and `scrollBy`, which scroll the viewport. */
pub(in crate::v9) fn window_scroll(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let by = super::dom::native_name(&call) == "scrollBy";
    scroll_method(vm, HTML, &call, by)?;
    Ok(JsValue::Undefined)
}
