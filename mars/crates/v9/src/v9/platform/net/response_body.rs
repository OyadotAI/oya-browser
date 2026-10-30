/** Private immutable response buffers preserve bytes while participating in ordinary heap accounting. */
use super::*;
use crate::v9::vm::data::Bytes;
use crate::v9::vm::heap::{Heap, Slot};

/** A private buffer is never returned directly by any body reader or clone. */
pub(super) const KEY: &str = "responseBodyBytes";

/** Body storage is bounded before allocating; clones may share the unobservable immutable buffer. */
pub(super) fn store(vm: &mut Vm, response: ObjectId, bytes: Vec<u8>) -> Res<()> {
    if bytes.len() > VM_RESPONSE_BYTES {
        return Err(EngineError::limit("response body bytes", VM_RESPONSE_BYTES).into());
    }
    if !bytes.is_empty() {
        let buffer = vm.alloc(Object::new(Class::ArrayBuffer(Bytes(bytes)), None))?;
        headers::put_private(vm, response, KEY, JsValue::Object(buffer));
    }
    Ok(())
}

/** Each consumer gets its own bytes, so mutating a returned view cannot change another clone. */
pub(super) fn bytes(vm: &Vm, response: ObjectId, data: &ResponseData) -> Res<Vec<u8>> {
    if let Some(value) = headers::get_private(vm, response, KEY) {
        let Some(Class::ArrayBuffer(bytes)) = value
            .object()
            .and_then(|id| vm.heap.get(id))
            .map(|object| &object.class)
        else {
            return Err(EngineError::invalid("Invalid response body buffer.").into());
        };
        return Ok(bytes.0.clone());
    }
    Ok(data.body.as_bytes().to_vec())
}

/** Fetch text and JSON consume UTF-8 with replacement and discard a leading UTF-8 BOM. */
pub(super) fn text(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
    String::from_utf8_lossy(bytes).into_owned()
}

/** Untrusted saved states cannot substitute a different object or hide an oversized body. */
pub(in crate::v9) fn validate(heap: &Heap) -> crate::protocol::EngineResult<()> {
    for object in heap.objects.values() {
        let Class::Host(host) = &object.class else {
            continue;
        };
        let HostData::Response(data) = host.as_ref() else {
            continue;
        };
        let bytes = match object.props.get(&headers::private_key(KEY)) {
            None => 0,
            Some(property) => {
                let Slot::Data(JsValue::Object(id)) = &property.slot else {
                    return Err(EngineError::invalid("Invalid response body buffer."));
                };
                let Some(Class::ArrayBuffer(bytes)) = heap.get(*id).map(|object| &object.class)
                else {
                    return Err(EngineError::invalid("Invalid response body buffer."));
                };
                bytes.0.len()
            }
        };
        if bytes.saturating_add(data.body.len()) > VM_RESPONSE_BYTES {
            return Err(EngineError::limit("response body bytes", VM_RESPONSE_BYTES));
        }
    }
    Ok(())
}
