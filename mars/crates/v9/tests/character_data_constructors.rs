/** The `Text` and `Comment` constructors in V9: `new Text(data)` and `new Comment(data)` make the
same nodes as createTextNode and createComment, with an omitted argument giving empty data. */
mod common;
use common::{environment, global, host, page};
use mars_v9::{DomLevel, PageInput, StorageState};

/** Run a script against the test page and return a global, failing on any script failure. */
fn eval(source: &str, name: &str) -> String {
    let host = host();
    let mut environment = environment();
    environment.dom_level = DomLevel::Extended;
    let execution = mars_v9::load_page(PageInput {
        sources: &[source.to_string()],
        source_keys: &[],
        document: page(),
        storage: StorageState::default(),
        host: &host,
        environment: &environment,
        modules: mars_v9::ModuleInput::NONE,
    })
    .expect("page loads");
    assert!(execution.failed.is_empty(), "{:?}", execution.failed);
    global(&execution, name)
}

#[test]
fn new_text_makes_a_text_node_with_its_data() {
    let source = "const t=new Text('a&b');const r=[t.nodeType,t.data,t instanceof Text,t.parentNode].join('|');";
    assert_eq!(eval(source, "r"), "3|a&b|true|");
}

#[test]
fn new_text_without_data_is_empty_not_undefined() {
    let source = "const r=[new Text().data===''].join('|');";
    assert_eq!(eval(source, "r"), "true");
}

#[test]
fn new_comment_makes_a_comment_node_with_its_data() {
    let source = "const c=new Comment('note');const r=[c.nodeType,c.data,c instanceof Comment,new Comment().data===''].join('|');";
    assert_eq!(eval(source, "r"), "8|note|true|true");
}
