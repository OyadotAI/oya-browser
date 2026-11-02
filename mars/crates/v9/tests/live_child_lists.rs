/** Live child lists in V9 (`children`, `childNodes`): index, length and item read the element's
current children, skip or keep text as the list says, and refresh after insertions and removals. */
use crate::common;
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
        module_keys: &[],
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

/** children length and indexes count element children and follow later insertions. */
#[test]
fn children_length_and_index_follow_insertions() {
    let source = "const p=document.createElement('div');document.body.append(p);\
        for(let i=0;i<3;i++){p.append(document.createElement('span'));}\
        const list=p.children;const before=list.length;\
        const added=document.createElement('b');p.append(added);\
        const r=[before,list.length,list[3]===added,list[0]===p.firstElementChild].join('|');";
    assert_eq!(eval(source, "r"), "3|4|true|true");
}

/** children leaves text out, while childNodes keeps it. */
#[test]
fn children_skip_text_and_child_nodes_keep_it() {
    let source = "const p=document.createElement('div');document.body.append(p);\
        p.append('words');p.append(document.createElement('i'));\
        const r=[p.children.length,p.childNodes.length,p.children[0].tagName].join('|');";
    assert_eq!(eval(source, "r"), "1|2|I");
}

/** An index past the end is undefined, and item answers null there and below zero. */
#[test]
fn index_past_the_end_is_undefined_and_item_is_null() {
    let source = "const p=document.createElement('div');document.body.append(p);\
        p.append(document.createElement('i'));\
        const r=[p.children[5]===undefined,p.children.item(5)===null,\
        p.children.item(-1)===null,p.children.item(0)===p.children[0]].join('|');";
    assert_eq!(eval(source, "r"), "true|true|true|true");
}

/** A removal is seen by the next read: the list shrinks and its first index moves on. */
#[test]
fn removal_refreshes_the_list() {
    let source = "const p=document.createElement('div');document.body.append(p);\
        const a=document.createElement('i');const b=document.createElement('u');\
        p.append(a);p.append(b);const list=p.children;const first=list[0];\
        p.removeChild(a);\
        const r=[first===a,list.length,list[0]===b].join('|');";
    assert_eq!(eval(source, "r"), "true|1|true");
}

/** An insertion before the first child is seen by the next index read. */
#[test]
fn insertion_at_the_front_is_seen_by_the_next_read() {
    let source = "const p=document.createElement('div');document.body.append(p);\
        p.append(document.createElement('i'));const list=p.children;const warm=list[0];\
        const front=document.createElement('em');p.insertBefore(front,p.firstChild);\
        const r=[warm!==undefined,list[0]===front,list.length].join('|');";
    assert_eq!(eval(source, "r"), "true|true|2");
}
