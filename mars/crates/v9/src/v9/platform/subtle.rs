/**
 * crypto.subtle is a SubtleCrypto instance, as Chrome's is: the interface object and its prototype
 * carry every member Chrome exposes, with the spec's length and name. `digest` is implemented for
 * SHA-1, SHA-256, SHA-384 and SHA-512, on the operating system's crypto library (ring, already
 * linked for TLS), and its promise settles with a fresh ArrayBuffer. Every other member is present
 * but rejects its promise with NotSupportedError, the spec's report for an unsupported operation,
 * so feature tests see a function and fail where Chrome would fail on an unknown algorithm.
 * Anti-bot challenges hash their proof of work through this path, so a missing `subtle` stops them
 * before they can answer.
 */
use super::dom;
use crate::v9::builtins::typed;
use crate::v9::value::JsValue;
use crate::v9::vm::data::PromiseState;
use crate::v9::vm::heap::{HIDDEN_ATTRS, Key, Slot};
use crate::v9::vm::realm::{Builder, Static};
use crate::v9::vm::{Call, Res, Thrown, Vm};

/** The private slot on a Crypto object where its SubtleCrypto instance is kept. */
const SLOT: &str = "subtle";
/** The algorithm names digest accepts, compared after ASCII upper-casing as Web Crypto normalizes them. */
const SHA1: &str = "SHA-1";
const SHA256: &str = "SHA-256";
const SHA384: &str = "SHA-384";
const SHA512: &str = "SHA-512";

/** Every SubtleCrypto member with its WebIDL length; only digest computes, the rest reject. */
const MEMBERS: [(&str, u32); 12] = [
    ("decrypt", 3),
    ("deriveBits", 2),
    ("deriveKey", 5),
    ("digest", 2),
    ("encrypt", 3),
    ("exportKey", 2),
    ("generateKey", 3),
    ("importKey", 5),
    ("sign", 3),
    ("unwrapKey", 7),
    ("verify", 4),
    ("wrapKey", 4),
];

/** Install the SubtleCrypto interface, then the single `crypto.subtle` instance that the getter returns. */
pub(super) fn install(b: &mut Builder, crypto_proto: u32, crypto: u32) {
    let proto = b.object(Some(b.ids.object_prototype));
    super::chrome_env::interface(b, "SubtleCrypto", proto);
    for (name, length) in MEMBERS {
        let implementation = if name == "digest" {
            digest
        } else {
            unsupported
        };
        b.host_method(proto, name, length, implementation);
    }
    let subtle = b.object(Some(proto));
    b.put(
        crypto,
        Key::private(SLOT),
        Static::Value(JsValue::Object(subtle)),
        HIDDEN_ATTRS,
    );
    b.getter(crypto_proto, "subtle", subtle_get);
}

/** A member Mars does not implement: a promise rejected with NotSupportedError naming the member. */
fn unsupported(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let message = format!(
        "Failed to execute '{}' on 'SubtleCrypto': this operation is not supported.",
        dom::native_name(&call)
    );
    let error = vm.dom_exception("NotSupportedError", &message);
    rejected(vm, error)
}

/** A promise already rejected with the given error; a non-catchable failure propagates instead. */
fn rejected(vm: &mut Vm, error: Thrown) -> Res<JsValue> {
    let promise = vm.new_promise()?;
    let state = match error {
        Thrown::Value(error) => PromiseState::Rejected(error),
        other => return Err(other),
    };
    vm.settle(promise, state)?;
    Ok(JsValue::Object(promise))
}

/** `crypto.subtle`: the instance install stored on the receiver, or a TypeError for another receiver. */
fn subtle_get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let JsValue::Object(id) = call.this else {
        return Err(vm.type_error("Illegal invocation"));
    };
    match vm.own(id, &Key::private(SLOT))? {
        Some(prop) => match prop.slot {
            Slot::Data(value) => Ok(value),
            _ => Ok(JsValue::Undefined),
        },
        None => Err(vm.type_error("Illegal invocation")),
    }
}

/** The digest function a name selects, or None when digest does not implement it. */
fn algorithm(name: &str) -> Option<&'static ring::digest::Algorithm> {
    match name.to_ascii_uppercase().as_str() {
        SHA1 => Some(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY),
        SHA256 => Some(&ring::digest::SHA256),
        SHA384 => Some(&ring::digest::SHA384),
        SHA512 => Some(&ring::digest::SHA512),
        _ => None,
    }
}

/** The algorithm name an argument gives: a string, or an object whose `name` is one. */
fn algorithm_name(vm: &mut Vm, value: &JsValue) -> Res<Option<String>> {
    match value {
        JsValue::String(text) => Ok(Some(text.lossy())),
        JsValue::Object(_) => {
            let name = vm.get(value, &Key::from("name"))?;
            match name {
                JsValue::String(text) => Ok(Some(text.lossy())),
                _ => Ok(None),
            }
        }
        _ => Ok(None),
    }
}

/** `crypto.subtle.digest(algorithm, data)`: a promise of the digest as an ArrayBuffer, or a rejected promise for a bad argument. */
fn digest(vm: &mut Vm, call: Call) -> Res<JsValue> {
    match digest_outcome(vm, &call) {
        Ok(buffer) => {
            let promise = vm.new_promise()?;
            vm.settle(promise, PromiseState::Fulfilled(buffer))?;
            Ok(JsValue::Object(promise))
        }
        Err(error) => rejected(vm, error),
    }
}

/** The digest as an ArrayBuffer, or the error the rejection carries. */
fn digest_outcome(vm: &mut Vm, call: &Call) -> Res<JsValue> {
    let name = algorithm_name(vm, &call.arg(0))?;
    let Some(algorithm) = name.as_deref().and_then(algorithm) else {
        return Err(vm.dom_exception(
            "NotSupportedError",
            "Failed to execute 'digest' on 'SubtleCrypto': the algorithm is not supported.",
        ));
    };
    let Some(bytes) = typed::source_bytes(vm, &call.arg(1)) else {
        return Err(vm.type_error(
            "Failed to execute 'digest' on 'SubtleCrypto': parameter 2 is not of type 'BufferSource'.",
        ));
    };
    let hash = ring::digest::digest(algorithm, &bytes).as_ref().to_vec();
    let view = typed::new_uint8(vm, hash)?;
    vm.get(&JsValue::Object(view), &Key::from("buffer"))
}
