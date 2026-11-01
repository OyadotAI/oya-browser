/**
 * The coordinator-backed platform APIs (`platform_apis` bootstrap) through V9's public surface:
 * a page calls them, each call becomes one "platform" HostRequest, and a fake coordinator here
 * answers the way the real one does, so the JavaScript side of every API is tested against the
 * protocol rather than against its own internals.
 */
use crate::common;

use common::{console, input};
use mars_v9::{Execution, HostRequest, HostResponse};
use serde_json::{Value, json};

/** Rounds of answering before a test gives up waiting for a page to go quiet. */
const ROUNDS: usize = 24;
/** Timer callbacks one timer turn may run. */
const TIMER_BUDGET: usize = 16;

/** The op and parsed arguments of one platform request. */
fn op_of(request: &HostRequest) -> (String, Value) {
    let body: Value = serde_json::from_str(request.body.as_deref().unwrap_or("{}")).unwrap();
    let args = serde_json::from_str(body["args"].as_str().unwrap_or("null")).unwrap();
    (request.method.clone(), args)
}

/** Whether the request carried transient user activation. */
fn active(request: &HostRequest) -> bool {
    let body: Value = serde_json::from_str(request.body.as_deref().unwrap_or("{}")).unwrap();
    body["active"].as_bool().unwrap_or(false)
}

/** A fake coordinator's answer: `Some(answer)` answers now, `None` leaves the request parked. */
type Answer = Option<(Value, Vec<u8>)>;

/** Load `source`, then keep answering platform requests with `serve` until nothing is answered. */
fn run(source: &str, mut serve: impl FnMut(&str, &Value, &HostRequest) -> Answer) -> Vec<String> {
    let sources = vec![source.to_string()];
    let environment = common::environment();
    let host = common::host();
    let mut execution: Execution =
        mars_v9::load_page(input(&sources, &host, &environment)).unwrap();
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    let mut lines = console(&execution);
    let mut parked: Vec<HostRequest> = Vec::new();
    for _ in 0..ROUNDS {
        parked.extend(execution.effects.requests.clone());
        let mut responses = Vec::new();
        parked.retain(|request| {
            let (op, args) = op_of(request);
            match serve(&op, &args, request) {
                Some((answer, bytes)) => {
                    responses.push(HostResponse {
                        id: request.id,
                        status: 200,
                        body: answer.to_string(),
                        body_bytes: bytes,
                        ..HostResponse::default()
                    });
                    false
                }
                None => true,
            }
        });
        if responses.is_empty() {
            let Some(due) = execution.effects.next_timer_ms else {
                break;
            };
            execution = mars_v9::run_timers(
                input(&sources, &host, &environment),
                common::round_trip(execution.state),
                due,
                TIMER_BUDGET,
            )
            .unwrap();
            lines.extend(console(&execution));
            continue;
        }
        execution = mars_v9::resume_requests(
            input(&sources, &host, &environment),
            common::round_trip(execution.state),
            responses,
        )
        .unwrap();
        lines.extend(console(&execution));
    }
    lines
}

/** `{ok: value}` with no bytes. */
fn ok(value: Value) -> Answer {
    Some((json!({ "ok": value }), Vec::new()))
}

/** `{error: {name, message}}`. */
fn error(name: &str, message: &str) -> Answer {
    Some((
        json!({ "error": { "name": name, "message": message } }),
        Vec::new(),
    ))
}

/** The bridge global is gone before page scripts run, and the APIs exist with their shapes. */
#[test]
fn bridge_is_private_and_apis_exist() {
    let lines = run(
        "console.log([typeof __marsPlatformHost, 'caches' in self, typeof cookieStore,
           typeof navigator.locks.request, typeof ClipboardItem, typeof PasswordCredential,
           typeof navigator.serviceWorker.register, String(navigator.getGamepads()),
           Object.prototype.toString.call(navigator.serviceWorker)].join('|'));",
        |_, _, _| None,
    );
    assert_eq!(
        lines,
        [
            "undefined|true|object|function|function|function|function|,,,|[object ServiceWorkerContainer]"
        ]
    );
}

