/** Canvas contexts retain real pixels across requests; native resources remain host-call local. */
use super::{Page, dom, encoding};
use crate::protocol::{EngineError, EngineResult};
use crate::v9::value::{JsValue, ObjectId};
use crate::v9::vm::data::Class;
use crate::v9::vm::heap::Object;
use crate::v9::vm::realm::Builder;
use crate::v9::vm::{Call, Res, Vm};
use mars_core::canvas::{self as limits, Bitmap, Font};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::OnceLock;
mod attributes;
mod clipping;
mod colors;
mod drawing;
mod font;
mod hit;
mod images;
mod lifecycle;
pub(in crate::v9) mod path2d;
mod paths;
mod placement;
mod results;
mod state;
use lifecycle::{admit, size};
pub(in crate::v9) use lifecycle::{prepare_resize, publish, resized, validate, validate_targets};
static PROTO: OnceLock<u32> = OnceLock::new();
/** The initial font size of an element when no computed size is available. */
const DEFAULT_FONT_PX: f64 = 16.0;
/// Private edge lets GC retain a context only when its canvas is reachable.
pub(in crate::v9) const CONTEXT_SLOT: &str = "canvas.context";

/** Serializable state owns no host authority and is bounded independently from JavaScript objects. */
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::v9) struct State {
    pub context: ObjectId,
    pub bitmap: Bitmap,
    /** Drawing an unclean source permanently blocks readback until this bitmap is reset. */
    #[serde(default = "clean_bitmap")]
    pub origin_clean: bool,
    /** Drawing state is copied independently from the shared backing allocation. */
    #[serde(default)]
    pub drawing: state::DrawingState,
    /** A bounded save stack survives VM serialization and clears on any resize. */
    #[serde(default)]
    pub saved: Vec<state::DrawingState>,
    /** The current path survives save/restore but clears with beginPath, reset or resize. */
    #[serde(default)]
    pub path: limits::Path,
    /** Saved states reference this pool so serialization writes each immutable clipping mask only once. */
    #[serde(default)]
    pub clips: BTreeMap<u8, limits::ClipMask>,
    pub dirty: bool,
}

/** Canvas-specific methods must not leak onto unrelated HTML elements. */
pub(super) fn install(b: &mut Builder, canvas: u32) {
    results::install(b);
    b.host_method(canvas, "getContext", 1, context);
    b.host_method(canvas, "toDataURL", 0, export);
    let proto = b.object(Some(b.ids.object_prototype));
    super::chrome_env::interface(b, "CanvasRenderingContext2D", proto);
    b.getter(proto, "canvas", owner);
    for name in ["fillStyle", "font"] {
        b.accessor(proto, name, Some(get), Some(set));
    }
    for (name, length) in [
        ("fillRect", 4),
        ("clearRect", 4),
        ("fillText", 3),
        ("measureText", 1),
        ("getImageData", 4),
    ] {
        b.host_method(proto, name, length, drawing::method);
    }
    state::install(b, proto);
    placement::install(b, proto);
    attributes::install(b, proto);
    hit::install(b, proto);
    images::install(b, proto);
    paths::install(b, proto);
    clipping::install(b, proto);
    let _ = PROTO.set(proto);
}

