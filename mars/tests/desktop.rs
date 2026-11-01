/** Real three-process desktop-runtime journeys pin layout, V9 events, history and ownership. */
#[allow(dead_code)]
mod support;
use mars::{
    protocol::{EngineResult, Request},
    runtime::{
        EnginePrograms, Runtime,
        authority::{Authority, Owner, Permit, SharedAuthority},
    },
};
use serde_json::{Value, json};
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};
use support::Fixture;

/** The GUI uses exactly this constructor and these actual service executables. */
fn runtime(origin: &str, authority: SharedAuthority) -> Runtime {
    Runtime::desktop(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![origin.into()],
        authority,
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap()
}

/** Desktop discovery matches the bounded host API without exposing profile contents. */
#[test]
fn desktop_capabilities_advertise_bounded_web_storage() {
    let fixture = fixture();
    let mut runtime = runtime(&fixture.url, Authority::shared(Owner::Agent));
    let capabilities = call(&mut runtime, "runtime.capabilities", json!({}), 1).unwrap();
    assert_eq!(
        capabilities["subsets"]["web_storage"]["localStorage"]["scope"],
        "canonical origin"
    );
    assert_eq!(
        capabilities["subsets"]["web_storage"]["sessionStorage"]["scope"],
        "tab and canonical origin"
    );
    for area in ["localStorage", "sessionStorage"] {
        assert_eq!(
            capabilities["subsets"]["web_storage"][area]["methods"],
            json!(["getItem", "setItem", "removeItem", "clear"])
        );
    }
    assert!(
        !capabilities["unavailable"]
            .as_array()
            .unwrap()
            .contains(&json!("storage"))
    );
    assert!(!capabilities.to_string().contains("secret-site-value"));
}

/** Session restoration preserves URL order and selects the previously active eligible tab. */
#[test]
fn saved_tabs_restore_in_order_with_active_selection() {
    use mars::profile::{
        ProfileStore, SessionStore, WebStorageStore,
        test_support::{MemorySessionStore, MemoryStore, MemoryWebStorageStore},
    };
    let first_body = "<p>first</p>";
    let first = Fixture::raw(
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{first_body}",
            first_body.len()
        )
        .into_bytes(),
    );
    let second_body = "<p>second</p>";
    let second = Fixture::raw(
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{second_body}",
            second_body.len()
        )
        .into_bytes(),
    );
    let sessions = Arc::new(MemorySessionStore::default());
    let cookies = Arc::new(MemoryStore::default());
    let storage = Arc::new(MemoryWebStorageStore::default());
    let mut initial = Runtime::desktop_with_session_profiles(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![first.url.clone(), second.url.clone()],
        Authority::shared(Owner::Human),
        Arc::new(AtomicBool::new(false)),
        cookies.clone() as Arc<dyn ProfileStore>,
        storage.clone() as Arc<dyn WebStorageStore>,
        sessions.clone() as Arc<dyn SessionStore>,
    )
    .unwrap();
    initial.restore_session().unwrap();
    human_page(&mut initial, 1, "page.navigate", json!({"url":first.url})).unwrap();
    initial
        .handle(Request::new("human-test", "tab.open", json!({})))
        .unwrap();
    human_page(&mut initial, 2, "page.navigate", json!({"url":second.url})).unwrap();
    drop(initial);
    let saved = mars::profile::SessionProfile::decode(&sessions.record().unwrap()).unwrap();
    assert_eq!(saved.active_index, 1);
    let mut restarted = Runtime::desktop_with_session_profiles(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![first.url.clone(), second.url.clone()],
        Authority::shared(Owner::Human),
        Arc::new(AtomicBool::new(false)),
        cookies as Arc<dyn ProfileStore>,
        storage as Arc<dyn WebStorageStore>,
        sessions as Arc<dyn SessionStore>,
    )
    .unwrap();
    assert_eq!(restarted.restore_session().unwrap(), 2);
    assert_eq!(restarted.selected_tab_id(), "tab-2");
    assert_eq!(restarted.snapshot().tabs["tabs"][0]["title"], "127.0.0.1");
    assert_eq!(restarted.snapshot().tabs["tabs"][1]["selected"], true);
}

/** A failed session write remains pending and retries on the next successful request. */
#[test]
fn failed_session_save_retries_without_claiming_the_record_was_updated() {
    use mars::profile::{
        ProfileStore, SessionProfile, SessionStore, WebStorageStore,
        test_support::{MemorySessionStore, MemoryStore, MemoryWebStorageStore},
    };
    let first = fixture();
    let second = fixture();
    let sessions = Arc::new(MemorySessionStore::default());
    let original = SessionProfile {
        version: 1,
        urls: vec![format!("{}/", first.url)],
        active_index: 0,
    }
    .encode()
    .unwrap();
    sessions.seed(original.clone());
    let mut runtime = Runtime::desktop_with_session_profiles(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![first.url.clone(), second.url.clone()],
        Authority::shared(Owner::Human),
        Arc::new(AtomicBool::new(false)),
        Arc::new(MemoryStore::default()) as Arc<dyn ProfileStore>,
        Arc::new(MemoryWebStorageStore::default()) as Arc<dyn WebStorageStore>,
        sessions.clone() as Arc<dyn SessionStore>,
    )
    .unwrap();
    runtime.restore_session().unwrap();
    sessions.fail_next_replace();
    human_page(&mut runtime, 1, "page.navigate", json!({"url":second.url})).unwrap();
    assert_eq!(sessions.record().as_deref(), Some(original.as_slice()));
    call(&mut runtime, "runtime.capabilities", json!({}), 1).unwrap();
    let saved = SessionProfile::decode(&sessions.record().unwrap()).unwrap();
    assert_eq!(saved.urls, vec![format!("{}/", second.url)]);
    human_page(&mut runtime, 1, "page.navigate", json!({"url":first.url})).unwrap();
    sessions.fail_next_replace();
    human_page(&mut runtime, 1, "page.navigate", json!({"url":second.url})).unwrap();
    sessions.fail_next_replace();
    let shutdown = runtime
        .handle_as(
            Request::new("shutdown-test", "runtime.shutdown", json!({})),
            Permit {
                owner: Owner::Human,
                epoch: 1,
                tab_id: Some(1),
            },
        )
        .unwrap();
    assert_eq!(shutdown["session_saved"], false);
    assert_eq!(
        SessionProfile::decode(&sessions.record().unwrap())
            .unwrap()
            .urls,
        vec![format!("{}/", first.url)]
    );
}

/** Explicit startup destinations leave the saved ordinary-launch session unchanged. */
#[test]
fn explicit_startup_url_does_not_restore_or_overwrite_saved_tabs() {
    use mars::profile::{
        ProfileStore, SessionProfile, SessionStore, WebStorageStore,
        test_support::{MemorySessionStore, MemoryStore, MemoryWebStorageStore},
    };
    let fixture = fixture();
    let sessions = Arc::new(MemorySessionStore::default());
    let stored = SessionProfile {
        version: 1,
        urls: vec![fixture.url.clone()],
        active_index: 0,
    }
    .encode()
    .unwrap();
    sessions.seed(stored.clone());
    let mut runtime = Runtime::desktop_with_session_profiles(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![fixture.url.clone()],
        Authority::shared(Owner::Human),
        Arc::new(AtomicBool::new(false)),
        Arc::new(MemoryStore::default()) as Arc<dyn ProfileStore>,
        Arc::new(MemoryWebStorageStore::default()) as Arc<dyn WebStorageStore>,
        Arc::clone(&sessions) as Arc<dyn SessionStore>,
    )
    .unwrap();
    human_page(&mut runtime, 1, "page.navigate", json!({"url":fixture.url})).unwrap();
    assert_eq!(sessions.record().unwrap(), stored);
}

/** Recovery delays session restoration until storage has been explicitly cleared. */
#[test]
fn session_restore_waits_for_web_storage_recovery() {
    use mars::profile::{
        ProfileStore, SessionProfile, SessionStore, WebStorageStore,
        test_support::{MemorySessionStore, MemoryStore, MemoryWebStorageStore},
    };
    let fixture = fixture();
    let sessions = Arc::new(MemorySessionStore::default());
    sessions.seed(
        SessionProfile {
            version: 1,
            urls: vec![fixture.url.clone()],
            active_index: 0,
        }
        .encode()
        .unwrap(),
    );
    let storage = Arc::new(MemoryWebStorageStore::default());
    storage.seed_recovery();
    let mut runtime = Runtime::desktop_with_session_profiles(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![fixture.url.clone()],
        Authority::shared(Owner::Human),
        Arc::new(AtomicBool::new(false)),
        Arc::new(MemoryStore::default()) as Arc<dyn ProfileStore>,
        Arc::clone(&storage) as Arc<dyn WebStorageStore>,
        sessions as Arc<dyn SessionStore>,
    )
    .unwrap();
    assert!(runtime.restore_session().is_err());
    assert_eq!(fixture.requests.load(Ordering::Relaxed), 0);
    runtime.clear_saved_web_storage().unwrap();
    assert_eq!(fixture.requests.load(Ordering::Relaxed), 1);
    assert!(runtime.snapshot().document.is_some());
}

/** Restored query strings stay in secure state and never enter inspection. */
#[test]
fn restored_session_queries_are_redacted_from_agent_output() {
    use mars::profile::{
        ProfileStore, SessionProfile, SessionStore, WebStorageStore,
        test_support::{MemorySessionStore, MemoryStore, MemoryWebStorageStore},
    };
    let fixture = fixture();
    let url = format!("{}?access_token=private-query", fixture.url);
    let sessions = Arc::new(MemorySessionStore::default());
    sessions.seed(
        SessionProfile {
            version: 1,
            urls: vec![url],
            active_index: 0,
        }
        .encode()
        .unwrap(),
    );
    let mut runtime = Runtime::desktop_with_session_profiles(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![fixture.url.clone()],
        Authority::shared(Owner::Human),
        Arc::new(AtomicBool::new(false)),
        Arc::new(MemoryStore::default()) as Arc<dyn ProfileStore>,
        Arc::new(MemoryWebStorageStore::default()) as Arc<dyn WebStorageStore>,
        sessions as Arc<dyn SessionStore>,
    )
    .unwrap();
    runtime.restore_session().unwrap();
    assert!(
        !runtime
            .snapshot()
            .inspection
            .to_string()
            .contains("private-query")
    );
    assert_eq!(
        runtime.snapshot().inspection["url"],
        format!("{}/", fixture.url)
    );
}