/** Clipboard writes send the text with the page's activation; a refusal is the coordinator's NotAllowedError. */
#[test]
fn clipboard_write_carries_activation_and_refusal() {
    let mut seen = Vec::new();
    let lines = run(
        "navigator.clipboard.writeText('hello').then(() => console.log('wrote'), (e) => console.log(e.name));",
        |op, args, request| {
            seen.push((op.to_string(), args.clone(), active(request)));
            error("NotAllowedError", "Write permission denied.")
        },
    );
    assert_eq!(lines, ["NotAllowedError"]);
    assert_eq!(seen[0].0, "clipboard.write");
    assert_eq!(seen[0].1["items"][0]["text"], "hello");
    assert!(!seen[0].2, "a page load is not a user activation");
}

/** readText resolves with the pasteboard's plain text the coordinator returns. */
#[test]
fn clipboard_read_text_returns_plain_text() {
    let lines = run(
        "navigator.clipboard.readText().then((t) => console.log('text:' + t));",
        |op, _, _| match op {
            "clipboard.read" => ok(json!([{ "type": "text/plain", "text": "pasted" }])),
            _ => None,
        },
    );
    assert_eq!(lines, ["text:pasted"]);
}

/** A granted lock runs the callback, resolves with its result, then releases. */
#[test]
fn lock_request_runs_callback_and_releases() {
    let mut released = false;
    let lines = run(
        "navigator.locks.request('res', { mode: 'shared' }, (lock) => lock.name + ':' + lock.mode)
           .then((value) => console.log(value));",
        |op, args, _| match op {
            "events.subscribe" => ok(Value::Null),
            "locks.request" => ok(json!({ "granted": true })),
            "locks.release" => {
                released = args["id"] == 1;
                ok(Value::Null)
            }
            _ => None,
        },
    );
    assert_eq!(lines, ["res:shared"]);
    assert!(released);
}

/** ifAvailable without a grant calls back with null; invalid option mixes are NotSupportedError. */
#[test]
fn lock_if_available_and_option_errors() {
    let lines = run(
        "navigator.locks.request('a', { ifAvailable: true }, (lock) => console.log(String(lock)));
         navigator.locks.request('a', { ifAvailable: true, steal: true }, () => {}).catch((e) => console.log(e.name));
         navigator.locks.request('-a', () => {}).catch((e) => console.log(e.name));",
        |op, _, _| match op {
            "events.subscribe" => ok(Value::Null),
            "locks.request" => ok(json!({ "granted": false })),
            _ => None,
        },
    );
    assert_eq!(lines, ["NotSupportedError", "NotSupportedError", "null"]);
}

/** A steal event rejects the holder's request() promise with AbortError. */
#[test]
fn stolen_lock_rejects_holder() {
    let mut granted = false;
    let mut events = 0;
    let lines = run(
        "navigator.locks.request('a', () => new Promise(() => {})).catch((e) => console.log(e.name));",
        |op, _, _| match op {
            "events.subscribe" => ok(Value::Null),
            "locks.request" => {
                granted = true;
                ok(json!({ "granted": true }))
            }
            "events.next" if granted => {
                events += 1;
                (events == 1).then(|| {
                    (
                        json!({ "ok": [{ "topic": "locks", "stolen": 1 }] }),
                        Vec::new(),
                    )
                })
            }
            _ => None,
        },
    );
    assert_eq!(lines, ["AbortError"]);
}

/** cookieStore.get answers from the jar's list; set validates and sends Secure-default fields. */
#[test]
fn cookie_store_get_and_set() {
    let mut set = Value::Null;
    let lines = run(
        "cookieStore.get('a').then((c) => console.log(c.name + '=' + c.value + ' ' + c.sameSite));
         cookieStore.set('b', '2').then(() => console.log('set'));
         cookieStore.set('x;', '1').catch((e) => console.log(e.name));",
        |op, args, _| match op {
            "cookies.get" => ok(
                json!([{ "name": "a", "value": "1", "domain": null, "path": "/",
                "expires": null, "secure": true, "sameSite": "lax", "partitioned": false }]),
            ),
            "cookies.set" => {
                set = args.clone();
                ok(Value::Null)
            }
            _ => None,
        },
    );
    assert_eq!(lines, ["TypeError", "a=1 lax", "set"]);
    assert_eq!(set["sameSite"], "strict");
    assert_eq!(set["path"], "/");
}

