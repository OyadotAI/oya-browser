/** window members: navigator, screen, location, history, storage, cookies, performance and friends. */
use super::dom::{native_name, wrap};
use super::{HistoryRecord, NavigationRequest};
use crate::constants::{VM_DOCUMENT_COOKIE_BYTES, VM_HOST_RECORDS};
use crate::protocol::EngineError;
use crate::v9::value::JsValue;
use crate::v9::vm::data::{Class, HostData};
use crate::v9::vm::heap::Object;
use crate::v9::vm::realm::realm;
use crate::v9::vm::{Call, Res, Vm};

/** The setter of a [Replaceable] window attribute: the assigned value becomes an own data
property of the global object, shadowing the attribute from then on. */
pub(in crate::v9) fn replace(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let name = native_name(&call);
    let key = crate::v9::vm::heap::Key::from(name);
    vm.define(
        vm.global_object(),
        key,
        crate::v9::vm::property::Descriptor {
            value: Some(call.arg(0)),
            writable: Some(true),
            enumerable: Some(true),
            configurable: Some(true),
            ..Default::default()
        },
    )?;
    Ok(JsValue::Undefined)
}

/** A browsing context's mutable name is a string, shared by global var and property access. */
pub(in crate::v9) fn set_name(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let value = JsValue::String(vm.to_string(&call.arg(0))?);
    vm.define(
        vm.global_object(),
        crate::v9::vm::heap::Key::from("name"),
        crate::v9::vm::property::Descriptor::data(value),
    )?;
    Ok(JsValue::Undefined)
}

/** Window getters by registered name. */
pub(in crate::v9) fn window_get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let environment = &vm.page.environment;
    Ok(match native_name(&call) {
        "window" | "self" | "globalThis" | "frames" | "top" | "parent" | "defaultView" => {
            JsValue::Object(vm.global_object())
        }
        "document" => JsValue::Object(realm().ids.document),
        "location" => JsValue::Object(realm().ids.location),
        "innerWidth" | "outerWidth" => JsValue::Number(environment.viewport_width),
        "innerHeight" | "outerHeight" => JsValue::Number(environment.viewport_height),
        "devicePixelRatio" => JsValue::Number(environment.device_pixel_ratio),
        "scrollY" | "pageYOffset" => JsValue::Number(environment.scroll_y),
        "scrollX" | "pageXOffset" => JsValue::Number(environment.scroll_x),
        "screenX" | "screenY" | "screenLeft" | "screenTop" => JsValue::Number(0.0),
        "length" => JsValue::Number(0.0),
        "closed" => JsValue::Boolean(false),
        "isSecureContext" => JsValue::Boolean(vm.page.state.href.starts_with("https:")),
        "origin" => {
            let origin = super::net::parse_url(&vm.page.state.href)
                .map(|url| url.origin().ascii_serialization())
                .unwrap_or_else(|| "null".into());
            JsValue::string(&origin)?
        }
        "opener" | "frameElement" => JsValue::Null,
        "name" => JsValue::string("")?,
        "localStorage" => storage_object(vm, true)?,
        "sessionStorage" => storage_object(vm, false)?,
        _ => JsValue::Undefined,
    })
}

/** The storage wrapper for an area, kept in page state so identity survives. */
fn storage_object(vm: &mut Vm, local: bool) -> Res<JsValue> {
    let slot = if local {
        super::nodes::VIRTUAL_BASE + 8
    } else {
        super::nodes::VIRTUAL_BASE + 9
    };
    if let Some(id) = vm.page.state.wrappers.get(&slot)
        && vm.heap.get(*id).is_some()
    {
        return Ok(JsValue::Object(*id));
    }
    let id = vm.alloc(Object::new(
        Class::Host(Box::new(HostData::Storage(local))),
        Some(realm().ids.storage_prototype),
    ))?;
    vm.page.state.wrappers.insert(slot, id);
    Ok(JsValue::Object(id))
}

