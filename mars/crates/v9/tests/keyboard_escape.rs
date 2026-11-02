/** Native Escape dispatches trusted keyboard events and only an uncanceled top popover close action. */
use crate::common;
use mars_v9::{DomLevel, KeyboardKey, PageInput, VmState};

/** The host projection already contains popover attributes before script state is committed. */
fn input<'a>(
    sources: &'a [String],
    host: &'a mars_v9::DocumentHost<'a>,
    environment: &'a mars_v9::PageEnvironment,
) -> PageInput<'a> {
    let mut document = common::page();
    document.nodes[0]
        .attributes
        .insert("popover".into(), "auto".into());
    document.nodes[1]
        .attributes
        .insert("popover".into(), "hint".into());
    document.nodes[2]
        .attributes
        .insert("popover".into(), "manual".into());
    PageInput {
        document,
        ..common::input(sources, host, environment)
    }
}

/** Run keyboard behavior with an optional serialized state round trip before native input. */
fn press(source: &str, transport: bool) -> String {
    let sources = vec![source.to_owned()];
    let host = common::host();
    let mut environment = common::environment();
    environment.dom_level = DomLevel::Extended;
    let loaded = mars_v9::load_page(input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let state: VmState = if transport {
        common::round_trip(loaded.state)
    } else {
        loaded.state
    };
    let ran = mars_v9::keyboard_page(
        input(&sources, &host, &environment),
        state,
        KeyboardKey::Escape,
        Some(3),
    )
    .unwrap();
    assert!(ran.failed.is_empty(), "{:?}", common::failure(&ran));
    common::global(&ran, "r")
}

/** Key events reach actual focus with native metadata and close only the newest registered popover. */
#[test]
fn escape_has_trusted_metadata_and_closes_only_the_top_auto_or_hint() {
    let source = "const p=document.getElementById('main'),h=document.getElementById('first'),m=document.querySelector('.note'),b=document.getElementById('go');p.showPopover();h.showPopover();m.showPopover();b.focus();let r='';b.addEventListener('keydown',e=>{r+=e.type+','+e.key+','+e.code+','+e.isTrusted+','+e.cancelable+','+(e instanceof KeyboardEvent)+','+(e.target===b)+','+h.matches(':popover-open')});b.addEventListener('keyup',e=>{r+='|'+e.type+','+e.isTrusted+','+p.matches(':popover-open')+','+h.matches(':popover-open')+','+m.matches(':popover-open')});";
    assert_eq!(
        press(source, false),
        "keydown,Escape,Escape,true,true,true,true,true|keyup,true,true,false,true"
    );
}

/** preventDefault on the real keydown suppresses the native close action while preserving keyup. */
#[test]
fn canceled_keydown_keeps_the_popover_open() {
    assert_eq!(
        press(
            "const p=document.getElementById('main'),b=document.getElementById('go');p.showPopover();b.focus();let r='';b.addEventListener('keydown',e=>{e.preventDefault();r+=e.defaultPrevented});b.addEventListener('keyup',()=>{r+=','+p.matches(':popover-open')});",
            false
        ),
        "true,true"
    );
}

/** Manual popovers register no native Escape close watcher. */
#[test]
fn escape_leaves_manual_popovers_open() {
    assert_eq!(
        press(
            "const p=document.querySelector('.note'),b=document.getElementById('go');p.showPopover();b.focus();let r='';b.addEventListener('keyup',()=>{r=p.matches(':popover-open')});",
            false
        ),
        "true"
    );
}

/** A state transport keeps real focus and private open-type metadata for subsequent trusted input. */
#[test]
fn escape_operates_on_restored_focus_and_popover_state() {
    assert_eq!(
        press(
            "const p=document.getElementById('main'),b=document.getElementById('go');p.showPopover();b.focus();let r='';b.addEventListener('keyup',()=>{r=!p.matches(':popover-open')});",
            true
        ),
        "true"
    );
}

/** The public keyboard boundary refuses a stale target rather than redirecting focus. */
#[test]
fn unrelated_keyboard_target_is_rejected() {
    let sources = vec!["document.getElementById('go').focus();".to_owned()];
    let host = common::host();
    let mut environment = common::environment();
    environment.dom_level = DomLevel::Extended;
    let loaded = mars_v9::load_page(input(&sources, &host, &environment)).unwrap();
    let error = mars_v9::keyboard_page(
        input(&sources, &host, &environment),
        loaded.state,
        KeyboardKey::Escape,
        Some(1),
    )
    .err()
    .unwrap();
    assert_eq!(error.code, "invalid_target");
}

/** Page-authored event dispatch never gains the default action reserved to native input. */
#[test]
fn synthetic_keydown_does_not_close_popovers() {
    let mut environment = common::environment();
    environment.dom_level = DomLevel::Extended;
    let execution = common::load_with(
        "const p=document.getElementById('first');p.popover='auto';p.showPopover();document.body.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true}));let r=p.matches(':popover-open');",
        &environment,
    );
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "true");
}
