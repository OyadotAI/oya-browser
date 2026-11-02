/** Hermetic native DOM control integration tests retain real service boundaries. */
use super::*;

/** Native text replacement must reject detached script roots before publishing any revision or DOM change. */
#[test]
fn native_text_replacement_rolls_back_when_it_would_detach_a_registered_button() {
    let body = "<div id=container><button id=button type=button>Click</button></div><p id=out>initial</p><script>let count=0;const out=document.getElementById('out');document.getElementById('button').addEventListener('click',()=>{count++;out.textContent=count;});</script>";
    let fixture = fixture(body);
    let mut runtime = runtime(&fixture.url);
    let page = call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    let mut params = target(&page, "container");
    params["text"] = json!("replacement");
    assert_eq!(
        call(&mut runtime, "page.set_text", params)
            .unwrap_err()
            .code,
        "unsupported_feature"
    );
    let inspected = call(&mut runtime, "page.inspect", json!({})).unwrap();
    assert_eq!(inspected["revision"], page["revision"]);
    assert_eq!(text(&inspected, "out"), "initial");
    let clicked = call(&mut runtime, "page.click", target(&inspected, "button")).unwrap();
    assert_eq!(text(&clicked, "out"), "1");
}

/** Human takeover invalidates the agent permit before any registered callback can mutate page state. */
#[test]
fn human_takeover_prevents_stale_agent_listener_dispatch() {
    let body = "<p id=out>initial</p><button id=button type=button>Click</button><script>let count=0;document.getElementById('button').addEventListener('click',()=>{count++;document.getElementById('out').textContent=count;});</script>";
    let fixture = fixture(body);
    let authority = Authority::shared(Owner::Agent);
    let mut runtime = Runtime::desktop(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![fixture.url.clone()],
        authority.clone(),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    let page = call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    let page = call(&mut runtime, "page.click", target(&page, "button")).unwrap();
    authority
        .lock()
        .unwrap()
        .change_tab(1, Owner::Human)
        .unwrap();
    assert_eq!(
        call(&mut runtime, "page.click", target(&page, "button"))
            .unwrap_err()
            .code,
        "control_denied"
    );
    let human = Permit {
        owner: Owner::Human,
        epoch: 2,
        tab_id: Some(1),
    };
    let inspected = call_as(&mut runtime, "page.inspect", json!({}), human).unwrap();
    assert_eq!(inspected["revision"], page["revision"]);
    assert_eq!(text(&inspected, "out"), "1");
    let clicked = call_as(
        &mut runtime,
        "page.click",
        target(&inspected, "button"),
        human,
    )
    .unwrap();
    assert_eq!(text(&clicked, "out"), "2");
}

/** The same source and author IDs in two tabs retain independent listener closures and document identities. */
#[test]
fn listener_closures_and_host_references_stay_isolated_between_tabs() {
    let body = "<p id=out>initial</p><button id=button type=button>Click</button><script>let count=0;const out=document.getElementById('out');document.getElementById('button').addEventListener('click',()=>{count++;out.textContent=count;});</script>";
    let fixture = fixture(body);
    let mut runtime = Runtime::desktop(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![fixture.url.clone()],
        Authority::shared(Owner::Human),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    let first_permit = Permit {
        owner: Owner::Human,
        epoch: 1,
        tab_id: Some(1),
    };
    let first = call_as(
        &mut runtime,
        "page.navigate",
        json!({"url":fixture.url}),
        first_permit,
    )
    .unwrap();
    let first = call_as(
        &mut runtime,
        "page.click",
        target(&first, "button"),
        first_permit,
    )
    .unwrap();
    call_as(&mut runtime, "tab.open", json!({}), first_permit).unwrap();
    let second_permit = Permit {
        tab_id: Some(2),
        ..first_permit
    };
    let second = call_as(
        &mut runtime,
        "page.navigate",
        json!({"url":fixture.url}),
        second_permit,
    )
    .unwrap();
    assert_ne!(first["page_id"], second["page_id"]);
    assert!(
        call_as(
            &mut runtime,
            "page.click",
            target(&first, "button"),
            second_permit
        )
        .is_err()
    );
    let second = call_as(
        &mut runtime,
        "page.click",
        target(&second, "button"),
        second_permit,
    )
    .unwrap();
    assert_eq!(text(&second, "out"), "1");
    let first = call_as(
        &mut runtime,
        "page.click",
        target(&first, "button"),
        first_permit,
    )
    .unwrap();
    assert_eq!(text(&first, "out"), "2");
    assert_eq!(runtime.snapshot().inspection["page_id"], second["page_id"]);
}

/** Native handles cannot redirect text replacement through a colliding synthetic or duplicate author ID. */
#[test]
fn ambiguous_author_ids_cannot_redirect_native_text_mutations() {
    for body in [
        "<p>anonymous</p><p id='@0'>author</p>",
        "<p id=same>first</p><p id=same>second</p>",
    ] {
        let fixture = fixture(body);
        let mut runtime = runtime(&fixture.url);
        let page = call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
        let nodes = page["nodes"].as_array().unwrap();
        let second = json!({"page_id":page["page_id"],"node_id":nodes[1]["node_id"],"expected_revision":page["revision"],"text":"wrong"});
        assert_eq!(
            call(&mut runtime, "page.set_text", second)
                .unwrap_err()
                .code,
            "unsupported_feature"
        );
        let inspected = call(&mut runtime, "page.inspect", json!({})).unwrap();
        assert_eq!(inspected["revision"], page["revision"]);
        assert_eq!(inspected["nodes"], page["nodes"]);
        let first = json!({"page_id":page["page_id"],"node_id":nodes[0]["node_id"],"expected_revision":page["revision"],"text":"correct"});
        let edited = call(&mut runtime, "page.set_text", first).unwrap();
        assert_eq!(edited["text"], "correct");
        let changed = call(&mut runtime, "page.inspect", json!({})).unwrap();
        assert_eq!(changed["nodes"][0]["text"], "correct");
        assert_eq!(changed["nodes"][1]["text"], page["nodes"][1]["text"]);
    }
}
