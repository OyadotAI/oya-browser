/** Browser globals: window, navigator, location, history, screen, console, cookies, storage, timers. */
mod common;
use common::{console, eval, input, load, load_with};
use mars_v9::{PageEvent, StorageState};

/** window, self and globalThis are the global object; var declarations land on it. */
#[test]
fn window_aliases_are_the_global_object() {
    let source = "var v=1;const r=[window===globalThis,self===window,window.window===window,window.v,top===window,typeof document,document.defaultView===window].join();";
    assert_eq!(eval(source, "r"), "true,true,true,1,true,object,true");
}

/** navigator reports the host's user agent and languages. */
#[test]
fn navigator_reports_host_facts() {
    let mut environment = common::environment();
    environment.user_agent = "TestAgent/1".into();
    let languages = vec!["fr-FR".to_string(), "en".to_string()];
    let host = mars_v9::DocumentHost {
        languages: &languages,
        fragments: &mars_v9::SimpleFragmentParser,
    };
    let source = "const r=[navigator.userAgent,navigator.language,navigator.languages.join('+'),navigator.cookieEnabled,navigator.onLine,typeof navigator.platform].join();".to_string();
    let execution =
        mars_v9::load_page(input(std::slice::from_ref(&source), &host, &environment)).unwrap();
    assert_eq!(
        common::global(&execution, "r"),
        "TestAgent/1,fr-FR,fr-FR+en,true,true,string"
    );
}

/** location exposes the URL's parts; assigning it records a navigation without performing it. */
#[test]
fn location_parts_and_assignment_record_navigation() {
    let source = "const r=[location.href,location.protocol,location.host,location.hostname,location.pathname,location.search,location.hash,location.origin].join(' ');location.href='/next?q=2';";
    let execution = load(source);
    assert_eq!(
        common::global(&execution, "r"),
        "https://example.com/app/page?x=1#top https: example.com example.com /app/page ?x=1 #top https://example.com"
    );
    let navigation = execution.effects.navigation.expect("navigation recorded");
    assert_eq!(navigation.kind, "assign");
    assert_eq!(navigation.url, "https://example.com/next?q=2");
}

/** Hash changes stay in the document, update location and enter history. */
#[test]
fn hash_assignment_is_same_document() {
    let execution = load("location.hash='section';const r=location.href;");
    assert_eq!(
        common::global(&execution, "r"),
        "https://example.com/app/page?x=1#section"
    );
    assert!(execution.effects.navigation.is_none());
    assert_eq!(execution.effects.history.len(), 1);
}

/** pushState and replaceState update location and are recorded; cross-origin URLs throw. */
#[test]
fn history_state_is_recorded() {
    let source = "history.pushState({n:1},'t','/a');history.replaceState(null,'','/b?z');let e;try{history.pushState(null,'','https://other.example/')}catch(x){e=x.message}\
        const r=[location.pathname+location.search,history.length,e.includes('SecurityError')].join();";
    let execution = load(source);
    assert_eq!(common::global(&execution, "r"), "/b?z,2,true");
    let urls: Vec<&str> = execution
        .effects
        .history
        .iter()
        .map(|record| record.url.as_str())
        .collect();
    assert_eq!(urls, ["https://example.com/a", "https://example.com/b?z"]);
    assert!(execution.effects.history[1].replace);
}

/** screen, innerWidth, innerHeight and devicePixelRatio come from the environment. */
#[test]
fn viewport_facts_come_from_the_host() {
    let mut environment = common::environment();
    environment.viewport_width = 800.0;
    environment.viewport_height = 600.0;
    environment.device_pixel_ratio = 1.5;
    let execution = load_with(
        "const r=[innerWidth,innerHeight,devicePixelRatio,screen.width>0,screen.colorDepth].join();",
        &environment,
    );
    assert_eq!(common::global(&execution, "r"), "800,600,1.5,true,24");
}

