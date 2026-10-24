/**
 * Pairing Mars with an Oya account from an `oya://connect?code=...&server=...` link, as the
 * Electron browser pairs (browser/src/main/connection/pairing.ts). A deep link is reachable by
 * any page, and connecting hands the server this browser's cookies, so: the link carries a
 * single-use code, never a key; the code is redeemed over TLS with the server the link names
 * (plaintext only on this machine); and the person confirms, with the destination host spelled
 * out, before anything is sent. The key that comes back is stored through `CredentialStore`,
 * which the app backs with the Keychain.
 */
use crate::ws::{post_json, success};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Mutex;
use url::Url;

/// Hosts a plaintext `ws://` server may be on: this machine only.
const LOCAL_HOSTS: [&str; 4] = ["localhost", "127.0.0.1", "::1", "[::1]"];

/// What pairing stores: the account key, the persona to run as, the server and this browser's id.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Credentials {
    /// The project key; a secret.
    pub api_key: String,
    /// The persona to run as.
    pub persona: String,
    /// The server's WebSocket URL.
    pub server_url: String,
    /// The id the server knows this browser by, kept across launches once assigned.
    #[serde(default)]
    pub browser_id: Option<String>,
}

/// Where credentials live. The app implements it over the macOS Keychain; tests use `MemoryCredentials`.
pub trait CredentialStore {
    /// The saved credentials, if any.
    fn load(&self) -> Result<Option<Credentials>, String>;
    /// Replaces the saved credentials.
    fn save(&self, credentials: &Credentials) -> Result<(), String>;
    /// Signs out: forgets the saved credentials.
    fn clear(&self) -> Result<(), String>;
}

/// Credentials held in memory only, for tests and a session that must not persist a key.
#[derive(Default)]
pub struct MemoryCredentials(Mutex<Option<Credentials>>);

impl CredentialStore for MemoryCredentials {
    fn load(&self) -> Result<Option<Credentials>, String> {
        Ok(self
            .0
            .lock()
            .map_err(|_| "credential store poisoned")?
            .clone())
    }
    fn save(&self, credentials: &Credentials) -> Result<(), String> {
        *self.0.lock().map_err(|_| "credential store poisoned")? = Some(credentials.clone());
        Ok(())
    }
    fn clear(&self) -> Result<(), String> {
        *self.0.lock().map_err(|_| "credential store poisoned")? = None;
        Ok(())
    }
}

/// A usable pairing link: its code and the server it names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PairingLink {
    /// The single-use pairing code.
    pub code: String,
    /// The server's WebSocket URL.
    pub server: Url,
}

/// The code and server of an `oya:` link, or None when it is not a usable pairing link.
pub fn parse_link(raw: &str) -> Option<PairingLink> {
    let url = Url::parse(raw).ok().filter(|u| u.scheme() == "oya")?;
    let param = |name: &str| {
        url.query_pairs()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.into_owned())
            .filter(|v| !v.is_empty())
    };
    let code = param("code")?;
    let server = pairing_server(&param("server")?)?;
    Some(PairingLink { code, server })
}

/// The server URL if it is `wss://`, or `ws://` on this machine.
pub fn pairing_server(server: &str) -> Option<Url> {
    let url = Url::parse(server).ok()?;
    let local = url.host_str().is_some_and(|h| LOCAL_HOSTS.contains(&h));
    match url.scheme() {
        "wss" => Some(url),
        "ws" if local => Some(url),
        _ => None,
    }
}

/// Where a code is redeemed: the server's own host over HTTP(S).
pub fn claim_url(server: &Url) -> Option<Url> {
    let scheme = if server.scheme() == "wss" {
        "https"
    } else {
        "http"
    };
    let port = server.port().map(|p| format!(":{p}")).unwrap_or_default();
    Url::parse(&format!(
        "{scheme}://{}{port}/api/pairing/claim",
        server.host_str()?
    ))
    .ok()
}

/// Redeems the code for a key and persona; the error is the server's reason.
pub fn claim(link: &PairingLink) -> Result<Credentials, String> {
    let url = claim_url(&link.server).ok_or("Pairing server has no host")?;
    let (status, body) = post_json(&url, &json!({"code": link.code}).to_string())
        .map_err(|e| format!("Pairing failed: {e}"))?;
    let answer: Value = serde_json::from_str(&body).unwrap_or_default();
    let text = |k: &str| {
        answer
            .get(k)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    };
    match text("apiKey") {
        Some(api_key) if success(status) => Ok(Credentials {
            api_key,
            persona: text("persona").unwrap_or_else(|| "default".into()),
            server_url: link.server.to_string(),
            browser_id: None,
        }),
        _ => Err(text("error").unwrap_or_else(|| format!("Pairing failed ({status})"))),
    }
}

/// Pairs from a deep link: asks the person (`confirm` gets the host; false is Cancel), redeems the
/// code, and saves the result. Ok(None) when the link is unusable or the person cancelled.
pub fn pair(
    raw: &str,
    confirm: impl FnOnce(&str) -> bool,
    store: &dyn CredentialStore,
) -> Result<Option<Credentials>, String> {
    let Some(link) = parse_link(raw) else {
        return Ok(None);
    };
    let host = link.server.host_str().unwrap_or_default().to_owned();
    if !confirm(&host) {
        return Ok(None);
    }
    let credentials = claim(&link)?;
    store.save(&credentials)?;
    Ok(Some(credentials))
}
