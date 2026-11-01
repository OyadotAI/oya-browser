/**
 * crypto.subtle is a SubtleCrypto instance, as Chrome's is: the interface object and its prototype
 * carry every member Chrome exposes, with the spec's length and name. Four members compute:
 * `digest` (SHA-1, SHA-256, SHA-384, SHA-512), and for AES-GCM `importKey` ("raw" keys), `encrypt`
 * and `decrypt`, all on ring through `subtle_aes`. Every other member is present but rejects its
 * promise with NotSupportedError, the spec's report for an unsupported operation.
 * Anti-bot challenges choose their path by what SubtleCrypto offers: with importKey and encrypt
 * present they run the native AES-GCM path, so those must answer as Chrome does. A stub that
 * rejected sent Amazon's WAF challenge down a path that failed, and the store never loaded.
 */
use super::dom;
use super::subtle_aes::{self, Fault};
use crate::v9::builtins::typed;
use crate::v9::value::{JsValue, ObjectId};
use crate::v9::vm::data::PromiseState;
use crate::v9::vm::heap::{HIDDEN_ATTRS, Key, Slot};
use crate::v9::vm::realm::{Builder, NativeFn, Static};
use crate::v9::vm::{Call, Res, Thrown, Vm};

/** The private slot on a Crypto object where its SubtleCrypto instance is kept. */
const SLOT: &str = "subtle";
/** The private slot on the SubtleCrypto instance holding the prototype CryptoKeys inherit. */
const KEY_PROTO: &str = "cryptoKeyPrototype";
/** The private slot on a CryptoKey holding its raw key bytes, which no script can read. */
const KEY_BYTES: &str = "keyBytes";
/** The algorithm names digest accepts, compared after ASCII upper-casing as Web Crypto normalizes them. */
const SHA1: &str = "SHA-1";
const SHA256: &str = "SHA-256";
const SHA384: &str = "SHA-384";
const SHA512: &str = "SHA-512";
/** The one AES mode importKey, encrypt and decrypt implement. */
const AES_GCM: &str = "AES-GCM";
/** The tag length Web Crypto defaults to, and the only one ring produces. */
const DEFAULT_TAG_BITS: f64 = 128.0;
/** Bits in a byte, to report a key's length in bits as a CryptoKey does. */
const BITS_PER_BYTE: usize = 8;

/** Every SubtleCrypto member with its WebIDL length. */
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

/** Install the SubtleCrypto and CryptoKey interfaces, then the single `crypto.subtle` instance the getter returns. */
pub(super) fn install(b: &mut Builder, crypto_proto: u32, crypto: u32) {
    let proto = b.object(Some(b.ids.object_prototype));
    super::chrome_env::interface(b, "SubtleCrypto", proto);
    let key_proto = b.object(Some(b.ids.object_prototype));
    super::chrome_env::interface(b, "CryptoKey", key_proto);
    for (name, length) in MEMBERS {
        b.host_method(proto, name, length, implementation(name));
    }
    let subtle = b.object(Some(proto));
    b.put(
        subtle,
        Key::private(KEY_PROTO),
        Static::Value(JsValue::Object(key_proto)),
        HIDDEN_ATTRS,
    );
    b.put(
        crypto,
        Key::private(SLOT),
        Static::Value(JsValue::Object(subtle)),
        HIDDEN_ATTRS,
    );
    b.getter(crypto_proto, "subtle", subtle_get);
}

/** The native function a member runs: the four that compute, or the rejecting stub for the rest. */
fn implementation(name: &str) -> NativeFn {
    match name {
        "digest" => digest,
        "importKey" => import_key,
        "encrypt" => encrypt,
        "decrypt" => decrypt,
        _ => unsupported,
    }
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

/** A promise fulfilled with the outcome's value, or rejected with the error it carries. */
fn settled(vm: &mut Vm, outcome: Res<JsValue>) -> Res<JsValue> {
    match outcome {
        Ok(value) => {
            let promise = vm.new_promise()?;
            vm.settle(promise, PromiseState::Fulfilled(value))?;
            Ok(JsValue::Object(promise))
        }
        Err(error) => rejected(vm, error),
    }
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
        JsValue::Object(_) => match vm.get(value, &Key::from("name"))? {
            JsValue::String(text) => Ok(Some(text.lossy())),
            _ => Ok(None),
        },
        _ => Ok(None),
    }
}

/** Whether an algorithm argument names AES-GCM, as Web Crypto's case-insensitive lookup does. */
fn is_aes_gcm(vm: &mut Vm, value: &JsValue) -> Res<bool> {
    Ok(algorithm_name(vm, value)?.is_some_and(|name| name.eq_ignore_ascii_case(AES_GCM)))
}

/** A JavaScript string value. */
fn text(value: &str) -> Res<JsValue> {
    JsValue::string(value).map_err(Into::into)
}