/** One dead saved destination does not prevent later eligible tabs from restoring. */
#[test]
fn session_restore_skips_individual_network_failures() {
    use mars::profile::{
        ProfileStore, SessionProfile, SessionStore, WebStorageStore,
        test_support::{MemorySessionStore, MemoryStore, MemoryWebStorageStore},
    };
    let fixture = fixture();
    let sessions = Arc::new(MemorySessionStore::default());
    sessions.seed(
        SessionProfile {
            version: 1,
            urls: vec!["http://127.0.0.1:1/unavailable".into(), fixture.url.clone()],
            active_index: 0,
        }
        .encode()
        .unwrap(),
    );
    let mut runtime = Runtime::desktop_with_session_profiles(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![fixture.url.clone()],
        Authority::shared(Owner::Human),
        Arc::new(AtomicBool::new(false)),
        Arc::new(MemoryStore::default()) as Arc<dyn ProfileStore>,
        Arc::new(MemoryWebStorageStore::default()) as Arc<dyn WebStorageStore>,
        sessions as Arc<dyn SessionStore>,
    )
    .unwrap();
    assert_eq!(runtime.restore_session().unwrap(), 1);
    assert_eq!(fixture.requests.load(Ordering::Relaxed), 1);
    assert!(runtime.snapshot().document.is_some());
}

/** Unreadable or malformed session data leaves a usable fresh browser tab. */
#[test]
fn invalid_saved_session_falls_back_without_blocking_navigation() {
    use mars::profile::{
        ProfileStore, SessionProfile, SessionStore, WebStorageStore,
        test_support::{MemorySessionStore, MemoryStore, MemoryWebStorageStore},
    };
    let fixture = fixture();
    let sessions = Arc::new(MemorySessionStore::default());
    sessions.seed(b"malformed session".to_vec());
    let mut runtime = Runtime::desktop_with_session_profiles(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![fixture.url.clone()],
        Authority::shared(Owner::Human),
        Arc::new(AtomicBool::new(false)),
        Arc::new(MemoryStore::default()) as Arc<dyn ProfileStore>,
        Arc::new(MemoryWebStorageStore::default()) as Arc<dyn WebStorageStore>,
        Arc::clone(&sessions) as Arc<dyn SessionStore>,
    )
    .unwrap();
    assert_eq!(runtime.restore_session().unwrap(), 0);
    assert!(
        SessionProfile::decode(&sessions.record().unwrap())
            .unwrap()
            .urls
            .is_empty()
    );
    human_page(&mut runtime, 1, "page.navigate", json!({"url":fixture.url})).unwrap();
    assert_eq!(fixture.requests.load(Ordering::Relaxed), 1);
}
/** Test callers use real epoch and revision admission rather than private mutation helpers. */
fn call(runtime: &mut Runtime, op: &str, mut params: Value, epoch: u64) -> EngineResult<Value> {
    if op.starts_with("page.") {
        params["tab_id"] = json!("tab-1");
    }
    if mars::runtime::mutating(op) {
        params["control_epoch"] = json!(epoch);
    }
    runtime.handle_as(
        Request::new("test", op, params),
        Permit {
            owner: Owner::Agent,
            epoch,
            tab_id: Some(1),
        },
    )
}
/** Human fixture calls carry the authority record for one exact tab. */
fn human_page(runtime: &mut Runtime, tab: u64, op: &str, mut params: Value) -> EngineResult<Value> {
    params["tab_id"] = json!(format!("tab-{tab}"));
    runtime.handle_as(
        Request::new("human-test", op, params),
        Permit {
            owner: Owner::Human,
            epoch: 1,
            tab_id: Some(tab),
        },
    )
}
/** Fixed synthetic source exercises the supported document and language subset. */
fn fixture() -> Fixture {
    let body = include_str!("../fixtures/desktop.html");
    Fixture::raw(
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        )
        .into_bytes(),
    )
}

/** A new desktop owner restores Keychain-shaped session state without exposing values. */
#[test]
fn secure_profile_restores_cookies_across_runtime_restart_and_clear() {
    use mars::profile::test_support::MemoryStore;
    let html = "<html><body><p>signed in</p></body></html>";
    let response = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nSet-Cookie: session=private-value; Path=/; SameSite=Lax\r\nContent-Length: {}\r\n\r\n{html}", html.len()).into_bytes();
    let fixture = Fixture::raw(response);
    let store = Arc::new(MemoryStore::default());
    let mut first = Runtime::desktop_with_profile(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![fixture.url.clone()],
        Authority::shared(Owner::Agent),
        Arc::new(AtomicBool::new(false)),
        Arc::clone(&store) as Arc<dyn mars::profile::ProfileStore>,
    )
    .unwrap();
    let page = call(&mut first, "page.navigate", json!({"url":fixture.url}), 1).unwrap();
    assert!(!page.to_string().contains("private-value"));
    drop(first);
    assert!(store.has_record());
    let mut restarted = Runtime::desktop_with_profile(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![fixture.url.clone()],
        Authority::shared(Owner::Agent),
        Arc::new(AtomicBool::new(false)),
        Arc::clone(&store) as Arc<dyn mars::profile::ProfileStore>,
    )
    .unwrap();
    call(
        &mut restarted,
        "page.navigate",
        json!({"url":fixture.url}),
        1,
    )
    .unwrap();
    assert!(
        fixture
            .last_request
            .lock()
            .unwrap()
            .contains("Cookie: session=private-value")
    );
    restarted.clear_profile().unwrap();
    assert!(!store.has_record());
}

/** A corrupt profile blocks network work until explicit recovery clears it. */
#[test]
fn corrupt_profile_fails_closed_and_clear_restores_navigation() {
    use mars::profile::test_support::MemoryStore;
    let fixture = fixture();
    let store = Arc::new(MemoryStore::default());
    store.seed(b"invalid profile".to_vec());
    let mut runtime = Runtime::desktop_with_profile(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![fixture.url.clone()],
        Authority::shared(Owner::Agent),
        Arc::new(AtomicBool::new(false)),
        Arc::clone(&store) as Arc<dyn mars::profile::ProfileStore>,
    )
    .unwrap();
    let error = call(&mut runtime, "page.navigate", json!({"url":fixture.url}), 1).unwrap_err();
    assert_eq!(error.code, "profile_corrupt");
    assert!(
        !serde_json::to_string(&error)
            .unwrap()
            .contains("private-value")
    );
    assert_eq!(fixture.requests.load(Ordering::Relaxed), 0);
    runtime.clear_profile().unwrap();
    assert!(!store.has_record());
    assert!(call(&mut runtime, "page.navigate", json!({"url":fixture.url}), 1).is_ok());
}

/** An unavailable profile read blocks startup navigation until a successful retry. */
#[test]
fn unavailable_profile_load_fails_closed_until_retry_succeeds() {
    use mars::profile::test_support::MemoryStore;
    let fixture = fixture();
    let store = Arc::new(MemoryStore::default());
    store.fail_next_load();
    let mut runtime = Runtime::desktop_with_profile(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![fixture.url.clone()],
        Authority::shared(Owner::Agent),
        Arc::new(AtomicBool::new(false)),
        Arc::clone(&store) as Arc<dyn mars::profile::ProfileStore>,
    )
    .unwrap();
    let error = call(&mut runtime, "page.navigate", json!({"url":fixture.url}), 1).unwrap_err();
    assert_eq!(error.code, "profile_unavailable");
    assert!(
        !serde_json::to_string(&error)
            .unwrap()
            .contains("private-value")
    );
    assert_eq!(fixture.requests.load(Ordering::Relaxed), 0);
    runtime.retry_profile().unwrap();
    assert!(call(&mut runtime, "page.navigate", json!({"url":fixture.url}), 1).is_ok());
}

