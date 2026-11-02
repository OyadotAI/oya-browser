/** The CDP front door drives a real headless Mars runtime through native operations only. */
#[allow(dead_code)]
mod support;
use serde_json::json;
use std::sync::Arc;
use support::{Client, Stack};

/** A form whose field submits to /done. */
const FORM: &str = r#"<title>Form</title><h1>Form</h1><form action="/done"><input id="name" name="name" aria-label="Name"><button>Go</button></form>"#;

/** The fixture: the form, and /done after it. */
fn stack() -> Stack {
    Stack::start(Arc::new(|path: &str| {
        if path.starts_with("/done") {
            "<title>Done</title><h1>Done</h1>".to_string()
        } else {
            FORM.to_string()
        }
    }))
}

/** Discovery and upgrades refuse foreign Host headers and any Origin. */
#[test]
fn foreign_hosts_and_origins_are_refused() {
    let stack = stack();
    assert!(
        stack
            .get("/json/version", "Host: evil.example\r\n")
            .starts_with("HTTP/1.1 403")
    );
    let with_origin = format!(
        "Host: 127.0.0.1:{}\r\nOrigin: http://evil.example\r\n",
        stack.port
    );
    assert!(
        stack
            .get("/json/version", &with_origin)
            .starts_with("HTTP/1.1 403")
    );
}

/** A basic script: open a tab, navigate, read the title, screenshot, click a field, type, press Enter. */
#[test]
fn basic_script_runs_through_native_operations() {
    let stack = stack();
    let mut client = Client::connect(&stack);
    let (created, _) = client.command("Target.createTarget", json!({"url":"about:blank"}), None);
    let target = created["result"]["targetId"].as_str().unwrap().to_owned();
    let (attached, _) = client.command(
        "Target.attachToTarget",
        json!({"targetId":target,"flatten":true}),
        None,
    );
    let session = attached["result"]["sessionId"].as_str().unwrap().to_owned();
    let session = Some(session.as_str());
    let (navigated, events) = client.command(
        "Page.navigate",
        json!({"url":format!("{}/", stack.origin)}),
        session,
    );
    assert!(navigated["result"]["loaderId"].is_string(), "{navigated}");
    assert!(
        events
            .iter()
            .any(|event| event["method"] == "Page.loadEventFired")
    );
    let (title, _) = client.command(
        "Runtime.evaluate",
        json!({"expression":"document.title"}),
        session,
    );
    assert_eq!(title["result"]["result"]["value"], "Form");
    let (called, _) = client.command(
        "Runtime.callFunctionOn",
        json!({"functionDeclaration":"() => document.title"}),
        session,
    );
    assert_eq!(called["result"]["result"]["value"], "Form");
    let (shot, _) = client.command("Page.captureScreenshot", json!({}), session);
    assert!(
        shot["result"]["data"]
            .as_str()
            .unwrap()
            .starts_with("iVBORw0KGgo")
    );
    client.command("DOM.getDocument", json!({}), session);
    let (found, _) = client.command(
        "DOM.querySelector",
        json!({"nodeId":1,"selector":"#name"}),
        session,
    );
    let node = found["result"]["nodeId"].clone();
    let (quads, _) = client.command("DOM.getContentQuads", json!({"nodeId":node}), session);
    let quad = quads["result"]["quads"][0].as_array().unwrap().clone();
    let (x, y) = (
        quad[0].as_f64().unwrap() + 2.0,
        (quad[1].as_f64().unwrap() + quad[5].as_f64().unwrap()) / 2.0,
    );
    for kind in ["mousePressed", "mouseReleased"] {
        let (clicked, _) = client.command(
            "Input.dispatchMouseEvent",
            json!({"type":kind,"x":x,"y":y,"button":"left","clickCount":1}),
            session,
        );
        assert!(clicked.get("error").is_none(), "{clicked}");
    }
    let (typed, _) = client.command("Input.insertText", json!({"text":"Ada"}), session);
    assert!(typed.get("error").is_none(), "{typed}");
    let (entered, events) = client.command(
        "Input.dispatchKeyEvent",
        json!({"type":"keyDown","key":"Enter","text":"\r"}),
        session,
    );
    assert!(entered.get("error").is_none(), "{entered}");
    assert!(
        events
            .iter()
            .any(|event| event["method"] == "Page.frameNavigated")
    );
    let (title, _) = client.command(
        "Runtime.evaluate",
        json!({"expression":"document.title"}),
        session,
    );
    assert_eq!(title["result"]["result"]["value"], "Done");
    let (closed, _) = client.command("Target.closeTarget", json!({"targetId":target}), None);
    assert_eq!(closed["result"]["success"], true);
}

/** Ownership holds over CDP: the person's tab refuses mutation; unsupported methods say so. */
#[test]
fn ownership_and_unsupported_methods_are_explicit() {
    let stack = stack();
    let mut client = Client::connect(&stack);
    let (attached, _) = client.command(
        "Target.attachToTarget",
        json!({"targetId":"tab-1","flatten":true}),
        None,
    );
    let session = attached["result"]["sessionId"].as_str().unwrap().to_owned();
    let (denied, _) = client.command(
        "Page.navigate",
        json!({"url":format!("{}/", stack.origin)}),
        Some(&session),
    );
    assert!(
        denied["error"]["message"]
            .as_str()
            .unwrap()
            .starts_with("control_denied"),
        "{denied}"
    );
    let (clicked, _) = client.command(
        "Input.dispatchMouseEvent",
        json!({"type":"mouseReleased","x":10,"y":10,"button":"left","clickCount":1}),
        Some(&session),
    );
    assert!(
        clicked["error"]["message"]
            .as_str()
            .unwrap()
            .starts_with("control_denied"),
        "{clicked}"
    );
    // Reads of the person's tab are not an ownership matter: the evaluation reaches the page
    // (which this tab does not have yet) instead of being refused.
    let (peeked, _) = client.command(
        "Runtime.evaluate",
        json!({"expression":"document.readyState","returnByValue":true}),
        Some(&session),
    );
    assert!(
        peeked["error"]["message"]
            .as_str()
            .unwrap()
            .starts_with("no_page"),
        "{peeked}"
    );
    let (mdp_caps, _) = client.command("Mdp.getCapabilities", json!({}), None);
    let (mars_caps, _) = client.command("Mars.getCapabilities", json!({}), None);
    assert!(mdp_caps["result"]["methods"].is_array(), "{mdp_caps}");
    assert_eq!(mdp_caps["result"], mars_caps["result"]);
    let (debugger, _) = client.command("Debugger.enable", json!({}), Some(&session));
    assert_eq!(debugger["error"]["code"], -32601);
    assert!(
        debugger["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Mars does not implement it")
    );
}
