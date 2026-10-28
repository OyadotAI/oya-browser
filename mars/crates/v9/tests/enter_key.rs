/** A native Enter reaches the page as Chrome sends it: the field takes focus, keydown and
keypress carry key "Enter" and keyCode 13, and the app does the native action only when nobody
cancelled them. Google submits its search from this keydown. */
mod common;

use common::{failure, host, input, load_with, round_trip};
use mars_v9::KeyboardKey;

/** Presses Enter at node 1 under `source`'s listeners. */
fn enter(source: &str) -> mars_v9::Execution {
    let environment = common::environment();
    let sources = vec![source.to_string()];
    let execution = load_with(&sources[0], &environment);
    assert_eq!(failure(&execution), None);
    let host = host();
    mars_v9::keyboard_page(
        input(&sources, &host, &environment),
        round_trip(execution.state),
        KeyboardKey::Enter,
        Some(1),
    )
    .expect("enter")
}

/** The three events arrive in order with Chrome's legacy codes, and the default goes ahead. */
#[test]
fn enter_fires_keydown_keypress_keyup_with_key_code_13() {
    let after = enter(
        "let seen=[],out='';for(const t of ['keydown','keypress','keyup'])document.addEventListener(t,e=>{seen.push(t+':'+e.key+':'+e.keyCode+':'+e.charCode+':'+(document.activeElement.id||''));out=seen.join(' ')});",
    );
    assert_eq!(
        common::global(&after, "out"),
        "keydown:Enter:13:0:first keypress:Enter:13:13:first keyup:Enter:13:0:first"
    );
    assert!(after.effects.key_default);
}

/** preventDefault on keydown cancels the native action, as Google's handler does. */
#[test]
fn a_cancelled_keydown_stops_the_native_action() {
    let after = enter("document.addEventListener('keydown',e=>{if(e.key==='Enter')e.preventDefault()});");
    assert!(!after.effects.key_default);
}