/** A change event from the coordinator reaches `change` listeners as a CookieChangeEvent. */
#[test]
fn cookie_change_events_dispatch() {
    let mut sent = false;
    let lines = run(
        "cookieStore.addEventListener('change', (e) => console.log(e.constructor.name + ' ' + e.changed[0].name + ' ' + e.deleted.length));",
        |op, _, _| match op {
            "events.subscribe" => ok(Value::Null),
            "events.next" if !sent => {
                sent = true;
                ok(
                    json!([{ "topic": "cookies", "changed": [{ "name": "k", "value": "v" }], "deleted": [] }]),
                )
            }
            _ => None,
        },
    );
    assert_eq!(lines, ["CookieChangeEvent k 0"]);
}

/** Clear Key access follows Get Supported Configuration; other key systems are NotSupportedError. */
#[test]
fn media_key_system_access_algorithm() {
    let lines = run(
        "const video = [{ contentType: 'video/mp4; codecs=\"avc1.42E01E\"' }];
         navigator.requestMediaKeySystemAccess('org.w3.clearkey', [{ initDataTypes: ['cenc'], videoCapabilities: video }])
           .then((a) => console.log(a.keySystem + ' ' + a.getConfiguration().initDataTypes + ' ' + a.getConfiguration().sessionTypes));
         navigator.requestMediaKeySystemAccess('com.widevine.alpha', [{ videoCapabilities: video }]).catch((e) => console.log(e.name));
         navigator.requestMediaKeySystemAccess('org.w3.clearkey', [{ videoCapabilities: [{ contentType: 'video/mp4', robustness: 'HW_SECURE_ALL' }] }]).catch((e) => console.log('robust ' + e.name));
         navigator.requestMediaKeySystemAccess('', [{}]).catch((e) => console.log(e.name));",
        |_, _, _| None,
    );
    assert_eq!(
        lines,
        [
            "TypeError",
            "org.w3.clearkey cenc temporary",
            "NotSupportedError",
            "robust NotSupportedError"
        ]
    );
}

/** A Clear Key session builds a keyids license request and accepts a JWK set license. */
#[test]
fn clear_key_session_license_exchange() {
    let mut keys = Value::Null;
    let lines = run(
        "(async () => {
           const access = await navigator.requestMediaKeySystemAccess('org.w3.clearkey', [{ initDataTypes: ['keyids'], audioCapabilities: [{ contentType: 'audio/mp4; codecs=\"mp4a.40.2\"' }] }]);
           const mediaKeys = await access.createMediaKeys();
           const session = mediaKeys.createSession();
           session.addEventListener('message', async (event) => {
             const request = JSON.parse(new TextDecoder().decode(event.message));
             console.log(event.messageType + ' ' + request.kids.join(',') + ' ' + request.type);
             const license = { keys: [{ kty: 'oct', kid: request.kids[0], k: 'AAECAwQFBgcICQoLDA0ODw' }] };
             await session.update(new TextEncoder().encode(JSON.stringify(license)));
             session.keyStatuses.forEach((status) => console.log('status ' + status));
           });
           const init = new TextEncoder().encode(JSON.stringify({ kids: ['AAAAAAAAAAAAAAAAAAAAAQ'] }));
           await session.generateRequest('keyids', init);
           console.log('id ' + session.sessionId);
         })();",
        |op, args, _| match op {
            "eme.keys" => {
                keys = args.clone();
                ok(Value::Null)
            }
            _ => None,
        },
    );
    assert_eq!(
        lines,
        [
            "id 1",
            "license-request AAAAAAAAAAAAAAAAAAAAAQ temporary",
            "status usable"
        ]
    );
    assert_eq!(keys["keys"][0]["kid"], "00000000000000000000000000000001");
    assert_eq!(keys["keys"][0]["key"], "000102030405060708090a0b0c0d0e0f");
}