/** Storage methods. */
pub(in crate::v9) fn storage_method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(HostData::Storage(local)) = super::host_data(vm, &call.this) else {
        return Err(vm.type_error("Illegal invocation"));
    };
    match native_name(&call) {
        "getItem" => {
            let key = vm.to_text(&call.arg(0))?;
            let area = if local {
                &vm.page.storage.local
            } else {
                &vm.page.storage.session
            };
            match area.get(&key).cloned() {
                Some(value) => JsValue::string(&value).map_err(Into::into),
                None => Ok(JsValue::Null),
            }
        }
        "setItem" => {
            let key = vm.to_text(&call.arg(0))?;
            let value = vm.to_text(&call.arg(1))?;
            storage_set(vm, local, key, value)?;
            Ok(JsValue::Undefined)
        }
        "removeItem" => {
            let key = vm.to_text(&call.arg(0))?;
            storage_remove(vm, local, &key)?;
            Ok(JsValue::Undefined)
        }
        "clear" => {
            if local {
                vm.page.storage.local.clear();
            } else {
                vm.page.storage.session.clear();
            }
            Ok(JsValue::Undefined)
        }
        "key" => {
            let index = vm.to_integer(&call.arg(0))?;
            let area = if local {
                &vm.page.storage.local
            } else {
                &vm.page.storage.session
            };
            match area.keys().nth(index.max(0.0) as usize).cloned() {
                Some(key) if index >= 0.0 => JsValue::string(&key).map_err(Into::into),
                _ => Ok(JsValue::Null),
            }
        }
        "length" => {
            let area = if local {
                &vm.page.storage.local
            } else {
                &vm.page.storage.session
            };
            Ok(JsValue::Number(area.len() as f64))
        }
        _ => Ok(JsValue::Undefined),
    }
}

/** Store one entry; past the area bounds it throws QuotaExceededError as browsers do. */
pub(in crate::v9) fn storage_set(vm: &mut Vm, local: bool, key: String, value: String) -> Res<()> {
    let mut candidate = vm.page.storage.clone();
    let area = if local {
        &mut candidate.local
    } else {
        &mut candidate.session
    };
    area.insert(key, value);
    if candidate.validate().is_err() {
        return Err(vm.dom_exception(
            "QuotaExceededError",
            "Failed to execute 'setItem' on 'Storage': the quota has been exceeded.",
        ));
    }
    vm.page.storage = candidate;
    Ok(())
}

/** Remove one entry. */
pub(in crate::v9) fn storage_remove(vm: &mut Vm, local: bool, key: &str) -> Res<()> {
    if local {
        vm.page.storage.local.remove(key);
    } else {
        vm.page.storage.session.remove(key);
    }
    Ok(())
}

/** navigator getters. */
pub(in crate::v9) fn navigator_get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let environment = vm.page.environment.clone();
    Ok(match native_name(&call) {
        "userAgent" => JsValue::string(&environment.user_agent)?,
        "appVersion" => JsValue::string(environment.user_agent.trim_start_matches("Mozilla/"))?,
        "platform" => JsValue::string(&environment.platform)?,
        "language" => JsValue::string(vm.page.languages.first().map_or("en-US", String::as_str))?,
        "languages" => {
            let tags: Vec<JsValue> = if vm.page.languages.is_empty() {
                vec![JsValue::string("en-US")?]
            } else {
                vm.page
                    .languages
                    .iter()
                    .map(|tag| JsValue::string(tag))
                    .collect::<Result<_, _>>()?
            };
            let id = vm.new_array(tags)?;
            if let Some(object) = vm.heap.get_mut(id) {
                object.extensible = false;
            }
            JsValue::Object(id)
        }
        "cookieEnabled" | "onLine" => JsValue::Boolean(if native_name(&call) == "onLine" {
            environment.online
        } else {
            true
        }),
        "vendor" => JsValue::string("Google Inc.")?,
        "appName" => JsValue::string("Netscape")?,
        "appCodeName" => JsValue::string("Mozilla")?,
        "product" => JsValue::string("Gecko")?,
        "productSub" => JsValue::string("20030107")?,
        "hardwareConcurrency" => JsValue::Number(NAVIGATOR_CORES),
        "deviceMemory" => JsValue::Number(NAVIGATOR_MEMORY),
        "maxTouchPoints" => JsValue::Number(0.0),
        "webdriver" | "pdfViewerEnabled" => JsValue::Boolean(false),
        "doNotTrack" => JsValue::Null,
        "plugins" | "mimeTypes" => JsValue::Object(vm.new_array(Vec::new())?),
        _ => JsValue::Undefined,
    })
}

