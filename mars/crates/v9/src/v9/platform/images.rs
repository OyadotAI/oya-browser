/** Image requests cross the host boundary as data; only a matching decoded outcome fires an event. */
use super::{Page, Target, dom, events};
use crate::constants::{IMAGE_COUNT, IMAGE_DIMENSION, NODES, URL_BYTES, VM_DETACHED_NODES};
use crate::protocol::{EngineError, EngineResult};
use crate::v9::value::JsValue;
use crate::v9::vm::realm::Builder;
use crate::v9::vm::{Call, Res, Thrown, Vm};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

mod adoption;
pub use adoption::ImageAdoption;

/** Keep DOM adoption on the platform facade used by other DOM components. */
pub(super) fn adopt(page: &mut Page, from: usize, to: usize) -> EngineResult<()> {
    adoption::adopt(page, from, to)
}

/// A particular source selection; the host must preserve the generation when returning its result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageRequest {
    /// Page-local image identity, including images not connected to the document.
    pub node: usize,
    /// A source mutation changes this even when the author later restores the same URL.
    pub generation: u64,
    /// Original src attribute, used to reject stale or mismatched deliveries.
    pub source: String,
    /// Absolute fallback URL resolved by V9 without accessing the network.
    pub url: String,
}

/// Actual host decoding completion; missing dimensions denotes a failed fetch or decode.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageResponse {
    /// Exact request identity; arbitrary node identities confer no capabilities.
    pub request: ImageRequest,
    /// Intrinsic pixel dimensions from the admitted decoder, never inferred from author attributes.
    pub dimensions: Option<(u32, u32)>,
}

/** Source and decode state contains no heap references and needs no additional GC roots. */
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(in crate::v9) struct State {
    /** Outstanding replies can still name this source generation's previous DOM identities. */
    pub previous_nodes: Vec<usize>,
    /** Advances on every source-selection mutation, never wraps. */
    pub generation: u64,
    /** Literal src before URL resolution, so restored text can be bounded and compared. */
    pub source: String,
    /** The host has received this generation and must eventually return its outcome. */
    pub requested: bool,
    /** No image work remains for this generation, either after success or failure. */
    pub complete: bool,
    /** Only a successful admitted decode provides intrinsic dimensions. */
    pub dimensions: Option<(u32, u32)>,
}

/** Only actual HTML images own these getters; unrelated elements do not acquire image state. */
pub(super) fn install(b: &mut Builder, prototype: u32) {
    for name in ["complete", "naturalWidth", "naturalHeight"] {
        b.getter(prototype, name, get);
    }
    b.host_method(prototype, "decode", 0, decode);
}

/** `img.decode()`: a promise that resolves once the image has loaded and rejects with an
EncodingError when it has no source or fails. Image components (YouTube's thumbnails) wait on
it before showing a picture, so without it they never showed one. */
fn decode(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let node = dom::this_node(vm, &call)?;
    if vm.page.tag(node) != "img" {
        return Err(vm.type_error("Illegal invocation"));
    }
    let promise = vm.new_promise()?;
    let (resolve, reject) = vm.resolving_functions(promise)?;
    let state = vm.page.state.images.get(&node).cloned();
    let settled = match &state {
        _ if !has_source(&mut vm.page, node) => Some(false),
        Some(state) if state.complete => Some(state.dimensions.is_some()),
        _ => None,
    };
    match settled {
        Some(true) => {
            vm.call(JsValue::Object(resolve), JsValue::Undefined, Vec::new())?;
        }
        Some(false) => {
            let error =
                match vm.dom_exception("EncodingError", "The source image cannot be decoded.") {
                    Thrown::Value(error) => error,
                    fatal => return Err(fatal),
                };
            vm.call(JsValue::Object(reject), JsValue::Undefined, vec![error])?;
        }
        None => {
            // Settled by the image's own load or error event, whichever comes first.
            let add = vm.get(
                &call.this,
                &crate::v9::vm::heap::Key::from("addEventListener"),
            )?;
            for (event, settle) in [("load", resolve), ("error", reject)] {
                let options = vm.new_object()?;
                vm.put(options, "once", JsValue::Boolean(true))?;
                let args = vec![
                    JsValue::string(event)?,
                    JsValue::Object(settle),
                    JsValue::Object(options),
                ];
                vm.call(add.clone(), call.this.clone(), args)?;
            }
        }
    }
    Ok(JsValue::Object(promise))
}

