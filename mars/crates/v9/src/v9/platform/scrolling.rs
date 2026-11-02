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

/** How `scrollIntoView` aligns a box on one axis (CSSOM View's ScrollLogicalPosition). */
#[derive(Clone, Copy, PartialEq)]
enum Align {
    Start,
    Center,
    End,
    Nearest,
}

/** `element.scrollIntoView(arg)`: scroll each overflowing ancestor, then the viewport, so the
element's box lands where `block` and `inline` ask (CSSOM View). `true` or no argument is
block start, `false` is block end, and an options dictionary names both; inline defaults to
nearest. Pages use it for anchors, chat views and invalid fields; it was a no-op before. */
pub(super) fn scroll_into_view(vm: &mut Vm, node: usize, call: &Call) -> Res<()> {
    let (block, inline) = alignment(vm, &call.arg(0))?;
    let (mut x, mut y, width, height) = super::dom::client_box(vm, node);
    let mut current = vm.page.parent(node);
    while let Some(container) = current.filter(|node| !is_viewport(*node)) {
        if vm.page.environment.scroll_max.contains_key(&container) {
            let (cx, cy, cw, ch) = super::dom::client_box(vm, container);
            let delta = [
                align(inline, x - cx, width, cw),
                align(block, y - cy, height, ch),
            ];
            let before = offset(vm, container);
            set(
                vm,
                container,
                [Some(before[X] + delta[X]), Some(before[Y] + delta[Y])],
            );
            let after = offset(vm, container);
            x -= after[X] - before[X];
            y -= after[Y] - before[Y];
        }
        current = vm.page.parent(container);
    }
    let environment = &vm.page.environment;
    let view = [environment.viewport_width, environment.viewport_height];
    let delta = [
        align(inline, x, width, view[X]),
        align(block, y, height, view[Y]),
    ];
    let before = offset(vm, HTML);
    set(
        vm,
        HTML,
        [Some(before[X] + delta[X]), Some(before[Y] + delta[Y])],
    );
    Ok(())
}

/** The block and inline alignment an argument asks for. */
fn alignment(vm: &mut Vm, arg: &JsValue) -> Res<(Align, Align)> {
    match arg {
        JsValue::Undefined | JsValue::Boolean(true) => Ok((Align::Start, Align::Nearest)),
        JsValue::Boolean(false) => Ok((Align::End, Align::Nearest)),
        JsValue::Object(_) => {
            let block = keyword(vm, arg, "block", Align::Start)?;
            let inline = keyword(vm, arg, "inline", Align::Nearest)?;
            Ok((block, inline))
        }
        _ => Ok((Align::Start, Align::Nearest)),
    }
}

/** One ScrollLogicalPosition member of the options; an unknown keyword is a TypeError. */
fn keyword(vm: &mut Vm, options: &JsValue, name: &str, fallback: Align) -> Res<Align> {
    let value = vm.get(options, &Key::from(name))?;
    if value == JsValue::Undefined {
        return Ok(fallback);
    }
    match vm.to_text(&value)?.as_str() {
        "start" => Ok(Align::Start),
        "center" => Ok(Align::Center),
        "end" => Ok(Align::End),
        "nearest" => Ok(Align::Nearest),
        other => Err(vm.type_error(&format!(
            "Failed to execute 'scrollIntoView': The provided value '{other}' is not a valid enum value of type ScrollLogicalPosition."
        ))),
    }
}

/** How far to scroll one axis so a box at `start` (relative to the view) of `size` lands as
`mode` asks in a view `view` long. */
fn align(mode: Align, start: f64, size: f64, view: f64) -> f64 {
    match mode {
        Align::Start => start,
        Align::End => start + size - view,
        Align::Center => start + size / 2.0 - view / 2.0,
        Align::Nearest if start < 0.0 || size > view => start,
        Align::Nearest if start + size > view => start + size - view,
        Align::Nearest => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /** Each alignment puts the box where CSSOM View says, and nearest leaves a visible box. */
    #[test]
    fn alignments_follow_cssom_view() {
        assert_eq!(align(Align::Start, 500.0, 50.0, 800.0), 500.0);
        assert_eq!(align(Align::End, 500.0, 50.0, 800.0), -250.0);
        assert_eq!(align(Align::Center, 500.0, 50.0, 800.0), 125.0);
        assert_eq!(align(Align::Nearest, 100.0, 50.0, 800.0), 0.0);
        assert_eq!(align(Align::Nearest, 900.0, 50.0, 800.0), 150.0);
        assert_eq!(align(Align::Nearest, -40.0, 50.0, 800.0), -40.0);
    }
}
