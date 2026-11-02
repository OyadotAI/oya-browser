/**
 * Cookie Store API over the real cookie jar. Reads are the script-visible cookies for the
 * context's URL (never HttpOnly); writes become a `document.cookie` field with the spec's
 * attributes (always Secure, SameSite Strict by default) and commit through the same
 * persisted-jar path as document.cookie. Change events come from comparing each watcher's last
 * snapshot with the jar on every platform poll, so HTTP Set-Cookie and document.cookie writes
 * fire them too.
 */
use super::{Ctx, Handler, Reply, fail, ok};
use crate::network::CookieEntry;
use crate::protocol::EngineResult;
use crate::runtime::supervisor::Runtime;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::time::{Duration, SystemTime};
use url::Url;

/** Cookie Store operations. */
pub(super) const HANDLERS: &[(&str, Handler)] = &[
    ("cookies.get", get),
    ("cookies.set", set),
    ("cookies.delete", delete),
];

/** Milliseconds per second, for Max-Age from an expiry in milliseconds. */
const MS_PER_SECOND: f64 = 1000.0;
/** The prefix whose cookies must be host-only with path "/". */
const HOST_PREFIX: &str = "__Host-";

/** Each watching context's URL and the cookies it last saw. */
#[derive(Default)]
pub(in crate::runtime) struct Watch {
    watchers: BTreeMap<String, (Url, Vec<Value>)>,
    pending: Vec<(String, Url)>,
}

impl Watch {
    /** Watch the jar for `ctx`; its first snapshot is taken on the next poll. */
    pub fn start(&mut self, ctx: &Ctx) {
        self.pending.push((ctx.identity.clone(), ctx.url.clone()));
    }
}

/** A jar entry as the Cookie Store's CookieListItem. */
fn item(entry: &CookieEntry) -> Value {
    let host_only = !entry.domain.starts_with('.');
    json!({
        "name": entry.name,
        "value": entry.value,
        "domain": if host_only { Value::Null } else { json!(entry.domain.trim_start_matches('.')) },
        "path": entry.path,
        "expires": entry.expires.map(|seconds| seconds as f64 * MS_PER_SECOND),
        "secure": entry.secure,
        "sameSite": entry.same_site.to_ascii_lowercase(),
        "partitioned": false,
    })
}

/** The script-visible cookies for `url`, as items. */
fn items(runtime: &mut Runtime, url: &Url) -> Vec<Value> {
    runtime
        .cookies
        .script_entries(url, SystemTime::now())
        .iter()
        .map(item)
        .collect()
}

/** getAll (and get, which takes the first): optionally by name and for a same-origin URL. */
fn get(runtime: &mut Runtime, ctx: &Ctx, args: &Value, _: &[u8]) -> EngineResult<Reply> {
    let url = match args["url"].as_str().map(Url::parse) {
        Some(Ok(url)) if url.origin() == ctx.url.origin() => url,
        Some(_) => return fail("TypeError", "URL must be same-origin."),
        None => ctx.url.clone(),
    };
    let name = args["name"].as_str();
    let list: Vec<Value> = items(runtime, &url)
        .into_iter()
        .filter(|cookie| name.is_none_or(|name| cookie["name"] == name))
        .collect();
    ok(Value::Array(list))
}

/** Why a cookie write is refused, as the spec's TypeError message, or None when it is valid. */
fn refusal(ctx: &Ctx, name: &str, domain: Option<&str>, path: &str) -> Option<&'static str> {
    let host = ctx.url.host_str().unwrap_or_default();
    if let Some(domain) = domain {
        if domain.starts_with('.') {
            return Some("Cookie domain cannot start with '.'.");
        }
        if host != domain && !host.ends_with(&format!(".{domain}")) {
            return Some("Cookie domain must domain-match the current host.");
        }
    }
    if name.starts_with(HOST_PREFIX) && (domain.is_some() || path != "/") {
        return Some("Cookies with the '__Host-' prefix must have no domain and path '/'.");
    }
    None
}

/** The `document.cookie` field a set() describes. */
fn field(args: &Value, name: &str, value: &str, path: &str) -> String {
    let mut field = format!(
        "{name}={value}; Path={path}; Secure; SameSite={}",
        args["sameSite"].as_str().unwrap_or("strict")
    );
    if let Some(domain) = args["domain"].as_str() {
        field.push_str(&format!("; Domain={domain}"));
    }
    if let Some(expires) = args["expires"].as_f64() {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs_f64()
            * MS_PER_SECOND;
        let seconds = ((expires - now) / MS_PER_SECOND).ceil().max(0.0);
        field.push_str(&format!("; Max-Age={seconds}"));
    }
    field
}

/** Write one field through the jar's script path and persist it. */
fn commit(runtime: &mut Runtime, ctx: &Ctx, field: String) -> EngineResult<Reply> {
    let url = ctx.url.clone();
    let mut refused = None;
    runtime.commit_jar(|jar| {
        refused = jar
            .set_document_cookie(&url, &field, SystemTime::now())
            .err();
        Vec::new()
    })?;
    runtime.poll_platform();
    match refused {
        Some(reason) => fail("TypeError", reason),
        None => ok(Value::Null),
    }
}

