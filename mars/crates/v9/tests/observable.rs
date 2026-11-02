/** Observable, Subscriber and EventTarget.when through page scripts: one test per rule of the proposal. */
use crate::common;
use common::{eval, failure, global, load};

/** The interfaces are globals, and Observable needs `new`, so the constructor is not callable. */
#[test]
fn observable_is_a_global_that_refuses_a_call_without_new() {
    let source = "let r = typeof Observable === 'function' && typeof Subscriber === 'function' \
        && !Object.keys(globalThis).includes('Observable') ? 'global' : 'missing';\
        try { Observable(() => {}); r += ' no-throw'; } catch (e) { r += ' ' + e.name; }";
    assert_eq!(eval(source, "r"), "global TypeError");
}

/** A Subscriber is handed to an initializer, never constructed by a script. */
#[test]
fn subscriber_is_not_constructible() {
    let source = "let r = 'no-throw'; try { new Subscriber(); } catch (e) { r = e.name; }";
    assert_eq!(eval(source, "r"), "TypeError");
}

/** Values emitted before complete arrive synchronously, and nothing arrives after complete. */
#[test]
fn subscribe_delivers_values_synchronously_and_ignores_values_after_complete() {
    let source = "let r = ''; new Observable(s => { s.next(1); s.next(2); s.complete(); s.next(3); })\
        .subscribe({ next: v => { r += v; }, complete: () => { r += 'C'; } });";
    assert_eq!(eval(source, "r"), "12C");
}

/** An error with no error handler is reported to the global as an error event carrying the error. */
#[test]
fn error_without_a_handler_is_reported_to_the_global_error_event() {
    let source = "let r = 'none'; const err = new Error('boom');\
        addEventListener('error', e => { r = e.error === err && e.message.includes('boom') ? 'reported' : 'wrong'; }, { once: true });\
        new Observable(s => { s.error(err); }).subscribe({ next() {} });";
    assert_eq!(eval(source, "r"), "reported");
}

/** Teardowns run last-in first-out when the subscription closes. */
#[test]
fn teardowns_run_last_in_first_out() {
    let source = "let r = ''; new Observable(s => { s.addTeardown(() => { r += 'a'; });\
        s.addTeardown(() => { r += 'b'; }); s.complete(); }).subscribe();";
    assert_eq!(eval(source, "r"), "ba");
}

/** A signal already aborted when subscribe runs closes the subscriber before the initializer. */
#[test]
fn an_aborted_signal_closes_the_subscriber_before_the_initializer_runs() {
    let source = "let r = ''; new Observable(s => { r = 'ran:' + s.active; })\
        .subscribe({}, { signal: AbortSignal.abort('stop') });";
    assert_eq!(eval(source, "r"), "ran:false");
}

/** take(0) completes without subscribing to its source. */
#[test]
fn take_zero_completes_without_subscribing_to_the_source() {
    let source = "let r = 'untouched'; new Observable(() => { r = 'subscribed'; }).take(0)\
        .subscribe({ complete: () => { r += ':completed'; } });";
    assert_eq!(eval(source, "r"), "untouched:completed");
}

/** A consumer that stops early makes the iterable's return() run once. */
#[test]
fn an_iterable_return_is_called_when_the_consumer_stops_early() {
    let source = "let r = ''; const it = { [Symbol.iterator]() { let i = 0; return {\
        next() { return { done: false, value: i++ }; }, return() { r += 'R'; return {}; } }; } };\
        Observable.from(it).take(2).subscribe(v => { r += v; });";
    assert_eq!(eval(source, "r"), "01R");
}

/** flatMap subscribes to one inner at a time: the second waits for the first to complete. */
#[test]
fn flat_map_subscribes_to_one_inner_at_a_time() {
    let source = "let r = ''; const subs = [];\
        new Observable(s => { s.next('a'); s.next('b'); s.complete(); })\
        .flatMap(() => new Observable(s => { subs.push(s); })).subscribe(() => {});\
        r = String(subs.length); subs[0].complete(); r += ':' + subs.length;";
    assert_eq!(eval(source, "r"), "1:2");
}

/** toArray resolves with every value once the source completes. */
#[test]
fn to_array_resolves_with_every_value_after_completion() {
    let source = "let r = 'pending'; Observable.from([1, 2, 3]).toArray()\
        .then(values => { r = values.join(','); });";
    let execution = load(source);
    assert!(failure(&execution).is_none(), "{:?}", failure(&execution));
    assert_eq!(global(&execution, "r"), "1,2,3");
}

/** An initializer that throws reaches the error handler instead of escaping subscribe. */
#[test]
fn an_initializer_that_throws_reaches_the_error_handler() {
    let source = "let r = ''; new Observable(() => { throw new Error('x'); })\
        .subscribe({ error: e => { r = 'err:' + e.message; } });";
    assert_eq!(eval(source, "r"), "err:x");
}

/** reduce without a seed rejects with a TypeError when the source is empty. */
#[test]
fn reduce_without_a_seed_rejects_with_a_type_error_on_an_empty_source() {
    let source = "let r = 'pending'; Observable.from([]).reduce((a, b) => a + b)\
        .then(() => {}, e => { r = e.name; });";
    let execution = load(source);
    assert!(failure(&execution).is_none(), "{:?}", failure(&execution));
    assert_eq!(global(&execution, "r"), "TypeError");
}

/** EventTarget.prototype.when listens only while the subscription lives. */
#[test]
fn event_target_when_listens_until_the_subscription_is_aborted() {
    let source = "let r = ''; const target = new EventTarget(); const ac = new AbortController();\
        target.when('ping').subscribe(() => { r += 'p'; }, { signal: ac.signal });\
        target.dispatchEvent(new Event('ping')); ac.abort(); target.dispatchEvent(new Event('ping'));";
    assert_eq!(eval(source, "r"), "p");
}

/** A reported error carries the line and column of the frame that made it, so the console and the error event point at real code. */
#[test]
fn reported_error_carries_the_position_of_the_error_frame() {
    let source = "let r = 'none'; const err = new Error('boom');\
        addEventListener('error', e => { r = e.lineno > 0 && e.colno > 0 ? 'positioned' : 'bare'; }, { once: true });\
        new Observable(s => { s.error(err); }).subscribe({ next() {} });";
    assert_eq!(eval(source, "r"), "positioned");
}
