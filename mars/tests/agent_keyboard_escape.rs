/** Real runtime Escape retains native page/revision/owner admission and honors page keydown cancellation. */
#[allow(dead_code)]
mod support;
use serde_json::{Value, json};
use support::{Fixture, agent_call, desktop};

/** A real served page records keyboard events and popover state from native callbacks. */
const PAGE: &str = "<!doctype html><button id='focus'>Focus</button><div id='pop' popover='auto'>Popover</div><script>let cancel=false;let keys=[];const p=document.getElementById('pop');p.showPopover();document.addEventListener('keydown',e=>{keys.push(e.key+':'+e.isTrusted);if(cancel)e.preventDefault()});document.addEventListener('keyup',()=>keys.push(p.matches(':popover-open')));</script>";

/** Read public page state through the native evaluator without implementing any input in script. */
fn state(runtime: &mut mars::runtime::Runtime) -> Value {
    agent_call(
        runtime,
        "page.devtools",
        json!({"request":{"kind":"evaluate","expression":"JSON.stringify(keys)"}}),
    )
    .unwrap()["result"]["value"]
        .clone()
}

/** The real input front door rejects wrong page/revision and closes the popover after trusted events. */
#[test]
fn escape_uses_native_input_and_retains_page_revision_checks() {
    let site = Fixture::raw(support::html("", PAGE));
    let mut runtime = desktop(&site.url);
    let loaded = agent_call(&mut runtime, "page.navigate", json!({"url":site.url})).unwrap();
    let at = agent_call(&mut runtime, "page.inspect", json!({})).unwrap();
    let wrong = agent_call(
        &mut runtime,
        "page.press",
        json!({"page_id":"page-wrong","expected_revision":at["revision"],"key":"Escape"}),
    )
    .unwrap_err();
    assert_eq!(wrong.code, "invalid_target");
    let stale = agent_call(
        &mut runtime,
        "page.press",
        json!({"page_id":loaded["page_id"],"expected_revision":at["revision"].as_u64().unwrap()+1,"key":"Escape"}),
    )
    .unwrap_err();
    assert_eq!(stale.code, "revision_conflict");
    agent_call(
        &mut runtime,
        "page.press",
        json!({"page_id":at["page_id"],"expected_revision":at["revision"],"key":"Escape"}),
    )
    .unwrap();
    assert_eq!(state(&mut runtime), json!("[\"Escape:true\",false]"));
}

/** Canceling keydown is respected, and releasing human control refuses another press before dispatch. */
#[test]
fn escape_respects_keydown_cancellation_and_human_ownership() {
    let site = Fixture::raw(support::html("", PAGE));
    let mut runtime = desktop(&site.url);
    agent_call(&mut runtime, "page.navigate", json!({"url":site.url})).unwrap();
    agent_call(
        &mut runtime,
        "page.devtools",
        json!({"request":{"kind":"evaluate","expression":"cancel=true"}}),
    )
    .unwrap();
    let at = agent_call(&mut runtime, "page.inspect", json!({})).unwrap();
    agent_call(
        &mut runtime,
        "page.press",
        json!({"page_id":at["page_id"],"expected_revision":at["revision"],"key":"Escape"}),
    )
    .unwrap();
    assert_eq!(state(&mut runtime), json!("[\"Escape:true\",true]"));
    agent_call(&mut runtime, "control.release", json!({})).unwrap();
    assert!(
        agent_call(
            &mut runtime,
            "page.press",
            json!({"page_id":at["page_id"],"expected_revision":at["revision"],"key":"Escape"})
        )
        .is_err()
    );
}

/** Reattaching a parsed element materializes implicit leading text without discarding element identity or its popover events. */
#[test]
fn removed_parsed_text_element_can_reopen_on_an_animation_frame() {
    let page = "<!doctype html><div id='pop'>popover</div><script>const p=document.getElementById('pop');p.popover='manual';p.showPopover();let r='pending';p.addEventListener('beforetoggle',e=>{if(e.newState==='closed'){p.remove();requestAnimationFrame(()=>{document.body.append(p);p.showPopover();r=p.popover+','+p.matches(':popover-open')+','+p.textContent+','+p.firstChild.nodeType+','+(document.getElementById('pop')===p)})}},{once:true});p.popover='auto';</script>";
    let site = Fixture::raw(support::html("", page));
    let mut runtime = desktop(&site.url);
    agent_call(&mut runtime, "page.navigate", json!({"url":site.url})).unwrap();
    let result = agent_call(
        &mut runtime,
        "page.devtools",
        json!({"request":{"kind":"evaluate","expression":"r"}}),
    )
    .unwrap();
    assert_eq!(
        result["result"]["value"],
        json!("auto,true,popover,3,true"),
        "{result}"
    );
}
