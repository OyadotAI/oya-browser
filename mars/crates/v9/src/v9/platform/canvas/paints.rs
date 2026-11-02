/** Gradient and pattern objects: branded wrappers whose paint lives in page state, owned by reachability and bounded. */
use super::*;
mod gradients;
mod patterns;
use super::state::Style;
use mars_core::canvas::{Gradient, Paint, Shader};

/** Live paint objects and their pixel copies have page bounds, so script cannot grow memory without limit. */
const OBJECTS: usize = 256;
/** Private self slot: identity needs both the page association and this brand, never a copied prototype. */
const BRAND: &str = "canvas.paint";

/** A paint plus whether its source was origin-clean; a tainted pattern taints every context that draws it. */
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::v9) struct Entry {
    /** The paint itself, mutated in place by addColorStop and setTransform. */
    pub paint: Paint,
    /** False when the pattern source was not origin-clean. */
    pub clean: bool,
}

/** Install the four factories on the context and the two paint interfaces. */
pub(super) fn install(b: &mut Builder, proto: u32) {
    for (name, length) in [
        ("createLinearGradient", 4),
        ("createRadialGradient", 6),
        ("createConicGradient", 3),
        ("createPattern", 2),
    ] {
        b.host_method(proto, name, length, factory);
    }
    gradients::install(b);
    patterns::install(b);
}

/** Factories share the canvas quota translation, so refusals stay catchable. */
fn factory(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let result = create(vm, &call);
    resource_result(vm, result)
}

/** The receiver is branded before any argument conversion can run script. */
fn create(vm: &mut Vm, call: &Call) -> Res<JsValue> {
    context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    match dom::native_name(call) {
        "createPattern" => patterns::create(vm, call),
        name => gradients::create(vm, call, name),
    }
}

/** A paint wrapper is identified by its page entry plus its private self slot. */
pub(super) fn identity(vm: &mut Vm, value: &JsValue) -> Option<ObjectId> {
    let id = value.object()?;
    (vm.page.state.canvas_paints.contains_key(&id)
        && encoding::read_slot(vm, value, BRAND) == Some(JsValue::Object(id)))
    .then_some(id)
}

/** Allocate and register a new paint after admitting it, so a refusal leaves no wrapper behind. */
pub(super) fn wrap(vm: &mut Vm, entry: Entry, proto: u32) -> Res<JsValue> {
    admit(vm, &entry)?;
    let id = vm.alloc(Object::new(Class::Plain, Some(proto)))?;
    encoding::write_slot(vm, id, BRAND, JsValue::Object(id));
    vm.page.state.canvas_paints.insert(id, entry);
    Ok(JsValue::Object(id))
}

/** Reclaim unreachable wrappers once under pressure before refusing the new paint. */
fn admit(vm: &mut Vm, incoming: &Entry) -> Res<()> {
    if !fits(&vm.page.state.canvas_paints, incoming) {
        vm.collect_now()?;
    }
    if !fits(&vm.page.state.canvas_paints, incoming) {
        return Err(quota(vm));
    }
    Ok(())
}

/** Count and pattern pixel bytes must both stay under their page bounds. */
fn fits(entries: &BTreeMap<ObjectId, Entry>, incoming: &Entry) -> bool {
    let bytes = entries
        .values()
        .map(|entry| pixel_bytes(&entry.paint))
        .sum::<usize>()
        + pixel_bytes(&incoming.paint);
    entries.len() < OBJECTS && bytes <= limits::PAGE_BYTES
}

/** Gradients are small and bounded by their stop count; patterns copy real pixels. */
fn pixel_bytes(paint: &Paint) -> usize {
    match paint {
        Paint::Pattern(pattern) => pattern.bitmap.pixels.len(),
        _ => 0,
    }
}

/** Serialized paint tables must satisfy the same bounds as live insertions. */
pub(in crate::v9) fn validate(entries: &BTreeMap<ObjectId, Entry>) -> EngineResult<()> {
    let bytes: usize = entries
        .values()
        .map(|entry| pixel_bytes(&entry.paint))
        .sum();
    if entries.len() > OBJECTS || bytes > limits::PAGE_BYTES {
        return Err(EngineError::invalid("Invalid canvas paint budget."));
    }
    entries
        .values()
        .try_for_each(|entry| entry.paint.validate())
}