/** Reflect the last admitted decode, or the specified HTML initial state before a request starts. */
fn get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let node = dom::this_node(vm, &call)?;
    if vm.page.tag(node) != "img" {
        return Err(vm.type_error("Illegal invocation"));
    }
    let initial_complete = !has_source(&mut vm.page, node);
    let state = vm.page.state.images.get(&node);
    match dom::native_name(&call) {
        "complete" => Ok(JsValue::Boolean(
            state.map_or(initial_complete, |s| s.complete),
        )),
        name => Ok(JsValue::Number(
            state.and_then(|s| s.dimensions).map_or(0.0, |(w, h)| {
                f64::from(if name == "naturalWidth" { w } else { h })
            }),
        )),
    }
}

/** Missing src/srcset starts complete; an explicitly empty src still attempts and fails loading. */
fn has_source(page: &mut Page, node: usize) -> bool {
    page.view(node)
        .is_some_and(|v| v.attributes.contains_key("src") || v.attributes.contains_key("srcset"))
}

/** Reset on every relevant mutation, including A->B->A and repeated assignment of the same src. */
pub(super) fn source_changed(page: &mut Page, node: usize, name: &str) -> EngineResult<()> {
    if !["src", "srcset", "sizes"].contains(&name) || page.tag(node) != "img" {
        return Ok(());
    }
    let source = page
        .view(node)
        .and_then(|v| v.attributes.get("src").cloned())
        .unwrap_or_default();
    let complete = !has_source(page, node);
    let state = page.state.images.entry(node).or_default();
    let generation = state
        .generation
        .checked_add(1)
        .ok_or_else(|| EngineError::invalid("Image generation exhausted."))?;
    *state = State {
        generation,
        source,
        complete,
        ..State::default()
    };
    Ok(())
}

/** Discover initial markup and detached Image objects after scripts finish, before host work starts. */
pub(in crate::v9) fn requests(vm: &mut Vm) -> EngineResult<()> {
    let mut nodes: Vec<usize> = vm
        .page
        .document
        .iter()
        .flat_map(|d| d.nodes.iter())
        .filter(|n| n.tag == "img")
        .map(|n| n.local_id)
        .collect();
    nodes.extend(
        vm.page
            .state
            .detached
            .iter()
            .filter(|(_, n)| n.tag == "img")
            .map(|(id, _)| *id),
    );
    for node in nodes {
        if !vm.page.state.images.contains_key(&node) && has_source(&mut vm.page, node) {
            source_changed(&mut vm.page, node, "src")?;
        }
        request(vm, node)?;
    }
    Ok(())
}

/** Emit a bounded batch and leave later images available for the following response task. */
fn request(vm: &mut Vm, node: usize) -> EngineResult<()> {
    let Some(state) = vm
        .page
        .state
        .images
        .get(&node)
        .filter(|s| !s.requested && !s.complete)
    else {
        return Ok(());
    };
    if vm.page.effects.image_requests.len() >= IMAGE_COUNT {
        return Ok(());
    }
    let source = state.source.clone();
    let url = super::net::resolve(vm, &source).unwrap_or_default();
    if source.len() > URL_BYTES || url.len() > URL_BYTES {
        return Err(EngineError::limit("Image URL bytes", URL_BYTES));
    }
    let state = vm.page.state.images.get_mut(&node).expect("image state");
    state.requested = true;
    vm.page.effects.image_requests.push(ImageRequest {
        node,
        generation: state.generation,
        source,
        url,
    });
    Ok(())
}

