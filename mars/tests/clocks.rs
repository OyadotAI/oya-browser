/** Native page clocks advance during a task through the browser's ordinary environment setup. */
#[allow(dead_code)]
mod support;
use serde_json::json;
use support::{Fixture, agent_call, desktop, html};

/** Native navigation must opt into live time without changing the standalone VM default. */
#[test]
fn page_clocks_advance_within_one_javascript_task() {
    let fixture = Fixture::routes(vec![(
        "/",
        html(
            "",
            "<script>const start=performance.now(),date=Date.now();while(performance.now()-start<2){}const result=performance.now()>start&&Date.now()>date;</script>",
        ),
    )]);
    let mut runtime = desktop(&fixture.url);
    agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    let result = agent_call(&mut runtime, "page.devtools_peek", json!({"request":{"kind":"evaluate","expression":"result","options":{"return_by_value":true}}})).unwrap();
    assert_eq!(result["result"]["value"], true);
}
