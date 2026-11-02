/**
 * DOM traversal rules in V9: whatToShow counts processing instructions, doctypes and script-made
 * documents by their node types, and a filter may not call back into the traversal running it.
 * Each rule has one test, and the refusal paths are covered as well as the happy one.
 */
use crate::common;
use common::{environment, global, host, page};
use mars_v9::{DomLevel, Execution, PageInput, StorageState};

/** Load one script against the test page with the extended change log. */
fn run(source: &str) -> Execution {
    let host = host();
    let mut environment = environment();
    environment.dom_level = DomLevel::Extended;
    mars_v9::load_page(PageInput {
        sources: &[source.to_string()],
        source_keys: &[],
        module_keys: &[],
        document: page(),
        storage: StorageState::default(),
        host: &host,
        environment: &environment,
        modules: mars_v9::ModuleInput::NONE,
    })
    .expect("page loads")
}

/** Run a script and return a global, failing on any script failure. */
fn eval(source: &str, name: &str) -> String {
    let execution = run(source);
    assert!(execution.failed.is_empty(), "{:?}", execution.failed);
    global(&execution, name)
}

#[test]
fn processing_instructions_are_shown_by_show_processing_instruction() {
    let source = "const d=document.createElement('div');\
        d.appendChild(document.createProcessingInstruction('t','x=\"1\"'));\
        const w=document.createTreeWalker(d,64);const r=w.nextNode().nodeType;";
    assert_eq!(eval(source, "r"), "7");
}

#[test]
fn processing_instructions_are_hidden_from_show_element() {
    let source = "const d=document.createElement('div');\
        d.appendChild(document.createProcessingInstruction('t',''));\
        const w=document.createTreeWalker(d,1);const r=String(w.nextNode());";
    assert_eq!(eval(source, "r"), "null");
}

#[test]
fn script_made_document_is_a_document_node_for_what_to_show() {
    let source = "const f=document.implementation.createHTMLDocument('');\
        const it=document.createNodeIterator(f,256);const r=String(it.nextNode()===f);";
    assert_eq!(eval(source, "r"), "true");
}

#[test]
fn script_made_document_is_hidden_from_show_element() {
    let source = "const f=document.implementation.createHTMLDocument('');\
        const it=document.createNodeIterator(f,1,null);const first=it.nextNode();\
        const r=String(first!==f);";
    assert_eq!(eval(source, "r"), "true");
}

#[test]
fn doctype_is_a_doctype_node_for_what_to_show() {
    let source = "const t=document.implementation.createDocumentType('html','','');\
        const it=document.createNodeIterator(t,512);const r=String(it.nextNode()===t);";
    assert_eq!(eval(source, "r"), "true");
}

#[test]
fn filter_calling_back_into_its_walker_throws_invalid_state() {
    let source = "const d=document.createElement('div');d.appendChild(document.createElement('i'));\
        let name='none';const w=document.createTreeWalker(d,0xFFFFFFFF,()=>{try{w.nextNode();}catch(e){name=e.name;}return 1;});\
        w.nextNode();const r=name;";
    assert_eq!(eval(source, "r"), "InvalidStateError");
}

#[test]
fn recursive_filter_error_propagates_out_of_the_outer_call() {
    let source = "const d=document.createElement('div');d.appendChild(document.createElement('i'));\
        const w=document.createTreeWalker(d,0xFFFFFFFF,()=>{w.firstChild();return 1;});\
        const r=(()=>{try{w.nextNode();return 'ok'}catch(e){return e.name}})();";
    assert_eq!(eval(source, "r"), "InvalidStateError");
}

#[test]
fn walker_works_again_after_a_filter_throws() {
    let source = "const d=document.createElement('div');d.appendChild(document.createElement('i'));\
        let fail=true;const w=document.createTreeWalker(d,0xFFFFFFFF,()=>{if(fail)throw new Error('x');return 1;});\
        try{w.nextNode();}catch(e){}\
        fail=false;const r=String(w.nextNode()!==null);";
    assert_eq!(eval(source, "r"), "true");
}