/** `crypto.subtle.digest(algorithm, data)`: a promise of the digest as an ArrayBuffer, or a rejected promise for a bad argument. */
fn digest(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let outcome = digest_outcome(vm, &call);
    settled(vm, outcome)
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
    array_buffer(vm, hash)
}

/** A fresh ArrayBuffer holding the bytes. */
fn array_buffer(vm: &mut Vm, bytes: Vec<u8>) -> Res<JsValue> {
    let view = typed::new_uint8(vm, bytes)?;
    vm.get(&JsValue::Object(view), &Key::from("buffer"))
}

/** `crypto.subtle.importKey(format, keyData, algorithm, extractable, usages)`: a promise of a CryptoKey for a raw AES-GCM key. */
fn import_key(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let outcome = import_key_outcome(vm, &call);
    settled(vm, outcome)
}

/** The CryptoKey an import makes, or the error the rejection carries. Usages are not enforced: a key may run any operation it is asked for. */
fn import_key_outcome(vm: &mut Vm, call: &Call) -> Res<JsValue> {
    let format = vm.to_text(&call.arg(0))?;
    if format != "raw" || !is_aes_gcm(vm, &call.arg(2))? {
        return Err(not_supported(
            vm,
            "importKey",
            "this key format or algorithm",
        ));
    }
    let Some(bytes) = typed::source_bytes(vm, &call.arg(1)) else {
        return Err(vm.type_error(
            "Failed to execute 'importKey' on 'SubtleCrypto': parameter 2 is not of type 'BufferSource'.",
        ));
    };
    if subtle_aes::Key::raw(&bytes).is_err() {
        return Err(vm.dom_exception(
            "DataError",
            "Failed to execute 'importKey' on 'SubtleCrypto': AES keys are 128 or 256 bits.",
        ));
    }
    let extractable = call.arg(3).truthy();
    make_key(vm, &call.this, bytes, extractable)
}

/** A CryptoKey whose raw bytes sit in a private slot, with its type, extractability and algorithm. */
fn make_key(vm: &mut Vm, subtle: &JsValue, bytes: Vec<u8>, extractable: bool) -> Res<JsValue> {
    let bits = bytes.len() * BITS_PER_BYTE;
    let proto = key_prototype(vm, subtle)?;
    let view = typed::new_uint8(vm, bytes)?;
    let id = vm.new_object()?;
    vm.set_proto(id, Some(proto))?;
    vm.put_key(id, Key::private(KEY_BYTES), JsValue::Object(view))?;
    let algorithm = vm.new_object()?;
    let name = text(AES_GCM)?;
    vm.put(algorithm, "name", name)?;
    vm.put(algorithm, "length", JsValue::Number(bits as f64))?;
    let kind = text("secret")?;
    vm.put(id, "type", kind)?;
    vm.put(id, "extractable", JsValue::Boolean(extractable))?;
    vm.put(id, "algorithm", JsValue::Object(algorithm))?;
    Ok(JsValue::Object(id))
}

/** The CryptoKey prototype the SubtleCrypto instance holds in its private slot. */
fn key_prototype(vm: &mut Vm, subtle: &JsValue) -> Res<ObjectId> {
    let JsValue::Object(id) = subtle else {
        return Err(vm.type_error("Illegal invocation"));
    };
    match vm.own(*id, &Key::private(KEY_PROTO))? {
        Some(prop) => match prop.slot {
            Slot::Data(JsValue::Object(proto)) => Ok(proto),
            _ => Err(vm.type_error("Illegal invocation")),
        },
        None => Err(vm.type_error("Illegal invocation")),
    }
}

/** `crypto.subtle.encrypt(algorithm, key, data)`: a promise of the AES-GCM ciphertext followed by its tag. */
fn encrypt(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let outcome = cipher_outcome(vm, &call, true);
    settled(vm, outcome)
}

/** `crypto.subtle.decrypt(algorithm, key, data)`: a promise of the plaintext, or OperationError when authentication fails. */
fn decrypt(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let outcome = cipher_outcome(vm, &call, false);
    settled(vm, outcome)
}

/** The AES-GCM seal or open of the call's data as an ArrayBuffer, or the error the rejection carries. */
fn cipher_outcome(vm: &mut Vm, call: &Call, seal: bool) -> Res<JsValue> {
    let method = if seal { "encrypt" } else { "decrypt" };
    let algorithm = call.arg(0);
    if !is_aes_gcm(vm, &algorithm)? {
        return Err(not_supported(vm, method, "this algorithm"));
    }
    check_tag_length(vm, &algorithm, method)?;
    let iv = required_bytes(vm, &algorithm, "iv", method)?;
    let aad = optional_bytes(vm, &algorithm, "additionalData")?;
    let key = key_material(vm, &call.arg(1), method)?;
    let Some(data) = typed::source_bytes(vm, &call.arg(2)) else {
        return Err(vm.type_error(&format!(
            "Failed to execute '{method}' on 'SubtleCrypto': parameter 3 is not of type 'BufferSource'."
        )));
    };
    let output = run_cipher(
        vm,
        &Sealing {
            key: &key,
            iv: &iv,
            aad: &aad,
            data: &data,
            seal,
        },
        method,
    )?;
    array_buffer(vm, output)
}