/** What navigator.hardwareConcurrency reports. */
const NAVIGATOR_CORES: f64 = 8.0;
/** What navigator.deviceMemory reports. */
const NAVIGATOR_MEMORY: f64 = 8.0;

/** navigator methods. */
pub(in crate::v9) fn navigator_method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    match native_name(&call) {
        "sendBeacon" => super::net::send_beacon(vm, call),
        "javaEnabled" => Ok(JsValue::Boolean(false)),
        "vibrate" => Ok(JsValue::Boolean(false)),
        name => Err(vm.unsupported_error(&format!("navigator.{name}"))),
    }
}

/** screen getters. */
pub(in crate::v9) fn screen_get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let environment = &vm.page.environment;
    Ok(match native_name(&call) {
        "width" | "availWidth" => JsValue::Number(environment.screen_width),
        "height" => JsValue::Number(environment.screen_height),
        "availHeight" => JsValue::Number(environment.screen_height - SCREEN_MENU_BAR),
        "colorDepth" | "pixelDepth" => JsValue::Number(SCREEN_DEPTH),
        "availLeft" | "availTop" => JsValue::Number(0.0),
        _ => JsValue::Undefined,
    })
}

/** The macOS menu bar height screen.availHeight leaves out. */
const SCREEN_MENU_BAR: f64 = 25.0;
/** Bits per pixel. */
const SCREEN_DEPTH: f64 = 24.0;

/** Record a navigation request; hash-only changes stay in the document and enter history. */
pub(in crate::v9) fn navigate(vm: &mut Vm, kind: &str, url: String, hash: bool) -> Res<()> {
    let same_document = hash || {
        let current = super::net::parse_url(&vm.page.state.href);
        let target = super::net::parse_url(&url);
        match (current, target) {
            (Some(mut current), Some(mut target)) if target.fragment().is_some() => {
                current.set_fragment(None);
                target.set_fragment(None);
                current == target
            }
            _ => false,
        }
    };
    if same_document {
        vm.page.state.href = url.clone();
        push_history(
            vm,
            HistoryRecord {
                replace: kind == "replace",
                url,
                title: String::new(),
            },
        )?;
        super::events::fire(vm, super::Target::Window, "hashchange", false, false)?;
        return Ok(());
    }
    vm.page.effects.navigation = Some(NavigationRequest {
        kind: kind.into(),
        url,
        delta: 0,
    });
    Ok(())
}

/** Append a history record within the bound. */
fn push_history(vm: &mut Vm, record: HistoryRecord) -> Res<()> {
    if vm.page.effects.history.len() >= VM_HOST_RECORDS {
        return Err(EngineError::limit("V9 history records", VM_HOST_RECORDS).into());
    }
    if !record.replace {
        vm.page.state.history_length += 1;
    }
    vm.page.effects.history.push(record);
    Ok(())
}

