/** Public projected-DOM lookup rules use the shared real-document fixture. */
use super::*;

/** Replacing an ancestor's text detaches a button even while its listener or a retained event still holds it. */
#[test]
fn descendant_removal_detaches_nodes_held_by_listeners_and_retained_events() {
    let document =
        document("<div id=parent><button id=button type=button>Click</button></div><p id=out></p>");
    let source = "document.getElementById('button').addEventListener('click',()=>{});document.getElementById('parent').textContent='removed';document.getElementById('out').textContent=document.getElementById('button')===null;";
    let loaded = load(source, &document);
    assert_eq!(loaded.changes.last().unwrap().text().unwrap(), "true");
    let source = "let saved=0;function remove(e){saved=e;this.removeEventListener('click',remove);document.getElementById('parent').textContent='removed';}document.getElementById('button').addEventListener('click',remove);";
    let clicked = click(source, load(source, &document).state, &document);
    assert_eq!(clicked.changes[0].text().unwrap(), "removed");
}

/** Lookup uses actual author identifiers, primitive DOMString conversion and stable host identities. */
#[test]
fn document_lookup_converts_primitives_and_returns_null_for_missing_or_empty_ids() {
    let document = document(
        "<p id=out></p><p id=7></p><p id=undefined></p><p id=true></p><p id=same>first</p><p id=same>second</p>",
    );
    let executed = load(
        "const lookup=document.getElementById;const first=lookup.call(document,'same');document.getElementById('out').textContent=(first===document.getElementById('same'))+' '+(document.getElementById('')===null)+' '+(document.getElementById('absent')===null)+' '+(document.getElementById(7)!==null)+' '+(document.getElementById()!==null)+' '+(document.getElementById(true)!==null);",
        &document,
    );
    assert_eq!(
        executed.changes[0].text().unwrap(),
        "true true true true true true"
    );
}

/** Retained input references read the current candidate value instead of the load-time string. */
#[test]
fn cached_input_references_observe_current_nonsecret_field_values() {
    let source = "const field=document.getElementById('name');const out=document.getElementById('out');document.getElementById('button').addEventListener('click',()=>{out.textContent=field.value||'world';});";
    let mut document = page();
    let mut state = load(source, &document).state;
    for name in ["Lin", "Rust 🐙", ""] {
        document
            .nodes
            .iter_mut()
            .find(|node| node.html_id == "name")
            .unwrap()
            .value = Some(name.into());
        let executed = click(source, state, &document);
        assert_eq!(
            executed.changes[0].text().unwrap(),
            if name.is_empty() { "world" } else { name }
        );
        state = executed.state;
    }
}

/** Password and hidden field values never reach scripts: value, attribute and serialization all omit them. */
#[test]
fn protected_field_values_are_never_readable() {
    for kind in ["password", "hidden"] {
        let html = format!("<input id=secret type={kind} value=private><p id=out></p>");
        let source = "const s=document.getElementById('secret');document.getElementById('out').textContent=[s.value,s.getAttribute('value'),s.defaultValue,s.outerHTML,document.body.innerHTML].join('|');";
        let executed = load(source, &document(&html));
        let text = executed.changes[0].text().unwrap();
        assert!(!text.contains("private"), "{kind}: {text}");
        assert!(
            !serde_json::to_string(&executed.state)
                .unwrap()
                .contains("private")
        );
    }
}

/** Host methods called on a receiver of the wrong interface throw TypeError (Illegal invocation). */
#[test]
fn detached_host_methods_require_the_matching_document_element_or_event_brand() {
    for source in [
        "const lookup=document.getElementById;lookup('out');",
        "document.getElementById.call({},'out');",
    ] {
        let error =
            load_document_scripts(&[source.into()], page(), StorageState::default()).unwrap_err();
        assert!(
            error.message.contains("TypeError"),
            "{source}: {}",
            error.message
        );
    }
    load(
        "const b=document.getElementById('button');b.addEventListener.call(document,'click',()=>{});",
        &page(),
    );
}

/** Detachment becomes visible immediately, and scripts may keep references to detached nodes. */
#[test]
fn descendant_removal_updates_lookup_and_keeps_retained_detached_nodes() {
    let document = document(
        "<div id=parent><p id=child>nested</p></div><p id=out></p><button id=button type=button>Click</button>",
    );
    let good = load(
        "document.getElementById('parent').textContent='new';document.getElementById('out').textContent=document.getElementById('child')===null;",
        &document,
    );
    assert_eq!(good.changes[1].text().unwrap(), "true");
    for retained in [
        "const retained=document.getElementById('child');",
        "const box={child:document.getElementById('child')};",
    ] {
        let source = format!("{retained}document.getElementById('parent').textContent='new';");
        let loaded = load(&source, &document);
        assert_eq!(loaded.changes[0].text().unwrap(), "new");
    }
}