/** Tab histories and page identities remain isolated while agents target background tabs. */
#[test]
fn tab_selection_restores_isolated_pages_and_background_requests_preserve_selection() {
    let fixture = Fixture::pages();
    let authority = Authority::shared(Owner::Human);
    let mut runtime = Runtime::desktop(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![fixture.url.clone()],
        Arc::clone(&authority),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    let human = Permit {
        owner: Owner::Human,
        epoch: 1,
        tab_id: Some(1),
    };
    let _first = runtime
        .handle_as(
            Request::new(
                "test",
                "page.navigate",
                json!({"url":format!("{}/plain",fixture.url),"tab_id":"tab-1"}),
            ),
            human,
        )
        .unwrap();
    let first_latest = runtime
        .handle_as(
            Request::new(
                "test",
                "page.navigate",
                json!({"url":format!("{}/styled",fixture.url),"tab_id":"tab-1"}),
            ),
            human,
        )
        .unwrap();
    let tabs = runtime
        .handle_as(Request::new("test", "tab.open", json!({})), human)
        .unwrap();
    let second_id = tabs["selected_tab_id"].as_str().unwrap().to_owned();
    assert_eq!(second_id, "tab-2");
    let human_second = Permit {
        owner: Owner::Human,
        epoch: 1,
        tab_id: Some(2),
    };
    let second = runtime
        .handle_as(
            Request::new(
                "test",
                "page.navigate",
                json!({"url":format!("{}/ordered",fixture.url),"tab_id":second_id}),
            ),
            human_second,
        )
        .unwrap();
    runtime
        .handle_as(
            Request::new("test", "tab.select", json!({"tab_id":"tab-1"})),
            human,
        )
        .unwrap();
    assert_eq!(
        runtime.snapshot().inspection["page_id"],
        first_latest["page_id"]
    );
    assert_eq!(
        runtime.snapshot().inspection["url"],
        format!("{}/styled", fixture.url)
    );
    assert!(runtime.snapshot().can_back);
    let backed = runtime
        .handle_as(
            Request::new("test", "page.back", json!({"tab_id":"tab-1"})),
            human,
        )
        .unwrap();
    assert_eq!(backed["url"], format!("{}/plain", fixture.url));
    runtime
        .handle_as(
            Request::new("test", "tab.select", json!({"tab_id":"tab-2"})),
            human,
        )
        .unwrap();
    assert_eq!(runtime.snapshot().inspection["page_id"], second["page_id"]);
    runtime
        .handle_as(
            Request::new("test", "tab.select", json!({"tab_id":"tab-1"})),
            human,
        )
        .unwrap();
    authority
        .lock()
        .unwrap()
        .change_tab(2, Owner::Agent)
        .unwrap();
    let agent = Permit {
        owner: Owner::Agent,
        epoch: 2,
        tab_id: Some(2),
    };
    let inspected = runtime
        .handle_as(
            Request::new("test", "page.inspect", json!({"tab_id":"tab-2"})),
            agent,
        )
        .unwrap();
    assert_eq!(inspected["page_id"], second["page_id"]);
    assert_eq!(runtime.snapshot().inspection["page_id"], backed["page_id"]);
    assert_eq!(runtime.snapshot().tabs["selected_tab_id"], "tab-1");
    assert_eq!(runtime.snapshot().tabs["tabs"][1]["owner"], "agent");
    let requests = fixture.requests.load(Ordering::Relaxed);
    assert_eq!(
        runtime
            .handle_as(
                Request::new(
                    "test",
                    "page.navigate",
                    json!({"url":format!("{}/plain",fixture.url),"tab_id":"tab-1","control_epoch":1}),
                ),
                Permit {
                    owner: Owner::Agent,
                    epoch: 1,
                    tab_id: Some(1),
                },
            )
            .unwrap_err()
            .code,
        "control_denied"
    );
    assert_eq!(fixture.requests.load(Ordering::Relaxed), requests);
    runtime
        .handle_as(
            Request::new("test", "tab.close", json!({"tab_id":"tab-2"})),
            human,
        )
        .unwrap();
    assert_eq!(runtime.snapshot().tabs["selected_tab_id"], "tab-1");
    runtime
        .handle_as(
            Request::new("test", "tab.close", json!({"tab_id":"tab-1"})),
            human,
        )
        .unwrap();
    assert_eq!(runtime.snapshot().tabs["selected_tab_id"], "tab-3");
    assert_eq!(
        runtime
            .handle(Request::new(
                "test",
                "page.inspect",
                json!({"tab_id":"tab-1"})
            ))
            .unwrap_err()
            .code,
        "invalid_request"
    );
}

/** Tab activation recomputes geometry for the current viewport and clamps saved scrolling. */
#[test]
fn switching_tabs_refreshes_viewport_layout_and_scroll_bounds() {
    let fixture = Fixture::pages();
    let mut runtime = Runtime::desktop(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![fixture.url.clone()],
        Authority::shared(Owner::Human),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    let first = runtime
        .handle_as(
            Request::new(
                "test",
                "page.navigate",
                json!({"url":fixture.url,"tab_id":"tab-1"}),
            ),
            Permit {
                owner: Owner::Human,
                epoch: 1,
                tab_id: Some(1),
            },
        )
        .unwrap();
    runtime
        .handle(Request::new("test", "tab.open", json!({})))
        .unwrap();
    let second = runtime
        .handle_as(
            Request::new(
                "test",
                "page.navigate",
                json!({"url":fixture.url,"tab_id":"tab-2"}),
            ),
            Permit {
                owner: Owner::Human,
                epoch: 1,
                tab_id: Some(2),
            },
        )
        .unwrap();
    runtime.set_viewport(280.0, 120.0).unwrap();
    runtime
        .handle(Request::new(
            "test",
            "tab.select",
            json!({"tab_id":"tab-1"}),
        ))
        .unwrap();
    let first_layout = call(&mut runtime, "page.layout", json!({"tab_id":"tab-1"}), 1).unwrap();
    assert_eq!(first_layout["viewport"]["width"], 280.0);
    runtime.observe_scroll(100000.0);
    runtime
        .handle(Request::new(
            "test",
            "tab.select",
            json!({"tab_id":"tab-2"}),
        ))
        .unwrap();
    let second_layout = runtime
        .handle_as(
            Request::new("test", "page.layout", json!({"tab_id":"tab-2"})),
            Permit {
                owner: Owner::Agent,
                epoch: 1,
                tab_id: Some(2),
            },
        )
        .unwrap();
    assert_eq!(second_layout["tab_id"], "tab-2");
    assert_eq!(second_layout["viewport"]["width"], 280.0);
    assert_eq!(second_layout["viewport"]["scroll_y"], 0.0);
    assert_ne!(first["page_id"], second["page_id"]);
}

/** Native tabs reject growth beyond the aggregate page-state capacity. */
#[test]
fn tab_open_stops_at_the_named_capacity() {
    let authority = Authority::shared(Owner::Human);
    let mut runtime = Runtime::desktop(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        Vec::new(),
        Arc::clone(&authority),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    let human = Permit {
        owner: Owner::Human,
        epoch: 1,
        tab_id: Some(1),
    };
    for _ in 1..mars::constants::TABS {
        runtime
            .handle_as(Request::new("test", "tab.open", json!({})), human)
            .unwrap();
    }
    assert_eq!(
        runtime
            .handle_as(Request::new("test", "tab.open", json!({})), human)
            .unwrap_err()
            .code,
        "limit_exceeded"
    );
    assert_eq!(
        runtime.snapshot().tabs["tabs"].as_array().unwrap().len(),
        mars::constants::TABS
    );
}

/** Inline page scripts share coordinator storage through load and event publication. */
#[test]
fn page_storage_calls_publish_without_exposing_values_to_inspection() {
    let body = "<html><body><p id=\"result\"></p><button id=\"show\" onclick=\"show()\">Show</button><script>window.localStorage.setItem('secret-key','secret-value'); function show(){document.getElementById('result').textContent=window.localStorage.getItem('secret-key')==='secret-value';}</script></body></html>";
    let fixture = Fixture::raw(
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        )
        .into_bytes(),
    );
    let mut runtime = runtime(&fixture.url, Authority::shared(Owner::Agent));
    let page = call(&mut runtime, "page.navigate", json!({"url":fixture.url}), 1).unwrap();
    let inspected = call(&mut runtime, "page.inspect", json!({}), 1).unwrap();
    let button = inspected["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["html_id"] == "show")
        .unwrap();
    let result = call(
        &mut runtime,
        "page.click",
        json!({"page_id":page["page_id"],"node_id":button["node_id"],"expected_revision":1}),
        1,
    )
    .unwrap();
    assert!(
        result["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|node| node["html_id"] == "result" && node["text"] == "true")
    );
    assert!(
        !serde_json::to_string(&result)
            .unwrap()
            .contains("secret-key")
    );
    assert!(
        !serde_json::to_string(&result)
            .unwrap()
            .contains("secret-value")
    );
}

/** Local storage survives restart in its own record and cookie clearing preserves it. */
#[test]
fn local_storage_profile_survives_restart_and_cookie_clear() {
    use mars::profile::{
        ProfileStore, WebStorageStore,
        test_support::{MemoryStore, MemoryWebStorageStore},
    };
    let body = "<html><body><p id=\"result\"></p><script>document.getElementById('result').textContent=window.localStorage.getItem('token')==='saved'; window.localStorage.setItem('token','saved');</script></body></html>";
    let fixture = Fixture::raw(
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        )
        .into_bytes(),
    );
    let cookies = Arc::new(MemoryStore::default());
    let storage = Arc::new(MemoryWebStorageStore::default());
    let mut first = Runtime::desktop_with_profiles(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![fixture.url.clone()],
        Authority::shared(Owner::Agent),
        Arc::new(AtomicBool::new(false)),
        Arc::clone(&cookies) as Arc<dyn ProfileStore>,
        Arc::clone(&storage) as Arc<dyn WebStorageStore>,
    )
    .unwrap();
    let initial = call(&mut first, "page.navigate", json!({"url":fixture.url}), 1).unwrap();
    assert_eq!(
        initial["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["html_id"] == "result")
            .unwrap()["text"],
        "false"
    );
    assert!(storage.has_record());
    first.clear_profile().unwrap();
    drop(first);
    let mut restarted = Runtime::desktop_with_profiles(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![fixture.url.clone()],
        Authority::shared(Owner::Agent),
        Arc::new(AtomicBool::new(false)),
        cookies,
        storage,
    )
    .unwrap();
    let restored = call(
        &mut restarted,
        "page.navigate",
        json!({"url":fixture.url}),
        1,
    )
    .unwrap();
    assert_eq!(
        restored["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["html_id"] == "result")
            .unwrap()["text"],
        "true"
    );
}

/** Explicit Web Storage recovery clears only site data and leaves saved cookies intact. */
#[test]
fn clearing_saved_web_storage_preserves_cookie_profile_and_current_page() {
    use mars::profile::{
        ProfileStore, WebStorageStore,
        test_support::{MemoryStore, MemoryWebStorageStore},
    };
    let html = "<html><body><script>window.localStorage.setItem('key','site');</script><p>still here</p></body></html>";
    let fixture = Fixture::raw(format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nSet-Cookie: session=private; Path=/; SameSite=Lax\r\nContent-Length: {}\r\n\r\n{html}", html.len()).into_bytes());
    let cookies = Arc::new(MemoryStore::default());
    let storage = Arc::new(MemoryWebStorageStore::default());
    let mut runtime = Runtime::desktop_with_profiles(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![fixture.url.clone()],
        Authority::shared(Owner::Agent),
        Arc::new(AtomicBool::new(false)),
        Arc::clone(&cookies) as Arc<dyn ProfileStore>,
        Arc::clone(&storage) as Arc<dyn WebStorageStore>,
    )
    .unwrap();
    let before = call(&mut runtime, "page.navigate", json!({"url":fixture.url}), 1).unwrap();
    assert!(cookies.has_record());
    assert!(storage.has_record());
    runtime.clear_saved_web_storage().unwrap();
    assert!(cookies.has_record());
    assert!(!storage.has_record());
    assert_eq!(runtime.snapshot().inspection["page_id"], before["page_id"]);
}

/** A secure storage write failure leaves both the previous page and profile untouched. */
#[test]
fn local_storage_profile_failure_does_not_publish_the_page() {
    use mars::profile::{
        ProfileStore, WebStorageStore,
        test_support::{MemoryStore, MemoryWebStorageStore},
    };
    let body =
        "<html><body><script>window.localStorage.setItem('key','value');</script></body></html>";
    let fixture = Fixture::raw(
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        )
        .into_bytes(),
    );
    let cookies = Arc::new(MemoryStore::default());
    let storage = Arc::new(MemoryWebStorageStore::default());
    storage.fail_next_replace();
    let mut runtime = Runtime::desktop_with_profiles(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![fixture.url.clone()],
        Authority::shared(Owner::Agent),
        Arc::new(AtomicBool::new(false)),
        cookies as Arc<dyn ProfileStore>,
        Arc::clone(&storage) as Arc<dyn WebStorageStore>,
    )
    .unwrap();
    let error = call(&mut runtime, "page.navigate", json!({"url":fixture.url}), 1).unwrap_err();
    assert_eq!(error.code, "web_storage_unavailable");
    assert!(runtime.snapshot().document.is_none());
    assert!(!storage.has_record());
    assert!(storage.has_recovery());
    drop(runtime);
    let mut restarted = Runtime::desktop_with_profiles(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![fixture.url.clone()],
        Authority::shared(Owner::Agent),
        Arc::new(AtomicBool::new(false)),
        Arc::new(MemoryStore::default()),
        Arc::clone(&storage) as Arc<dyn WebStorageStore>,
    )
    .unwrap();
    let blocked = call(
        &mut restarted,
        "page.navigate",
        json!({"url":fixture.url}),
        1,
    )
    .unwrap_err();
    assert_eq!(blocked.code, "web_storage_commit_uncertain");
    restarted.clear_saved_web_storage().unwrap();
    assert!(!storage.has_recovery());
}

/** A recovery marker prevents restart from trusting a write that may finish late. */
#[test]
fn recovery_marker_blocks_restart_until_origin_map_is_cleared() {
    use mars::profile::{
        ProfileStore, WebStorageStore,
        test_support::{MemoryStore, MemoryWebStorageStore},
    };
    let fixture = fixture();
    let storage = Arc::new(MemoryWebStorageStore::default());
    storage.seed_recovery();
    let mut runtime = Runtime::desktop_with_profiles(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![fixture.url.clone()],
        Authority::shared(Owner::Agent),
        Arc::new(AtomicBool::new(false)),
        Arc::new(MemoryStore::default()) as Arc<dyn ProfileStore>,
        Arc::clone(&storage) as Arc<dyn WebStorageStore>,
    )
    .unwrap();
    let requests_before = fixture.requests.load(Ordering::Relaxed);
    let blocked = call(&mut runtime, "page.navigate", json!({"url":fixture.url}), 1).unwrap_err();
    assert_eq!(blocked.code, "web_storage_commit_uncertain");
    assert_eq!(fixture.requests.load(Ordering::Relaxed), requests_before);
    storage.fail_next_clear();
    assert!(runtime.clear_saved_web_storage().is_err());
    assert!(storage.has_recovery());
    storage.fail_next_recovery_clear();
    assert!(runtime.clear_saved_web_storage().is_err());
    assert!(storage.has_recovery());
    assert!(!storage.has_record());
    runtime.clear_saved_web_storage().unwrap();
    assert!(!storage.has_recovery());
}

/** A marker read failure blocks startup rather than trusting an unchecked origin map. */
#[test]
fn recovery_marker_read_failure_blocks_startup() {
    use mars::profile::{
        ProfileStore, WebStorageStore,
        test_support::{MemoryStore, MemoryWebStorageStore},
    };
    let fixture = fixture();
    let storage = Arc::new(MemoryWebStorageStore::default());
    storage.fail_next_recovery_read();
    let mut runtime = Runtime::desktop_with_profiles(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![fixture.url.clone()],
        Authority::shared(Owner::Agent),
        Arc::new(AtomicBool::new(false)),
        Arc::new(MemoryStore::default()) as Arc<dyn ProfileStore>,
        Arc::clone(&storage) as Arc<dyn WebStorageStore>,
    )
    .unwrap();
    let blocked = call(&mut runtime, "page.navigate", json!({"url":fixture.url}), 1).unwrap_err();
    assert_eq!(blocked.code, "web_storage_unavailable");
}

/** A failed recovery clear persists a restart lock before returning to the UI. */
#[test]
fn failed_site_storage_clear_keeps_restart_blocked() {
    use mars::profile::{
        ProfileStore, WebStorageProfile, WebStorageStore,
        test_support::{MemoryStore, MemoryWebStorageStore},
    };
    let fixture = fixture();
    let storage = Arc::new(MemoryWebStorageStore::default());
    storage.seed(
        WebStorageProfile {
            version: 1,
            origins: [(
                "http://127.0.0.1".into(),
                [("key".into(), "value".into())].into(),
            )]
            .into(),
        }
        .encode()
        .unwrap(),
    );
    storage.fail_next_recovery_read();
    let mut runtime = Runtime::desktop_with_profiles(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![fixture.url.clone()],
        Authority::shared(Owner::Agent),
        Arc::new(AtomicBool::new(false)),
        Arc::new(MemoryStore::default()) as Arc<dyn ProfileStore>,
        Arc::clone(&storage) as Arc<dyn WebStorageStore>,
    )
    .unwrap();
    storage.fail_next_clear();
    assert!(runtime.clear_saved_web_storage().is_err());
    assert!(storage.has_recovery());
    drop(runtime);
    let mut restarted = Runtime::desktop_with_profiles(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![fixture.url.clone()],
        Authority::shared(Owner::Agent),
        Arc::new(AtomicBool::new(false)),
        Arc::new(MemoryStore::default()) as Arc<dyn ProfileStore>,
        Arc::clone(&storage) as Arc<dyn WebStorageStore>,
    )
    .unwrap();
    let blocked = call(
        &mut restarted,
        "page.navigate",
        json!({"url":fixture.url}),
        1,
    )
    .unwrap_err();
    assert_eq!(blocked.code, "web_storage_commit_uncertain");
    restarted.clear_saved_web_storage().unwrap();
    assert!(!storage.has_recovery());
}

/** Failed marker persistence remains blocked locally and reports uncertain restart safety. */
#[test]
fn failed_recovery_marker_persistence_reports_restart_risk() {
    use mars::profile::{
        ProfileStore, WebStorageStore,
        test_support::{MemoryStore, MemoryWebStorageStore},
    };
    let body =
        "<html><body><script>window.localStorage.setItem('key','value');</script></body></html>";
    let fixture = Fixture::raw(
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        )
        .into_bytes(),
    );
    let storage = Arc::new(MemoryWebStorageStore::default());
    storage.fail_next_replace();
    storage.fail_next_mark();
    let mut runtime = Runtime::desktop_with_profiles(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![fixture.url.clone()],
        Authority::shared(Owner::Agent),
        Arc::new(AtomicBool::new(false)),
        Arc::new(MemoryStore::default()) as Arc<dyn ProfileStore>,
        Arc::clone(&storage) as Arc<dyn WebStorageStore>,
    )
    .unwrap();
    let error = call(&mut runtime, "page.navigate", json!({"url":fixture.url}), 1).unwrap_err();
    assert_eq!(error.code, "web_storage_recovery_unpersisted");
}

/** Local maps share exact origins while session maps stay isolated to each tab. */
#[test]
fn storage_is_origin_and_tab_scoped() {
    let body = "<html><body><p id=\"result\"></p><script>document.getElementById('result').textContent=(window.localStorage.getItem('l')==='1')+' '+(window.sessionStorage.getItem('s')==='1'); window.localStorage.setItem('l','1'); window.sessionStorage.setItem('s','1');</script></body></html>";
    let response = || {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        )
        .into_bytes()
    };
    let first_origin = Fixture::raw(response());
    let other_origin = Fixture::raw(response());
    let mut runtime = Runtime::desktop(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![first_origin.url.clone(), other_origin.url.clone()],
        Authority::shared(Owner::Human),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    let first = human_page(
        &mut runtime,
        1,
        "page.navigate",
        json!({"url":first_origin.url}),
    )
    .unwrap();
    assert_eq!(result_text(&first), "false false");
    runtime
        .handle(Request::new("human-test", "tab.open", json!({})))
        .unwrap();
    let second = human_page(
        &mut runtime,
        2,
        "page.navigate",
        json!({"url":first_origin.url}),
    )
    .unwrap();
    assert_eq!(result_text(&second), "true false");
    runtime
        .handle(Request::new(
            "human-test",
            "tab.close",
            json!({"tab_id":"tab-2"}),
        ))
        .unwrap();
    runtime
        .handle(Request::new("human-test", "tab.open", json!({})))
        .unwrap();
    let reopened = human_page(
        &mut runtime,
        3,
        "page.navigate",
        json!({"url":first_origin.url}),
    )
    .unwrap();
    assert_eq!(result_text(&reopened), "true false");
    runtime
        .handle(Request::new("human-test", "tab.open", json!({})))
        .unwrap();
    let third = human_page(
        &mut runtime,
        4,
        "page.navigate",
        json!({"url":other_origin.url}),
    )
    .unwrap();
    assert_eq!(result_text(&third), "false false");
}

/** Read only the synthetic result node so secrets and storage maps stay out of assertions. */
fn result_text(page: &Value) -> &str {
    page["nodes"]
        .as_array()
        .and_then(|nodes| nodes.iter().find(|node| node["html_id"] == "result"))
        .and_then(|node| node["text"].as_str())
        .unwrap_or_default()
}

/** A failed Keychain replacement neither publishes candidate page nor cookie state. */
#[test]
fn profile_commit_failure_keeps_the_previous_page_and_blocks_navigation() {
    use mars::profile::test_support::MemoryStore;
    let fixture = Fixture::pages();
    let store = Arc::new(MemoryStore::default());
    let mut runtime = Runtime::desktop_with_profile(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![fixture.url.clone()],
        Authority::shared(Owner::Agent),
        Arc::new(AtomicBool::new(false)),
        Arc::clone(&store) as Arc<dyn mars::profile::ProfileStore>,
    )
    .unwrap();
    call(&mut runtime, "page.navigate", json!({"url":fixture.url}), 1).unwrap();
    store.fail_next_replace();
    let url = format!("{}/set-cookie", fixture.url);
    let error = call(&mut runtime, "page.navigate", json!({"url":url}), 1).unwrap_err();
    assert_eq!(error.code, "profile_commit_uncertain");
    assert!(
        !serde_json::to_string(&error)
            .unwrap()
            .contains("private-value")
    );
    let page = call(&mut runtime, "page.inspect", json!({}), 1).unwrap();
    assert_eq!(page["url"], format!("{}/", fixture.url));
    let requests = fixture.requests.load(Ordering::Relaxed);
    assert_eq!(
        call(&mut runtime, "page.navigate", json!({"url":fixture.url}), 1)
            .unwrap_err()
            .code,
        "profile_commit_uncertain"
    );
    assert_eq!(fixture.requests.load(Ordering::Relaxed), requests);
    assert!(store.has_record());
    drop(runtime);
    let mut restarted = Runtime::desktop_with_profile(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![fixture.url.clone()],
        Authority::shared(Owner::Agent),
        Arc::new(AtomicBool::new(false)),
        Arc::clone(&store) as Arc<dyn mars::profile::ProfileStore>,
    )
    .unwrap();
    assert_eq!(
        call(
            &mut restarted,
            "page.navigate",
            json!({"url":fixture.url}),
            1
        )
        .unwrap_err()
        .code,
        "profile_corrupt"
    );
    restarted.clear_profile().unwrap();
    assert!(
        call(
            &mut restarted,
            "page.navigate",
            json!({"url":fixture.url}),
            1
        )
        .is_ok()
    );
}

/** A lost owner plus failed rollback opens the same recovery state as the UI prompt. */
#[test]
fn stale_authority_and_failed_profile_rollback_require_recovery() {
    use mars::profile::{ProfileError, ProfileStore};
    struct RollbackFailureStore {
        authority: SharedAuthority,
        replacements: std::sync::atomic::AtomicUsize,
        value: std::sync::Mutex<Option<Vec<u8>>>,
    }
    impl ProfileStore for RollbackFailureStore {
        fn load(&self) -> Result<Option<Vec<u8>>, ProfileError> {
            self.value
                .lock()
                .map(|value| value.clone())
                .map_err(|_| ProfileError::Unavailable)
        }
        fn replace(&self, bytes: &[u8]) -> Result<(), ProfileError> {
            match self.replacements.fetch_add(1, Ordering::SeqCst) {
                0 => {
                    self.authority.lock().unwrap().change(Owner::Human);
                    *self.value.lock().unwrap() = Some(bytes.to_vec());
                    Ok(())
                }
                1 => Err(ProfileError::Unavailable),
                _ => {
                    *self.value.lock().unwrap() = Some(bytes.to_vec());
                    Ok(())
                }
            }
        }
        fn clear(&self) -> Result<(), ProfileError> {
            Ok(())
        }
    }
    let fixture = Fixture::pages();
    let authority = Authority::shared(Owner::Agent);
    let store = Arc::new(RollbackFailureStore {
        authority: Arc::clone(&authority),
        replacements: std::sync::atomic::AtomicUsize::new(0),
        value: std::sync::Mutex::new(None),
    });
    let mut runtime = Runtime::desktop_with_profile(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![fixture.url.clone()],
        Arc::clone(&authority),
        Arc::new(AtomicBool::new(false)),
        Arc::clone(&store) as Arc<dyn mars::profile::ProfileStore>,
    )
    .unwrap();
    call(&mut runtime, "page.navigate", json!({"url":fixture.url}), 1).unwrap();
    let response = format!("{}/set-cookie", fixture.url);
    let error = call(&mut runtime, "page.navigate", json!({"url":response}), 1).unwrap_err();
    assert_eq!(error.code, "profile_commit_uncertain");
    let epoch = authority.lock().unwrap().epoch;
    let requests = fixture.requests.load(Ordering::Relaxed);
    assert_eq!(
        runtime
            .handle_as(
                Request::new(
                    "test",
                    "page.navigate",
                    json!({"url":fixture.url,"tab_id":"tab-1"})
                ),
                Permit {
                    owner: Owner::Human,
                    epoch,
                    tab_id: Some(1),
                },
            )
            .unwrap_err()
            .code,
        "profile_commit_uncertain"
    );
    assert_eq!(fixture.requests.load(Ordering::Relaxed), requests);
    drop(runtime);
    let mut restarted = Runtime::desktop_with_profile(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![fixture.url.clone()],
        Authority::shared(Owner::Agent),
        Arc::new(AtomicBool::new(false)),
        store as Arc<dyn mars::profile::ProfileStore>,
    )
    .unwrap();
    assert_eq!(
        call(
            &mut restarted,
            "page.navigate",
            json!({"url":fixture.url}),
            1
        )
        .unwrap_err()
        .code,
        "profile_corrupt"
    );
}

/** A failed explicit clear blocks the live jar until secure deletion succeeds. */
#[test]
fn failed_profile_clear_blocks_cookie_use_until_clear_succeeds() {
    use mars::profile::test_support::MemoryStore;
    let html = "<html><body><p>signed in</p></body></html>";
    let response = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nSet-Cookie: session=private-value; Path=/; SameSite=Lax\r\nContent-Length: {}\r\n\r\n{html}", html.len()).into_bytes();
    let fixture = Fixture::raw(response);
    let store = Arc::new(MemoryStore::default());
    let mut runtime = Runtime::desktop_with_profile(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![fixture.url.clone()],
        Authority::shared(Owner::Agent),
        Arc::new(AtomicBool::new(false)),
        Arc::clone(&store) as Arc<dyn mars::profile::ProfileStore>,
    )
    .unwrap();
    call(&mut runtime, "page.navigate", json!({"url":fixture.url}), 1).unwrap();
    store.fail_next_clear();
    let error = runtime.clear_profile().unwrap_err();
    assert_eq!(error.code, "profile_commit_uncertain");
    assert!(
        !serde_json::to_string(&error)
            .unwrap()
            .contains("private-value")
    );
    let requests = fixture.requests.load(Ordering::Relaxed);
    assert_eq!(
        call(&mut runtime, "page.navigate", json!({"url":fixture.url}), 1)
            .unwrap_err()
            .code,
        "profile_commit_uncertain"
    );
    assert_eq!(fixture.requests.load(Ordering::Relaxed), requests);
    assert!(store.has_record());
    drop(runtime);
    let mut restarted = Runtime::desktop_with_profile(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![fixture.url.clone()],
        Authority::shared(Owner::Agent),
        Arc::new(AtomicBool::new(false)),
        Arc::clone(&store) as Arc<dyn mars::profile::ProfileStore>,
    )
    .unwrap();
    assert_eq!(
        call(
            &mut restarted,
            "page.navigate",
            json!({"url":fixture.url}),
            1
        )
        .unwrap_err()
        .code,
        "profile_corrupt"
    );
    restarted.clear_profile().unwrap();
    assert!(!store.has_record());
    assert!(
        call(
            &mut restarted,
            "page.navigate",
            json!({"url":fixture.url}),
            1
        )
        .is_ok()
    );
    assert!(!fixture.last_request.lock().unwrap().contains("Cookie:"));
}

/** A failed clear and failed marker give an explicit no-restart recovery error. */
#[test]
fn failed_clear_and_marker_report_restart_risk_until_clear_succeeds() {
    use mars::profile::test_support::MemoryStore;
    let fixture = Fixture::pages();
    let store = Arc::new(MemoryStore::default());
    let mut runtime = Runtime::desktop_with_profile(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![fixture.url.clone()],
        Authority::shared(Owner::Agent),
        Arc::new(AtomicBool::new(false)),
        Arc::clone(&store) as Arc<dyn mars::profile::ProfileStore>,
    )
    .unwrap();
    call(&mut runtime, "page.navigate", json!({"url":fixture.url}), 1).unwrap();
    store.fail_next_clear();
    store.fail_next_replace();
    assert_eq!(
        runtime.clear_profile().unwrap_err().code,
        "profile_recovery_unpersisted"
    );
    let requests = fixture.requests.load(Ordering::Relaxed);
    assert_eq!(
        call(&mut runtime, "page.navigate", json!({"url":fixture.url}), 1)
            .unwrap_err()
            .code,
        "profile_recovery_unpersisted"
    );
    assert_eq!(fixture.requests.load(Ordering::Relaxed), requests);
    runtime.clear_profile().unwrap();
    assert!(!store.has_record());
}
/** Lookup uses author IDs solely for discovery; mutations use opaque native IDs. */
fn target(page: &Value, id: &str) -> Value {
    let node = page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["html_id"] == id)
        .unwrap();
    json!({"page_id":page["page_id"],"node_id":node["node_id"],"expected_revision":page["revision"]})
}

/** Input, click and script execution publish one document consumed by both layout and UI. */
#[test]
fn input_and_v9_events_change_the_same_visible_document() {
    let fixture = fixture();
    let mut runtime = runtime(&fixture.url, Authority::shared(Owner::Agent));
    let page = call(&mut runtime, "page.navigate", json!({"url":fixture.url}), 1).unwrap();
    let mut params = target(&page, "name");
    params["text"] = json!("Rust agent");
    call(&mut runtime, "page.type", params, 1).unwrap();
    let current = call(&mut runtime, "page.inspect", json!({}), 1).unwrap();
    let changed = call(&mut runtime, "page.click", target(&current, "greet"), 1).unwrap();
    let result = changed["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["html_id"] == "result")
        .unwrap();
    assert_eq!(result["text"], "Hello, Rust agent! From your own Rust V9.");
    assert_eq!(changed["revision"], 3);
    let layout = call(&mut runtime, "page.layout", json!({}), 1).unwrap();
    let painted = layout["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["node_id"] == result["node_id"])
        .unwrap();
    assert_eq!(painted["lines"][0]["text"], result["text"]);
    assert_eq!(painted["y"], result["bounds"]["y"]);
    assert_eq!(runtime.snapshot().inspection, changed);
    assert_eq!(
        call(&mut runtime, "page.click", target(&current, "greet"), 1)
            .unwrap_err()
            .code,
        "revision_conflict"
    );
}
/** Typed values survive private service messages; a handler that throws keeps the writes made
before the throw, as a browser does, and the inspection, layout and console agree on them. */
#[test]
fn source_guided_arithmetic_reaches_visible_layout_without_partial_publication() {
    let source = "<p id=out>Initial</p><button id=calculate onclick='calculate(4,7)'>Calculate</button><button id=fail onclick='fail()'>Fail</button><script>const model={values:[2,3]};function sum(a,b){return a+b;}let total=sum(model.values[0],model.values[1])+9;model.values[0]=total;document.getElementById('out').textContent='Ready: '+model.values[0];function calculate(a,b){let next=sum(a,b);if(next>10){model.values[0]=next;}else{model.values[0]=10;}document.getElementById('out').textContent=model.values[0]+model.values.length;return model.values[0];}function fail(){model.values[0]=99;document.getElementById('out').textContent='partial';missing();}</script>";
    let fixture = Fixture::raw(
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{source}",
            source.len()
        )
        .into_bytes(),
    );
    let mut runtime = runtime(&fixture.url, Authority::shared(Owner::Agent));
    let initial = call(&mut runtime, "page.navigate", json!({"url":fixture.url}), 1).unwrap();
    assert_eq!(initial["nodes"][0]["text"], "Ready: 14");
    let updated = call(&mut runtime, "page.click", target(&initial, "calculate"), 1).unwrap();
    assert_eq!(updated["nodes"][0]["text"], "13");
    let layout = call(&mut runtime, "page.layout", json!({}), 1).unwrap();
    assert_eq!(layout["items"][0]["lines"][0]["text"], "13");
    assert_eq!(updated["revision"], 2);
    let failed = call(&mut runtime, "page.click", target(&updated, "fail"), 1).unwrap();
    assert_eq!(failed["nodes"][0]["text"], "partial");
    assert_eq!(failed["revision"], 3);
    assert_console_reports(&mut runtime, "ReferenceError", "missing");
    assert_eq!(
        call(&mut runtime, "page.inspect", json!({}), 1).unwrap(),
        failed
    );
    let layout = call(&mut runtime, "page.layout", json!({}), 1).unwrap();
    assert_eq!(layout["items"][0]["lines"][0]["text"], "partial");
}

/** The console holds one uncaught-error entry naming the error class and its cause. */
fn assert_console_reports(runtime: &mut Runtime, class: &str, cause: &str) {
    let console = call(runtime, "page.console", json!({}), 1).unwrap();
    let entries = console["entries"].to_string();
    assert!(entries.contains("Uncaught"), "{entries}");
    assert!(entries.contains(class), "{entries}");
    assert!(entries.contains(cause), "{entries}");
}

/** Ordered external and inline classic scripts share one candidate and native event program. */
#[test]
fn ordered_classic_scripts_load_events_and_fail_atomically() {
    let fixture = Fixture::pages();
    let mut runtime = runtime(&fixture.url, Authority::shared(Owner::Agent));
    let original = call(
        &mut runtime,
        "page.navigate",
        json!({"url":format!("{}/plain", fixture.url)}),
        1,
    )
    .unwrap();
    let ordered = call(
        &mut runtime,
        "page.navigate",
        json!({"url":format!("{}/ordered", fixture.url)}),
        1,
    )
    .unwrap();
    assert_eq!(node(&ordered, "out")["text"], "11");
    let shown = call(&mut runtime, "page.click", target(&ordered, "show"), 1).unwrap();
    assert_eq!(node(&shown, "out")["text"], "11");
    let inline = call(
        &mut runtime,
        "page.navigate",
        json!({"url":format!("{}/inline", fixture.url)}),
        1,
    )
    .unwrap();
    assert_eq!(node(&inline, "out")["text"], "8");

    // A failing, mistyped, missing or redirected script stops only itself; the page commits.
    for path in [
        "later-failure",
        "wrong-mime",
        "wrong-query",
        "missing-script",
        "redirect-script",
    ] {
        let requests = fixture.requests.load(Ordering::Relaxed);
        let page = call(
            &mut runtime,
            "page.navigate",
            json!({"url":format!("{}/{path}", fixture.url)}),
            1,
        )
        .unwrap();
        assert_eq!(page["degraded"], json!(["scripts"]), "{path}");
        let diagnostics = page["diagnostics"].to_string();
        assert!(diagnostics.contains("did not run"), "{path}: {diagnostics}");
        if path == "later-failure" {
            assert!(diagnostics.contains("Script 2 "), "{diagnostics}");
        }
        if path == "redirect-script" {
            assert_eq!(fixture.requests.load(Ordering::Relaxed) - requests, 2);
        }
        if path == "wrong-query" {
            assert!(!diagnostics.contains("private-test"));
        }
    }
    assert_eq!(original["page_id"], "page-1");
}

/** Static PNGs share exact agent/native bounds; failed optional images use alt text. */
#[test]
fn static_images_paint_with_inspectable_bounds_and_alt_fallback() {
    let fixture = Fixture::pages();
    let mut runtime = runtime(&fixture.url, Authority::shared(Owner::Agent));
    let page = call(
        &mut runtime,
        "page.navigate",
        json!({"url":format!("{}/with-image", fixture.url)}),
        1,
    )
    .unwrap();
    let image = node(&page, "hero");
    assert_eq!(image["role"], "image");
    assert_eq!(image["alt"], "Red pixel");
    assert_eq!(image["bounds"]["width"], 1.0);
    assert_eq!(image["bounds"]["height"], 1.0);
    assert_eq!(runtime.snapshot().images.len(), 1);
    let layout = call(&mut runtime, "page.layout", json!({}), 1).unwrap();
    let item = layout["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["node_id"] == image["node_id"])
        .unwrap();
    assert_eq!(item["x"], image["bounds"]["x"]);
    assert_eq!(item["width"], image["bounds"]["width"]);
    let inline = call(
        &mut runtime,
        "page.navigate",
        json!({"url":format!("{}/inline-image", fixture.url)}),
        1,
    )
    .unwrap();
    let inline_image = node(&inline, "inline");
    let layout = call(&mut runtime, "page.layout", json!({}), 1).unwrap();
    let inline_item = layout["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["node_id"] == inline_image["node_id"])
        .unwrap();
    assert_eq!(inline_item["lines"].as_array().unwrap().len(), 0);
    let siblings = layout["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|item| {
            item["node_id"] != inline_image["node_id"]
                && item["lines"]
                    .as_array()
                    .is_some_and(|lines| !lines.is_empty())
        })
        .collect::<Vec<_>>();
    // The image sits on the baseline of the text line it shares, inside that line's extent.
    let (top, bottom) = (
        inline_item["y"].as_f64().unwrap(),
        inline_item["y"].as_f64().unwrap() + inline_item["height"].as_f64().unwrap(),
    );
    assert!(
        siblings.iter().any(|item| {
            let (y, height) = (
                item["y"].as_f64().unwrap(),
                item["height"].as_f64().unwrap(),
            );
            y <= top && bottom <= y + height
        }),
        "{inline_item} not inside any text line of {siblings:?}"
    );
    let missing = call(
        &mut runtime,
        "page.navigate",
        json!({"url":format!("{}/missing-image", fixture.url)}),
        1,
    )
    .unwrap();
    assert_eq!(node(&missing, "missing")["text"], "Missing picture");
    assert!(runtime.snapshot().images.is_empty());
}

/** Static table grids expose headers and identical human/agent cell geometry; misplaced table parts still read. */
#[test]
fn tables_render_bounded_cells_and_header_relationships() {
    let fixture = Fixture::pages();
    let mut runtime = runtime(&fixture.url, Authority::shared(Owner::Agent));
    let page = call(
        &mut runtime,
        "page.navigate",
        json!({"url":format!("{}/table", fixture.url)}),
        1,
    )
    .unwrap();
    let amount = node(&page, "tea-amount");
    assert_eq!(amount["row_index"], 2);
    assert_eq!(amount["column_index"], 2);
    assert_eq!(amount["header_labels"], json!(["Amount", "Tea"]));
    assert!(amount["bounds"]["width"].as_f64().unwrap() > 0.0);
    let wide = runtime.snapshot().display.unwrap();
    runtime.set_viewport(320.0, 400.0).unwrap();
    let narrow = runtime.snapshot();
    let cell = narrow
        .display
        .as_ref()
        .unwrap()
        .items
        .iter()
        .find(|item| {
            item.local_id
                == amount["node_id"]
                    .as_str()
                    .unwrap()
                    .rsplit("node-")
                    .next()
                    .unwrap()
                    .parse::<usize>()
                    .unwrap()
        })
        .unwrap();
    assert!(cell.x + cell.width <= 320.0);
    assert!(narrow.display.as_ref().unwrap().height >= wide.height);
    let coffee = node(&page, "coffee-amount");
    assert!(coffee["bounds"]["width"].as_f64().unwrap() >= 136.0);
    let orphan = call(
        &mut runtime,
        "page.navigate",
        json!({"url":format!("{}/bad-table", fixture.url)}),
        1,
    )
    .unwrap();
    let text = orphan["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|value| value["text"] == "Orphan")
        .unwrap();
    assert!(text["bounds"]["width"].as_f64().unwrap() > 0.0);
    assert!(
        orphan["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value
                .as_str()
                .unwrap()
                .contains("table part outside any table")),
        "{}",
        orphan["diagnostics"]
    );
    let hidden = call(
        &mut runtime,
        "page.navigate",
        json!({"url":format!("{}/hidden-table", fixture.url)}),
        1,
    )
    .unwrap();
    assert!(node(&hidden, "hidden")["bounds"].is_null());
    let hidden_id = node(&hidden, "hidden")["node_id"]
        .as_str()
        .unwrap()
        .rsplit("node-")
        .next()
        .unwrap()
        .parse::<usize>()
        .unwrap();
    assert!(
        !runtime
            .snapshot()
            .display
            .unwrap()
            .items
            .iter()
            .any(|item| item.local_id == hidden_id)
    );
    let inferred = call(
        &mut runtime,
        "page.navigate",
        json!({"url":format!("{}/empty-header-table", fixture.url)}),
        1,
    )
    .unwrap();
    assert_eq!(node(&inferred, "value")["header_labels"], json!(["Header"]));
    assert!(
        call(
            &mut runtime,
            "page.navigate",
            json!({"url":format!("{}/inert-span", fixture.url)}),
            1
        )
        .is_ok()
    );
}

/** Inspection lookup keeps assertions tied to the visible semantic node. */
fn node(page: &Value, id: &str) -> Value {
    page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|value| value["html_id"] == id)
        .unwrap()
        .clone()
}
/** Binding, heap, storage and DOM proposals pass through the same native transaction: a thrown
handler commits what it wrote before throwing, while an engine limit rolls every domain back. */
#[test]
fn counter_branches_and_failed_heap_storage_candidates_share_one_native_commit() {
    let source = "<p id=count>0</p><p id=branch>Keep going.</p><button id=add onclick='add()'>Add one</button><button id=fail onclick='fail()'>Fail</button><button id=hang onclick='hang()'>Hang</button><button id=show onclick='show()'>Show committed state</button><script>let count=0;const target=3;const model={last:0};function show(){document.getElementById('count').textContent=count+' '+model.last+' '+window.sessionStorage.getItem('count');}function add(){count=count+1;model.last=count;window.sessionStorage.setItem('count',count);show();if(count>=target){document.getElementById('branch').textContent='Target reached.';}}function fail(){count=99;model.last=99;window.sessionStorage.setItem('count',99);show();target=9;}function hang(){count=5;model.last=5;window.sessionStorage.setItem('count',5);show();while(true){}}</script>";
    let fixture = Fixture::raw(
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{source}",
            source.len()
        )
        .into_bytes(),
    );
    let mut runtime = runtime(&fixture.url, Authority::shared(Owner::Agent));
    let mut page = call(&mut runtime, "page.navigate", json!({"url":fixture.url}), 1).unwrap();
    for count in 1..=3 {
        page = call(&mut runtime, "page.click", target(&page, "add"), 1).unwrap();
        assert_eq!(page["nodes"][0]["text"], format!("{count} {count} {count}"));
    }
    assert_eq!(page["nodes"][1]["text"], "Target reached.");
    let before = page.clone();
    let failed = call(&mut runtime, "page.click", target(&page, "fail"), 1).unwrap();
    assert_eq!(failed["nodes"][0]["text"], "99 99 99");
    assert_console_reports(&mut runtime, "TypeError", "constant");
    assert_eq!(
        call(&mut runtime, "page.inspect", json!({}), 1).unwrap(),
        failed
    );
    let error = call(&mut runtime, "page.click", target(&failed, "hang"), 1).unwrap_err();
    assert_eq!(error.code, "limit_exceeded");
    assert_eq!(
        call(&mut runtime, "page.inspect", json!({}), 1).unwrap(),
        failed
    );
    let after = call(&mut runtime, "page.click", target(&failed, "show"), 1).unwrap();
    assert_eq!(after["nodes"][0]["text"], "99 99 99");
    assert_eq!(
        call(&mut runtime, "page.click", target(&before, "add"), 1)
            .unwrap_err()
            .code,
        "revision_conflict"
    );
    let layout = call(&mut runtime, "page.layout", json!({}), 1).unwrap();
    assert_eq!(layout["items"][0]["lines"][0]["text"], "99 99 99");
}
/** Standard-for execution reaches native layout; a throw mid-loop keeps the iteration's writes
across every state domain and reports the error to the console. */
#[test]
fn standard_for_sum_events_and_failed_updates_share_the_visible_transaction() {
    let source = "<p id=out>Initial</p><button id=add onclick='add()'>Add one</button><button id=fail onclick='fail()'>Fail</button><button id=show onclick='show()'>Show</button><script>let count=0;const model={sum:0};function add(){count=count+1;let sum=0;for(let i=1;i<=count;i=i+1){sum=sum+i;}model.sum=sum;window.sessionStorage.setItem('sum',sum);document.getElementById('out').textContent=count+' '+sum;}function fail(){for(let i=0;i<1;i=missing){count=99;model.sum=99;window.sessionStorage.setItem('sum',99);document.getElementById('out').textContent='partial';}}function show(){document.getElementById('out').textContent=count+' '+model.sum+' '+window.sessionStorage.getItem('sum');}</script>";
    let fixture = Fixture::raw(
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{source}",
            source.len()
        )
        .into_bytes(),
    );
    let mut runtime = runtime(&fixture.url, Authority::shared(Owner::Agent));
    let mut page = call(&mut runtime, "page.navigate", json!({"url":fixture.url}), 1).unwrap();
    for (count, sum) in [(1, 1), (2, 3), (3, 6), (4, 10)] {
        page = call(&mut runtime, "page.click", target(&page, "add"), 1).unwrap();
        assert_eq!(page["nodes"][0]["text"], format!("{count} {sum}"));
    }
    let failed = call(&mut runtime, "page.click", target(&page, "fail"), 1).unwrap();
    assert_eq!(failed["nodes"][0]["text"], "partial");
    assert_console_reports(&mut runtime, "ReferenceError", "missing");
    assert_eq!(
        call(&mut runtime, "page.inspect", json!({}), 1).unwrap(),
        failed
    );
    let shown = call(&mut runtime, "page.click", target(&failed, "show"), 1).unwrap();
    assert_eq!(shown["nodes"][0]["text"], "99 99 99");
    assert_eq!(
        call(&mut runtime, "page.layout", json!({}), 1).unwrap()["items"][0]["lines"][0]["text"],
        "99 99 99"
    );
}
/** Resize and scrolling update presentation while page identities and revisions stay stable. */
#[test]
fn resize_wraps_and_scroll_clamps_without_mutating_document_revision() {
    let fixture = fixture();
    let mut runtime = runtime(&fixture.url, Authority::shared(Owner::Agent));
    let page = call(&mut runtime, "page.navigate", json!({"url":fixture.url}), 1).unwrap();
    let wide = runtime.snapshot().display.unwrap();
    runtime.set_viewport(640.0, 380.0).unwrap();
    let narrow = runtime.snapshot();
    assert!(narrow.display.as_ref().unwrap().height > wide.height);
    assert_eq!(narrow.inspection["revision"], page["revision"]);
    assert!(narrow.inspection["layout_generation"].as_u64().unwrap() > 1);
    call(
        &mut runtime,
        "page.scroll",
        json!({"page_id":page["page_id"],"expected_revision":1,"y":1e9}),
        1,
    )
    .unwrap();
    let bottom = runtime.snapshot();
    assert_eq!(bottom.scroll_y, bottom.display.unwrap().height - 380.0);
    assert!(runtime.set_viewport(f64::NAN, 380.0).is_err());
}
/** Only successful candidates enter history; stale handles cannot follow a later page. */
#[test]
fn links_history_and_reload_use_committed_urls_only() {
    let fixture = fixture();
    let mut runtime = runtime(&fixture.url, Authority::shared(Owner::Agent));
    let first = call(&mut runtime, "page.navigate", json!({"url":fixture.url}), 1).unwrap();
    let second = call(&mut runtime, "page.click", target(&first, "next"), 1).unwrap();
    assert!(second["url"].as_str().unwrap().ends_with("/second"));
    assert!(runtime.snapshot().can_back);
    let error = call(
        &mut runtime,
        "page.navigate",
        json!({"url":"http://169.254.169.254/"}),
        1,
    )
    .unwrap_err();
    assert!(error.previous_page_retained);
    assert_eq!(runtime.snapshot().inspection, second);
    let back = call(&mut runtime, "page.back", json!({}), 1).unwrap();
    assert_eq!(back["url"], first["url"]);
    assert!(runtime.snapshot().can_forward);
    let forward = call(&mut runtime, "page.forward", json!({}), 1).unwrap();
    assert_eq!(forward["url"], second["url"]);
    let reload = call(&mut runtime, "page.reload", json!({}), 1).unwrap();
    assert_ne!(reload["page_id"], forward["page_id"]);
    assert_eq!(reload["url"], second["url"]);
    assert_eq!(
        call(&mut runtime, "page.click", target(&first, "next"), 1)
            .unwrap_err()
            .code,
        "invalid_target"
    );
}
/** Taking control invalidates old admissions and agents cannot grant themselves a new epoch. */
#[test]
fn human_takeover_and_release_reject_old_control_epochs() {
    let fixture = fixture();
    let authority = Authority::shared(Owner::Agent);
    let mut runtime = runtime(&fixture.url, authority.clone());
    let page = call(&mut runtime, "page.navigate", json!({"url":fixture.url}), 1).unwrap();
    authority.lock().unwrap().change(Owner::Human);
    assert_eq!(
        call(&mut runtime, "page.click", target(&page, "greet"), 1)
            .unwrap_err()
            .code,
        "control_denied"
    );
    authority.lock().unwrap().change(Owner::Agent);
    assert_eq!(
        call(&mut runtime, "page.click", target(&page, "greet"), 1)
            .unwrap_err()
            .code,
        "control_denied"
    );
    call(&mut runtime, "control.release", json!({}), 3).unwrap();
    assert_eq!(authority.lock().unwrap().owner, Owner::Human);
    assert_eq!(
        call(&mut runtime, "control.grant", json!({}), 4)
            .unwrap_err()
            .code,
        "unsupported_operation"
    );
}
/** Revocation occurs during actual network I/O and is checked again at page publication. */
#[test]
fn an_in_flight_navigation_cannot_publish_after_takeover() {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::mpsc,
        time::Duration,
    };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let (accepted, ready) = mpsc::channel();
    let (release, wait) = mpsc::channel();
    let server = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut bytes = [0; 4096];
        let _ = socket.read(&mut bytes).unwrap();
        accepted.send(()).unwrap();
        wait.recv_timeout(Duration::from_secs(2)).unwrap();
        let body = "<html><body><p>late candidate</p></body></html>";
        socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{body}",body.len()).as_bytes()).unwrap();
    });
    let authority = Authority::shared(Owner::Agent);
    let mut runtime = runtime(&origin, authority.clone());
    let worker = thread::spawn(move || {
        let result = call(&mut runtime, "page.navigate", json!({"url":origin}), 1);
        (runtime, result)
    });
    ready.recv_timeout(Duration::from_secs(2)).unwrap();
    authority.lock().unwrap().change(Owner::Human);
    release.send(()).unwrap();
    let (runtime, result) = worker.join().unwrap();
    assert_eq!(result.unwrap_err().code, "control_denied");
    assert!(runtime.snapshot().document.is_none());
    server.join().unwrap();
}