/** Cache.put sends the request, response metadata and body bytes; match rebuilds the Response. */
#[test]
fn cache_put_and_match_round_trip() {
    let mut stored: Option<(Value, Vec<u8>)> = None;
    let lines = run(
        "(async () => {
           const cache = await caches.open('v1');
           await cache.put('/a.txt', new Response('body!', { headers: { 'content-type': 'text/plain' } }));
           const hit = await cache.match('/a.txt');
           console.log(hit.status + ' ' + hit.headers.get('content-type') + ' ' + (await hit.text()) + ' ' + hit.url);
           console.log(String(await cache.match('/missing')));
         })();",
        |op, args, request| match op {
            "caches.open" => ok(json!(7)),
            "cache.put" => {
                stored = Some((args.clone(), request.body_bytes.clone()));
                ok(Value::Null)
            }
            "cache.match" if args["request"]["url"] == "https://example.com/a.txt" => {
                let (put, bytes) = stored.clone().unwrap();
                let mut record = put["entries"][0]["response"].clone();
                record["url"] = json!("https://example.com/a.txt");
                Some((json!({ "ok": [record] }), bytes))
            }
            "cache.match" => ok(json!([])),
            _ => None,
        },
    );
    assert_eq!(
        lines,
        [
            "200 text/plain body! https://example.com/a.txt",
            "undefined"
        ]
    );
    let (put, bytes) = stored.unwrap();
    assert_eq!(put["cache"], 7);
    assert_eq!(put["entries"][0]["request"]["method"], "GET");
    assert_eq!(bytes, b"body!");
}

/** register() validates and resolves scope, then exposes the coordinator's registration. */
#[test]
fn service_worker_register_builds_registration() {
    let mut registered = Value::Null;
    let lines = run(
        "navigator.serviceWorker.register('sw.js').then((r) => console.log(r.scope + ' ' + r.installing.state + ' ' + r.installing.scriptURL + ' ' + (r === r)));
         navigator.serviceWorker.register('ftp://x/sw.js').catch((e) => console.log(e.name));
         navigator.serviceWorker.getRegistrations().then((list) => console.log('list ' + list.length));",
        |op, args, _| match op {
            "events.subscribe" => ok(Value::Null),
            "sw.register" => {
                registered = args.clone();
                ok(json!({ "id": 1, "scope": args["scope"], "updateViaCache": "imports",
                    "installing": { "id": 1, "scriptURL": args["scriptURL"], "state": "installing" },
                    "waiting": null, "active": null, "preload": { "enabled": false, "headerValue": "true" } }))
            }
            "sw.getRegistrations" => ok(json!([])),
            _ => None,
        },
    );
    assert_eq!(
        lines,
        [
            "TypeError",
            "https://example.com/app/ installing https://example.com/app/sw.js true",
            "list 0"
        ]
    );
    assert_eq!(registered["type"], "classic");
}

/** Structured data crossing to a service worker keeps types JSON loses. */
#[test]
fn service_worker_post_message_encodes_structured_data() {
    let mut data = Value::Null;
    run(
        "navigator.serviceWorker.register('sw.js').then((r) => r.installing.postMessage({ when: new Date(5), list: new Set([1]), u: undefined, n: NaN }));",
        |op, args, _| match op {
            "events.subscribe" => ok(Value::Null),
            "sw.register" => ok(
                json!({ "id": 1, "scope": "https://example.com/app/", "updateViaCache": "imports",
                "installing": { "id": 1, "scriptURL": "https://example.com/app/sw.js", "state": "installing" },
                "waiting": null, "active": null, "preload": { "enabled": false, "headerValue": "true" } }),
            ),
            "sw.postMessage" => {
                data = args["data"].clone();
                ok(Value::Null)
            }
            _ => None,
        },
    );
    let records = data["records"].as_array().unwrap();
    assert!(records.iter().any(|record| record["t"] == "Date"));
    assert!(records.iter().any(|record| record["t"] == "Set"));
}

