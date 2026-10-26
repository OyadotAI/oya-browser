/** The HTML popover API in V9: the `popover` reflection, show/hide/toggle with Blink's
exceptions, beforetoggle and coalesced toggle events, the auto stack, and popovertarget. */
mod common;
use common::{console, environment, global, host, page};
use mars_v9::{DomLevel, PageInput, StorageState};

/** Run a script against the test page and return a global, failing on any script failure. */
fn eval(source: &str, name: &str) -> String {
    let host = host();
    let mut environment = environment();
    environment.dom_level = DomLevel::Extended;
    let execution = mars_v9::load_page(PageInput {
        sources: &[source.to_string()],
        document: page(),
        storage: StorageState::default(),
        host: &host,
        environment: &environment,
        modules: mars_v9::ModuleInput::NONE,
    })
    .expect("page loads");
    assert!(execution.failed.is_empty(), "{:?}", execution.failed);
    global(&execution, name)
}

#[test]
fn popover_attribute_reflects_auto_manual_hint_and_null() {
    let source = "const d=document.createElement('div');const a=[d.popover];d.popover='';a.push(d.popover);\
        d.setAttribute('popover','HINT');a.push(d.popover);d.setAttribute('popover','bogus');a.push(d.popover);\
        d.popover=null;a.push(d.hasAttribute('popover'));const r=JSON.stringify(a);";
    assert_eq!(
        eval(source, "r"),
        "[null,\"auto\",\"hint\",\"manual\",false]"
    );
}

#[test]
fn show_and_hide_throw_as_blink_and_do_not_throw_when_already_in_state() {
    let source = "const a=[];const t=(f)=>{try{f();a.push('ok')}catch(e){a.push(e.name)}};\
        const d=document.createElement('div');t(()=>d.showPopover());d.popover='manual';t(()=>d.showPopover());\
        document.body.append(d);t(()=>d.showPopover());t(()=>d.showPopover());t(()=>d.hidePopover());\
        t(()=>d.hidePopover());const r=a.join('|');";
    assert_eq!(
        eval(source, "r"),
        "NotSupportedError|InvalidStateError|ok|ok|ok|ok"
    );
}

#[test]
fn toggle_popover_honors_force_and_returns_the_new_state() {
    let source = "const d=document.createElement('div');d.popover='manual';document.body.append(d);\
        const r=[d.togglePopover(),d.togglePopover(true),d.togglePopover({force:false}),d.togglePopover(false),\
        d.togglePopover()].join('|');";
    assert_eq!(eval(source, "r"), "true|true|false|false|true");
}

#[test]
fn canceled_beforetoggle_keeps_the_popover_closed_and_states_are_reported() {
    let source = "const d=document.createElement('div');d.popover='';document.body.append(d);const a=[];\
        d.addEventListener('beforetoggle',e=>{a.push(e.oldState+'>'+e.newState+':'+e.cancelable);\
        if(a.length===1)e.preventDefault()});d.showPopover();a.push(d.matches?d.togglePopover(false):'');\
        d.showPopover();d.hidePopover();const r=a.join('|');";
    assert_eq!(
        eval(source, "r"),
        "closed>open:true|false|closed>open:true|open>closed:false"
    );
}

#[test]
fn toggle_events_are_queued_and_coalesced() {
    let sources = vec![
        "const d=document.createElement('div');d.popover='manual';document.body.append(d);\
        window.log='';d.addEventListener('toggle',e=>{log+=e.oldState+'>'+e.newState+';'});\
        d.showPopover();d.hidePopover();d.showPopover();console.log('sync:'+log);\
        setTimeout(()=>console.log('later:'+log),0);"
            .to_string(),
    ];
    let host = host();
    let mut environment = environment();
    environment.dom_level = DomLevel::Extended;
    let input = || PageInput {
        sources: &sources,
        document: page(),
        storage: StorageState::default(),
        host: &host,
        environment: &environment,
        modules: mars_v9::ModuleInput::NONE,
    };
    let loaded = mars_v9::load_page(input()).expect("page loads");
    let mut lines = console(&loaded);
    let ran = mars_v9::run_timers(input(), loaded.state, 10.0, 10).expect("timers run");
    lines.extend(console(&ran));
    assert_eq!(lines, ["sync:", "later:closed>open;"]);
}

#[test]
fn showing_an_auto_popover_hides_unrelated_auto_popovers_but_not_ancestors_or_manual() {
    let source = "const mk=(t,p)=>{const d=document.createElement('div');d.popover=t;(p||document.body).append(d);return d};\
        const open=x=>{let f=false;const h=e=>{f=true;e.preventDefault()};x.addEventListener('beforetoggle',h);\
        x.showPopover();x.removeEventListener('beforetoggle',h);return !f};\
        const a=mk('auto'),m=mk('manual'),b=mk('auto'),child=mk('auto',b);a.showPopover();m.showPopover();\
        b.showPopover();const s=[open(a),open(m),open(b)];child.showPopover();s.push(open(b),open(child));\
        a.showPopover();s.push(open(b),open(child),open(m),open(a));b.showPopover();child.showPopover();\
        b.hidePopover();s.push(open(child),open(a));const r=s.join();";
    assert_eq!(
        eval(source, "r"),
        "false,true,true,true,true,false,false,true,true,false,false"
    );
}

#[test]
fn popovertarget_buttons_toggle_show_and_hide_their_target() {
    let source = "const p=document.createElement('div');p.id='pop';p.popover='manual';document.body.append(p);\
        const b=document.createElement('button');b.setAttribute('popovertarget','pop');document.body.append(b);\
        const a=[b.popoverTargetElement===p,b.popoverTargetAction];b.click();a.push(p.togglePopover(true));\
        b.click();a.push(p.togglePopover(false));b.popoverTargetAction='hide';b.click();a.push(p.togglePopover(false));\
        b.popoverTargetAction='SHOW';a.push(b.popoverTargetAction);b.click();b.click();a.push(p.togglePopover(true));\
        b.addEventListener('click',e=>e.preventDefault());p.hidePopover();b.click();a.push(p.togglePopover(false));\
        const r=a.join('|');";
    assert_eq!(
        eval(source, "r"),
        "true|toggle|true|false|false|show|true|false"
    );
}

/** Opening and closing a popover leaves no attribute a page can see: its open state reaches
layout through an engine-only marker that attribute lists and serialization leave out. */
#[test]
fn open_state_is_invisible_to_the_page() {
    let source = "const d=document.createElement('div');d.popover='manual';document.body.append(d);\
        const before=d.getAttributeNames().join();d.showPopover();\
        const open=[d.getAttributeNames().join(),d.outerHTML,d.attributes.length,d.matches(':popover-open')];\
        d.hidePopover();const r=JSON.stringify([before,open,d.getAttributeNames().join()]);";
    assert_eq!(
        eval(source, "r"),
        "[\"popover\",[\"popover\",\"<div popover=\\\"manual\\\"></div>\",1,true],\"popover\"]"
    );
}
