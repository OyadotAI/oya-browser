/**
 * A persona as one immutable unit: the device (fingerprint), its cookie jar and saved storage, and
 * its proxy, exactly as the Oya server sent them in `auth_ok`. Nothing here changes a loaded
 * persona; switching means loading another whole unit, and a persona id that comes back with a
 * different device is refused, never re-rolled (the device is what keeps the cookies coherent).
 * `Identity` is what the integration hands V9 (navigator, screen, WebGL, noise) and Relay
 * (User-Agent, client hints, Accept-Language, proxy).
 */
use crate::constants::{HTTP_PORT, HTTPS_PORT};
use crate::identity::{IdentityError, NativePolicy, accept_language, client_hints, native_policy};
use crate::profile::{DeviceScreen, Profile};
use serde::{Deserialize, Serialize};
use serde_json::{Number, Value};
use std::sync::Arc;

/// The client hints every secure request carries, in Chrome's order; the rest only on Accept-CH.
pub const ALWAYS_HINTS: [&str; 3] = ["sec-ch-ua", "sec-ch-ua-mobile", "sec-ch-ua-platform"];
/// Headers Chrome leads a navigation with, in order.
pub const LEADING_NAVIGATION: [&str; 4] = [
    "sec-ch-ua",
    "sec-ch-ua-mobile",
    "sec-ch-ua-platform",
    "accept-language",
];
/// Headers Chrome leads a fetch or subresource with, in order.
pub const LEADING_SUBRESOURCE: [&str; 5] = [
    "sec-ch-ua-platform",
    "accept-language",
    "sec-ch-ua",
    "user-agent",
    "sec-ch-ua-mobile",
];

/// A proxy in one shape, whichever the server sent (`{url, username, password}` or `{host, port, type}`).
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Proxy {
    /// `http`, `https`, `socks5` and the like.
    pub scheme: String,
    /// The proxy host.
    pub host: String,
    /// The proxy port.
    pub port: u16,
    /// Proxy-auth user, if any.
    pub username: Option<String>,
    /// Proxy-auth password, if any; never logged.
    #[serde(skip_serializing)]
    pub password: Option<String>,
    /// The operator's metered gateway: bytes must be reported as `proxy_bytes`.
    pub metered: bool,
}

/// The persona as `auth_ok` names it.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct PersonaName {
    /// The persona id.
    #[serde(default)]
    pub id: String,
    /// Its display name.
    #[serde(default)]
    pub name: Option<String>,
}

/// One persona, loaded whole. Fields are private: a loaded persona cannot be edited in place.
#[derive(Debug, PartialEq)]
pub struct Persona {
    /// Its id and name.
    name: PersonaName,
    /// Its device.
    profile: Profile,
    /// Its cookie jar, in the server's cookie shape.
    cookies: Vec<Value>,
    /// Its saved localStorage by origin.
    origins: Value,
    /// Its proxy; None is direct egress.
    proxy: Option<Proxy>,
}

/// What V9 and Relay must apply for a persona, derived once from the immutable unit.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Identity {
    /// User-Agent, navigator.platform/languages/hardwareConcurrency, timezone, locale, userAgentData.
    pub policy: NativePolicy,
    /// The Accept-Language header.
    pub accept_language: Option<String>,
    /// Every client hint's value, in Chrome's order; send `ALWAYS_HINTS` always, the rest on Accept-CH.
    pub client_hints: Vec<(String, String)>,
    /// navigator.deviceMemory.
    pub device_memory: Option<Number>,
    /// navigator.maxTouchPoints.
    pub max_touch_points: u32,
    /// navigator.vendor.
    pub vendor: String,
    /// screen.* and devicePixelRatio; None leaves the real screen.
    pub screen: Option<DeviceScreen>,
    /// UNMASKED_VENDOR_WEBGL answer.
    pub webgl_vendor: Option<String>,
    /// UNMASKED_RENDERER_WEBGL answer.
    pub webgl_renderer: Option<String>,
    /// Canvas, audio and client-rects noise seeds.
    pub noise: [Option<f64>; 3],
    /// Fonts reported as installed; None reports none specially.
    pub fonts: Option<Vec<String>>,
    /// Where requests go out; None is direct.
    pub proxy: Option<Proxy>,
}