/** set(): validate as "set a cookie" does, then write. */
fn set(runtime: &mut Runtime, ctx: &Ctx, args: &Value, _: &[u8]) -> EngineResult<Reply> {
    let name = args["name"].as_str().unwrap_or_default();
    let value = args["value"].as_str().unwrap_or_default();
    let path = args["path"].as_str().unwrap_or("/");
    if let Some(reason) = refusal(ctx, name, args["domain"].as_str(), path) {
        return fail("TypeError", reason);
    }
    commit(runtime, ctx, field(args, name, value, path))
}

/** delete(): write the same cookie expired. */
fn delete(runtime: &mut Runtime, ctx: &Ctx, args: &Value, _: &[u8]) -> EngineResult<Reply> {
    let name = args["name"].as_str().unwrap_or_default();
    let path = args["path"].as_str().unwrap_or("/");
    if let Some(reason) = refusal(ctx, name, args["domain"].as_str(), path) {
        return fail("TypeError", reason);
    }
    let mut expired = args.clone();
    expired["expires"] = json!(0.0);
    commit(runtime, ctx, field(&expired, name, "", path))
}

/** The change between two snapshots: (changed or added, deleted). */
fn diff(before: &[Value], after: &[Value]) -> (Vec<Value>, Vec<Value>) {
    let key = |cookie: &Value| {
        (
            cookie["name"].clone(),
            cookie["domain"].clone(),
            cookie["path"].clone(),
        )
    };
    let changed = after
        .iter()
        .filter(|cookie| !before.contains(cookie))
        .cloned()
        .collect();
    let deleted = before
        .iter()
        .filter(|old| !after.iter().any(|new| key(new) == key(old)))
        .map(|old| json!({"name":old["name"],"value":Value::Null,"domain":old["domain"],"path":old["path"],"partitioned":false}))
        .collect();
    (changed, deleted)
}

impl Runtime {
    /** Compare every watcher's snapshot with the jar and queue change events; true on any. */
    pub(in crate::runtime) fn poll_cookies(&mut self) -> bool {
        for (identity, url) in std::mem::take(&mut self.platform.cookie_watch.pending) {
            let snapshot = items(self, &url);
            self.platform
                .cookie_watch
                .watchers
                .insert(identity, (url, snapshot));
        }
        let identities: Vec<String> = self
            .platform
            .cookie_watch
            .watchers
            .keys()
            .cloned()
            .collect();
        let mut changed = false;
        for identity in identities {
            changed |= self.poll_watcher(&identity);
        }
        changed
    }

    /** One watcher: queue a change event when its cookies changed since its snapshot. */
    fn poll_watcher(&mut self, identity: &str) -> bool {
        if !self.platform.events.subscribed(identity, "cookies") {
            self.platform.cookie_watch.watchers.remove(identity);
            return false;
        }
        let Some((url, before)) = self.platform.cookie_watch.watchers.get(identity).cloned() else {
            return false;
        };
        let after = items(self, &url);
        if after == before {
            return false;
        }
        let (changed, deleted) = diff(&before, &after);
        self.platform
            .cookie_watch
            .watchers
            .insert(identity.into(), (url, after));
        self.platform.events.push(
            identity,
            json!({"topic":"cookies","changed":changed,"deleted":deleted}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /** A changed value is reported as changed; a vanished cookie as deleted with a null value. */
    #[test]
    fn diff_reports_changes_and_deletions() {
        let a = json!({"name":"a","value":"1","domain":null,"path":"/"});
        let a2 = json!({"name":"a","value":"2","domain":null,"path":"/"});
        let b = json!({"name":"b","value":"1","domain":null,"path":"/"});
        let (changed, deleted) = diff(&[a.clone(), b], std::slice::from_ref(&a2));
        assert_eq!(changed, vec![a2]);
        assert_eq!(deleted.len(), 1);
        assert_eq!(deleted[0]["name"], "b");
        assert!(deleted[0]["value"].is_null());
        assert_eq!(
            diff(std::slice::from_ref(&a), std::slice::from_ref(&a)),
            (vec![], vec![])
        );
    }

    /** Domain must domain-match the host, and __Host- cookies must be host-only at "/". */
    #[test]
    fn set_refuses_bad_domains_and_host_prefix() {
        let url = Url::parse("https://www.example.com/a/").unwrap();
        let ctx = Ctx {
            identity: "c".into(),
            kind: super::super::Kind::Window,
            origin: "https://www.example.com".into(),
            top_level: url.clone(),
            url,
            active: false,
            selected: true,
            tab: "tab-1".into(),
        };
        assert!(refusal(&ctx, "a", Some("example.com"), "/").is_none());
        assert!(refusal(&ctx, "a", Some("other.com"), "/").is_some());
        assert!(refusal(&ctx, "a", Some(".example.com"), "/").is_some());
        assert!(refusal(&ctx, "__Host-a", None, "/x").is_some());
        assert!(refusal(&ctx, "__Host-a", None, "/").is_none());
    }

    /** The written field always carries Secure and the chosen SameSite; expiry becomes Max-Age. */
    #[test]
    fn field_carries_spec_attributes() {
        let args = json!({"sameSite":"lax","domain":"example.com","expires":0.0});
        let text = field(&args, "n", "v", "/");
        assert!(text.starts_with("n=v; Path=/; Secure; SameSite=lax; Domain=example.com"));
        assert!(text.ends_with("Max-Age=0"));
    }
}
