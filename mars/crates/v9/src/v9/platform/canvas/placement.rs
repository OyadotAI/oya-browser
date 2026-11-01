/** textAlign, textBaseline and direction, plus the anchor and metrics fillText and measureText derive from them. */
use super::*;
use mars_core::canvas::{self as limits, Align, Baseline, Direction};

/** Installs the three placement accessors; an assignment outside their keywords keeps the old value. */
pub(super) fn install(b: &mut Builder, proto: u32) {
    for name in ["textAlign", "textBaseline", "direction"] {
        b.accessor(proto, name, Some(get), Some(set));
    }
}

/** Reads the canonical keyword for the attribute named by the accessor. */
fn get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let node = context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    let drawing = &vm.page.state.canvases[&node].drawing;
    let keyword = match dom::native_name(&call) {
        "textAlign" => limits::keyword(&limits::ALIGNS, drawing.align),
        "textBaseline" => limits::keyword(&limits::BASELINES, drawing.baseline),
        _ => limits::keyword(&limits::DIRECTIONS, drawing.direction),
    };
    Ok(JsValue::string(keyword)?)
}

/** Stores a recognised keyword; anything else is ignored, as the spec requires. */
fn set(vm: &mut Vm, call: Call) -> Res<JsValue> {
    context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    let value = vm.to_text(&call.arg(0))?;
    let node = context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    let drawing = &mut vm
        .page
        .state
        .canvases
        .get_mut(&node)
        .expect("canvas")
        .drawing;
    match dom::native_name(&call) {
        "textAlign" => store(&mut drawing.align, limits::lookup(&limits::ALIGNS, &value)),
        "textBaseline" => store(
            &mut drawing.baseline,
            limits::lookup(&limits::BASELINES, &value),
        ),
        _ => store(
            &mut drawing.direction,
            limits::lookup(&limits::DIRECTIONS, &value),
        ),
    }
    Ok(JsValue::Undefined)
}

/** Assigns only a parsed keyword. */
fn store<T>(slot: &mut T, value: Option<T>) {
    if let Some(value) = value {
        *slot = value;
    }
}

/** The direction a run resolves to; inherit reads the canvas element's computed direction. */
fn rtl(vm: &mut Vm, node: usize, direction: Direction) -> bool {
    match direction {
        Direction::Ltr => false,
        Direction::Rtl => true,
        Direction::Inherit => dom::computed_value(vm, node, "direction") == "rtl",
    }
}

/** Start and end depend on direction; the other keywords never do. */
fn resolved_rtl(vm: &mut Vm, node: usize) -> (Align, bool) {
    let drawing = &vm.page.state.canvases[&node].drawing;
    let (align, direction) = (drawing.align, drawing.direction);
    let resolves = matches!(align, Align::Start | Align::End);
    (align, resolves && rtl(vm, node, direction))
}

/** Where the alphabetic baseline must sit for an anchor: shifted by alignment, moved by the baseline. */
pub(super) fn origin(vm: &mut Vm, node: usize, value: &str, at: [f64; 2]) -> Res<[f64; 2]> {
    let (align, rtl) = resolved_rtl(vm, node);
    let shifts = match align {
        Align::Left => false,
        Align::Start => rtl,
        Align::End => !rtl,
        _ => true,
    };
    let width = if shifts {
        measure(vm, node, value)?
    } else {
        0.0
    };
    let (baseline, font) = {
        let drawing = &vm.page.state.canvases[&node].drawing;
        (drawing.baseline, drawing.font.clone())
    };
    let lift = if baseline == Baseline::Alphabetic {
        0.0
    } else {
        let metrics = vm.page.canvas_host.font_metrics(&font)?;
        limits::baseline_offset(baseline, metrics, font.size)
    };
    Ok([at[0] + limits::align_shift(align, width, rtl), at[1] + lift])
}

/** Advance width of the run in the current font. */
fn measure(vm: &mut Vm, node: usize, value: &str) -> Res<f64> {
    let font = vm.page.state.canvases[&node].drawing.font.clone();
    Ok(vm.page.canvas_host.measure(&font, value)?)
}

/** The TextMetrics members in the interface's order; each getter reads the slot of the same name. */
pub(super) const MEASURE_FIELDS: [&str; 12] = [
    "width",
    "actualBoundingBoxLeft",
    "actualBoundingBoxRight",
    "actualBoundingBoxAscent",
    "actualBoundingBoxDescent",
    "fontBoundingBoxAscent",
    "fontBoundingBoxDescent",
    "emHeightAscent",
    "emHeightDescent",
    "hangingBaseline",
    "alphabeticBaseline",
    "ideographicBaseline",
];

/** Every measureText member, paired with its name, from the host's width and font box. */
pub(super) fn measurement(vm: &mut Vm, node: usize, value: &str) -> Res<Vec<(&'static str, f64)>> {
    let (align, rtl) = resolved_rtl(vm, node);
    let width = measure(vm, node, value)?;
    let (baseline, font) = {
        let drawing = &vm.page.state.canvases[&node].drawing;
        (drawing.baseline, drawing.font.clone())
    };
    let metrics = vm.page.canvas_host.font_metrics(&font)?;
    let m = limits::text_measure(font.size, metrics, width, (baseline, align, rtl));
    let values = [
        m.width,
        m.actual_left,
        m.actual_right,
        m.actual_ascent,
        m.actual_descent,
        m.font_ascent,
        m.font_descent,
        m.em_ascent,
        m.em_descent,
        m.hanging,
        m.alphabetic,
        m.ideographic,
    ];
    Ok(MEASURE_FIELDS.into_iter().zip(values).collect())
}
