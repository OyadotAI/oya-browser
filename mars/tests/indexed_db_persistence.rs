/** IndexedDB through the real coordinator and V9 service: a page's committed database survives a reload and a runtime restart over the same profile store, and clearing saved site storage removes it. */
#[allow(dead_code)]
mod support;
use mars::{
    protocol::Request,
    runtime::{
        EnginePrograms, Runtime,
        authority::{Authority, Owner, Permit},
    },
};
use serde_json::{Value, json};
use std::{
    path::Path,
    sync::{Arc, atomic::AtomicBool},
};
use support::Fixture;

/** A page that counts its visits in IndexedDB and shows the count in its title once the write commits. */
const PAGE: &str = r#"<html><head><title>loading</title></head><body><script>
const open = indexedDB.open('visits', 1);
open.onupgradeneeded = () => open.result.createObjectStore('counter');
open.onsuccess = () => {
  const tx = open.result.transaction('counter', 'readwrite');
  const store = tx.objectStore('counter');
  store.get('n').onsuccess = (e) => {
    const n = (e.target.result || 0) + 1;
    store.put(n, 'n');
    tx.oncomplete = () => { document.title = 'visits ' + n; };
  };
};
open.onerror = () => { document.title = 'error ' + open.error.name; };
</script></body></html>"#;

/** A loopback fixture serving PAGE. */
fn fixture() -> Fixture {
    Fixture::raw(
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{PAGE}",
            PAGE.len()
        )
        .into_bytes(),
    )
}

/** A desktop runtime over the given Web Storage store. */
fn runtime(
    url: &str,
    storage: &Arc<mars::profile::test_support::MemoryWebStorageStore>,
) -> Runtime {
    Runtime::desktop_with_profiles(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![url.into()],
        Authority::shared(Owner::Agent),
        Arc::new(AtomicBool::new(false)),
        Arc::new(mars::profile::test_support::MemoryStore::default()),
        Arc::clone(storage) as Arc<dyn mars::profile::WebStorageStore>,
    )
    .unwrap()
}

/** Navigate tab 1 as the agent and return the selected tab's title. */
fn visit(runtime: &mut Runtime, url: &str) -> Value {
    runtime
        .handle_as(
            Request::new(
                "test",
                "page.navigate",
                json!({"url":url,"tab_id":"tab-1","control_epoch":1}),
            ),
            Permit {
                owner: Owner::Agent,
                epoch: 1,
                tab_id: Some(1),
            },
        )
        .unwrap();
    runtime.snapshot().tabs["tabs"][0]["title"].clone()
}

/** The count survives a reload and a restart, and clearing saved site storage starts it over. */
#[test]
fn indexed_db_survives_reload_and_restart_until_cleared() {
    let fixture = fixture();
    let storage = Arc::new(mars::profile::test_support::MemoryWebStorageStore::default());
    let mut first = runtime(&fixture.url, &storage);
    assert_eq!(visit(&mut first, &fixture.url), "visits 1");
    assert_eq!(visit(&mut first, &fixture.url), "visits 2");
    drop(first);
    let mut restarted = runtime(&fixture.url, &storage);
    assert_eq!(visit(&mut restarted, &fixture.url), "visits 3");
    restarted.clear_web_storage().unwrap();
    assert_eq!(visit(&mut restarted, &fixture.url), "visits 1");
}