/// Why `auth_ok` could not be loaded as a persona.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonaError(pub String);

impl std::fmt::Display for PersonaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for PersonaError {}

impl Persona {
    /// The persona in an `auth_ok` message: its fingerprint, cookies, storage and proxy, as one unit.
    pub fn from_welcome(message: &Value) -> Result<Arc<Self>, PersonaError> {
        let fingerprint = message
            .get("fingerprint")
            .ok_or_else(|| PersonaError("auth_ok carries no fingerprint".into()))?;
        let profile: Profile = serde_json::from_value(fingerprint.clone())
            .map_err(|e| PersonaError(format!("Invalid persona fingerprint: {e}")))?;
        if profile.id.is_empty() {
            return Err(PersonaError("Persona fingerprint has no id".into()));
        }
        let mut name: PersonaName = message
            .get("persona")
            .cloned()
            .map(serde_json::from_value)
            .transpose()
            .map_err(|_| PersonaError("Invalid persona name".into()))?
            .unwrap_or_default();
        if name.id.is_empty() {
            name.id = profile.id.clone();
        }
        let proxy = profile.proxy.as_ref().map(proxy_of).transpose()?.flatten();
        let cookies = message
            .get("cookies")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let origins = message.get("origins").cloned().unwrap_or(Value::Null);
        Ok(Arc::new(Self {
            name,
            profile,
            cookies,
            origins,
            proxy,
        }))
    }

    /// The persona id.
    pub fn id(&self) -> &str {
        &self.name.id
    }
    /// The display name, `Default` when the server gives none.
    pub fn name(&self) -> &str {
        self.name.name.as_deref().unwrap_or("Default")
    }
    /// The device.
    pub fn profile(&self) -> &Profile {
        &self.profile
    }
    /// The cookie jar the persona arrived with.
    pub fn cookies(&self) -> &[Value] {
        &self.cookies
    }
    /// The saved localStorage by origin.
    pub fn origins(&self) -> &Value {
        &self.origins
    }
    /// The proxy; None is direct egress.
    pub fn proxy(&self) -> Option<&Proxy> {
        self.proxy.as_ref()
    }

    /// Whether `other` is the same device: everything but the per-connection proxy is equal.
    pub fn same_device(&self, other: &Persona) -> bool {
        let strip = |p: &Profile| Profile {
            proxy: None,
            ..p.clone()
        };
        strip(&self.profile) == strip(&other.profile)
    }

    /// What V9 and Relay apply, claiming the Chrome version in `engine_user_agent`.
    pub fn identity(&self, engine_user_agent: &str) -> Result<Identity, IdentityError> {
        let policy = native_policy(&self.profile, engine_user_agent)?;
        let (webgl_vendor, webgl_renderer) = unmasked_webgl(&self.profile);
        let navigator = &self.profile.navigator;
        Ok(Identity {
            accept_language: accept_language(&policy.languages),
            client_hints: client_hints(&policy.user_agent_metadata)
                .into_iter()
                .map(|(k, v)| (k.into(), v))
                .collect(),
            policy,
            device_memory: navigator.device_memory.clone(),
            max_touch_points: navigator.max_touch_points.unwrap_or_default(),
            vendor: navigator
                .vendor
                .clone()
                .unwrap_or_else(|| "Google Inc.".into()),
            screen: self.profile.screen.clone(),
            webgl_vendor,
            webgl_renderer,
            noise: [
                &self.profile.canvas,
                &self.profile.audio,
                &self.profile.rects,
            ]
            .map(|n| n.map(|n| n.noise_seed)),
            fonts: self.profile.fonts.as_ref().map(|f| f.available.clone()),
            proxy: self.proxy.clone(),
        })
    }
}