/** Credentials: a stored password credential comes back as a PasswordCredential; WebAuthn rejects. */
#[test]
fn credentials_get_and_public_key_refusal() {
    let lines = run(
        "navigator.credentials.get({ password: true }).then((c) => console.log(c.constructor.name + ' ' + c.id + ' ' + c.type));
         navigator.credentials.get({ publicKey: { challenge: new Uint8Array(1) } }).catch((e) => console.log(e.name));
         PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable().then((v) => console.log('uvpa ' + v));",
        |op, _, _| match op {
            "credentials.get" => ok(json!({ "type": "password", "id": "me", "password": "pw", "name": "", "iconURL": "" })),
            _ => None,
        },
    );
    assert_eq!(
        lines,
        [
            "NotAllowedError",
            "uvpa false",
            "PasswordCredential me password"
        ]
    );
}

/** getInstalledRelatedApps is the empty list in a top-level page; userActivation starts inactive. */
#[test]
fn related_apps_and_user_activation() {
    let lines = run(
        "navigator.getInstalledRelatedApps().then((apps) => console.log('apps ' + apps.length));
         console.log('active ' + navigator.userActivation.isActive + ' ' + navigator.userActivation.hasBeenActive);",
        |_, _, _| None,
    );
    assert_eq!(lines, ["active false false", "apps 0"]);
}

/** Wake lock sentinels come from the coordinator and fire `release` when released. */
#[test]
fn wake_lock_sentinel_release() {
    let lines = run(
        "navigator.wakeLock.request('screen').then(async (s) => {
           s.onrelease = () => console.log('released event');
           console.log(s.type + ' ' + s.released);
           await s.release();
           console.log('released ' + s.released);
         });
         navigator.wakeLock.request('system').catch((e) => console.log(e.name));",
        |op, _, _| match op {
            "events.subscribe" | "wakelock.acquire" | "wakelock.release" => ok(Value::Null),
            _ => None,
        },
    );
    assert_eq!(
        lines,
        [
            "TypeError",
            "screen false",
            "released event",
            "released true"
        ]
    );
}

/** MIDI access lists the coordinator's ports; send validates messages before they cross. */
#[test]
fn midi_access_lists_ports_and_validates_send() {
    let mut sent = Value::Null;
    let lines = run(
        "navigator.requestMIDIAccess().then((access) => {
           console.log(access.inputs.size + ' ' + access.outputs.size + ' ' + access.sysexEnabled);
           const out = access.outputs.get('out-1');
           console.log(out.name + ' ' + out.type + ' ' + out.state);
           try { out.send([0x90, 0x45]); } catch (e) { console.log(e.name); }
           try { out.send([0xf0, 1, 0xf7]); } catch (e) { console.log(e.name); }
           out.send([0x90, 0x45, 0x7f]);
         });",
        |op, args, _| match op {
            "midi.access" => ok(json!({ "ports": [
                { "id": "in-1", "type": "input", "name": "Keys", "manufacturer": "M", "version": "1" },
                { "id": "out-1", "type": "output", "name": "Synth", "manufacturer": "M", "version": "1" }] })),
            "events.subscribe" | "midi.open" => ok(Value::Null),
            "midi.send" => {
                sent = args.clone();
                ok(Value::Null)
            }
            _ => None,
        },
    );
    assert_eq!(
        lines,
        [
            "1 1 false",
            "Synth output connected",
            "TypeError",
            "InvalidAccessError"
        ]
    );
    assert_eq!(sent["data"], json!([0x90, 0x45, 0x7f]));
}

/** Storage access answers come from the coordinator; execCommand copy needs activation. */
#[test]
fn storage_access_and_exec_copy() {
    let lines = run(
        "document.hasStorageAccess().then((v) => console.log('has ' + v));
         console.log('copy ' + document.execCommand('copy') + ' ' + document.queryCommandSupported('copy'));",
        |op, _, _| match op {
            "storage.has" => ok(json!(true)),
            _ => None,
        },
    );
    assert_eq!(lines, ["copy false true", "has true"]);
}