/** console output is captured with levels and bounded. */
#[test]
fn console_output_is_captured_and_bounded() {
    let execution = load(
        "console.log('a',1,{b:[2]});console.warn('w');console.error(new TypeError('t'));for(let i=0;i<100;i++)console.info(i);",
    );
    assert_eq!(
        &console(&execution)[..3],
        ["a 1 {b: [2]}", "w", "TypeError: t"]
    );
    assert_eq!(execution.effects.console[1].level, "warn");
    assert_eq!(
        execution.effects.console.len(),
        mars_v9::constants::VM_CONSOLE_ENTRIES
    );
    assert!(execution.effects.console_dropped > 0);
}

/** performance.now reads the virtual clock, which only the app advances. */
#[test]
fn performance_now_is_virtual() {
    assert_eq!(
        eval(
            "const r=[performance.now(),performance.timeOrigin].join();",
            "r"
        ),
        "0,1700000000000"
    );
}

/** document.cookie reads the host's string; writes are recorded for the jar and mirrored. */
#[test]
fn document_cookie_reads_and_records_writes() {
    let mut environment = common::environment();
    environment.cookie = "a=1; b=2".into();
    let source =
        "document.cookie='c=3; Path=/';document.cookie='a=; Max-Age=0';const r=document.cookie;";
    let execution = load_with(source, &environment);
    assert_eq!(common::global(&execution, "r"), "b=2; c=3");
    assert_eq!(execution.effects.cookies, ["c=3; Path=/", "a=; Max-Age=0"]);
}

/** localStorage and sessionStorage write the coordinator's candidates. */
#[test]
fn storage_writes_go_to_the_candidates() {
    let execution = load(
        "localStorage.setItem('k','v');sessionStorage.s='1';localStorage.removeItem('gone');const r=[localStorage.getItem('k'),localStorage.length,sessionStorage.getItem('s'),localStorage.getItem('none')].join();",
    );
    assert_eq!(common::global(&execution, "r"), "v,1,1,");
    assert_eq!(execution.storage.local["k"], "v");
    assert_eq!(execution.storage.session["s"], "1");
}

/** Timers wait for run_timers; due callbacks run in order, then microtasks, on the virtual clock. */
#[test]
fn timers_run_on_the_virtual_clock() {
    let sources = vec!["setTimeout(()=>{console.log('t20',performance.now());Promise.resolve().then(()=>console.log('micro'))},20);\
        setTimeout(()=>console.log('t10'),10);const id=setInterval(()=>{console.log('tick');if(++window.n>=2)clearInterval(id)},5);window.n=0;\
        requestAnimationFrame(t=>console.log('frame',t));".to_string()];
    let environment = common::environment();
    let host = common::host();
    let loaded = mars_v9::load_page(input(&sources, &host, &environment)).unwrap();
    assert!(console(&loaded).is_empty());
    assert_eq!(loaded.effects.next_timer_ms, Some(5.0));
    let state = common::round_trip(loaded.state);
    let ran = mars_v9::run_timers(input(&sources, &host, &environment), state, 30.0, 100).unwrap();
    // The interval's second tick was scheduled after the 10 ms timeout, so the timeout fires first.
    assert_eq!(
        console(&ran),
        ["tick", "t10", "tick", "frame 16", "t20 20", "micro"]
    );
    assert_eq!(ran.effects.next_timer_ms, None);
}

/** The callback budget leaves later timers pending for the next call. */
#[test]
fn timer_budget_limits_one_call() {
    let sources = vec!["for(let i=0;i<5;i++)setTimeout(()=>console.log(i),i);".to_string()];
    let environment = common::environment();
    let host = common::host();
    let loaded = mars_v9::load_page(input(&sources, &host, &environment)).unwrap();
    let ran =
        mars_v9::run_timers(input(&sources, &host, &environment), loaded.state, 100.0, 2).unwrap();
    assert_eq!(console(&ran), ["0", "1"]);
    assert_eq!(ran.effects.next_timer_ms, Some(2.0));
}