impl Identity {
    /// A request's headers as this persona sends them (browser/src/main/identity/client-hints.ts):
    /// the persona's User-Agent and Accept-Language in place of the engine's, the engine's own
    /// `sec-ch-ua*` removed, and on HTTPS the always-sent hints plus those `asked` (the detailed
    /// hints this request's first-party origin requested with Accept-CH), all in Chrome's order.
    pub fn request_headers(
        &self,
        base: &[(String, String)],
        https: bool,
        navigation: bool,
        asked: &[&str],
    ) -> Vec<(String, String)> {
        let own = [
            ("user-agent", Some(self.policy.user_agent.clone())),
            ("accept-language", self.accept_language.clone()),
        ];
        let mut entries: Vec<(String, String)> = base
            .iter()
            .filter(|(name, _)| !name.to_ascii_lowercase().starts_with("sec-ch-ua"))
            .cloned()
            .collect();
        for (name, value) in own.into_iter().filter_map(|(n, v)| Some((n, v?))) {
            match entries
                .iter_mut()
                .find(|(k, _)| k.eq_ignore_ascii_case(name))
            {
                Some(entry) => entry.1 = value,
                None => entries.push((name.into(), value)),
            }
        }
        if !https {
            return entries;
        }
        let hints = self.client_hints.iter().filter(|(name, _)| {
            ALWAYS_HINTS.contains(&name.as_str()) || asked.contains(&name.as_str())
        });
        let mut all: Vec<(String, String)> = hints.cloned().chain(entries).collect();
        let leading: &[&str] = if navigation {
            &LEADING_NAVIGATION
        } else {
            &LEADING_SUBRESOURCE
        };
        // A stable sort: headers Chrome does not lead with keep their order after the leading ones.
        all.sort_by_key(|(name, _)| {
            leading
                .iter()
                .position(|l| name.eq_ignore_ascii_case(l))
                .unwrap_or(leading.len())
        });
        all
    }
}

/// The strings UNMASKED_VENDOR/RENDERER_WEBGL answer with (anonymity/fingerprint.js): Chrome's ANGLE
/// pair for a `webglChrome` device, else the bare driver pair; a stored "WebKit WebGL" never shows.
pub fn unmasked_webgl(profile: &Profile) -> (Option<String>, Option<String>) {
    let gl = &profile.webgl;
    let masked = |r: &str| r == "WebKit" || r == "WebKit WebGL";
    let chrome = profile.webgl_chrome
        && gl
            .renderer
            .as_deref()
            .is_some_and(|r| !r.is_empty() && !masked(r));
    if chrome {
        (gl.vendor.clone(), gl.renderer.clone())
    } else {
        (gl.unmasked_vendor.clone(), gl.unmasked_renderer.clone())
    }
}

/// One proxy shape from the server's `{url, ...}` or `{host, port, type}`; `{direct: true}` is none.
fn proxy_of(value: &Value) -> Result<Option<Proxy>, PersonaError> {
    let field = |name: &str| value.get(name).and_then(Value::as_str).map(str::to_owned);
    if value.is_null() || value.get("direct").and_then(Value::as_bool) == Some(true) {
        return Ok(None);
    }
    let metered = value.get("metered").and_then(Value::as_bool) == Some(true);
    let (scheme, host, port) = match (field("host"), field("url")) {
        (Some(host), _) => {
            let port = value
                .get("port")
                .and_then(Value::as_u64)
                .and_then(|p| u16::try_from(p).ok());
            (
                field("type").unwrap_or_else(|| "http".into()),
                host,
                port.unwrap_or(HTTP_PORT),
            )
        }
        (None, Some(url)) => {
            let url = url::Url::parse(&url)
                .map_err(|_| PersonaError("Invalid persona proxy URL".into()))?;
            let scheme = url.scheme().to_owned();
            let default = if scheme == "https" {
                HTTPS_PORT
            } else {
                HTTP_PORT
            };
            let host = url
                .host_str()
                .ok_or_else(|| PersonaError("Persona proxy has no host".into()))?
                .to_owned();
            (scheme, host, url.port().unwrap_or(default))
        }
        (None, None) => return Err(PersonaError("Persona proxy has no address".into())),
    };
    Ok(Some(Proxy {
        scheme,
        host,
        port,
        username: field("username"),
        password: field("password"),
        metered,
    }))
}