/** location methods. */
pub(in crate::v9) fn location_method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let name = native_name(&call);
    match name {
        "assign" | "replace" => {
            let target = vm.to_text(&call.arg(0))?;
            let Some(url) = super::net::resolve(vm, &target) else {
                return Err(vm.syntax_error(&format!(
                    "Failed to execute '{name}' on 'Location': '{target}' is not a valid URL."
                )));
            };
            navigate(vm, name, url, false)?;
            Ok(JsValue::Undefined)
        }
        "reload" => {
            vm.page.effects.navigation = Some(NavigationRequest {
                kind: "reload".into(),
                url: vm.page.state.href.clone(),
                delta: 0,
            });
            Ok(JsValue::Undefined)
        }
        _ => JsValue::string(&vm.page.state.href).map_err(Into::into),
    }
}

/** history members. */
pub(in crate::v9) fn history(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let name = native_name(&call);
    match name {
        "length" => Ok(JsValue::Number(f64::from(
            vm.page.state.history_length.max(1),
        ))),
        "state" => Ok(vm.page.state.history_state.clone().unwrap_or(JsValue::Null)),
        "scrollRestoration" => JsValue::string("auto").map_err(Into::into),
        "pushState" | "replaceState" => {
            let state = call.arg(0);
            let title = if call.arg(1).nullish() {
                String::new()
            } else {
                vm.to_text(&call.arg(1))?
            };
            let url = if call.arg(2).nullish() {
                vm.page.state.href.clone()
            } else {
                let target = vm.to_text(&call.arg(2))?;
                super::net::resolve(vm, &target)
                    .ok_or_else(|| vm.syntax_error("Invalid history URL"))?
            };
            let same_origin = match (
                super::net::parse_url(&url),
                super::net::parse_url(&vm.page.state.href),
            ) {
                (Some(target), Some(current)) => target.origin() == current.origin(),
                _ => false,
            };
            if !same_origin {
                return Err(vm.throw(crate::v9::vm::error::ErrorKind::Error, "SecurityError: A history state object with a different origin cannot be created."));
            }
            vm.page.state.history_state = Some(state);
            vm.page.state.href = url.clone();
            push_history(
                vm,
                HistoryRecord {
                    replace: name == "replaceState",
                    url,
                    title,
                },
            )?;
            Ok(JsValue::Undefined)
        }
        "back" | "forward" | "go" => {
            let delta = match name {
                "back" => -1,
                "forward" => 1,
                _ => vm.to_integer(&call.arg(0))? as i32,
            };
            vm.page.effects.navigation = Some(NavigationRequest {
                kind: name.into(),
                url: String::new(),
                delta,
            });
            Ok(JsValue::Undefined)
        }
        _ => Ok(JsValue::Undefined),
    }
}

/** `document.cookie = "name=value; attributes"`: recorded for the jar, mirrored locally. */
pub(in crate::v9) fn write_cookie(vm: &mut Vm, field: &str) -> Res<()> {
    if field.len() > VM_DOCUMENT_COOKIE_BYTES {
        return Ok(());
    }
    if vm.page.effects.cookies.len() >= VM_HOST_RECORDS {
        return Err(EngineError::limit("V9 cookie writes", VM_HOST_RECORDS).into());
    }
    vm.page.effects.cookies.push(field.to_string());
    let mut parts = field.split(';');
    let pair = parts.next().unwrap_or("").trim();
    let (name, value) = pair
        .split_once('=')
        .map_or(("", pair), |(name, value)| (name.trim(), value.trim()));
    let expired = parts.any(|attribute| {
        let attribute = attribute.trim().to_ascii_lowercase();
        attribute.starts_with("max-age=")
            && attribute[8..]
                .trim()
                .parse::<f64>()
                .is_ok_and(|age| age <= 0.0)
            || attribute.starts_with("expires=") && attribute.contains("1970")
    });
    let mut cookies: Vec<(String, String)> = vm
        .page
        .state
        .cookie
        .split(';')
        .filter_map(|entry| {
            let entry = entry.trim();
            if entry.is_empty() {
                return None;
            }
            Some(
                entry
                    .split_once('=')
                    .map_or((String::new(), entry.to_string()), |(a, b)| {
                        (a.to_string(), b.to_string())
                    }),
            )
        })
        .collect();
    cookies.retain(|(existing, _)| existing != name);
    if !expired {
        cookies.push((name.to_string(), value.to_string()));
    }
    let joined: Vec<String> = cookies
        .into_iter()
        .map(|(name, value)| {
            if name.is_empty() {
                value
            } else {
                format!("{name}={value}")
            }
        })
        .collect();
    let text = joined.join("; ");
    if text.len() <= VM_DOCUMENT_COOKIE_BYTES {
        vm.page.state.cookie = text;
    }
    Ok(())
}

