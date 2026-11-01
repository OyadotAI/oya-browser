/**
 * A task that runs out of wall-clock time is dropped like an uncaught exception, not like a crash:
 * the page keeps its scripts and the tasks queued behind it still run. A page that runs out of
 * time on every task is not making progress, so its scripts stop after a few such steps.
 * Both are proven with a short script deadline, so a busy loop reaches it in milliseconds.
 */
#[allow(dead_code)]
mod support;
use mars::protocol::Request;
use mars::runtime::authority::Owner;
use serde_json::{Value, json};
use std::sync::Once;
use std::thread;
use std::time::{Duration, Instant};
use support::{Fixture, desktop_as, html};

/** The script deadline the tests run under: short enough that a busy loop reaches it at once. */
const SCRIPT_DEADLINE_MS: &str = "300";
/** The longest a test waits for the page's later task to run. */
const TASK_WAIT: Duration = Duration::from_secs(20);
/** Pause between idle runs while a test waits for a task. */
const IDLE_PAUSE: Duration = Duration::from_millis(20);
/** The title the later task sets, proving it ran. */
const LATER_TITLE: &str = "later task ran";

/** Sets the short script deadline once, before any page in this test process runs a script. */
fn short_deadline() {
    static SET: Once = Once::new();
    SET.call_once(|| unsafe {
        std::env::set_var("MARS_V9_SCRIPT_DEADLINE_MS", SCRIPT_DEADLINE_MS);
    });
}

/** A page whose first timer spins past the deadline; `later` is the script after the spin. */
fn spinning_page(later: &str) -> Fixture {
    let page = format!(
        "<title>start</title><script>\
         setTimeout(function () {{ var end = Date.now() + 60000; while (Date.now() < end) {{}} }}, 0);\
         {later}\
         </script>"
    );
    Fixture::start(Box::new(move |_| html("", &page)), false)
}

/** One call as the person's window: control held by the human, on tab 1. */
fn human_call(runtime: &mut mars::runtime::Runtime, op: &str, url: Option<&str>) -> Value {
    let permit = mars::runtime::authority::Permit {
        owner: Owner::Human,
        epoch: 1,
        tab_id: Some(1),
    };
    let params = match url {
        Some(url) => json!({"tab_id": "tab-1", "url": url}),
        None => json!({"tab_id": "tab-1"}),
    };
    runtime
        .handle_as(Request::new("support", op, params), permit)
        .unwrap()
}

/** Opens  in the person's tab and returns the window's runtime. */
fn open(url: &str) -> mars::runtime::Runtime {
    let mut runtime = desktop_as(url, Owner::Human);
    human_call(&mut runtime, "page.navigate", Some(url));
    runtime
}

/** The later timer runs after a task that ran out of time: its title lands, scripts stay on. */
#[test]
fn task_that_runs_out_of_time_is_dropped_and_the_next_task_runs() {
    short_deadline();
    let fixture = spinning_page(&format!(
        "setTimeout(function () {{ document.title = '{LATER_TITLE}'; }}, 0);"
    ));
    let mut runtime = open(&fixture.url);
    let started = Instant::now();
    loop {
        runtime.run_page_scripts();
        let inspection = human_call(&mut runtime, "page.inspect", None);
        assert!(
            inspection["degraded"]
                .as_array()
                .is_none_or(|degraded| degraded.is_empty()),
            "scripts stopped after one overrun: {inspection}"
        );
        if inspection["title"] == LATER_TITLE {
            break;
        }
        assert!(started.elapsed() < TASK_WAIT, "no later task: {inspection}");
        thread::sleep(IDLE_PAUSE);
    }
}

/** A page that runs out of time on every task is stopped rather than spinning forever. */
#[test]
fn page_that_overruns_every_task_has_its_scripts_stopped() {
    short_deadline();
    let fixture = Fixture::start(
        Box::new(move |_| {
            html(
                "",
                "<title>start</title><script>\
                 function spin() { setTimeout(spin, 0); var end = Date.now() + 60000; \
                 while (Date.now() < end) {} }\
                 setTimeout(spin, 0);\
                 </script>",
            )
        }),
        false,
    );
    let mut runtime = open(&fixture.url);
    let started = Instant::now();
    loop {
        runtime.run_page_scripts();
        let inspection = human_call(&mut runtime, "page.inspect", None);
        let stopped = inspection["degraded"]
            .as_array()
            .is_some_and(|degraded| degraded.iter().any(|flag| flag == "scripts"));
        if stopped {
            break;
        }
        assert!(
            started.elapsed() < TASK_WAIT,
            "scripts never stopped: {inspection}"
        );
        thread::sleep(IDLE_PAUSE);
    }
}
