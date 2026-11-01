/** Image requests cross the host boundary as data; only a matching decoded outcome fires an event. */
use super::{Page, Target, dom, events};
use crate::constants::{IMAGE_COUNT, IMAGE_DIMENSION, NODES, VM_DETACHED_NODES};
use crate::protocol::{EngineError, EngineResult};
use crate::v9::value::JsValue;
use crate::v9::vm::realm::Builder;
use crate::v9::vm::{Call, Res, Thrown, Vm};
use mars_core::image_source::{REQUEST_TEXT_BYTES, SOURCE_TOTAL_BYTES, valid_url_text};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

mod adoption;
pub use adoption::ImageAdoption;
mod cors;
pub use cors::ImageCors;
mod blob;
pub use blob::ImageCancellation;

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
    /// Fetch policy is captured with the source generation, never inferred from a later DOM value.
    #[serde(default)]
    pub cors: ImageCors,
    /// Immutable bytes captured from this context when the selected src generation is admitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blob_source: Option<super::BlobUrlSource>,
}

/// Actual host decoding completion; missing dimensions denotes a failed fetch or decode.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageResponse {
    /// Exact request identity; arbitrary node identities confer no capabilities.
    pub request: ImageRequest,
    /// Intrinsic pixel dimensions from the admitted decoder, never inferred from author attributes.
    pub dimensions: Option<(u32, u32)>,
    /// Optional admitted RGBA data; absent data keeps oversized or unsupported sources explicit.
    #[serde(default)]
    pub bitmap: Option<mars_core::canvas::Bitmap>,
    /// Only the host's actual acquisition provenance can grant clean canvas readback.
    #[serde(default)]
    pub origin_clean: bool,
    /// The absolute URL the host selected from the page's src, srcset or picture sources; it is
    /// what `currentSrc` reports, empty when the host selected none.
    #[serde(default)]
    pub current_src: String,
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
    /** An inadmissible optional source keeps its DOM text but only queues an error completion. */
    pub source_rejected: bool,
    /** The crossorigin state belongs to the same generation as the pixels and pending request. */
    pub cors: ImageCors,
    /** The host has received this generation and must eventually return its outcome. */
    pub requested: bool,
    /** No image work remains for this generation, either after success or failure. */
    pub complete: bool,
    /** Only a successful admitted decode provides intrinsic dimensions. */
    pub dimensions: Option<(u32, u32)>,
    /** Immutable decoded pixels share candidate storage and reset with every source generation. */
    pub bitmap: Option<mars_core::canvas::Bitmap>,
    /** False is the safe default for hosts or restored records without acquisition provenance. */
    pub origin_clean: bool,
    /** Native pending captures remain charged until cancellation or matching completion. */
    pub blob_bytes: usize,
    /** The URL the host selected for the last completed request, which `currentSrc` reports. */
    #[serde(default)]
    pub current_src: String,
}

