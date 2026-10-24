/** Platform APIs real pages reach for at load: document.fonts, structuredClone, bare window calls. */
mod common;
use common::eval;

/** Google preloads emoji fonts through document.fonts.load(...).catch(...). */
#[test]
fn document_fonts_is_a_loaded_font_face_set() {
    let source = "let r='';document.fonts.load('10pt Noto').then(f=>{r+=f.length});const s=[document.fonts.status,document.fonts.check('10pt x'),typeof document.fonts.ready.then].join();";
    assert_eq!(eval(source, "s"), "loaded,true,function");
}

/** structuredClone copies data deeply, keeps cycles and refuses functions with DataCloneError. */
#[test]
fn structured_clone_copies_data_and_refuses_functions() {
    let source = "const a={n:1,list:[1,{x:2}],when:new Date(5),m:new Map([[1,{y:3}]]),s:new Set(['q'])};a.self=a;\
        const c=structuredClone(a);let e='';try{structuredClone({f(){}})}catch(x){e=x.name}\
        const r=[c!==a,c.self===c,c.list[1].x,c.list!==a.list,c.when.getTime(),c.m.get(1).y,c.s.has('q'),e].join();";
    assert_eq!(
        eval(source, "r"),
        "true,true,2,true,5,3,true,DataCloneError"
    );
}

/** A bare addEventListener call (no receiver) registers on the window, as CNN's inline code does. */
#[test]
fn bare_window_event_target_calls_use_the_window() {
    let source = "let hit=0;function on(){hit++}addEventListener('ping',on);dispatchEvent(new Event('ping'));removeEventListener('ping',on);dispatchEvent(new Event('ping'));const r=hit;";
    assert_eq!(eval(source, "r"), "1");
}

/** Element.animate is absent so Web Animations polyfills install their own. */
#[test]
fn element_animate_is_left_for_polyfills() {
    assert_eq!(
        eval("const r=typeof document.createElement('div').animate;", "r"),
        "undefined"
    );
}

/** AbortSignal is a real interface: abort() fires `abort`, sets reason, and statics exist. */
#[test]
fn abort_signal_fires_and_reports_its_reason() {
    let source = "const c=new AbortController();let seen='';c.signal.addEventListener('abort',()=>{seen+='e'});c.signal.onabort=()=>{seen+='h'};c.abort();\
        const r=[c.signal instanceof AbortSignal,c.signal.aborted,c.signal.reason.name,seen,AbortSignal.abort('x').reason,typeof AbortSignal.timeout(5).aborted,AbortSignal.any([c.signal]).aborted].join();";
    assert_eq!(eval(source, "r"), "true,true,AbortError,eh,x,boolean,true");
}

/** WeakRef derefs to its target; FinalizationRegistry accepts registrations. */
#[test]
fn weak_refs_deref_and_registries_register() {
    let source = "const o={n:1},w=new WeakRef(o),f=new FinalizationRegistry(()=>{});f.register(o,1);const r=[w.deref()===o,Object.keys(w).length].join();";
    assert_eq!(eval(source, "r"), "true,0");
}

/** CSS.escape follows CSSOM and CSS.supports lets pages take their modern paths. */
#[test]
fn css_namespace_escapes_identifiers() {
    let source = "const r=[CSS.escape('a.b'),CSS.escape('1x'),CSS.escape('-'),CSS.escape('ok_-1'),CSS.supports('display','grid')].join(' ');";
    assert_eq!(eval(source, "r"), "a\\.b \\31 x \\- ok_-1 true");
}

/** Lazy loaders see every observed target intersect once, after the current task; resize
observers get their first observation; idle callbacks get a deadline. */
#[test]
fn observers_deliver_first_observations_and_idle_callbacks_get_deadlines() {
    let sources = vec!["const img=document.getElementById('first');new IntersectionObserver((entries,o)=>{console.log('io '+entries[0].isIntersecting+' '+(entries[0].target===img)+' '+(o instanceof IntersectionObserver))}).observe(img);\
        new ResizeObserver(e=>console.log('ro '+e[0].contentRect.width+' '+e[0].borderBoxSize[0].inlineSize)).observe(img);\
        requestIdleCallback(d=>console.log('idle '+(d.timeRemaining()>0)+' '+d.didTimeout));console.log('sync');".to_string()];
    let environment = common::environment();
    let host = common::host();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert_eq!(common::console(&loaded), ["sync"]);
    let ran = mars_v9::run_timers(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        5.0,
        10,
    )
    .unwrap();
    assert_eq!(
        common::console(&ran),
        ["io true true true", "ro 0 0", "idle true false"]
    );
}
