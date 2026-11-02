/**
 * Credential Management's password and federated credentials, kept per origin in the profile's
 * sealed Credentials record. Mars has no account chooser, so `get` returns a credential only when
 * the origin stored exactly one of the requested kind and silent access was not prevented (the
 * cases a browser answers without showing UI); "required" mediation, which needs a chooser,
 * resolves null. Cross-origin frames are refused, as Chrome refuses them.
 */
use super::{Ctx, Handler, Kind, Reply, fail, ok};
use crate::profile::Record;
use crate::protocol::EngineResult;
use crate::runtime::supervisor::Runtime;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/** Credential operations. */
pub(super) const HANDLERS: &[(&str, Handler)] = &[
    ("credentials.get", get),
    ("credentials.store", store),
    ("credentials.preventSilentAccess", prevent_silent_access),
];

/** Credentials one origin may store. */
const PER_ORIGIN: usize = 64;
/** The longest field a stored credential keeps. */
const FIELD_BYTES: usize = 4096;

/** One stored credential, as the page's PasswordCredential or FederatedCredential carries it. */
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Credential {
    #[serde(rename = "type")]
    kind: String,
    id: String,
    password: String,
    name: String,
    #[serde(rename = "iconURL")]
    icon_url: String,
    provider: String,
    protocol: Option<String>,
}

/** One origin's credentials and whether it asked to prevent silent access. */
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Origin {
    credentials: Vec<Credential>,
    prevent_silent: bool,
}

/** Every origin's stored credentials. */
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(in crate::runtime) struct Store {
    origins: BTreeMap<String, Origin>,
}

impl Store {
    /** The single credential a no-UI get may return, or None. */
    fn pick(
        &self,
        origin: &str,
        password: bool,
        providers: &[String],
        mediation: &str,
    ) -> Option<&Credential> {
        let entry = self.origins.get(origin)?;
        if mediation == "required" || mediation == "conditional" || entry.prevent_silent {
            return None;
        }
        let mut matching = entry.credentials.iter().filter(|credential| {
            (password && credential.kind == "password")
                || (credential.kind == "federated" && providers.contains(&credential.provider))
        });
        let first = matching.next()?;
        matching.next().is_none().then_some(first)
    }
    /** Store or replace (by type and id); false past the per-origin bound. */
    fn put(&mut self, origin: &str, credential: Credential) -> bool {
        let entry = self.origins.entry(origin.into()).or_default();
        entry
            .credentials
            .retain(|old| !(old.kind == credential.kind && old.id == credential.id));
        if entry.credentials.len() >= PER_ORIGIN {
            return false;
        }
        entry.credentials.push(credential);
        entry.prevent_silent = false;
        true
    }
}

/** Refuse cross-origin frames, which cannot use the credential store. */
fn refused(ctx: &Ctx) -> bool {
    ctx.kind == Kind::Frame && ctx.top_level.origin().ascii_serialization() != ctx.origin
}

/** navigator.credentials.get for password and federated credentials. */
fn get(runtime: &mut Runtime, ctx: &Ctx, args: &Value, _: &[u8]) -> EngineResult<Reply> {
    if refused(ctx) {
        return fail(
            "NotAllowedError",
            "The credential operation is not allowed in a cross-origin iframe.",
        );
    }
    let providers: Vec<String> = args["providers"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    let password = args["password"].as_bool() == Some(true);
    let mediation = args["mediation"].as_str().unwrap_or("optional");
    let found = runtime
        .platform
        .credentials
        .pick(&ctx.origin, password, &providers, mediation);
    ok(found.map_or(Value::Null, |credential| json!(credential)))
}

/** navigator.credentials.store: persist one credential for the origin. */
fn store(runtime: &mut Runtime, ctx: &Ctx, args: &Value, _: &[u8]) -> EngineResult<Reply> {
    if refused(ctx) {
        return fail(
            "NotAllowedError",
            "The credential operation is not allowed in a cross-origin iframe.",
        );
    }
    let Ok(credential) = serde_json::from_value::<Credential>(args.clone()) else {
        return fail(
            "TypeError",
            "The credential is not a PasswordCredential or FederatedCredential.",
        );
    };
    let fields = [
        &credential.id,
        &credential.password,
        &credential.name,
        &credential.icon_url,
        &credential.provider,
    ];
    if !matches!(credential.kind.as_str(), "password" | "federated")
        || fields.iter().any(|field| field.len() > FIELD_BYTES)
    {
        return fail(
            "TypeError",
            "The credential is not a PasswordCredential or FederatedCredential.",
        );
    }
    let mut next = runtime.platform.credentials.clone();
    if !next.put(&ctx.origin, credential)
        || !runtime.platform.records.save(Record::Credentials, &next)
    {
        return fail("QuotaExceededError", "The credential could not be stored.");
    }
    runtime.platform.credentials = next;
    ok(Value::Null)
}

/** navigator.credentials.preventSilentAccess: no credential returns without UI until a new store. */
fn prevent_silent_access(
    runtime: &mut Runtime,
    ctx: &Ctx,
    _: &Value,
    _: &[u8],
) -> EngineResult<Reply> {
    let mut next = runtime.platform.credentials.clone();
    next.origins
        .entry(ctx.origin.clone())
        .or_default()
        .prevent_silent = true;
    runtime.platform.records.save(Record::Credentials, &next);
    runtime.platform.credentials = next;
    ok(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;

    /** A password credential for `id`. */
    fn password(id: &str) -> Credential {
        Credential {
            kind: "password".into(),
            id: id.into(),
            password: "pw".into(),
            ..Credential::default()
        }
    }

    /** Exactly one matching credential returns silently; two need a chooser and return none. */
    #[test]
    fn only_a_single_match_returns_without_ui() {
        let mut store = Store::default();
        store.put("https://a.test", password("one"));
        assert_eq!(
            store
                .pick("https://a.test", true, &[], "optional")
                .unwrap()
                .id,
            "one"
        );
        assert!(
            store
                .pick("https://a.test", true, &[], "required")
                .is_none()
        );
        store.put("https://a.test", password("two"));
        assert!(store.pick("https://a.test", true, &[], "silent").is_none());
        assert!(store.pick("https://b.test", true, &[], "silent").is_none());
    }

    /** preventSilentAccess hides credentials until the next store; storing replaces by id. */
    #[test]
    fn prevent_silent_access_until_next_store() {
        let mut store = Store::default();
        store.put("https://a.test", password("one"));
        store
            .origins
            .get_mut("https://a.test")
            .unwrap()
            .prevent_silent = true;
        assert!(
            store
                .pick("https://a.test", true, &[], "optional")
                .is_none()
        );
        store.put("https://a.test", password("one"));
        assert_eq!(store.origins["https://a.test"].credentials.len(), 1);
        assert!(
            store
                .pick("https://a.test", true, &[], "optional")
                .is_some()
        );
    }

    /** Federated credentials match only the requested providers. */
    #[test]
    fn federated_credentials_match_providers() {
        let mut store = Store::default();
        let federated = Credential {
            kind: "federated".into(),
            id: "me".into(),
            provider: "https://idp.test".into(),
            ..Credential::default()
        };
        store.put("https://a.test", federated);
        assert!(
            store
                .pick(
                    "https://a.test",
                    false,
                    &["https://idp.test".into()],
                    "optional"
                )
                .is_some()
        );
        assert!(
            store
                .pick(
                    "https://a.test",
                    false,
                    &["https://other.test".into()],
                    "optional"
                )
                .is_none()
        );
    }
}
