/**
 * A person's or agent's form submission runs the page's `submit` handlers first: the event is
 * trusted, bubbles and is cancelable, and the app submits natively only when nobody cancelled it.
 * Single-page apps send their forms with fetch from this handler, and client validation refuses
 * bad input here.
 */
mod common;

use common::{failure, host, input, load_with, round_trip};
use mars_v9::PageEvent;

/** Fires Submit at the page's first element (id 0) under `source`'s listeners. */
fn submit(source: &str) -> mars_v9::Execution {
    let environment = common::environment();
    let sources = vec![source.to_string()];
    let execution = load_with(&sources[0], &environment);
    assert_eq!(failure(&execution), None);
    let host = host();
    mars_v9::page_event(
        input(&sources, &host, &environment),
        round_trip(execution.state),
        PageEvent::Submit { form: 0 },
    )
    .expect("submit event")
}

/** An unprevented submit is trusted, bubbles to the document and lets the app submit. */
#[test]
fn an_unprevented_submit_goes_ahead() {
    let after = submit(
        "let seen='';document.addEventListener('submit',e=>{seen=[e.type,e.isTrusted,e.bubbles,e.cancelable,e.target.id].join()});",
    );
    assert_eq!(common::global(&after, "seen"), "submit,true,true,true,main");
    assert_eq!(after.effects.submit, Some(0));
}

/** preventDefault keeps the native submission from happening. */
#[test]
fn a_prevented_submit_does_not_submit() {
    let after =
        submit("document.getElementById('main').addEventListener('submit',e=>e.preventDefault());");
    assert_eq!(after.effects.submit, None);
}

/** A handler that throws does not cancel the submission. */
#[test]
fn a_throwing_handler_still_submits() {
    let after = submit("document.addEventListener('submit',()=>{throw new Error('x')});");
    assert_eq!(after.effects.submit, Some(0));
}
