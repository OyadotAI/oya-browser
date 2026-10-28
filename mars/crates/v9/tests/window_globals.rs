/** The page's window owns every global interface, as a browser's does, so feature detection works. */
mod common;
use common::eval;

/** Constructors and functions are own, writable, configurable, non-enumerable data properties. */
#[test]
fn globals_are_own_hidden_data_properties_of_window() {
    let source = "const g=globalThis;const d=Object.getOwnPropertyDescriptor(g,'Date');\
        const r=[g.hasOwnProperty('Promise'),d.value===Date,d.writable,d.enumerable,d.configurable,\
        Object.keys(g).includes('Date'),Object.getOwnPropertyNames(g).includes('parseInt'),\
        Object.prototype.toString.call(g)].join();";
    assert_eq!(
        eval(source, "r"),
        "true,true,true,false,true,false,true,[object Window]"
    );
}

/** A page may replace or remove a global on the window, and the replacement is what names resolve to. */
#[test]
fn window_globals_can_be_replaced_and_deleted() {
    let source = "globalThis.Date=7;const replaced=Date;\
        const removed=delete globalThis.parseFloat;\
        const r=[replaced,removed,globalThis.hasOwnProperty('parseFloat')].join();";
    assert_eq!(eval(source, "r"), "7,true,false");
}