/** performance members. */
pub(in crate::v9) fn performance(vm: &mut Vm, call: Call) -> Res<JsValue> {
    match native_name(&call) {
        "now" => Ok(JsValue::Number(vm.page.state.clock)),
        "timeOrigin" => Ok(JsValue::Number(vm.page.state.epoch)),
        "getEntries" | "getEntriesByType" | "getEntriesByName" => {
            Ok(JsValue::Object(vm.new_array(Vec::new())?))
        }
        "timing" => {
            let id = vm.new_object()?;
            let start = vm.page.state.epoch;
            for name in [
                "navigationStart",
                "fetchStart",
                "requestStart",
                "responseStart",
                "responseEnd",
                "domLoading",
                "domInteractive",
                "domContentLoadedEventStart",
                "domContentLoadedEventEnd",
                "domComplete",
                "loadEventStart",
                "loadEventEnd",
                "connectStart",
                "connectEnd",
                "domainLookupStart",
                "domainLookupEnd",
            ] {
                vm.put(id, name, JsValue::Number(start))?;
            }
            for name in [
                "redirectStart",
                "redirectEnd",
                "unloadEventStart",
                "unloadEventEnd",
                "secureConnectionStart",
            ] {
                vm.put(id, name, JsValue::Number(0.0))?;
            }
            Ok(JsValue::Object(id))
        }
        "navigation" => {
            let id = vm.new_object()?;
            vm.put(id, "type", JsValue::Number(0.0))?;
            vm.put(id, "redirectCount", JsValue::Number(0.0))?;
            Ok(JsValue::Object(id))
        }
        _ => Ok(JsValue::Undefined),
    }
}