/** Validate transport before dispatching any callback; stale and duplicate outcomes are harmless. */
pub(in crate::v9) fn deliver(vm: &mut Vm, responses: Vec<ImageResponse>) -> EngineResult<()> {
    if responses.len() > IMAGE_COUNT {
        return Err(EngineError::limit("Image completions", IMAGE_COUNT));
    }
    for response in &responses {
        if !valid_request(&response.request) || !valid_dimensions(response.dimensions) {
            return Err(EngineError::invalid("Invalid image completion."));
        }
    }
    for response in responses {
        complete(vm, response)?;
    }
    Ok(())
}

/** One response is its own task, followed by microtasks, after changing the image's actual state. */
fn complete(vm: &mut Vm, response: ImageResponse) -> EngineResult<()> {
    let request = response.request;
    let Some(node) = adoption::target(&vm.page.state.images, request.node) else {
        return Ok(());
    };
    let state = vm.page.state.images.get_mut(&node).expect("resolved image");
    if state.generation != request.generation
        || state.source != request.source
        || !state.requested
        || state.complete
    {
        return Ok(());
    }
    state.complete = true;
    state.dimensions = response.dimensions;
    let kind = if response.dimensions.is_some() {
        "load"
    } else {
        "error"
    };
    vm.remaining = vm.limits.operations;
    let result =
        events::fire(vm, Target::Node(node), kind, false, false).and_then(|_| vm.drain_jobs());
    match result {
        Ok(()) => Ok(()),
        Err(Thrown::Value(value)) => {
            vm.report_error(&value);
            Ok(())
        }
        Err(Thrown::Fatal(error)) => Err(error),
    }
}

/** Saved state and transport use the same finite image and URL bounds as actual host work. */
pub(in crate::v9) fn validate(states: &BTreeMap<usize, State>) -> EngineResult<()> {
    adoption::validate(states)?;
    if states.len() > NODES + VM_DETACHED_NODES {
        return Err(EngineError::limit(
            "Image states",
            NODES + VM_DETACHED_NODES,
        ));
    }
    if states.values().any(|s| {
        s.generation == 0
            || s.source.len() > URL_BYTES
            || !valid_dimensions(s.dimensions)
            || s.dimensions.is_some() && (!s.complete || !s.requested)
    }) {
        return Err(EngineError::invalid("Invalid image state."));
    }
    Ok(())
}

/** Serialized image records must name actual image nodes, including genuinely detached nodes. */
pub(in crate::v9) fn validate_targets(page: &Page) -> EngineResult<()> {
    let valid = page.state.images.keys().all(|id| {
        page.state
            .detached
            .get(id)
            .map(|n| n.tag.as_str())
            .or_else(|| {
                page.document
                    .as_ref()?
                    .nodes
                    .iter()
                    .find(|n| n.local_id == *id)
                    .map(|n| n.tag.as_str())
            })
            == Some("img")
    });
    if valid {
        Ok(())
    } else {
        Err(EngineError::invalid("Invalid image node identity."))
    }
}

/** Finite dimensions cannot smuggle an oversized decoded allocation across the host boundary. */
fn valid_dimensions(size: Option<(u32, u32)>) -> bool {
    size.is_none_or(|(w, h)| {
        w > 0
            && h > 0
            && w <= IMAGE_DIMENSION
            && h <= IMAGE_DIMENSION
            && u64::from(w) * u64::from(h) * 4 <= crate::constants::IMAGE_DECODED_BYTES as u64
    })
}

/** Requests contain inert bounded text and a nonzero generation. */
fn valid_request(request: &ImageRequest) -> bool {
    request.generation != 0 && request.source.len() <= URL_BYTES && request.url.len() <= URL_BYTES
}
