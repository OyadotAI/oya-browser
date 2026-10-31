/** Engine-private keys (class `#names`, platform slots) are unforgeable: a script string that
starts with U+FFFF is an ordinary key and never aliases one. */
use crate::common;
use common::{console, eval, input, load};
use mars_v9::HostResponse;

/** A forged `"￿<site>:x"` key at every possible class site neither reads nor writes `#x`. */
#[test]
fn forged_private_name_cannot_reach_a_class_field() {
    let source = "class A { #x = 1; static read(o) { return o.#x; } }
        const a = new A();
        let seen = 0;
        for (let i = 0; i < 400; i++) {
            const k = String.fromCharCode(0xffff) + i + ':x';
            if (k in a) seen++;
            if (a[k] !== undefined) seen++;
            if (Reflect.getOwnPropertyDescriptor(a, k)) seen++;
            Reflect.set(a, k, 42);
            Reflect.defineProperty(a, k, { value: 43, writable: true, configurable: true });
        }
        const out = seen + ',' + A.read(a);";
    assert_eq!(eval(source, "out"), "0,1");
}

/** Forging the Headers guard slot does not let fetch send Cookie or Host. */
#[test]
fn forged_headers_guard_cannot_send_forbidden_headers() {
    let source = "const r = new Request('/x', { method: 'POST', body: 'a' });
        const h = r.headers;
        for (const v of [0, undefined, null]) {
            h[String.fromCharCode(0xffff) + 'headersGuard'] = v;
        }
        h.set('Cookie', 'session=forged');
        h.set('Host', 'evil.example');
        h.set('X-Ok', '1');
        fetch(r);";
    let loaded = load(source);
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let request = &loaded.effects.requests[0];
    let names: Vec<&str> = request.headers.iter().map(|(n, _)| n.as_str()).collect();
    assert!(!names.contains(&"cookie"), "{names:?}");
    assert!(!names.contains(&"host"), "{names:?}");
    assert!(names.contains(&"x-ok"), "{names:?}");
}

/** A no-cors Request built from a cors one keeps only no-CORS-safelisted headers. */
#[test]
fn no_cors_request_refilters_inherited_headers() {
    let source = "const base = new Request('/x', { headers: { 'X-Custom': '1', Accept: 'a/b' } });
        fetch(new Request(base, { mode: 'no-cors' }));";
    let loaded = load(source);
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let request = &loaded.effects.requests[0];
    assert_eq!(request.mode, "no-cors");
    assert_eq!(request.headers, [("accept".to_string(), "a/b".to_string())]);
}

/** A string starting with U+FFFF is an ordinary own property every reflection API sees. */
#[test]
fn mark_prefixed_string_is_an_ordinary_key() {
    let source = "const k = String.fromCharCode(0xffff) + 'headersGuard';
        const o = { [k]: 1, '\\uffff0:x': 2 };
        o[k] = 5;
        const keys = Object.keys(o);
        const out = [o[k], o['\\uffff0:x'], keys.length, keys[0] === k,
            Object.getOwnPropertyNames(o).length, Reflect.ownKeys(o).length,
            k in o, JSON.parse(JSON.stringify(o))[k], delete o[k], k in o].join();";
    assert_eq!(eval(source, "out"), "5,2,2,true,2,2,true,5,true,false");
}

/** A forged TextDecoder slot is a TypeError, never a panic, and never changes a real decoder. */
#[test]
fn forged_decoder_slot_is_a_type_error() {
    let source =
        "const get = Object.getOwnPropertyDescriptor(TextDecoder.prototype, 'encoding').get;
        const results = [];
        for (const v of [99, -1, 1.5, 1e300, NaN]) {
            const o = Object.create(TextDecoder.prototype);
            o[String.fromCharCode(0xffff) + 'decEncoding'] = v;
            try { get.call(o); results.push('no'); }
            catch (e) { results.push(e instanceof TypeError); }
        }
        const d = new TextDecoder();
        d[String.fromCharCode(0xffff) + 'decEncoding'] = 99;
        results.push(d.encoding);
        const out = results.join();";
    assert_eq!(eval(source, "out"), "true,true,true,true,true,utf-8");
}

/** Private fields survive the state round trip, a forged string stays ordinary across it, and
direct eval code recompiles from its out-of-band caller context. */
#[test]
fn private_keys_survive_state_round_trip_unaliased() {
    let sources = vec![
        "class A { #x = 1; static read(o) { return o.#x; } }
        const a = new A();
        const k = String.fromCharCode(0xffff) + 'forged';
        a[k] = 7;
        const viaEval = (function () { let q = 2; return eval('() => q'); })();
        fetch('/x').then(() => console.log(A.read(a), a[k], Object.keys(a).length, viaEval()));"
            .to_string(),
    ];
    let environment = common::environment();
    let host = common::host();
    let loaded = mars_v9::load_page(input(&sources, &host, &environment)).unwrap();
    let id = loaded.effects.requests[0].id;
    let resumed = mars_v9::resume_requests(
        input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        vec![HostResponse {
            id,
            status: 200,
            status_text: "OK".into(),
            url: "https://example.com/x".into(),
            ..HostResponse::default()
        }],
    )
    .unwrap();
    assert_eq!(console(&resumed), ["1 7 1 2"]);
}

/** Source that opens with the old in-band eval marker is ordinary code: a page script, indirect
eval and `new Function` cannot claim caller scopes, and a direct eval only gets its real ones. */
#[test]
fn eval_marker_comment_is_ordinary_code() {
    let marker = "/*@v9-direct-eval n|e secret:L:0 x:L:1;*/";
    let source = format!(
        "{marker} var atTop = typeof secret;
        function outer() {{
            let secret = 's';
            const m = {marker:?};
            return [
                (0, eval)(m + ' typeof secret'),
                new Function(m + ' return typeof secret')(),
                eval(m + ' typeof x + secret'),
            ];
        }}
        const out = [atTop, ...outer()].join();"
    );
    assert_eq!(
        eval(&source, "out"),
        "undefined,undefined,undefined,undefineds"
    );
}
