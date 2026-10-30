/** Context-owned Blob URL bytes survive candidate cloning without acquiring network authority. */
use super::{Page, blob, window};
use crate::constants::{URL_BYTES, VM_STATE_BYTES};
use crate::protocol::{EngineError, EngineResult};
use crate::v9::value::JsValue;
use crate::v9::vm::{Call, Res, Vm};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;
use url::Url;

/** Bound URL metadata as well as retained bodies in each document or worker context. */
const URL_COUNT: usize = 64;
/** Leave room for the corresponding Blob heap storage in the serialized VM envelope. */
const STORAGE_BYTES: usize = VM_STATE_BYTES / 16;
/** A canonical UUID has fixed punctuation and version/variant positions. */
const UUID_LENGTH: usize = 36;
const UUID_DASHES: [usize; 4] = [8, 13, 18, 23];
const UUID_VERSION: usize = 14;
const UUID_VARIANT: usize = 19;

/// An immutable snapshot from a genuinely branded Blob, captured before URL revocation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlobUrlSource {
    /// Candidate clones share immutable storage; scripts cannot mutate retained bytes.
    pub bytes: Arc<Vec<u8>>,
    /// Normalized Blob MIME metadata, independent of script-visible properties.
    pub mime_type: String,
    /// Creator origin binds the URL to the context that supplied its bytes.
    pub origin: String,
}

impl BlobUrlSource {
    /// Validate restored or IPC-carried bytes without granting authority from URL text alone.
    pub fn validate(&self, address: &str) -> EngineResult<()> {
        let key = key(address).ok_or_else(invalid)?;
        if !canonical(&key, &self.origin)
            || self.mime_type.len() > URL_BYTES
            || !self
                .mime_type
                .bytes()
                .all(|byte| (0x20..=0x7e).contains(&byte))
            || self.mime_type != self.mime_type.to_ascii_lowercase()
        {
            return Err(invalid());
        }
        if self.storage(&key) > STORAGE_BYTES {
            return Err(EngineError::limit("Blob URL bytes", STORAGE_BYTES));
        }
        Ok(())
    }

    /** Charge metadata and body together, including repeated references to the same Blob. */
    fn storage(&self, address: &str) -> usize {
        self.bytes
            .len()
            .saturating_add(address.len())
            .saturating_add(self.mime_type.len())
            .saturating_add(self.origin.len())
    }
}

/** Restore accepts only bounded, canonical records produced by this context's registry. */
pub(in crate::v9) fn validate(entries: &BTreeMap<String, BlobUrlSource>) -> EngineResult<()> {
    if entries.len() > URL_COUNT {
        return Err(EngineError::limit("Blob URLs", URL_COUNT));
    }
    let mut total = 0usize;
    for (address, source) in entries {
        source.validate(address)?;
        if key(address).as_deref() != Some(address.as_str()) {
            return Err(invalid());
        }
        total = total.saturating_add(source.storage(address));
    }
    if total > STORAGE_BYTES {
        return Err(EngineError::limit("Blob URL bytes", STORAGE_BYTES));
    }
    Ok(())
}

/** URL strings never resolve outside the current context's owned registry. */
pub(in crate::v9) fn resolve(page: &Page, address: &str) -> Option<BlobUrlSource> {
    let source = page.state.blob_urls.get(&key(address)?)?;
    let origin = Url::parse(&page.environment.url)
        .ok()?
        .origin()
        .ascii_serialization();
    (source.origin == origin).then(|| source.clone())
}

/** Allocate an unguessable URL only after reading actual private Blob slots. */
pub(super) fn create(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let value = call.arg(0);
    let bytes = blob::blob_bytes(vm, &value)
        .ok_or_else(|| vm.type_error("URL.createObjectURL requires a Blob"))?;
    let origin = Url::parse(&vm.page.environment.url)
        .map_err(|_| vm.dom_exception("SecurityError", "Invalid Blob URL origin"))?
        .origin()
        .ascii_serialization();
    let source = BlobUrlSource {
        bytes: Arc::new(bytes),
        mime_type: blob::type_of(vm, &value),
        origin,
    };
    let address = format!("blob:{}/{}", source.origin, window::random_uuid()?);
    admit(&vm.page.state.blob_urls, &address, &source)?;
    let result = JsValue::string(&address)?;
    vm.page.state.blob_urls.insert(address, source);
    Ok(result)
}

/** Revocation releases this context's mapping; already captured worker sources stay alive. */
pub(super) fn revoke(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if call.args.is_empty() {
        return Err(vm.type_error("URL.revokeObjectURL requires a URL"));
    }
    let address = vm.to_text(&call.arg(0))?;
    if let Some(address) = key(&address) {
        vm.page.state.blob_urls.remove(&address);
    }
    Ok(JsValue::Undefined)
}

/** Reject exhaustion before mutation so failed allocation leaves every existing URL usable. */
fn admit(
    entries: &BTreeMap<String, BlobUrlSource>,
    address: &str,
    source: &BlobUrlSource,
) -> EngineResult<()> {
    source.validate(address)?;
    if entries.len() >= URL_COUNT || entries.contains_key(address) {
        return Err(EngineError::limit("Blob URLs", URL_COUNT));
    }
    let used = entries
        .iter()
        .fold(source.storage(address), |total, (url, value)| {
            total.saturating_add(value.storage(url))
        });
    if used > STORAGE_BYTES {
        return Err(EngineError::limit("Blob URL bytes", STORAGE_BYTES));
    }
    Ok(())
}

/** Fragments do not identify a different Blob; queries remain part of the lookup key. */
fn key(address: &str) -> Option<String> {
    if address.len() > URL_BYTES {
        return None;
    }
    let mut url = Url::parse(address).ok()?;
    if url.scheme() != "blob" {
        return None;
    }
    url.set_fragment(None);
    Some(url.into())
}

/** Only canonical origins and CSPRNG UUID forms are accepted in serialized registries. */
fn canonical(address: &str, origin: &str) -> bool {
    if origin != "null"
        && !Url::parse(origin).is_ok_and(|url| url.origin().ascii_serialization() == origin)
    {
        return false;
    }
    let Some(token) = address.strip_prefix(&format!("blob:{origin}/")) else {
        return false;
    };
    token.len() == UUID_LENGTH
        && token.bytes().enumerate().all(|(index, byte)| {
            if UUID_DASHES.contains(&index) {
                return byte == b'-';
            }
            if index == UUID_VERSION {
                return byte == b'4';
            }
            if index == UUID_VARIANT {
                return matches!(byte, b'8' | b'9' | b'a' | b'b');
            }
            byte.is_ascii_digit() || matches!(byte, b'a'..=b'f')
        })
}

/** Keep malformed serialized records distinct from exhausted resource bounds. */
fn invalid() -> EngineError {
    EngineError::invalid("Invalid Blob URL state.")
}
