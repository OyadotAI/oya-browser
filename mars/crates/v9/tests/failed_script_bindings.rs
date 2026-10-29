/**
 * A script that throws keeps its top-level declarations, as in browsers, and a later page step
 * still compiles them: a `let` that threw before its initializer ran is in the temporal dead zone
 * rather than gone. Blanking the failed source dropped those declarations, so every later step
 * refused the page's own state as forged.
 */
mod common;

use common::{host, input, round_trip};
use mars_v9::PageEvent;

/** A failed script's declarations survive a later event step, initialized or in the dead zone. */
#[test]
fn failed_script_declarations_survive_later_steps() {
    let sources = vec![
        "let a=1;throw 0;".to_string(),
        "let b=(()=>{throw 0})();".to_string(),
    ];
    let environment = common::environment();
    let host = host();
    let loaded = mars_v9::load_page(input(&sources, &host, &environment)).unwrap();
    assert_eq!(loaded.failed.len(), 2);
    let next = mars_v9::page_event(
        input(&sources, &host, &environment),
        round_trip(loaded.state),
        PageEvent::DomContentLoaded,
    )
    .expect("a failed script's declarations are still declared by its source");
    assert_eq!(common::global(&next, "a"), "1");
    assert!(next.state.globals()["b"].value.is_none());
}

/** A binding that no supplied script declares is still refused, so a blanked declaring source
is refused rather than silently accepted. */
#[test]
fn declarations_missing_from_the_sources_are_refused() {
    let sources = vec!["let a=1;throw 0;".to_string()];
    let environment = common::environment();
    let host = host();
    let loaded = mars_v9::load_page(input(&sources, &host, &environment)).unwrap();
    let blanked = vec![" ".repeat(sources[0].len())];
    let error = mars_v9::page_event(
        input(&blanked, &host, &environment),
        round_trip(loaded.state),
        PageEvent::DomContentLoaded,
    )
    .expect_err("a binding with no declaration is forged");
    assert!(
        error.message.contains("Invalid V9 state graph"),
        "{}",
        error.message
    );
}
