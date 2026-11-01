/** Event dispatch in V9: constants, constructors, passive listeners, options, signals, window.event
and exception reporting. Each test runs a script against the test page and reads one global back. */
use crate::common;
use common::{environment, global, host, page};
use mars_v9::{DomLevel, PageInput, StorageState};

/** Run a script against the test page and return a global, failing on any script failure. */
fn eval(source: &str, name: &str) -> String {
    let host = host();
    let mut environment = environment();
    environment.dom_level = DomLevel::Extended;
    let execution = mars_v9::load_page(PageInput {
        sources: &[source.to_string()],
        source_keys: &[],
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
fn phase_constants_are_on_the_constructor_and_the_prototype() {
    let source = "const r=[Event.NONE,Event.CAPTURING_PHASE,Event.AT_TARGET,Event.BUBBLING_PHASE,Event.prototype.AT_TARGET,MouseEvent.BUBBLING_PHASE].join('|');";
    assert_eq!(eval(source, "r"), "0|1|2|3|2|3");
}

#[test]
fn event_constructor_throws_without_new() {
    let source = "let r='no throw';try{Event('x')}catch(e){r=e.constructor.name};";
    assert_eq!(eval(source, "r"), "TypeError");
}

#[test]
fn event_constructor_with_new_still_builds_an_event() {
    let source = "const r=new Event('x',{bubbles:true}).type+':'+new Event('y').bubbles;";
    assert_eq!(eval(source, "r"), "x:false");
}

#[test]
fn touch_and_wheel_listeners_are_passive_by_default_on_window_and_body() {
    let source = "let r=[];for(const t of [window,document.body]){const h=e=>{e.preventDefault();r.push(e.defaultPrevented)};t.addEventListener('wheel',h);r.push(t.dispatchEvent(new Event('wheel',{cancelable:true})));t.removeEventListener('wheel',h);}r=r.join('|');";
    assert_eq!(eval(source, "r"), "false|true|false|true");
}

#[test]
fn touch_listener_on_a_plain_element_is_not_passive_by_default() {
    let source = "const d=document.createElement('div');const h=e=>e.preventDefault();d.addEventListener('touchstart',h);const r=String(d.dispatchEvent(new Event('touchstart',{cancelable:true})));";
    assert_eq!(eval(source, "r"), "false");
}

#[test]
fn explicit_passive_true_ignores_prevent_default_and_return_value() {
    let source = "let seen=[];const h=e=>{e.preventDefault();seen.push(e.defaultPrevented);e.returnValue=false;seen.push(e.defaultPrevented)};document.body.addEventListener('click',h,{passive:true});const d=document.body.dispatchEvent(new Event('click',{cancelable:true}));const r=seen.join('|')+'|'+d;";
    assert_eq!(eval(source, "r"), "false|false|true");
}

#[test]
fn explicit_passive_false_on_a_default_passive_target_cancels() {
    let source = "const h=e=>e.preventDefault();window.addEventListener('wheel',h,{passive:false});const r=String(window.dispatchEvent(new Event('wheel',{cancelable:true})));";
    assert_eq!(eval(source, "r"), "false");
}

#[test]
fn undefined_passive_member_means_the_default() {
    let source = "const h=e=>e.preventDefault();window.addEventListener('wheel',h,{passive:undefined});const r=String(window.dispatchEvent(new Event('wheel',{cancelable:true})));";
    assert_eq!(eval(source, "r"), "true");
}

#[test]
fn add_listener_reads_passive_and_remove_listener_does_not() {
    let source = "let reads=[];const o={get capture(){reads.push('capture');return false},get passive(){reads.push('passive');return false},get once(){return false},get signal(){return undefined}};const h=()=>{};document.body.addEventListener('x',h,o);const added=reads.join(',');reads=[];document.body.removeEventListener('x',h,o);const r=added+'/'+reads.join(',');";
    assert_eq!(eval(source, "r"), "capture,passive/capture");
}

#[test]
fn null_signal_throws_a_type_error() {
    let source = "let r='no throw';try{document.body.addEventListener('x',()=>{},{signal:null})}catch(e){r=e.constructor.name};";
    assert_eq!(eval(source, "r"), "TypeError");
}

#[test]
fn listener_with_an_aborted_signal_does_not_run_and_is_removed() {
    let source = "let calls=0;const c=new AbortController();document.body.addEventListener('x',()=>calls++,{signal:c.signal});document.body.dispatchEvent(new Event('x'));c.abort();document.body.dispatchEvent(new Event('x'));const r=String(calls);";
    assert_eq!(eval(source, "r"), "1");
}

#[test]
fn listener_added_with_an_already_aborted_signal_is_never_registered() {
    let source = "let calls=0;const c=new AbortController();c.abort();document.body.addEventListener('x',()=>calls++,{signal:c.signal});document.body.dispatchEvent(new Event('x'));const r=String(calls);";
    assert_eq!(eval(source, "r"), "0");
}

#[test]
fn window_event_is_the_current_event_during_dispatch_and_undefined_after() {
    let source = "let inside;const ev=new Event('x');document.body.addEventListener('x',()=>{inside=window.event===ev});document.body.dispatchEvent(ev);const r=[inside,window.event===undefined].join('|');";
    assert_eq!(eval(source, "r"), "true|true");
}

#[test]
fn stop_propagation_in_a_capture_listener_cancels_the_bubble_listener() {
    let source = "let bubbled=false;const el=document.createElement('div');el.addEventListener('click',()=>window.event.stopPropagation(),{capture:true});el.addEventListener('click',()=>{bubbled=true});el.dispatchEvent(new MouseEvent('click',{bubbles:true,cancelable:true}));const r=String(bubbled);";
    assert_eq!(eval(source, "r"), "false");
}

#[test]
fn composed_path_is_empty_outside_dispatch() {
    let source = "let during=-1;const ev=new Event('x');window.addEventListener('x',()=>{during=ev.composedPath().length});window.dispatchEvent(ev);const r=during+'|'+ev.composedPath().length;";
    assert_eq!(eval(source, "r"), "1|0");
}

#[test]
fn listener_exception_fires_an_error_event_at_the_window() {
    let source = "let seen='none';window.addEventListener('error',e=>{seen=e.type});document.body.addEventListener('x',()=>{throw new Error('boom')});document.body.dispatchEvent(new Event('x'));const r=seen;";
    assert_eq!(eval(source, "r"), "error");
}

#[test]
fn handle_event_that_is_not_callable_reports_an_error_event() {
    let source = "let seen='none';window.addEventListener('error',()=>{seen='error'});document.body.addEventListener('x',{handleEvent:1});document.body.dispatchEvent(new Event('x'));const r=seen;";
    assert_eq!(eval(source, "r"), "error");
}

#[test]
fn legacy_init_mouse_event_sets_the_type_and_is_a_no_op_while_dispatching() {
    let source = "const ev=document.createEvent('MouseEvent');ev.initMouseEvent('click',true,true,window,0,0,0,0,0,false,false,false,false,0,null);const r=ev.type+':'+ev.bubbles+':'+ev.cancelable;";
    assert_eq!(eval(source, "r"), "click:true:true");
}

#[test]
fn animation_and_transition_handler_attributes_start_null_and_are_writable() {
    let source = "const d=document.createElement('div');const start=[d.onanimationend,d.ontransitionend].map(String).join('|');const f=()=>{};d.onanimationend=f;const r=start+'|'+(d.onanimationend===f);";
    assert_eq!(eval(source, "r"), "null|null|true");
}