/** Window functions: atob, btoa, alert, open, matchMedia, getComputedStyle, postMessage, scroll. */
pub(in crate::v9) fn window_method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let name = native_name(&call);
    match name {
        "btoa" => {
            let text = vm.to_string(&call.arg(0))?;
            if text.as_units().iter().any(|unit| *unit > 0xff) {
                return Err(vm.throw(crate::v9::vm::error::ErrorKind::Error, "InvalidCharacterError: The string to be encoded contains characters outside of the Latin1 range."));
            }
            let bytes: Vec<u8> = text.as_units().iter().map(|unit| *unit as u8).collect();
            JsValue::string(&base64_encode(&bytes)).map_err(Into::into)
        }
        "atob" => {
            let text = vm.to_text(&call.arg(0))?;
            match base64_decode(&text) {
                Some(bytes) => {
                    let units: Vec<u16> = bytes.into_iter().map(u16::from).collect();
                    Ok(JsValue::String(crate::v9::value::JsString::from_units(
                        units,
                    )?))
                }
                None => Err(vm.throw(
                    crate::v9::vm::error::ErrorKind::Error,
                    "InvalidCharacterError: The string to be decoded is not correctly encoded.",
                )),
            }
        }
        "alert" | "confirm" | "prompt" | "print" => {
            let message = if call.args.is_empty() {
                String::new()
            } else {
                vm.to_text(&call.arg(0))?
            };
            vm.page
                .console("warn", format!("{name}() is not shown by V9: {message}"));
            Ok(match name {
                "confirm" => JsValue::Boolean(false),
                "prompt" => JsValue::Null,
                _ => JsValue::Undefined,
            })
        }
        "open" => {
            let target = if call.arg(0).nullish() {
                String::new()
            } else {
                vm.to_text(&call.arg(0))?
            };
            if let Some(url) = super::net::resolve(vm, &target) {
                vm.page.effects.navigation = Some(NavigationRequest {
                    kind: "open".into(),
                    url,
                    delta: 0,
                });
            }
            Ok(JsValue::Null)
        }
        "matchMedia" => {
            let query = vm.to_text(&call.arg(0))?;
            let matches = media_matches(vm, &query);
            let id = vm.alloc(Object::new(
                Class::Plain,
                Some(realm().ids.event_target_prototype),
            ))?;
            vm.put(id, "matches", JsValue::Boolean(matches))?;
            vm.put(id, "media", JsValue::string(&query)?)?;
            let noop = JsValue::Object(realm().ids.fields_dummy);
            vm.put(id, "addListener", noop.clone())?;
            vm.put(id, "removeListener", noop)?;
            Ok(JsValue::Object(id))
        }
        "getComputedStyle" => {
            let element = call.arg(0);
            if super::dom::node_of(vm, &element).is_none() {
                return Err(vm.type_error(
                    "Failed to execute 'getComputedStyle': parameter 1 is not of type 'Element'.",
                ));
            }
            let node = super::dom::node_of(vm, &element).unwrap_or_default();
            let id = vm.alloc(Object::new(
                Class::Host(Box::new(HostData::Computed(node))),
                Some(realm().ids.style_prototype),
            ))?;
            Ok(JsValue::Object(id))
        }
        "getSelection" => super::range::selection(vm),
        "postMessage" => {
            let data = call.arg(0);
            let event = super::events::create_event(vm, "message", false, false)?;
            if let JsValue::Object(id) = event {
                vm.put(id, "data", data)?;
                vm.put(
                    id,
                    "origin",
                    JsValue::string(
                        &super::net::parse_url(&vm.page.state.href)
                            .map(|u| u.origin().ascii_serialization())
                            .unwrap_or_default(),
                    )?,
                )?;
                vm.put(id, "source", JsValue::Object(vm.global_object()))?;
                let dispatcher = JsValue::Object(realm().ids.fields_dummy);
                let _ = dispatcher;
                super::events::dispatch(vm, super::Target::Window, id)?;
            }
            Ok(JsValue::Undefined)
        }
        "randomUUID" => {
            let text = random_uuid().map_err(crate::v9::vm::Thrown::Fatal)?;
            JsValue::string(&text).map_err(Into::into)
        }
        _ => Ok(JsValue::Undefined),
    }
}

/**
 * crypto.randomUUID: version-4 UUIDs from the operating system's CSPRNG. Pages use these for
 * CSRF nonces, OAuth state and PKCE verifiers, so they must never come from the page's seeded
 * Math.random generator, which exists only to keep Math.random deterministic across requests.
 */
pub(in crate::v9) fn random_uuid() -> Result<String, EngineError> {
    use ring::rand::SecureRandom;
    let mut bytes = [0u8; UUID_BYTES];
    ring::rand::SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| {
            EngineError::unsupported("The operating system random source is unavailable to V9.")
        })?;
    bytes[UUID_VERSION_BYTE] = (bytes[UUID_VERSION_BYTE] & 0x0f) | 0x40;
    bytes[UUID_VARIANT_BYTE] = (bytes[UUID_VARIANT_BYTE] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    Ok(format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    ))
}

/** Bytes in a UUID. */
const UUID_BYTES: usize = 16;
/** The byte holding the RFC 4122 version nibble. */
const UUID_VERSION_BYTE: usize = 6;
/** The byte holding the RFC 4122 variant bits. */
const UUID_VARIANT_BYTE: usize = 8;