/** Closing an agent-owned tab while navigation is blocked prevents its late commit. */
#[test]
fn closing_tab_during_navigation_rejects_late_publication() {
    let fixture = Fixture::slow();
    let origin = fixture.url.clone();
    let authority = Authority::shared(Owner::Agent);
    let mut runtime = runtime(&origin, authority.clone());
    runtime
        .handle(Request::new("test", "tab.open", json!({})))
        .unwrap();
    authority
        .lock()
        .unwrap()
        .change_tab(2, Owner::Agent)
        .unwrap();
    let permit = authority
        .lock()
        .unwrap()
        .permit_for(Owner::Agent, 2)
        .unwrap();
    let url = origin.clone();
    let worker = thread::spawn(move || {
        let result = runtime.handle_as(
            Request::new(
                "closing",
                "page.navigate",
                json!({"url":url,"tab_id":"tab-2"}),
            ),
            permit,
        );
        (runtime, result)
    });
    authority.lock().unwrap().remove_tab(2);
    let (runtime, result) = worker.join().unwrap();
    assert_eq!(result.unwrap_err().code, "control_denied");
    assert!(runtime.snapshot().document.is_none());
    drop(fixture);
}
/** Closing the owner cancels child RPC work without waiting for the network budget. */
#[test]
fn window_cancellation_reaps_engines_during_network_wait() {
    use std::time::{Duration, Instant};
    let fixture = Fixture::slow();
    let cancel = Arc::new(AtomicBool::new(false));
    let mut runtime = Runtime::desktop(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![fixture.url.clone()],
        Authority::shared(Owner::Agent),
        cancel.clone(),
    )
    .unwrap();
    let pids = runtime
        .process_ids()
        .into_iter()
        .map(|(_, pid)| pid)
        .collect::<Vec<_>>();
    let url = fixture.url.clone();
    let worker = thread::spawn(move || call(&mut runtime, "page.navigate", json!({"url":url}), 1));
    let deadline = Instant::now() + Duration::from_secs(2);
    while fixture.requests.load(Ordering::Relaxed) == 0 {
        assert!(Instant::now() < deadline);
        thread::sleep(support::POLL);
    }
    let started = Instant::now();
    cancel.store(true, Ordering::Relaxed);
    assert!(worker.join().unwrap().is_err());
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "{:?}",
        started.elapsed()
    );
    support::assert_gone(&pids);
}

