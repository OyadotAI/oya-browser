/**
 * Platform pieces LinkedIn's sign-in app needs to start: WebIDL's `DOMException` (its bundle
 * builds `new DOMException("cancelled by new navigation", "AbortError")` at module top level),
 * exact `\p{Zs}`, `\p{Zl}`, `\p{Zp}` regex classes, and the WHATWG streams React Server
 * Components hydrate from. Without them every app module failed, or React replaced the page
 * with "Something went wrong".
 */
mod common;

use common::eval;

/** The constructor sets name, message and the legacy code, and instances are Errors. */
#[test]
fn dom_exception_constructs_named_errors() {
    let out = eval(
        "const e = new DOMException('cancelled', 'AbortError');
         const d = new DOMException();
         const out = [e.name, e.message, e.code, e instanceof DOMException, e instanceof Error,
           String(e), d.name, d.message, d.code, DOMException.ABORT_ERR].join('|');",
        "out",
    );
    assert_eq!(
        out,
        "AbortError|cancelled|20|true|true|AbortError: cancelled|Error||0|20"
    );
}

/** WebIDL constructors refuse a call without `new`. */
#[test]
fn dom_exception_needs_new() {
    let out = eval(
        "let out; try { DOMException('x') } catch (e) { out = e instanceof TypeError }",
        "out",
    );
    assert_eq!(out, "true");
}

/** Exceptions the platform throws are DOMExceptions too, as pages test with `instanceof`. */
#[test]
fn platform_exceptions_are_dom_exceptions() {
    let out = eval(
        "let out; try { structuredClone(() => 1) }
         catch (e) { out = [e instanceof DOMException, e.name, e.code].join('|') }",
        "out",
    );
    assert_eq!(out, "true|DataCloneError|25");
}

/** The separator and control categories are exact, and LinkedIn's whitespace class compiles
 * and matches a no-break space, an ideographic space and a line separator. */
#[test]
fn separator_property_escapes_match_exactly() {
    let out = eval(
        "const re = /[\\t\\n\\u000B\\f\\r\\u001C-\\u001F\\p{Zs}\\p{Zl}\\p{Zp}]/gu;
         const out = [re.test('\\u00a0'), /\\p{Zs}/u.test('\\u3000'), /\\p{Zl}/u.test('\\u2028'),
           /\\p{Zs}/u.test('\\u2028'), /\\p{Z}/u.test('\\u2029'), /\\p{Cc}/u.test('\\u0085'),
           /\\p{Zs}/u.test('a')].join('|');",
        "out",
    );
    assert_eq!(out, "true|true|true|false|true|true|false");
}

/** A source's chunks reach a default reader in order, then `done`, as React Server Components
 * read their payload (`reader.read().then(function next(r) { ... reader.read().then(next) })`). */
#[test]
fn a_readable_stream_delivers_enqueued_chunks_then_done() {
    let out = eval(
        "let out = '';
         const encoder = new TextEncoder(), decoder = new TextDecoder();
         const stream = new ReadableStream({ start(c) {
           c.enqueue(encoder.encode('ab')); c.enqueue(encoder.encode('cd')); c.close(); } });
         const reader = stream.getReader();
         out += stream.locked + ':';
         reader.read().then(function next(r) {
           if (r.done) { out += '|done'; return; }
           out += decoder.decode(r.value, { stream: true });
           return reader.read().then(next);
         });",
        "out",
    );
    assert_eq!(out, "true:abcd|done");
}

/** A read made before any chunk waits, and `pull` is asked for more while one is wanted. */
#[test]
fn a_waiting_read_is_answered_by_pull() {
    let out = eval(
        "let out = '', n = 0;
         const reader = new ReadableStream({ pull(c) { n++; if (n > 3) c.close(); else c.enqueue(n); } })
           .getReader();
         (async () => { for (;;) { const r = await reader.read(); if (r.done) break; out += r.value; } out += '!'; })();",
        "out",
    );
    assert_eq!(out, "123!");
}

/** A TransformStream passes written chunks to its readable side, through `transform` when it
 * has one, and closing the writer ends the readable. */
#[test]
fn a_transform_stream_feeds_its_readable_side() {
    let out = eval(
        "let out = '';
         const t = new TransformStream({ transform(chunk, c) { c.enqueue(chunk * 2) } });
         const writer = t.writable.getWriter(), reader = t.readable.getReader();
         writer.write(1); writer.write(2); writer.close();
         (async () => { for (;;) { const r = await reader.read(); if (r.done) break; out += r.value + ','; } out += 'end'; })();",
        "out",
    );
    assert_eq!(out, "2,4,end");
}

/** Errors reject reads, a second reader is refused while locked, and a BYOB reader is the
 * spec's TypeError for a stream that is not a byte stream. */
#[test]
fn stream_errors_locks_and_byob_follow_the_spec() {
    let out = eval(
        "let out = [];
         let ctl; const s = new ReadableStream({ start(c) { ctl = c } });
         try { s.getReader({ mode: 'byob' }) } catch (e) { out.push(e instanceof TypeError) }
         const r = s.getReader();
         try { s.getReader() } catch (e) { out.push('locked') }
         r.read().catch(e => { out.push('rejected:' + e) });
         ctl.error('boom');
         Promise.resolve().then(() => Promise.resolve()).then(() => { out = out.join('|') });",
        "out",
    );
    assert_eq!(out, "true|locked|rejected:boom");
}