/** Evaluate simple width and color-scheme media queries against the viewport. */
pub(super) fn media_matches(vm: &Vm, query: &str) -> bool {
    let width = vm.page.environment.viewport_width;
    let query = query.to_ascii_lowercase();
    query.split(" and ").all(|part| {
        let part = part
            .trim()
            .trim_start_matches("only ")
            .trim_start_matches("screen")
            .trim();
        if part.is_empty() || part == "all" {
            return true;
        }
        let inner = part.trim_start_matches('(').trim_end_matches(')');
        let Some((feature, value)) = inner.split_once(':') else {
            return inner == "screen";
        };
        let number = value
            .trim()
            .trim_end_matches("px")
            .parse::<f64>()
            .unwrap_or(f64::NAN);
        match feature.trim() {
            "min-width" => width >= number,
            "max-width" => width <= number,
            "prefers-color-scheme" => value.trim() == "light",
            "prefers-reduced-motion" => value.trim() == "no-preference",
            "hover" => value.trim() == "hover",
            "pointer" => value.trim() == "fine",
            "orientation" => value.trim() == "landscape",
            _ => false,
        }
    })
}

/** The base64 alphabet. */
const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/** Standard base64 with padding. */
fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let word = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for index in 0..4 {
            if index <= chunk.len() {
                out.push(ALPHABET[(word >> (18 - 6 * index) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/** Forgiving base64 decoding (whitespace ignored, padding optional). */
fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let clean: Vec<u8> = text.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    let clean: &[u8] = clean
        .strip_suffix(b"==")
        .or_else(|| clean.strip_suffix(b"="))
        .unwrap_or(&clean);
    if clean.len() % 4 == 1 {
        return None;
    }
    let mut out = Vec::with_capacity(clean.len() * 3 / 4);
    let mut buffer = 0u32;
    let mut bits = 0;
    for byte in clean {
        let value = ALPHABET.iter().position(|a| a == byte)? as u32;
        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
        }
    }
    Some(out)
}

/** Legacy pages address fields by id without a projection: unknown ids still get an element. */
pub(in crate::v9) fn legacy_element(vm: &mut Vm, id: &str) -> Res<Option<usize>> {
    let existing = vm
        .page
        .document
        .as_ref()
        .and_then(|document| document.get_by_id(id));
    if let Some(node) = existing {
        return Ok(Some(node));
    }
    let Some(document) = vm.page.document.as_mut() else {
        return Ok(None);
    };
    let local_id = document
        .nodes
        .iter()
        .map(|node| node.local_id + 1)
        .max()
        .unwrap_or(0);
    document.nodes.push(crate::document::ScriptNode {
        local_id,
        html_id: id.into(),
        parent: None,
        value: None,
        text_writable: true,
        click_listener: false,
        inline_handler: None,
        tag: "div".into(),
        attributes: [("id".to_string(), id.to_string())].into_iter().collect(),
    });
    vm.page.invalidate();
    vm.page.next_id = vm.page.next_id.max(local_id + 1);
    let _ = wrap;
    Ok(Some(local_id))
}

/** The hidden slot an observer keeps its callback in. */
fn observer_callback_key() -> crate::v9::vm::heap::Key {
    crate::v9::vm::heap::Key::private("observerCallback")
}

/** Remember an observer's callback for later deliveries. */
pub(in crate::v9) fn keep_observer_callback(
    vm: &mut Vm,
    observer: u32,
    callback: JsValue,
) -> Res<()> {
    vm.heap.put(
        observer,
        observer_callback_key(),
        crate::v9::vm::heap::Prop {
            slot: crate::v9::vm::heap::Slot::Data(callback),
            attrs: crate::v9::vm::heap::HIDDEN_ATTRS,
        },
    );
    Ok(())
}

/** A zero rectangle at the origin with a given size, as DOMRectReadOnly reads. */
fn rect_object(vm: &mut Vm, width: f64, height: f64) -> Res<JsValue> {
    rect_at(vm, (0.0, 0.0, width, height))
}

/** A DOMRect-shaped object for the box (x, y, width, height). */
fn rect_at(vm: &mut Vm, (x, y, width, height): (f64, f64, f64, f64)) -> Res<JsValue> {
    let id = vm.new_object()?;
    for (name, value) in [
        ("x", x),
        ("y", y),
        ("top", y),
        ("left", x),
        ("width", width),
        ("height", height),
        ("right", x + width),
        ("bottom", y + height),
    ] {
        vm.put(id, name, JsValue::Number(value))?;
    }
    Ok(JsValue::Object(id))
}

/**
 * IntersectionObserver/ResizeObserver.observe(target): queue one task that calls the observer's
 * callback with an entry for the target, measured from the app's last layout. A target always
 * intersects the viewport (lazy loaders load, which is what an agent reading the page wants), and
 * its intersection is never empty: YouTube's lazy image loader keeps only entries whose
 * intersectionRect has a height, and with an empty one no thumbnail ever got its src.
 */
pub(in crate::v9) fn observe(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let target = call.arg(0);
    if super::events::target_of(vm, &target).is_none() || !matches!(target, JsValue::Object(_)) {
        return Err(
            vm.type_error("Failed to execute 'observe': parameter 1 is not of type 'Element'.")
        );
    }
    let Some(observer) = call.this.object() else {
        return Err(vm.type_error("Illegal invocation"));
    };
    let callback = match vm
        .heap
        .get(observer)
        .and_then(|object| object.props.get(&observer_callback_key()))
    {
        Some(crate::v9::vm::heap::Prop {
            slot: crate::v9::vm::heap::Slot::Data(callback),
            ..
        }) => callback.clone(),
        _ => return Err(vm.type_error("Illegal invocation")),
    };
    let entry = vm.new_object()?;
    vm.pin(JsValue::Object(entry));
    let measured = match super::events::target_of(vm, &target) {
        Some(super::Target::Node(node)) => super::dom::client_box(vm, node),
        _ => (0.0, 0.0, 0.0, 0.0),
    };
    vm.put(entry, "target", target)?;
    let rect = rect_at(vm, measured)?;
    let resize = vm
        .get(
            &JsValue::Object(observer),
            &crate::v9::vm::heap::Key::from("constructor"),
        )
        .and_then(|ctor| vm.get(&ctor, &crate::v9::vm::heap::Key::from("name")))?;
    if vm.to_text(&resize)? == "ResizeObserver" {
        vm.put(entry, "contentRect", rect)?;
        let size = vm.new_object()?;
        vm.put(size, "inlineSize", JsValue::Number(measured.2))?;
        vm.put(size, "blockSize", JsValue::Number(measured.3))?;
        for name in [
            "borderBoxSize",
            "contentBoxSize",
            "devicePixelContentBoxSize",
        ] {
            let list = vm.new_array(vec![JsValue::Object(size)])?;
            vm.put(entry, name, JsValue::Object(list))?;
        }
    } else {
        let viewport = (
            vm.page.environment.viewport_width,
            vm.page.environment.viewport_height,
        );
        let root = rect_object(vm, viewport.0, viewport.1)?;
        let (x, y, width, height) = measured;
        let intersection = rect_at(vm, (x, y, width.max(1.0), height.max(1.0)))?;
        vm.put(entry, "boundingClientRect", rect)?;
        vm.put(entry, "intersectionRect", intersection)?;
        vm.put(entry, "rootBounds", root)?;
        vm.put(entry, "isIntersecting", JsValue::Boolean(true))?;
        vm.put(entry, "isVisible", JsValue::Boolean(true))?;
        vm.put(entry, "intersectionRatio", JsValue::Number(1.0))?;
        vm.put(entry, "time", JsValue::Number(vm.page.state.clock))?;
    }
    let entries = vm.new_array(vec![JsValue::Object(entry)])?;
    super::timers::schedule(
        vm,
        callback,
        vec![JsValue::Object(entries), JsValue::Object(observer)],
    )?;
    Ok(JsValue::Undefined)
}