/** Voluntary cancellation interrupts blocked I/O without exhausting the crash-recovery budget. */
#[test]
fn repeated_navigation_stop_retires_only_the_waiting_service() {
    use std::time::{Duration, Instant};
    let authority = Authority::shared(Owner::Agent);
    let mut fixtures = (0..5).map(|_| Fixture::slow()).collect::<Vec<_>>();
    let fast = fixture();
    let mut origins = fixtures
        .iter()
        .map(|fixture| fixture.url.clone())
        .collect::<Vec<_>>();
    origins.push(fast.url.clone());
    let mut runtime = Runtime::desktop(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        origins,
        authority.clone(),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    call(&mut runtime, "page.navigate", json!({"url":fast.url}), 1).unwrap();
    let committed = runtime.snapshot().inspection["page_id"].clone();
    for slow in fixtures.drain(..) {
        let instances = |runtime: &Runtime| {
            runtime
                .service_instances()
                .into_iter()
                .map(|(name, instance)| (name.to_owned(), instance))
                .collect::<std::collections::BTreeMap<_, _>>()
        };
        let before = instances(&runtime);
        let network_pid = runtime
            .process_ids()
            .into_iter()
            .find(|(name, _)| *name == "network")
            .map(|(_, pid)| pid)
            .unwrap();
        let permit = authority
            .lock()
            .unwrap()
            .permit_for(Owner::Agent, 1)
            .unwrap();
        let url = slow.url.clone();
        let worker = thread::spawn(move || {
            let result = runtime.handle_as(
                Request::new(
                    "stop",
                    "page.navigate",
                    json!({"url":url,"tab_id":"tab-1","control_epoch":permit.epoch}),
                ),
                permit,
            );
            (runtime, result)
        });
        let deadline = Instant::now() + Duration::from_secs(2);
        while slow.requests.load(Ordering::Relaxed) == 0 {
            assert!(Instant::now() < deadline);
            thread::sleep(support::POLL);
        }
        let started = Instant::now();
        authority
            .lock()
            .unwrap()
            .change_tab(1, Owner::Agent)
            .unwrap();
        let (returned, result) = worker.join().unwrap();
        runtime = returned;
        assert_eq!(result.unwrap_err().code, "control_denied");
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "{:?}",
            started.elapsed()
        );
        assert_eq!(runtime.snapshot().inspection["page_id"], committed);
        let after = instances(&runtime);
        assert_ne!(before["network"], after["network"]);
        assert_eq!(before["document"], after["document"]);
        assert_eq!(before["v9"], after["v9"]);
        support::assert_gone(&[network_pid]);
    }
    let epoch = authority
        .lock()
        .unwrap()
        .permit_for(Owner::Agent, 1)
        .unwrap()
        .epoch;
    call(
        &mut runtime,
        "page.navigate",
        json!({"url":fast.url}),
        epoch,
    )
    .unwrap();
}

