/**
 * Service worker registrations as the Service Workers spec models them: per storage key, keyed
 * by scope URL, each with up to three versions (installing, waiting, active). Matching picks the
 * registration whose scope is the longest prefix of a URL; scope validation enforces the
 * script's maximum scope (its directory, or what Service-Worker-Allowed allows).
 */
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/** One worker version: its script, its lifecycle state and what it told us about itself. */
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(in crate::runtime) struct Version {
    pub id: u64,
    pub script_url: String,
    pub source: String,
    /** "parsed", "installing", "installed", "activating", "activated" or "redundant". */
    pub state: String,
    /** Whether the script registered a fetch handler, learned when it installed. */
    pub has_fetch: bool,
    /** skipWaiting() was called. */
    pub skip_waiting: bool,
}

/** One registration. */
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub(in crate::runtime) struct Registration {
    pub id: u64,
    /** The storage key (origin and top-level site) it belongs to. */
    pub key: String,
    /** The registering document's top-level URL, which partitions the worker's own loads. */
    pub top_level: String,
    pub scope: String,
    pub update_via_cache: String,
    /** "classic" or "module". */
    pub script_type: String,
    #[serde(skip)]
    pub installing: Option<Version>,
    pub waiting: Option<Version>,
    pub active: Option<Version>,
    pub preload_enabled: bool,
    pub preload_header: String,
}

impl Registration {
    /** The newest version: installing, else waiting, else active. */
    pub fn newest(&self) -> Option<&Version> {
        self.installing
            .as_ref()
            .or(self.waiting.as_ref())
            .or(self.active.as_ref())
    }
    /** Every version it holds. */
    pub fn versions(&self) -> impl Iterator<Item = &Version> {
        self.installing
            .iter()
            .chain(&self.waiting)
            .chain(&self.active)
    }
    /** The version with this id, mutably. */
    pub fn version_mut(&mut self, id: u64) -> Option<&mut Version> {
        [&mut self.installing, &mut self.waiting, &mut self.active]
            .into_iter()
            .find_map(|slot| slot.as_mut().filter(|version| version.id == id))
    }
    /** The record pages see. */
    pub fn record(&self) -> Value {
        json!({"id":self.id,"scope":self.scope,"updateViaCache":self.update_via_cache,
            "installing":self.installing.as_ref().map(Version::record),
            "waiting":self.waiting.as_ref().map(Version::record),
            "active":self.active.as_ref().map(Version::record),
            "preload":{"enabled":self.preload_enabled,"headerValue":self.preload_header}})
    }
}

impl Version {
    /** The ServiceWorker record pages see. */
    pub fn record(&self) -> Value {
        json!({"id":self.id,"scriptURL":self.script_url,"state":self.state})
    }
}

/** Every registration and the next id for registrations and versions. */
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub(in crate::runtime) struct Registry {
    pub registrations: Vec<Registration>,
    pub next_id: u64,
}

impl Registry {
    /** A fresh id. */
    pub fn id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }
    /** The registration for exactly this scope in this key. */
    pub fn by_scope(&mut self, key: &str, scope: &str) -> Option<&mut Registration> {
        self.registrations
            .iter_mut()
            .find(|r| r.key == key && r.scope == scope)
    }
    /** "Match Service Worker Registration": the longest scope that prefixes `url`. */
    pub fn matching(&self, key: &str, url: &str) -> Option<&Registration> {
        self.registrations
            .iter()
            .filter(|r| r.key == key && url.starts_with(&r.scope))
            .max_by_key(|r| r.scope.len())
    }
    /** The registration holding version `id`. */
    pub fn holding(&mut self, id: u64) -> Option<&mut Registration> {
        self.registrations
            .iter_mut()
            .find(|r| r.versions().any(|v| v.id == id))
    }
    /** The registration with this id. */
    pub fn by_id(&mut self, id: u64) -> Option<&mut Registration> {
        self.registrations.iter_mut().find(|r| r.id == id)
    }
}

/** Why a scope is refused for a script, as Chrome's SecurityError message, or None. */
pub(in crate::runtime) fn scope_refusal(
    script: &url::Url,
    scope: &url::Url,
    allowed: Option<&str>,
) -> Option<String> {
    let max = match allowed {
        Some(header) => script.join(header.trim()).ok()?,
        None => script.join("./").ok()?,
    };
    if scope.path().starts_with(max.path()) {
        return None;
    }
    Some(format!(
        "The path of the provided scope ('{}') is not under the max scope allowed ('{}'). Adjust the scope, move the Service Worker script, or use the Service-Worker-Allowed HTTP header to allow the scope.",
        scope.path(),
        max.path()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /** A registration with this scope in key "k". */
    fn registration(id: u64, scope: &str) -> Registration {
        Registration {
            id,
            key: "k".into(),
            scope: scope.into(),
            ..Registration::default()
        }
    }

    /** The longest matching scope wins; other keys never match. */
    #[test]
    fn longest_scope_matches() {
        let registry = Registry {
            registrations: vec![
                registration(1, "https://a.test/"),
                registration(2, "https://a.test/app/"),
            ],
            next_id: 2,
        };
        assert_eq!(
            registry
                .matching("k", "https://a.test/app/page")
                .unwrap()
                .id,
            2
        );
        assert_eq!(
            registry.matching("k", "https://a.test/other").unwrap().id,
            1
        );
        assert!(registry.matching("other", "https://a.test/app/").is_none());
    }

    /** A scope must sit under the script's directory unless Service-Worker-Allowed widens it. */
    #[test]
    fn max_scope_is_the_script_directory_or_the_header() {
        let script = url::Url::parse("https://a.test/js/sw.js").unwrap();
        let root = url::Url::parse("https://a.test/").unwrap();
        let js = url::Url::parse("https://a.test/js/app/").unwrap();
        assert!(scope_refusal(&script, &js, None).is_none());
        assert!(
            scope_refusal(&script, &root, None)
                .unwrap()
                .contains("max scope allowed ('/js/')")
        );
        assert!(scope_refusal(&script, &root, Some("/")).is_none());
    }

    /** Installing versions are not persisted; waiting and active ones are. */
    #[test]
    fn installing_versions_are_not_persisted() {
        let mut stored = registration(1, "https://a.test/");
        stored.installing = Some(Version {
            id: 2,
            ..Version::default()
        });
        stored.active = Some(Version {
            id: 3,
            ..Version::default()
        });
        let back: Registration =
            serde_json::from_value(serde_json::to_value(&stored).unwrap()).unwrap();
        assert!(back.installing.is_none());
        assert_eq!(back.newest().map(|v| v.id), Some(3));
    }
}
