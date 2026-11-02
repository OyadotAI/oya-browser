/**
 * Same-document history as page scripts see it: pushState keeps its state as JSON on the entry,
 * and Back or Forward to an entry of the same document fires `popstate` with that state (then
 * `hashchange` for a fragment-only move) instead of reloading, so single-page routers work.
 */
use crate::common;

use common::{failure, host, input, load_with, round_trip};
use mars_v9::PageEvent;

/** pushState records its state as JSON; replaceState with null records none. */
#[test]
fn push_state_saves_its_state_as_json() {
    let environment = common::environment();
    let execution = load_with(
        "history.pushState({a:1,b:'x'}, '', '/next'); history.replaceState(null, '', '/r');",
        &environment,
    );
    assert_eq!(failure(&execution), None);
    let records = &execution.effects.history;
    assert_eq!(records.len(), 2);
    assert!(records[0].url.ends_with("/next") && !records[0].replace);
    assert_eq!(records[0].state.as_deref(), Some("{\"a\":1,\"b\":\"x\"}"));
    assert!(records[1].replace);
    assert_eq!(records[1].state, None);
}

/** Runs one PopState event against a page whose listener records what it saw. */
fn pop(url: &str, state: Option<&str>, hash_changed: bool) -> String {
    let mut environment = common::environment();
    let sources = vec![
        "let seen = '';
         addEventListener('popstate', e => { seen += location.pathname + location.hash + ':'
           + JSON.stringify(e.state) + ':' + JSON.stringify(history.state) });
         addEventListener('hashchange', () => { seen += '|hash' });"
            .to_string(),
    ];
    let execution = load_with(&sources[0], &environment);
    assert_eq!(failure(&execution), None);
    environment.url = url.into();
    let host = host();
    let event = PageEvent::PopState {
        state: state.map(str::to_owned),
        hash_changed,
    };
    let after = mars_v9::page_event(
        input(&sources, &host, &environment),
        round_trip(execution.state),
        event,
    )
    .expect("popstate");
    common::global(&after, "seen")
}

/** popstate carries the entry's state, and location and history.state already match it. */
#[test]
fn pop_state_restores_the_url_and_state() {
    assert_eq!(
        pop("https://example.com/app/list", Some("{\"n\":2}"), false),
        "/app/list:{\"n\":2}:{\"n\":2}"
    );
}

/** A fragment-only move fires hashchange after popstate; a missing state reads as null. */
#[test]
fn a_fragment_move_fires_hashchange_after_popstate() {
    assert_eq!(
        pop("https://example.com/app/page?x=1#second", None, true),
        "/app/page#second:null:null|hash"
    );
}
