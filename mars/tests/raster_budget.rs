/** The windowless painter's glyph work stays bounded: a run off the canvas allocates no bitmap,
and one screenshot's glyph pixels have a total budget, so thousands of large runs cannot stall
or exhaust memory. */
#[allow(dead_code)]
mod support;
use serde_json::json;
use std::time::{Duration, Instant};
use support::{Fixture, agent_call, desktop, html};

/** Large off-canvas text runs on the hostile page. */
const RUNS: usize = 2_000;
/** The screenshot bound, generous for debug builds on a loaded machine. */
const LIMIT: Duration = Duration::from_secs(5);

/** Thousands of large runs far below the viewport screenshot quickly. */
#[test]
fn off_canvas_runs_cost_no_glyph_work() {
    let run = format!("<p>{}</p>", "W".repeat(60));
    let page = format!(
        "<style>body{{margin:0;font:20px/1 monospace}}#far{{position:absolute;top:60000px;\
         font:120px/1 monospace}}</style>XXXX<div id=far>{}</div>",
        run.repeat(RUNS)
    );
    let page: &'static str = Box::leak(page.into_boxed_str());
    let fixture = Fixture::routes(vec![("/", html("", page))]);
    let mut runtime = desktop(&fixture.url);
    agent_call(
        &mut runtime,
        "page.emulate",
        json!({"viewport": {"width": 400.0, "height": 300.0}}),
    )
    .unwrap();
    agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    let started = Instant::now();
    let shot = agent_call(&mut runtime, "page.screenshot", json!({})).unwrap();
    let elapsed = started.elapsed();
    assert!(elapsed < LIMIT, "screenshot took {elapsed:?}");
    assert!(
        shot["png_base64"]
            .as_str()
            .is_some_and(|png| !png.is_empty())
    );
}
