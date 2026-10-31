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

/** Bare handler reads must work before any page has assigned a callback. */
#[test]
fn window_handlers_start_null_and_are_visible_as_global_bindings() {
    let source = "const r=[onload===null,onerror===null,onresize===null,onmessage===null,\
        onbeforeunload===null,onunhandledrejection===null,'onload' in window,\
        document.body.onload===null].join();";
    assert_eq!(eval(source, "r"), "true,true,true,true,true,true,true,true");
}

/** Strict assignment uses the existing window binding and dispatch sees the assigned callback. */
#[test]
fn bare_window_handlers_share_assignment_dispatch_and_body_forwarding() {
    let source = "'use strict';let calls=0;const f=function(){calls+=this===window?1:100};\
        const old=onload;onload=f;window.dispatchEvent(new Event('load'));\
        const assigned=window.onload===f&&document.body.onload===f;\
        document.body.onload=null;window.dispatchEvent(new Event('load'));\
        const r=[old===null,assigned,onload===null,calls].join();";
    assert_eq!(eval(source, "r"), "true,true,true,1");
}

/** A fresh page must not inherit a handler assigned by a previous page's mutable global. */
#[test]
fn window_handler_defaults_are_isolated_between_pages() {
    assert_eq!(
        eval("onload=()=>{};const r=typeof onload;", "r"),
        "function"
    );
    assert_eq!(eval("const r=String(onload===null);", "r"), "true");
}

/** Plain platform interfaces detected by modern web apps exist on globalThis. */
#[test]
fn plain_platform_interfaces_exist_on_window() {
    let source = "const r=[typeof ScreenOrientation==='function',typeof Scheduler==='function',typeof SVGFETurbulenceElement==='function'].join();";
    assert_eq!(eval(source, "r"), "true,true,true");
}
