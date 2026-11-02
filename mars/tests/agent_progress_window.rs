/**
 * An agent-controlled page keeps running its background work while that work changes the page.
 * A quiet agent page sleeps TICK_WINDOW after the agent's last action, but a page whose scripts
 * keep changing the document (a booting app, a timer chain) must not freeze at that point: a
 * browser never pauses a page's timers because an agent is driving it. The cap that bounds an
 * endless animation is AGENT_AWAKE_CAP and is not exercised here (it would take minutes).
 */
#[allow(dead_code)]
mod support;
use mars::runtime::authority::Owner;
use serde_json::{Value, json};
use std::thread;
use std::time::{Duration, Instant};
use support::{Fixture, agent_call, desktop_as, html};

/** How long the test watches: past the 20 s quiet window, so a page without progress sleeps. */
const WATCH: Duration = Duration::from_secs(25);
/** When the counter is first read: after the quiet window has run out for an idle page. */
const FIRST_READ: Duration = Duration::from_secs(21);
/** Pause between background runs while the test watches. */
const IDLE_PAUSE: Duration = Duration::from_millis(20);

/** A page whose interval rewrites a counter every quarter second: real progress, no network. */
fn counting_fixture() -> Fixture {
    let page = "<title>count</title><p id=c>0</p><script>\
         var n = 0;\
         setInterval(function () { n++; document.getElementById('c').textContent = 'count ' + n; }, 250);\
         </script>";
    Fixture::start(Box::new(move |_| html("", page)), false)
}

/** The text of every node in the page's inspection, joined. */
fn page_text(inspection: &Value) -> String {
    inspection["nodes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|node| node["text"].as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

/** The counter the page shows, read from its inspection. */
fn counter(runtime: &mut mars::runtime::Runtime) -> u64 {
    let inspection = agent_call(runtime, "page.inspect", json!({})).unwrap();
    page_text(&inspection)
        .split_whitespace()
        .filter_map(|word| word.parse::<u64>().ok())
        .next_back()
        .unwrap_or(0)
}

/** Past the quiet window, a page whose timers keep changing it still advances. */
#[test]
fn agent_page_keeps_running_while_its_timers_change_the_document() {
    let fixture = counting_fixture();
    let mut runtime = desktop_as(&fixture.url, Owner::Agent);
    agent_call(&mut runtime, "page.navigate", json!({"url": fixture.url})).unwrap();
    let started = Instant::now();
    let mut at_first_read = None;
    while started.elapsed() < WATCH {
        runtime.run_page_scripts();
        if at_first_read.is_none() && started.elapsed() >= FIRST_READ {
            at_first_read = Some(counter(&mut runtime));
        }
        thread::sleep(IDLE_PAUSE);
    }
    let first = at_first_read.expect("the first read happened");
    let last = counter(&mut runtime);
    // Four ticks a second: well over a second of progress must show between the two reads.
    assert!(
        last >= first + 4,
        "the page froze after its quiet window: {first} at {FIRST_READ:?}, {last} at {WATCH:?}"
    );
}
