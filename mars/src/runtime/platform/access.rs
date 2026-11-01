/**
 * Storage Access API and the permission decisions the platform APIs share. Mars has no
 * permission prompt, so a request that a browser would prompt for is decided by the person's
 * transient activation: a click or key press in the page grants it (as Safari and Firefox
 * grant storage access) and the grant is remembered for the session; without one the request is
 * refused with NotAllowedError. `navigator.permissions.query` reports the same decisions.
 */
use super::{Ctx, Handler, Kind, Reply, fail, ok};
use crate::protocol::EngineResult;
use crate::runtime::supervisor::Runtime;
use serde_json::{Value, json};
use std::collections::BTreeSet;

/** Storage Access and permission operations. */
pub(super) const HANDLERS: &[(&str, Handler)] = &[
    ("storage.has", has),
    ("storage.request", request),
    ("storage.requestFor", request_for),
    ("permissions.query", query),
];

/** Grants made this session: storage access by (top-level site, embedded site), MIDI by origin. */
#[derive(Default)]
pub(in crate::runtime) struct Grants {
    storage: BTreeSet<(String, String)>,
    midi: BTreeSet<(String, bool)>,
}

impl Grants {
    /** Grant MIDI to an origin (with or without sysex). */
    pub fn grant_midi(&mut self, origin: &str, sysex: bool) {
        self.midi.insert((origin.into(), sysex));
        if sysex {
            self.midi.insert((origin.into(), false));
        }
    }
    /** Whether an origin holds MIDI access of this kind. */
    pub fn midi(&self, origin: &str, sysex: bool) -> bool {
        self.midi.contains(&(origin.to_string(), sysex))
    }
}

/** The (top-level site, embedded site) pair a frame's storage access is keyed by. */
fn pair(ctx: &Ctx) -> (String, String) {
    let site = crate::network::partition_site(&ctx.url).unwrap_or_else(|| ctx.origin.clone());
    (ctx.partition(), site)
}

/** Whether the context can use its unpartitioned storage and cookies now. */
fn has_access(runtime: &Runtime, ctx: &Ctx) -> bool {
    ctx.kind != Kind::Frame
        || !ctx.cross_site()
        || runtime.platform.access.storage.contains(&pair(ctx))
}

/** document.hasStorageAccess() and hasUnpartitionedCookieAccess(). */
fn has(runtime: &mut Runtime, ctx: &Ctx, _: &Value, _: &[u8]) -> EngineResult<Reply> {
    ok(json!(has_access(runtime, ctx)))
}

/** document.requestStorageAccess(): granted with activation in a cross-site frame. */
fn request(runtime: &mut Runtime, ctx: &Ctx, _: &Value, _: &[u8]) -> EngineResult<Reply> {
    if has_access(runtime, ctx) {
        return ok(json!(true));
    }
    if !ctx.active {
        return fail(
            "NotAllowedError",
            "requestStorageAccess: Must be handling a user gesture to use.",
        );
    }
    runtime.platform.access.storage.insert(pair(ctx));
    ok(json!(true))
}

/** document.requestStorageAccessFor(origin): only for a same-site origin, from a top-level page. */
fn request_for(runtime: &mut Runtime, ctx: &Ctx, args: &Value, _: &[u8]) -> EngineResult<Reply> {
    let _ = runtime;
    if ctx.kind != Kind::Window {
        return fail(
            "NotAllowedError",
            "requestStorageAccessFor: Only supported in primary top-level browsing contexts.",
        );
    }
    if !ctx.active {
        return fail(
            "NotAllowedError",
            "requestStorageAccessFor: Must be handling a user gesture to use.",
        );
    }
    let requested = args["origin"]
        .as_str()
        .and_then(|origin| url::Url::parse(origin).ok());
    let same_site = requested.is_some_and(|url| {
        crate::network::partition_site(&url) == crate::network::partition_site(&ctx.top_level)
    });
    if same_site {
        ok(Value::Null)
    } else {
        fail(
            "NotAllowedError",
            "requestStorageAccessFor: The origin is not in the top-level site's related website set.",
        )
    }
}

/** navigator.permissions.query for the permissions these APIs decide. */
fn query(runtime: &mut Runtime, ctx: &Ctx, args: &Value, _: &[u8]) -> EngineResult<Reply> {
    let granted = match args["name"].as_str().unwrap_or_default() {
        "storage-access" => has_access(runtime, ctx),
        "midi" => runtime
            .platform
            .access
            .midi(&ctx.origin, args["sysex"].as_bool() == Some(true)),
        "clipboard-write" | "screen-wake-lock" => true,
        _ => false,
    };
    ok(json!(if granted { "granted" } else { "prompt" }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /** A sysex grant implies the plain MIDI grant, not the reverse. */
    #[test]
    fn sysex_grant_implies_plain_midi() {
        let mut grants = Grants::default();
        grants.grant_midi("https://a.test", false);
        assert!(grants.midi("https://a.test", false));
        assert!(!grants.midi("https://a.test", true));
        grants.grant_midi("https://b.test", true);
        assert!(grants.midi("https://b.test", false) && grants.midi("https://b.test", true));
    }
}