/** Revocation before request admission preserves idle engines and committed script state. */
#[test]
fn stop_at_first_paint_preserves_idle_v9_and_committed_state() {
    let fixture = Fixture::routes(vec![
        (
            "/",
            support::html("", "<p>committed</p><script>window.marker=42</script>"),
        ),
        (
            "/next",
            support::html("", "<p>candidate</p><script>window.marker=0</script>"),
        ),
    ]);
    let authority = Authority::shared(Owner::Agent);
    let mut runtime = runtime(&fixture.url, authority.clone());
    call(&mut runtime, "page.navigate", json!({"url":fixture.url}), 1).unwrap();
    call(&mut runtime, "page.devtools", json!({"request":{"kind":"evaluate","expression":"window.marker=77","options":{"return_by_value":true}}}), 1).unwrap();
    let before = runtime
        .process_ids()
        .into_iter()
        .map(|(_, pid)| pid)
        .collect::<Vec<_>>();
    let committed = runtime.snapshot().inspection["page_id"].clone();
    let stopping = authority.clone();
    runtime.show_first_paint(Box::new(move |_| {
        stopping
            .lock()
            .unwrap()
            .change_tab(1, Owner::Agent)
            .unwrap();
    }));
    let error = call(
        &mut runtime,
        "page.navigate",
        json!({"url":format!("{}/next",fixture.url)}),
        1,
    )
    .unwrap_err();
    assert_eq!(error.code, "control_denied");
    assert_eq!(
        runtime
            .process_ids()
            .into_iter()
            .map(|(_, pid)| pid)
            .collect::<Vec<_>>(),
        before
    );
    assert_eq!(runtime.snapshot().inspection["page_id"], committed);
    let epoch = authority
        .lock()
        .unwrap()
        .permit_for(Owner::Agent, 1)
        .unwrap()
        .epoch;
    let value = call(&mut runtime, "page.devtools_peek", json!({"request":{"kind":"evaluate","expression":"window.marker","options":{"return_by_value":true}}}), epoch).unwrap();
    assert_eq!(value["result"]["value"], 77);
}

