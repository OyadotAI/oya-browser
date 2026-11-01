/**
 * A navigation's commit point depends on who asked for it. The person's navigation in the visible
 * tab publishes once its load event has run, and the script fetches that load started answer the
 * page afterwards, through the idle schedule. An agent's navigation returns a settled page, as it
 * always has. Both are proven against a fetch the server holds for FETCH_HOLD.
 */
#[allow(dead_code)]
mod support;
use mars::protocol::Request;
use mars::runtime::authority::{Owner, Permit};
use serde_json::{Value, json};
use std::thread;
use std::time::{Duration, Instant};
use support::{Fixture, agent_call, desktop_as, html};

/** How long the server holds the script's fetch: well past a person's 150 ms settle, so their
navigation publishes first, and inside an agent's settle wall, so theirs waits for it. */
const FETCH_HOLD: Duration = Duration::from_millis(700);
/** The longest a test waits for the late answer to reach the page's inspection. */
const LATE_ANSWER_WAIT: Duration = Duration::from_secs(10);
/** Pause between idle runs while a test waits for the late answer. */
const IDLE_PAUSE: Duration = Duration::from_millis(20);

/** A page that paints `initial` at load and rewrites it once its fetch answers. */
fn held_fetch_fixture() -> Fixture {
    let page = "<p id=out>initial</p><script>fetch('/data.json').then(r=>r.text()).then(t=>{\
         document.getElementById('out').textContent='late '+t})</script>";
    let body = "answer";
    Fixture::start(
        Box::new(move |request| {
            let path = request.split_whitespace().nth(1).unwrap_or_default();
            match path {
                "/" => html("", page),
                "/data.json" => {
                    thread::sleep(FETCH_HOLD);
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .into_bytes()
                }
                _ => b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    .to_vec(),
            }
        }),
        false,
    )
}

/** One call as the person's window: control held by the human, on tab 1. */
fn human_call(
    runtime: &mut mars::runtime::Runtime,
    op: &str,
    mut params: Value,
) -> Result<Value, mars::protocol::EngineError> {
    params["tab_id"] = json!("tab-1");
    let permit = Permit {
        owner: Owner::Human,
        epoch: 1,
        tab_id: Some(1),
    };
    runtime.handle_as(Request::new("support", op, params), permit)
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

/** The person's navigation publishes at load: it returns before the script's fetch answers. */
#[test]
fn window_publishes_at_load_before_a_script_fetch_answers() {
    let fixture = held_fetch_fixture();
    let mut runtime = desktop_as(&fixture.url, Owner::Human);
    let started = Instant::now();
    human_call(&mut runtime, "page.navigate", json!({"url": fixture.url})).unwrap();
    assert!(
        started.elapsed() < FETCH_HOLD,
        "the person's navigation waited {:?} for a fetch held {FETCH_HOLD:?}",
        started.elapsed()
    );
    let inspection = human_call(&mut runtime, "page.inspect", json!({})).unwrap();
    let text = page_text(&inspection);
    assert!(text.contains("initial"), "{inspection}");
    assert!(!text.contains("late"), "{inspection}");
}

/** A late answer is never dropped: idle runs deliver it to the already-published page. */
#[test]
fn late_script_fetch_reaches_the_published_page() {
    let fixture = held_fetch_fixture();
    let mut runtime = desktop_as(&fixture.url, Owner::Human);
    human_call(&mut runtime, "page.navigate", json!({"url": fixture.url})).unwrap();
    let deadline = Instant::now() + LATE_ANSWER_WAIT;
    loop {
        runtime.run_page_scripts();
        let inspection = human_call(&mut runtime, "page.inspect", json!({})).unwrap();
        if page_text(&inspection).contains("late answer") {
            break;
        }
        assert!(Instant::now() < deadline, "no late answer: {inspection}");
        thread::sleep(IDLE_PAUSE);
    }
}

/** An agent's navigation still returns the settled page, answer included. */
#[test]
fn agent_navigation_returns_the_settled_page() {
    let fixture = held_fetch_fixture();
    let mut runtime = desktop_as(&fixture.url, Owner::Agent);
    agent_call(&mut runtime, "page.navigate", json!({"url": fixture.url})).unwrap();
    let inspection = agent_call(&mut runtime, "page.inspect", json!({})).unwrap();
    assert!(
        page_text(&inspection).contains("late answer"),
        "{inspection}"
    );
}