/** A borrowed method must brand-check its canvas before converting arguments. */
fn canvas_node(vm: &mut Vm, call: &Call) -> Res<usize> {
    let node = dom::this_node(vm, call)?;
    if vm.page.tag(node) != "canvas" {
        return Err(vm.type_error("Illegal invocation"));
    }
    Ok(node)
}
/** Context receivers are actual allocated identities, never objects with a forged prototype. */
pub(super) fn context_node(vm: &Vm, this: &JsValue) -> Option<usize> {
    let id = this.object()?;
    vm.page
        .state
        .canvases
        .iter()
        .find_map(|(node, state)| (state.context == id).then_some(*node))
}
/** Return the same branded context while enforcing per-page bitmap and context limits. */
fn context(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let result = create_context(vm, call);
    resource_result(vm, result)
}
/** Script-controlled range and quota refusals are catchable; invalid native contracts remain fatal. */
pub(super) fn resource_result(vm: &mut Vm, result: Res<JsValue>) -> Res<JsValue> {
    match result {
        Err(crate::v9::vm::Thrown::Fatal(error)) if error.code == "canvas_geometry" => {
            Err(vm.dom_exception("NotSupportedError", &error.message))
        }
        Err(crate::v9::vm::Thrown::Fatal(error))
            if error.message.starts_with("QuotaExceededError: ") =>
        {
            Err(dom::dom_error(vm, error))
        }
        other => other,
    }
}
/** Allocate pixels before installing any heap edges, so an allocation refusal leaves no partial context. */
fn create_context(vm: &mut Vm, call: Call) -> Res<JsValue> {
    canvas_node(vm, &call)?;
    if vm.to_text(&call.arg(0))? != "2d" {
        return Ok(JsValue::Null);
    }
    let node = canvas_node(vm, &call)?;
    if let Some(state) = vm.page.state.canvases.get(&node) {
        return Ok(JsValue::Object(state.context));
    }
    let [width, height] = size(&mut vm.page, node);
    if !vm.page.canvas_host.available() || Bitmap::length(width, height).is_none() {
        return Ok(JsValue::Null);
    }
    if vm.page.state.canvases.len() >= limits::CONTEXTS {
        return Err(quota(vm));
    }
    admit(&vm.page, node, Bitmap::length(width, height).unwrap())?;
    let bitmap = Bitmap::new(width, height)?;
    let id = vm.alloc(Object::new(
        Class::Plain,
        Some(*PROTO.get().expect("canvas prototype")),
    ))?;
    encoding::write_slot(vm, id, "canvas.owner", call.this.clone());
    encoding::write_slot(
        vm,
        call.this.object().expect("canvas wrapper"),
        CONTEXT_SLOT,
        JsValue::Object(id),
    );
    vm.page.state.canvases.insert(
        node,
        State {
            context: id,
            bitmap,
            origin_clean: true,
            drawing: state::DrawingState::default(),
            saved: Vec::new(),
            path: limits::Path::default(),
            clips: BTreeMap::new(),
            dirty: true,
        },
    );
    Ok(JsValue::Object(id))
}
/** Catchable resource failure leaves previously drawn pixels intact. */
fn quota(vm: &mut Vm) -> crate::v9::vm::Thrown {
    vm.dom_exception("QuotaExceededError", "Canvas resource budget exceeded.")
}
/** Bound native work before starting loops or asking the OS to draw. */
fn work(vm: &mut Vm, pixels: usize) -> Res<()> {
    vm.page.canvas_work = vm
        .page
        .canvas_work
        .checked_sub(pixels)
        .ok_or_else(|| quota(vm))?;
    Ok(())
}
/** The context retains its actual canvas wrapper across adoption and resizing. */
fn owner(vm: &mut Vm, call: Call) -> Res<JsValue> {
    context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    Ok(encoding::read_slot(vm, &call.this, "canvas.owner").unwrap_or(JsValue::Undefined))
}
/** Expose only state actually used by drawing. */
fn get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let node = context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    let state = &vm.page.state.canvases[&node];
    let value = if dom::native_name(&call) == "font" {
        state.drawing.font.text.clone()
    } else {
        colors::serialize(state.drawing.fill)
    };
    Ok(JsValue::string(&value)?)
}
/** Invalid assignments preserve the previous drawing state. */
fn set(vm: &mut Vm, call: Call) -> Res<JsValue> {
    context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    let value = vm.to_text(&call.arg(0))?;
    let node = context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    if dom::native_name(&call) == "font" {
        let parent = font_size(vm, node);
        let font = font::parse(&value, parent);
        let state = vm.page.state.canvases.get_mut(&node).expect("canvas");
        if let Some(font) = font {
            state.drawing.font = font;
        }
        return Ok(JsValue::Undefined);
    }
    let state = vm.page.state.canvases.get_mut(&node).expect("canvas");
    if let Some(color) = colors::parse(&value) {
        state.drawing.fill = color;
    }
    Ok(JsValue::Undefined)
}
/** The canvas element's computed font size: em and percentage font lengths resolve against it. */
fn font_size(vm: &mut Vm, node: usize) -> f64 {
    let computed = dom::computed_value(vm, node, "font-size");
    computed
        .strip_suffix("px")
        .and_then(|number| number.parse::<f64>().ok())
        .filter(|px| px.is_finite() && *px > 0.0)
        .unwrap_or(DEFAULT_FONT_PX)
}
/** Export exactly the same bitmap scripts read and painters consume. */
fn export(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let result = encode(vm, call);
    resource_result(vm, result)
}
/** Encoding failures are transactional and leave the source bitmap untouched. */
fn encode(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let node = canvas_node(vm, &call)?;
    require_clean(vm, node)?;
    let [width, height] = size(&mut vm.page, node);
    if width == 0 || height == 0 || Bitmap::length(width, height).is_none() {
        return Ok(JsValue::string("data:,")?);
    }
    work(vm, width as usize * height as usize)?;
    let blank;
    let bitmap = if let Some(state) = vm.page.state.canvases.get(&node) {
        &state.bitmap
    } else {
        blank = Bitmap::new(width, height)?;
        &blank
    };
    let png = vm.page.canvas_host.png(bitmap)?;
    if png.len() > limits::PNG_BYTES {
        return Err(quota(vm));
    }
    Ok(JsValue::string(&format!(
        "data:image/png;base64,{}",
        super::window::base64_encode(&png)
    ))?)
}

/** Old snapshots contain only canvas-owned pixels, so their initial clean flag is true. */
fn clean_bitmap() -> bool {
    true
}

/** Drawing-state save/restore cannot undo taint; only bitmap reset may restore readback. */
fn require_clean(vm: &mut Vm, node: usize) -> Res<()> {
    if vm
        .page
        .state
        .canvases
        .get(&node)
        .is_some_and(|state| !state.origin_clean)
    {
        return Err(vm.dom_exception("SecurityError", "Canvas pixels are not origin-clean."));
    }
    Ok(())
}