/** Native selection refreshes current option state before trusted input/change handlers run. */
#[test]
fn selecting_an_optgroup_option_updates_script_reads() {
    let body = "<select id=s><option value=a>A</option><optgroup><option id=b value=b>B</option></optgroup></select><p id=result>before</p><script>const s=document.getElementById('s');s.value='a';s.addEventListener('change',()=>{document.getElementById('result').textContent=s.value+':'+s.selectedIndex})</script>";
    let fixture = Fixture::raw(
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        )
        .into_bytes(),
    );
    let mut runtime = runtime(&fixture.url, Authority::shared(Owner::Agent));
    let page = call(&mut runtime, "page.navigate", json!({"url":fixture.url}), 1).unwrap();
    let nodes = page["nodes"].as_array().unwrap();
    let select = nodes.iter().find(|node| node["html_id"] == "s").unwrap();
    let option = nodes.iter().find(|node| node["html_id"] == "b").unwrap();
    let selected = call(&mut runtime,"page.select",json!({"page_id":page["page_id"],"expected_revision":page["revision"],"node_id":select["node_id"],"option_id":option["node_id"]}),1).unwrap();
    assert!(
        selected["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|node| node["html_id"] == "result" && node["text"] == "b:1"),
        "{selected}"
    );
}

/** Vendor animation descriptors do not disable a real document's link controls. */
#[test]
fn unsupported_vendor_keyframes_do_not_block_native_link_activation() {
    let fixture = Fixture::routes(vec![
        (
            "/",
            support::html(
                "",
                "<style>@-moz-keyframes spinner{0%,39%,100%{opacity:0}40%{opacity:1}}</style><a id=next href=/next>Next</a>",
            ),
        ),
        ("/next", support::html("", "<h1>Destination</h1>")),
    ]);
    let mut runtime = runtime(&fixture.url, Authority::shared(Owner::Human));
    let page = human_page(&mut runtime, 1, "page.navigate", json!({"url":fixture.url})).unwrap();
    let link = page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["html_id"] == "next")
        .unwrap();
    assert_eq!(link["enabled"], true);
    let destination = human_page(&mut runtime, 1, "page.click", json!({"page_id":page["page_id"],"expected_revision":page["revision"],"node_id":link["node_id"]})).unwrap();
    assert!(destination["url"].as_str().unwrap().ends_with("/next"));
}