/** Every entry is a plain branded wrapper, and every style a context names must have an entry. */
pub(in crate::v9) fn validate_targets(vm: &mut Vm) -> EngineResult<()> {
    let ids: Vec<ObjectId> = vm.page.state.canvas_paints.keys().copied().collect();
    for id in ids {
        let plain = vm
            .heap
            .objects
            .get(&id)
            .is_some_and(|object| matches!(object.class, Class::Plain));
        if !plain || identity(vm, &JsValue::Object(id)) != Some(id) {
            return Err(EngineError::invalid("Invalid canvas paint association."));
        }
    }
    let named = referenced(&vm.page.state.canvases);
    if named
        .iter()
        .any(|id| !vm.page.state.canvas_paints.contains_key(id))
    {
        return Err(EngineError::invalid("Invalid canvas paint reference."));
    }
    Ok(())
}

/** Paint objects named by any context's current or saved fill and stroke, which GC must keep alive. */
pub(in crate::v9) fn referenced(canvases: &BTreeMap<usize, State>) -> Vec<ObjectId> {
    let mut ids = Vec::new();
    for state in canvases.values() {
        for drawing in std::iter::once(&state.drawing).chain(state.saved.iter()) {
            ids.extend(
                [&drawing.fill, &drawing.stroke]
                    .into_iter()
                    .filter_map(Style::object),
            );
        }
    }
    ids
}

/** The paint a style currently names, and whether its source is origin-clean; a missing entry paints nothing. */
pub(super) fn resolve(entries: &BTreeMap<ObjectId, Entry>, style: &Style) -> (Paint, bool) {
    match style {
        Style::Color(color) => (Paint::Solid(*color), true),
        Style::Object(id) => entries
            .get(id)
            .map_or((Paint::Solid(limits::TRANSPARENT), true), |entry| {
                (entry.paint.clone(), entry.clean)
            }),
    }
}

/** Getters return the same paint object that was assigned, or the canonical color text. */
pub(super) fn style_value(style: &Style) -> Res<JsValue> {
    Ok(match style {
        Style::Color(color) => JsValue::string(&colors::serialize(*color))?,
        Style::Object(id) => JsValue::Object(*id),
    })
}

/** Fill and stroke assignments bind paint objects by identity; anything else must parse as a color. */
pub(super) fn assign(vm: &mut Vm, call: &Call, stroke: bool) -> Res<JsValue> {
    let object = identity(vm, &call.arg(0));
    let color = match object {
        Some(_) => None,
        None => colors::parse(&vm.to_text(&call.arg(0))?),
    };
    let style = match (object, color) {
        (Some(id), _) => Style::Object(id),
        (None, Some(color)) => Style::Color(color),
        (None, None) => return Ok(JsValue::Undefined),
    };
    let node = context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    let drawing = &mut vm
        .page
        .state
        .canvases
        .get_mut(&node)
        .expect("canvas")
        .drawing;
    if stroke {
        drawing.stroke = style;
    } else {
        drawing.fill = style;
    }
    Ok(JsValue::Undefined)
}

/** The paint a context's fill or stroke selects, bound to its transform; None means nothing is painted. */
pub(super) fn source(vm: &Vm, node: usize, stroke: bool) -> Option<(Shader, bool)> {
    let state = &vm.page.state.canvases[&node];
    let style = if stroke {
        &state.drawing.stroke
    } else {
        &state.drawing.fill
    };
    let (paint, clean) = resolve(&vm.page.state.canvas_paints, style);
    Shader::new(paint, state.drawing.transform).map(|shader| (shader, clean))
}

/** Borrow a gradient entry for mutation; anything else is an illegal receiver. */
pub(super) fn gradient_mut<'a>(vm: &'a mut Vm, id: ObjectId) -> Option<&'a mut Gradient> {
    match &mut vm.page.state.canvas_paints.get_mut(&id)?.paint {
        Paint::Gradient(gradient) => Some(gradient),
        _ => None,
    }
}
