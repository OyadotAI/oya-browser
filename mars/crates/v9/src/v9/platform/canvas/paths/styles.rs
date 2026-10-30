/** Stroke properties use saved drawing state and reject invalid assignments without resetting it. */
use super::*;
use limits::{LineCap, LineJoin};

/** Each property feeds the actual stroke rasterizer rather than a generic expando. */
pub(super) fn install(b: &mut Builder, proto: u32) {
    for name in [
        "strokeStyle",
        "lineWidth",
        "lineCap",
        "lineJoin",
        "miterLimit",
    ] {
        b.accessor(proto, name, Some(get), Some(set));
    }
}

/** Getters report the canonical accepted style from the branded context. */
fn get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let node = context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    let drawing = &vm.page.state.canvases[&node].drawing;
    let text = match dom::native_name(&call) {
        "lineWidth" => return Ok(JsValue::Number(drawing.pen.width)),
        "miterLimit" => return Ok(JsValue::Number(drawing.pen.miter_limit)),
        "lineCap" => match drawing.pen.cap {
            LineCap::Butt => "butt",
            LineCap::Round => "round",
            LineCap::Square => "square",
        }
        .into(),
        "lineJoin" => match drawing.pen.join {
            LineJoin::Miter => "miter",
            LineJoin::Round => "round",
            LineJoin::Bevel => "bevel",
        }
        .into(),
        _ => colors::serialize(drawing.stroke),
    };
    Ok(JsValue::string(&text)?)
}

/** Conversion callbacks can reset the context, so assignments reacquire state afterwards. */
fn set(vm: &mut Vm, call: Call) -> Res<JsValue> {
    context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    let name = dom::native_name(&call);
    if ["lineWidth", "miterLimit"].contains(&name) {
        return number(vm, &call, name);
    }
    let text = vm.to_text(&call.arg(0))?;
    let node = context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    let drawing = &mut vm
        .page
        .state
        .canvases
        .get_mut(&node)
        .expect("canvas")
        .drawing;
    match (name, text.as_str()) {
        ("lineCap", "butt") => drawing.pen.cap = LineCap::Butt,
        ("lineCap", "round") => drawing.pen.cap = LineCap::Round,
        ("lineCap", "square") => drawing.pen.cap = LineCap::Square,
        ("lineJoin", "miter") => drawing.pen.join = LineJoin::Miter,
        ("lineJoin", "round") => drawing.pen.join = LineJoin::Round,
        ("lineJoin", "bevel") => drawing.pen.join = LineJoin::Bevel,
        ("strokeStyle", _) => {
            if let Some(color) = colors::parse(&text) {
                drawing.stroke = color;
            }
        }
        _ => {}
    }
    Ok(JsValue::Undefined)
}

/** Width and miter limit admit positive finite values; unsupported derived geometry is refused at draw time. */
fn number(vm: &mut Vm, call: &Call, name: &str) -> Res<JsValue> {
    let value = vm.to_number(&call.arg(0))?;
    let node = context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    if value.is_finite() && value > 0.0 {
        let pen = &mut vm
            .page
            .state
            .canvases
            .get_mut(&node)
            .expect("canvas")
            .drawing
            .pen;
        if name == "lineWidth" {
            pen.width = value;
        } else {
            pen.miter_limit = value;
        }
    }
    Ok(JsValue::Undefined)
}