/** V9 selectedness resets are replayed exactly into the native document after option removal. */
#[test]
fn removing_selected_option_updates_the_native_dropdown() {
    let source = "<select id=s><option value=a>A</option><option value=b>B</option></select><p id=out></p><script>const s=document.getElementById('s');s.value='b';s.options[1].remove();document.getElementById('out').textContent=s.value+':'+s.selectedIndex;</script>";
    let fixture = Fixture::raw(support::html("", source));
    let mut runtime = runtime(&fixture.url, Authority::shared(Owner::Human));
    let page = human_page(&mut runtime, 1, "page.navigate", json!({"url":fixture.url})).unwrap();
    let nodes = page["nodes"].as_array().unwrap();
    assert_eq!(
        nodes.iter().find(|node| node["html_id"] == "out").unwrap()["text"],
        "a:0"
    );
    assert_eq!(
        nodes.iter().find(|node| node["tag"] == "option").unwrap()["selected"],
        true
    );
    assert!(
        !page["degraded"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item == "scripts")
    );
}

/** Moving a selected option repairs both dropdowns before native change replay. */
#[test]
fn moving_selected_options_replays_one_selection_per_native_dropdown() {
    let source = "<select id=s><option value=a>A</option><option value=b>B</option></select><select id=t><option value=c>C</option></select><p id=out></p><script>const s=document.getElementById('s'),t=document.getElementById('t');s.value='b';const b=s.options[1];b.remove();s.appendChild(b);t.appendChild(b);document.getElementById('out').textContent=s.value+':'+t.value;</script>";
    let fixture = Fixture::raw(support::html("", source));
    let mut runtime = runtime(&fixture.url, Authority::shared(Owner::Human));
    let page = human_page(&mut runtime, 1, "page.navigate", json!({"url":fixture.url})).unwrap();
    assert!(
        !page["degraded"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item == "scripts"),
        "{}",
        page["diagnostics"]
    );
    let nodes = page["nodes"].as_array().unwrap();
    assert_eq!(
        nodes.iter().find(|node| node["html_id"] == "out").unwrap()["text"],
        "a:b"
    );
    assert_eq!(
        nodes
            .iter()
            .filter(|node| node["tag"] == "option" && node["selected"] == true)
            .count(),
        2
    );
    assert!(
        !page["degraded"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item == "scripts")
    );
}
