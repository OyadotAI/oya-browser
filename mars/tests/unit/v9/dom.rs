/** Projected DOM tests exercise retained references, callback dispatch and candidate rollback publicly. */
use mars::{
    document::{ScriptDocument, parse, script_document},
    v9::{Execution, StorageState, VmState, click_document_scripts, event, load_document_scripts},
};
use serde_json::json;

/** All capabilities originate from actual parsed elements rather than handcrafted permissive nodes. */
fn document(html: &str) -> ScriptDocument {
    script_document(&parse(html).unwrap(), "page-a").unwrap()
}
/** A small page gives every test a real editable input, ordinary button and text target. */
fn page() -> ScriptDocument {
    document(
        "<input id=name value=Ada><p id=out>initial</p><button id=button type=button>Click</button>",
    )
}
/** Source is parsed in projected mode and receives a fresh isolated storage candidate. */
fn load(source: &str, document: &ScriptDocument) -> Execution {
    load_document_scripts(&[source.into()], document.clone(), StorageState::default()).unwrap()
}
/** Serialized state round trips through the same public API used by private service requests. */
fn click(source: &str, state: VmState, document: &ScriptDocument) -> Execution {
    let state = serde_json::from_value(serde_json::to_value(state).unwrap()).unwrap();
    click_document_scripts(
        &[source.into()],
        document.get_by_id("button").unwrap(),
        state,
        document.clone(),
        StorageState::default(),
    )
    .unwrap()
}
/** Text proposals expose script results without a privileged evaluator or fabricated DOM. */
fn result(source: &str, expression: &str) -> String {
    let executed = load(
        &format!("{source}document.getElementById('out').textContent={expression};"),
        &page(),
    );
    executed.changes.last().unwrap().text().unwrap().to_owned()
}

mod inline;
mod listeners;
mod lookup;
mod validation;
