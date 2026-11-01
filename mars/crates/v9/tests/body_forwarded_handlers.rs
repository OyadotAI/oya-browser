/** Body-forwarded window handlers in V9: `document.body.onload` and its siblings are the window's
own handler properties, so a handler set from script is the one dispatch runs, while other body
handlers stay on the body. */
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
fn body_onload_set_from_script_is_the_window_handler() {
    let source = "const f=()=>{};document.body.onload=f;const r=[window.onload===f,document.body.onload===f].join('|');";
    assert_eq!(eval(source, "r"), "true|true");
}

#[test]
fn window_onload_is_read_back_through_the_body() {
    let source = "const f=()=>{};window.onload=f;const r=String(document.body.onload===f);";
    assert_eq!(eval(source, "r"), "true");
}

#[test]
fn body_handlers_that_are_not_forwarded_stay_on_the_body() {
    let source = "const f=()=>{};document.body.onclick=f;const r=[window.onclick===undefined,document.body.onclick===f].join('|');";
    assert_eq!(eval(source, "r"), "true|true");
}