/** One AES-GCM operation's inputs: the key bytes, IV, additional data, the data and the direction. */
struct Sealing<'a> {
    /** The raw AES key bytes. */
    key: &'a [u8],
    /** The IV the operation uses. */
    iv: &'a [u8],
    /** The additional authenticated data, empty when none was given. */
    aad: &'a [u8],
    /** The plaintext to seal, or the ciphertext with its tag to open. */
    data: &'a [u8],
    /** Whether to seal (true) or open (false). */
    seal: bool,
}

/** Seal or open with the key; a fault becomes the DOMException Web Crypto names for it. */
fn run_cipher(vm: &mut Vm, job: &Sealing<'_>, method: &str) -> Res<Vec<u8>> {
    let outcome = subtle_aes::Key::raw(job.key).and_then(|key| {
        if job.seal {
            key.seal(job.iv, job.aad, job.data)
        } else {
            key.open(job.iv, job.aad, job.data)
        }
    });
    outcome.map_err(|fault| fault_error(vm, fault, method))
}

/** The DOMException a cipher fault rejects with; the IV message names the limit Mars has, never a silent cut. */
fn fault_error(vm: &mut Vm, fault: Fault, method: &str) -> Thrown {
    match fault {
        Fault::KeyLength => vm.dom_exception(
            "OperationError",
            &format!("Failed to execute '{method}' on 'SubtleCrypto': the key is not an AES-GCM key."),
        ),
        Fault::IvLength => vm.dom_exception(
            "NotSupportedError",
            &format!("Failed to execute '{method}' on 'SubtleCrypto': only 96-bit AES-GCM IVs are supported."),
        ),
        Fault::Authentication => vm.dom_exception(
            "OperationError",
            &format!("Failed to execute '{method}' on 'SubtleCrypto': the data failed authentication."),
        ),
    }
}

/** The raw key bytes a CryptoKey argument holds, or a TypeError when the argument is not a CryptoKey. */
fn key_material(vm: &mut Vm, value: &JsValue, method: &str) -> Res<Vec<u8>> {
    let JsValue::Object(id) = value else {
        return Err(not_a_key(vm, method));
    };
    let Some(prop) = vm.own(*id, &Key::private(KEY_BYTES))? else {
        return Err(not_a_key(vm, method));
    };
    let Slot::Data(bytes) = prop.slot else {
        return Err(not_a_key(vm, method));
    };
    match typed::source_bytes(vm, &bytes) {
        Some(bytes) => Ok(bytes),
        None => Err(not_a_key(vm, method)),
    }
}

/** The TypeError for a key argument that is not a CryptoKey. */
fn not_a_key(vm: &mut Vm, method: &str) -> Thrown {
    vm.type_error(&format!(
        "Failed to execute '{method}' on 'SubtleCrypto': parameter 2 is not of type 'CryptoKey'."
    ))
}

/** A tag length other than 128 bits is refused, since ring seals only 128-bit tags. */
fn check_tag_length(vm: &mut Vm, algorithm: &JsValue, method: &str) -> Res<()> {
    match vm.get(algorithm, &Key::from("tagLength"))? {
        JsValue::Undefined => Ok(()),
        JsValue::Number(bits) if bits == DEFAULT_TAG_BITS => Ok(()),
        _ => Err(not_supported(vm, method, "a tagLength other than 128 bits")),
    }
}

/** A required BufferSource member of the algorithm, or a TypeError naming it. */
fn required_bytes(vm: &mut Vm, algorithm: &JsValue, member: &str, method: &str) -> Res<Vec<u8>> {
    let value = vm.get(algorithm, &Key::from(member))?;
    match typed::source_bytes(vm, &value) {
        Some(bytes) => Ok(bytes),
        None => Err(vm.type_error(&format!(
            "Failed to execute '{method}' on 'SubtleCrypto': the algorithm's {member} is required."
        ))),
    }
}

/** An optional BufferSource member of the algorithm; absent means empty, as GCM's default. */
fn optional_bytes(vm: &mut Vm, algorithm: &JsValue, member: &str) -> Res<Vec<u8>> {
    let value = vm.get(algorithm, &Key::from(member))?;
    Ok(typed::source_bytes(vm, &value).unwrap_or_default())
}

/** The NotSupportedError for an input Mars does not implement, naming the method and what it refused. */
fn not_supported(vm: &mut Vm, method: &str, what: &str) -> Thrown {
    vm.dom_exception(
        "NotSupportedError",
        &format!("Failed to execute '{method}' on 'SubtleCrypto': {what} is not supported."),
    )
}