/** Only actual HTML images own these getters; unrelated elements do not acquire image state. */
pub(super) fn install(b: &mut Builder, prototype: u32) {
    for name in ["complete", "naturalWidth", "naturalHeight", "currentSrc"] {
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
        "currentSrc" => Ok(JsValue::string(
            state.map_or("", |s| s.current_src.as_str()),
        )?),
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
    if !["src", "srcset", "sizes", "crossorigin"].contains(&name) || page.tag(node) != "img" {
        return Ok(());
    }
    let cors = page
        .view(node)
        .map(|view| {
            ImageCors::from_attribute(view.attributes.get("crossorigin").map(String::as_str))
        })
        .unwrap_or_default();
    if name == "crossorigin"
        && page
            .state
            .images
            .get(&node)
            .is_some_and(|state| state.cors == cors)
    {
        return Ok(());
    }
    let (source, mut source_rejected) = page
        .view(node)
        .and_then(|v| {
            v.attributes.get("src").map(|source| {
                if valid_url_text(source) {
                    (source.clone(), false)
                } else {
                    (String::new(), true)
                }
            })
        })
        .unwrap_or_default();
    let retained = page
        .state
        .images
        .iter()
        .filter(|(id, _)| **id != node)
        .fold(0usize, |used, (_, state)| {
            used.saturating_add(state.source.len())
        });
    // Preserve the authored DOM attribute; an unavailable optional source follows the empty-src error task.
    let source = if source.len() <= SOURCE_TOTAL_BYTES.saturating_sub(retained) {
        source
    } else {
        source_rejected = true;
        String::new()
    };
    let complete = !has_source(page, node);
    blob::cancel(page, node)?;
    let state = page.state.images.entry(node).or_default();
    let generation = state
        .generation
        .checked_add(1)
        .ok_or_else(|| EngineError::invalid("Image generation exhausted."))?;
    *state = State {
        generation,
        source,
        source_rejected,
        cors,
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
    let url = if state.source_rejected {
        String::new()
    } else {
        super::net::resolve(vm, &source)
            .filter(|url| valid_url_text(url))
            .unwrap_or_default()
    };
    let retained = vm
        .page
        .effects
        .image_requests
        .iter()
        .fold(0usize, |used, request| {
            used.saturating_add(request.source.len())
                .saturating_add(request.url.len())
        });
    if source.len().saturating_add(url.len()) > REQUEST_TEXT_BYTES.saturating_sub(retained) {
        return Ok(());
    }
    let blob_source = blob::capture(&vm.page, &url);
    let blob_bytes = blob_source
        .as_ref()
        .map_or(0, |source| blob::charge(&url, source));
    let state = vm.page.state.images.get_mut(&node).expect("image state");
    state.requested = true;
    state.blob_bytes = blob_bytes;
    vm.page.effects.image_requests.push(ImageRequest {
        node,
        generation: state.generation,
        source,
        url,
        cors: state.cors,
        blob_source,
    });
    Ok(())
}

/** Validate transport before dispatching any callback; stale and duplicate outcomes are harmless. */
pub(in crate::v9) fn deliver(vm: &mut Vm, responses: Vec<ImageResponse>) -> EngineResult<()> {
    if responses.len() > IMAGE_COUNT {
        return Err(EngineError::limit("Image completions", IMAGE_COUNT));
    }
    let text_bytes = responses.iter().fold(0usize, |used, response| {
        used.saturating_add(response.request.source.len())
            .saturating_add(response.request.url.len())
            .saturating_add(response.current_src.len())
    });
    if text_bytes > REQUEST_TEXT_BYTES {
        return Err(EngineError::limit(
            "Image completion text bytes",
            REQUEST_TEXT_BYTES,
        ));
    }
    for response in &responses {
        if !valid_request(&response.request)
            || !valid_dimensions(response.dimensions)
            || !valid_url_text(&response.current_src)
        {
            return Err(EngineError::invalid("Invalid image completion."));
        }
        validate_bitmap(response.dimensions, response.bitmap.as_ref())?;
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
        || state.cors != request.cors
        || !state.requested
        || state.complete
    {
        return Ok(());
    }
    state.complete = true;
    state.blob_bytes = 0;
    state.dimensions = response.dimensions;
    state.bitmap = response.bitmap;
    state.origin_clean = response.origin_clean;
    state.current_src = response.current_src;
    validate_pixels(&vm.page.state.images)?;
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
    blob::validate(states)?;
    adoption::validate(states)?;
    validate_pixels(states)?;
    let source_bytes = states.values().fold(0usize, |used, state| {
        used.saturating_add(state.source.len())
    });
    if source_bytes > SOURCE_TOTAL_BYTES {
        return Err(EngineError::limit(
            "Image source text bytes",
            SOURCE_TOTAL_BYTES,
        ));
    }
    if states.len() > NODES + VM_DETACHED_NODES {
        return Err(EngineError::limit(
            "Image states",
            NODES + VM_DETACHED_NODES,
        ));
    }
    if states.values().any(|s| {
        s.generation == 0
            || !valid_url_text(&s.source)
            || !valid_url_text(&s.current_src)
            || s.source_rejected && (!s.source.is_empty() || s.dimensions.is_some())
            || !valid_dimensions(s.dimensions)
            || s.dimensions.is_some() && (!s.complete || !s.requested)
    }) {
        return Err(EngineError::invalid("Invalid image state."));
    }
    Ok(())
}

/** Pixel storage cannot outlive successful decoding or exceed the page's independent source cap. */
fn validate_pixels(states: &BTreeMap<usize, State>) -> EngineResult<()> {
    let mut bytes = 0usize;
    for state in states.values() {
        validate_bitmap(state.dimensions, state.bitmap.as_ref())?;
        bytes = bytes.saturating_add(
            state
                .bitmap
                .as_ref()
                .map_or(0, |bitmap| bitmap.pixels.len()),
        );
    }
    if bytes > mars_core::canvas::IMAGE_SOURCE_BYTES {
        return Err(EngineError::limit(
            "Canvas image source bytes",
            mars_core::canvas::IMAGE_SOURCE_BYTES,
        ));
    }
    Ok(())
}

/** Source pixels can differ from density-adjusted natural dimensions but must be a real bitmap. */
fn validate_bitmap(
    dimensions: Option<(u32, u32)>,
    bitmap: Option<&mars_core::canvas::Bitmap>,
) -> EngineResult<()> {
    if let Some(bitmap) = bitmap {
        bitmap.validate()?;
        if dimensions.is_none() || !bitmap.drawable() {
            return Err(EngineError::invalid("Invalid decoded image bitmap."));
        }
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
    request.generation != 0
        && valid_url_text(&request.source)
        && valid_url_text(&request.url)
        && request.blob_source.as_ref().is_none_or(|source| {
            source.bytes.len() <= crate::constants::IMAGE_BYTES
                && source.validate(&request.url).is_ok()
        })
}
