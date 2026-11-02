/**
 * The Async Clipboard API's system side: the macOS general pasteboard (NSPasteboard). As in
 * Chrome, a page may read or write only while its document is focused (its tab is the selected
 * one) and it holds transient user activation; anything else is NotAllowedError. Plain text, HTML
 * and PNG cross; other types are refused before they reach the pasteboard.
 */
use super::{Ctx, Handler, Reply, fail, ok};
use crate::protocol::EngineResult;
use crate::runtime::supervisor::Runtime;
use serde_json::{Value, json};

/** Clipboard operations. */
pub(super) const HANDLERS: &[(&str, Handler)] =
    &[("clipboard.write", write), ("clipboard.read", read)];

/** Web MIME types and the pasteboard (UTI) types they map to. */
const TYPES: [(&str, &str); 3] = [
    ("text/plain", "public.utf8-plain-text"),
    ("text/html", "public.html"),
    ("image/png", "public.png"),
];

/** Why the context may not use the clipboard now, or None when it may. */
fn gate(ctx: &Ctx, verb: &str) -> Option<String> {
    if !ctx.selected {
        return Some("Document is not focused.".into());
    }
    if !ctx.active {
        return Some(format!("{verb} permission denied."));
    }
    None
}

/** One representation from the page: its pasteboard type and bytes. */
fn representation(item: &Value) -> Option<(&'static str, Vec<u8>)> {
    let kind = item["type"].as_str()?;
    let uti = TYPES.iter().find(|(mime, _)| *mime == kind)?.1;
    let bytes = match item["text"].as_str() {
        Some(text) => text.as_bytes().to_vec(),
        None => crate::runtime::images::decode_base64(item["base64"].as_str()?).ok()?,
    };
    Some((uti, bytes))
}

/** navigator.clipboard.write / writeText. */
fn write(_: &mut Runtime, ctx: &Ctx, args: &Value, _: &[u8]) -> EngineResult<Reply> {
    if let Some(reason) = gate(ctx, "Write") {
        return fail("NotAllowedError", &reason);
    }
    let items = args["items"].as_array().cloned().unwrap_or_default();
    let Some(representations) = items.iter().map(representation).collect::<Option<Vec<_>>>() else {
        return fail("NotAllowedError", "Type not supported on write.");
    };
    if native::write(&representations) {
        ok(Value::Null)
    } else {
        fail("NotAllowedError", "The system clipboard is not available.")
    }
}

/** navigator.clipboard.read / readText: the requested types the pasteboard holds. */
fn read(_: &mut Runtime, ctx: &Ctx, args: &Value, _: &[u8]) -> EngineResult<Reply> {
    if let Some(reason) = gate(ctx, "Read") {
        return fail("NotAllowedError", &reason);
    }
    let wanted: Vec<&str> = args["types"]
        .as_array()
        .map(|list| list.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let mut found = Vec::new();
    for (mime, uti) in TYPES.iter().filter(|(mime, _)| wanted.contains(mime)) {
        let Some(bytes) = native::read(uti) else {
            continue;
        };
        found.push(if mime.starts_with("text/") {
            json!({"type": mime, "text": String::from_utf8_lossy(&bytes)})
        } else {
            json!({"type": mime, "base64": crate::runtime::base64(&bytes)})
        });
    }
    ok(Value::Array(found))
}

/** The general pasteboard through AppKit. */
#[cfg(target_os = "macos")]
mod native {
    use objc2::msg_send;
    use objc2::rc::Retained;
    use objc2::runtime::{AnyClass, AnyObject};
    use objc2_foundation::{NSData, NSString};

    /** The general pasteboard, or None when AppKit is unavailable. */
    fn board() -> Option<Retained<AnyObject>> {
        let class = AnyClass::get(c"NSPasteboard")?;
        // SAFETY: generalPasteboard is a class method returning the shared NSPasteboard.
        Some(unsafe { msg_send![class, generalPasteboard] })
    }

    /** Replace the pasteboard contents with these representations of one item. */
    pub fn write(representations: &[(&str, Vec<u8>)]) -> bool {
        let Some(board) = board() else {
            return false;
        };
        // SAFETY: clearContents and setData:forType: take an NSData and an NSString type.
        unsafe {
            let _: isize = msg_send![&board, clearContents];
            representations.iter().all(|(uti, bytes)| {
                let data = NSData::with_bytes(bytes);
                msg_send![&board, setData: &*data, forType: &*NSString::from_str(uti)]
            })
        }
    }

    /** The bytes of one pasteboard type, when present. */
    pub fn read(uti: &str) -> Option<Vec<u8>> {
        let board = board()?;
        // SAFETY: dataForType: returns an autoreleased NSData or nil.
        let data: Option<Retained<NSData>> =
            unsafe { msg_send![&board, dataForType: &*NSString::from_str(uti)] };
        data.map(|data| data.to_vec())
    }
}

/** Platforms without AppKit have no system clipboard Mars can reach. */
#[cfg(not(target_os = "macos"))]
mod native {
    /** No pasteboard to write. */
    pub fn write(_: &[(&str, Vec<u8>)]) -> bool {
        false
    }
    /** No pasteboard to read. */
    pub fn read(_: &str) -> Option<Vec<u8>> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /** A context with these focus and activation facts. */
    fn ctx(selected: bool, active: bool) -> Ctx {
        let url = url::Url::parse("https://example.com/").unwrap();
        Ctx {
            identity: "c".into(),
            kind: super::super::Kind::Window,
            origin: "https://example.com".into(),
            top_level: url.clone(),
            url,
            active,
            selected,
            tab: "tab-1".into(),
        }
    }

    /** Focus and transient activation are both required. */
    #[test]
    fn clipboard_needs_focus_and_activation() {
        assert!(gate(&ctx(true, true), "Write").is_none());
        assert_eq!(
            gate(&ctx(false, true), "Write").unwrap(),
            "Document is not focused."
        );
        assert_eq!(
            gate(&ctx(true, false), "Read").unwrap(),
            "Read permission denied."
        );
    }

    /** Only the three supported types map to pasteboard types. */
    #[test]
    fn representations_map_supported_types() {
        let text = representation(&json!({"type":"text/plain","text":"hi"})).unwrap();
        assert_eq!(text, ("public.utf8-plain-text", b"hi".to_vec()));
        assert!(representation(&json!({"type":"text/rtf","text":"x"})).is_none());
    }
}
