/** Canvas state is copied independently of pixels and applied only after argument conversion. */
use super::*;
use mars_core::canvas::Affine;
mod composite;
mod matrix;
pub(super) use matrix::dictionary_members as path_matrix;

/** A bounded stack prevents save-only scripts from retaining unbounded native drawing state. */
pub(super) const SAVED_STATES: usize = 64;

/** Only implemented drawing properties participate in save, restore and reset. */
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(in crate::v9) struct DrawingState {
    /** Straight-alpha paint is combined with global alpha only when rasterizing. */
    pub fill: [u8; 4],
    /** Stroke paint is separate from fill paint and shares the same global alpha. */
    pub stroke: [u8; 4],
    /** Pen dimensions, caps and joins are saved independently from the current path. */
    pub pen: limits::StrokeStyle,
    /** Canonical dash lists and offset follow save/restore and reset with the rest of the pen state. */
    pub dash: limits::DashStyle,
    /** Measurement and glyph rasterization share this resolved bounded font. */
    pub font: Font,
    /** Pixel read/write deliberately bypasses this drawing-coordinate transform. */
    pub transform: Affine,
    /** Invalid assignments retain the prior finite value in the unit interval. */
    pub alpha: f64,
    /** Only genuinely implemented composition operators can survive serialization or save/restore. */
    pub composite: limits::Composite,
    /** Image copies use this setting without changing stored source pixels. */
    pub smoothing: bool,
    /** None is the initial unbounded clip; saved references retain one immutable mask-pool entry. */
    pub clip: Option<u8>,
    /** Vertical anchor of text: the alphabetic baseline unless a script names another line. */
    pub baseline: limits::Baseline,
    /** Horizontal anchor of text, resolved against the direction at draw time. */
    pub align: limits::Align,
    /** Direction used to resolve start and end; inherit reads the canvas element. */
    pub direction: limits::Direction,
}

impl Default for DrawingState {
    /** Defaults match a newly created CanvasRenderingContext2D. */
    fn default() -> Self {
        Self {
            fill: [0, 0, 0, 255],
            stroke: [0, 0, 0, 255],
            pen: limits::StrokeStyle::default(),
            dash: limits::DashStyle::default(),
            font: Font::default(),
            transform: Affine::IDENTITY,
            alpha: 1.0,
            composite: limits::Composite::default(),
            smoothing: true,
            clip: None,
            baseline: limits::Baseline::default(),
            align: limits::Align::default(),
            direction: limits::Direction::default(),
        }
    }
}

/** Install state operations separately from drawing and image-data dispatch. */
pub(super) fn install(b: &mut Builder, proto: u32) {
    matrix::install(b);
    composite::install(b, proto);
    for name in ["globalAlpha", "imageSmoothingEnabled"] {
        b.accessor(proto, name, Some(get), Some(set));
    }
    for (name, length) in [
        ("save", 0),
        ("restore", 0),
        ("reset", 0),
        ("translate", 2),
        ("scale", 2),
        ("rotate", 1),
        ("transform", 6),
        ("setTransform", 0),
        ("resetTransform", 0),
        ("getTransform", 0),
    ] {
        b.host_method(proto, name, length, method);
    }
}

/** Invalid serialized state cannot enter raster loops or grow a restored stack. */
pub(super) fn validate(drawing: &DrawingState) -> EngineResult<()> {
    drawing.pen.validate()?;
    drawing.dash.validate()?;
    let font = &drawing.font;
    if !drawing.transform.finite()
        || !drawing.alpha.is_finite()
        || !(0.0..=1.0).contains(&drawing.alpha)
        || !font.size.is_finite()
        || font.size <= 0.0
        || font.size > f64::from(limits::EDGE)
        || !["serif", "sans-serif", "monospace"].contains(&font.family.as_str())
        || !limits::WEIGHT_RANGE.contains(&font.weight)
        || font.text.len() > limits::FONT_TEXT_BYTES
    {
        return Err(EngineError::invalid("Invalid canvas drawing state."));
    }
    Ok(())
}

/** Property reads require the actual allocated context, never a copied prototype. */
fn get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let node = context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    let drawing = &vm.page.state.canvases[&node].drawing;
    Ok(if dom::native_name(&call) == "globalAlpha" {
        JsValue::Number(drawing.alpha)
    } else {
        JsValue::Boolean(drawing.smoothing)
    })
}

