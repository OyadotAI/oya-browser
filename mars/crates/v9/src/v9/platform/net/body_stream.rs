/** Fetch bodies expose one native stream whose disturbance and lock govern every consumer. */
use super::*;
use crate::v9::builtins;

/** The body owns its stream even when only a reader remains visible to script. */
pub(super) const STREAM: &str = "fetchBodyStream";
/** An absent body differs from a present, zero-length body. */
pub(super) const NULL: &str = "fetchNullBody";
/** Whole-body consumers need no stream allocation until script asks for the body property. */
const USED: &str = "fetchBodyUsed";
/** Fetched streams retain cancellation even when their original Response wrapper is collected. */
const FETCHED: &str = "fetchBodyStreamSignal";

/** Preserve absence separately from the response's text and binary representations. */
pub(super) fn set_null(vm: &mut Vm, owner: ObjectId, absent: bool) {
    headers::put_private(vm, owner, NULL, JsValue::Boolean(absent));
}

/** A previously exposed body stream, read without author getters. */
fn stored(vm: &Vm, owner: ObjectId) -> Option<ObjectId> {
    headers::get_private(vm, owner, STREAM)?.object()
}

/** Reading the body property preserves stream identity and does not disturb its bytes. */
pub(super) fn get(
    vm: &mut Vm,
    owner: ObjectId,
    bytes: impl FnOnce(&Vm) -> Res<Vec<u8>>,
) -> Res<JsValue> {
    if headers::get_private(vm, owner, NULL) == Some(JsValue::Boolean(true)) {
        return Ok(JsValue::Null);
    }
    if let Some(stream) = stored(vm, owner) {
        return Ok(JsValue::Object(stream));
    }
    let consumed = used(vm, owner);
    let bytes = if consumed { Vec::new() } else { bytes(vm)? };
    let stream = builtins::readable_from_bytes(vm, bytes)?;
    headers::put_private(vm, owner, STREAM, JsValue::Object(stream));
    if is_response(vm, owner)
        && let Some(signal) = headers::get_private(vm, owner, abort::SIGNAL)
    {
        headers::put_private(vm, stream, abort::SIGNAL, signal);
        headers::put_private(vm, stream, FETCHED, JsValue::Boolean(true));
    }
    if consumed {
        builtins::consume_readable(vm, stream)?;
    }
    if is_response(vm, owner)
        && let Some(reason) = abort::body_reason(vm, owner)
    {
        builtins::error_readable(vm, stream, reason)?;
    }
    Ok(JsValue::Object(stream))
}

/** A body is used only after a read or cancellation, not merely after acquiring its reader. */
pub(super) fn used(vm: &Vm, owner: ObjectId) -> bool {
    headers::get_private(vm, owner, USED) == Some(JsValue::Boolean(true))
        || stored(vm, owner).is_some_and(|stream| builtins::readable_disturbed(vm, stream))
}

/** Cloning and whole-body readers refuse both disturbed bodies and active reader locks. */
pub(super) fn unusable(vm: &Vm, owner: ObjectId) -> bool {
    used(vm, owner)
        || stored(vm, owner).is_some_and(|stream| {
            builtins::readable_disturbed(vm, stream) || builtins::readable_locked(vm, stream)
        })
}

/** A whole-body reader owns and drains the same stream exposed by the body getter. */
pub(super) fn consume(vm: &mut Vm, owner: ObjectId) -> Res<()> {
    if unusable(vm, owner) {
        return Err(vm.type_error("Body stream is locked or already read"));
    }
    if headers::get_private(vm, owner, NULL) == Some(JsValue::Boolean(true)) {
        return Ok(());
    }
    if let Some(stream) = stored(vm, owner) {
        builtins::consume_readable(vm, stream)?;
    }
    headers::put_private(vm, owner, USED, JsValue::Boolean(true));
    Ok(())
}

/** Request signals govern fetch, while only fetched response bodies inherit its cancellation. */
fn is_response(vm: &Vm, owner: ObjectId) -> bool {
    matches!(vm.heap.get(owner).map(|object| &object.class),
        Some(Class::Host(host)) if matches!(host.as_ref(), HostData::Response(_)))
}

/** Abort every exposed fetched stream before script observes any abort event. */
pub(super) fn abort_streams(vm: &mut Vm) -> Res<()> {
    vm.charge(vm.heap.objects.len())?;
    let streams: Vec<_> = vm
        .heap
        .objects
        .keys()
        .filter_map(|stream| {
            if headers::get_private(vm, *stream, FETCHED) != Some(JsValue::Boolean(true)) {
                return None;
            }
            let reason = abort::body_reason(vm, *stream)?;
            Some((*stream, reason))
        })
        .collect();
    for (stream, reason) in streams {
        builtins::error_readable(vm, stream, reason)?;
    }
    Ok(())
}