/** DOMContentLoaded and load fire when the app asks, with readyState moving forward. */
#[test]
fn lifecycle_events_follow_ready_state() {
    let sources = vec!["console.log(document.readyState);document.addEventListener('DOMContentLoaded',()=>console.log('dcl',document.readyState));\
        window.addEventListener('load',()=>console.log('load',document.readyState));window.onload=()=>console.log('onload');".to_string()];
    let environment = common::environment();
    let host = common::host();
    let loaded = mars_v9::load_page(input(&sources, &host, &environment)).unwrap();
    assert_eq!(console(&loaded), ["loading"]);
    let ready = mars_v9::page_event(
        input(&sources, &host, &environment),
        loaded.state,
        PageEvent::DomContentLoaded,
    )
    .unwrap();
    assert_eq!(console(&ready), ["dcl interactive"]);
    let done = mars_v9::page_event(
        input(&sources, &host, &environment),
        ready.state,
        PageEvent::Load,
    )
    .unwrap();
    assert_eq!(console(&done), ["load complete", "onload"]);
}

/** `<body onload="...">` is the window's load handler, as HTML forwards it; WPT's reftests end
their wait with it. */
#[test]
fn body_onload_attribute_runs_on_window_load() {
    let sources = vec!["console.log('script')".to_string()];
    let mut environment = common::environment();
    environment.body_attributes.insert(
        "onload".into(),
        "console.log('body onload', event.type)".into(),
    );
    let host = common::host();
    let loaded = mars_v9::load_page(input(&sources, &host, &environment)).unwrap();
    let done = mars_v9::page_event(
        input(&sources, &host, &environment),
        loaded.state,
        PageEvent::Load,
    )
    .unwrap();
    assert_eq!(console(&done), ["body onload load"]);
}

/** A script error inside a listener is reported, and other listeners still run. */
#[test]
fn listener_errors_are_reported_not_fatal() {
    let sources = vec!["window.addEventListener('load',()=>{throw new Error('bad')});window.addEventListener('load',()=>console.log('second'));".to_string()];
    let environment = common::environment();
    let host = common::host();
    let loaded = mars_v9::load_page(input(&sources, &host, &environment)).unwrap();
    let done = mars_v9::page_event(
        input(&sources, &host, &environment),
        loaded.state,
        PageEvent::Load,
    )
    .unwrap();
    let lines = console(&done);
    // The location names the script; an inline one by its document's address, as in Chrome.
    assert!(
        lines[0]
            .starts_with("Uncaught Error: bad (https://example.com/app/page?x=1 line 1, column"),
        "{}",
        lines[0]
    );
    assert_eq!(lines[1], "second");
}

/** crypto.randomUUID uses the OS generator: the same seed still gives different version-4 UUIDs. */
#[test]
fn random_uuid_is_unpredictable_and_well_formed() {
    let mut environment = common::environment();
    environment.random_seed = 7;
    let first = common::global(
        &load_with("const r=crypto.randomUUID();", &environment),
        "r",
    );
    let second = common::global(
        &load_with("const r=crypto.randomUUID();", &environment),
        "r",
    );
    assert_ne!(first, second);
    for uuid in [&first, &second] {
        let bytes: Vec<char> = uuid.chars().collect();
        assert_eq!(bytes.len(), 36);
        assert_eq!(bytes[14], '4', "{uuid}");
        assert!(matches!(bytes[19], '8' | '9' | 'a' | 'b'), "{uuid}");
    }
}

/** Native shared-memory APIs are present while the unsupported WebSocket surface remains absent. */
#[test]
fn supported_and_absent_platform_apis_are_distinct() {
    assert_eq!(
        eval(
            "const r=[typeof Atomics,typeof SharedArrayBuffer,typeof WebSocket,typeof fetch,typeof XMLHttpRequest].join();",
            "r"
        ),
        "object,function,undefined,function,function"
    );
}

/** Storage a script wrote before it failed stays written, as in browsers. */
#[test]
fn failed_scripts_keep_the_storage_they_wrote() {
    let execution = load("localStorage.setItem('k','v');throw new Error('stop');");
    assert_ne!(execution.storage, StorageState::default());
    assert_eq!(execution.failed.len(), 1);
}