/** Numeric conversion may resize the canvas, so the final assignment reacquires its state. */
fn set(vm: &mut Vm, call: Call) -> Res<JsValue> {
    context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    let alpha = dom::native_name(&call) == "globalAlpha";
    let value = if alpha {
        vm.to_number(&call.arg(0))?
    } else {
        0.0
    };
    let node = context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    let drawing = &mut vm
        .page
        .state
        .canvases
        .get_mut(&node)
        .expect("canvas")
        .drawing;
    if alpha && value.is_finite() && (0.0..=1.0).contains(&value) {
        drawing.alpha = value;
    }
    if !alpha {
        drawing.smoothing = call.arg(0).truthy();
    }
    Ok(JsValue::Undefined)
}

/** Shared resource translation turns stack and backing exhaustion into catchable exceptions. */
fn method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let result = dispatch(vm, &call);
    resource_result(vm, result)
}

/** Methods change either drawing state or pixels; argument callbacks run before both. */
fn dispatch(vm: &mut Vm, call: &Call) -> Res<JsValue> {
    let node = context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    let name = dom::native_name(call);
    if name == "getTransform" {
        return matrix::record(vm, vm.page.state.canvases[&node].drawing.transform);
    }
    if ["save", "restore", "reset"].contains(&name) {
        return stack(vm, node, name);
    }
    let transform = transform_arguments(vm, call, name)?;
    let node = context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    if transform.finite() {
        apply_transform(vm, node, name, transform);
    }
    Ok(JsValue::Undefined)
}

/** No-op restore and failed saves leave all observable state and pixels intact. */
fn stack(vm: &mut Vm, node: usize, name: &str) -> Res<JsValue> {
    if name == "reset" {
        return reset(vm, node);
    }
    if name == "save" && vm.page.state.canvases[&node].saved.len() >= SAVED_STATES {
        return Err(quota(vm));
    }
    let state = vm.page.state.canvases.get_mut(&node).expect("canvas");
    if name == "save" {
        state.saved.push(state.drawing.clone());
    } else if let Some(drawing) = state.saved.pop() {
        state.drawing = drawing;
    }
    clipping::prune(state);
    Ok(JsValue::Undefined)
}

/** Reset allocates before changing either stack or pixels and preserves the context identity. */
fn reset(vm: &mut Vm, node: usize) -> Res<JsValue> {
    let bitmap = &vm.page.state.canvases[&node].bitmap;
    let dimensions = [bitmap.width, bitmap.height];
    work(vm, bitmap.pixels.len() / limits::CHANNELS)?;
    let bitmap = Bitmap::new(dimensions[0], dimensions[1])?;
    lifecycle::resized(&mut vm.page, node, bitmap);
    Ok(JsValue::Undefined)
}

/** Nonfinite derived products are ignored instead of poisoning serialized drawing state. */
fn apply_transform(vm: &mut Vm, node: usize, name: &str, transform: Affine) {
    let drawing = &mut vm
        .page
        .state
        .canvases
        .get_mut(&node)
        .expect("canvas")
        .drawing;
    let next = if ["setTransform", "resetTransform"].contains(&name) {
        transform
    } else {
        drawing.transform.compose(transform)
    };
    if next.finite() {
        drawing.transform = next;
    }
}

/** Dictionary overload defaults to identity; numeric overloads enforce their required arguments. */
fn transform_arguments(vm: &mut Vm, call: &Call, name: &str) -> Res<Affine> {
    if name == "resetTransform" {
        return Ok(Affine::IDENTITY);
    }
    if name == "setTransform" && call.args.len() <= 1 {
        return matrix::dictionary(vm, &call.arg(0));
    }
    let count = if name == "rotate" {
        1
    } else if ["scale", "translate"].contains(&name) {
        2
    } else {
        6
    };
    if call.args.len() < count {
        return Err(vm.type_error("Not enough arguments for canvas transform."));
    }
    let mut values = [0.0; 6];
    for (index, value) in values.iter_mut().take(count).enumerate() {
        *value = vm.to_number(&call.arg(index))?;
    }
    Ok(operation(name, values))
}

/** All transforms use post-multiplication in the same six-component convention as Core. */
fn operation(name: &str, values: [f64; 6]) -> Affine {
    let [x, y, ..] = values;
    match name {
        "translate" => Affine([1.0, 0.0, 0.0, 1.0, x, y]),
        "scale" => Affine([x, 0.0, 0.0, y, 0.0, 0.0]),
        "rotate" => Affine([x.cos(), x.sin(), -x.sin(), x.cos(), 0.0, 0.0]),
        _ => Affine(values),
    }
}
