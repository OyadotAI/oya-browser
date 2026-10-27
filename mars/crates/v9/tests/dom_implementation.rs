/**
 * DOMImplementation in V9: createDocumentType, createDocument, `new Document()`,
 * Document.createProcessingInstruction and the ownerDocument each of them answers with, plus
 * the rule that replaced children stay reachable for mutation records.
 */
mod common;
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
fn new_document_has_no_doctype_root_or_children() {
    let source = "const d=new Document();\
        const r=[d.nodeType,d.firstChild,d.documentElement,d.doctype,d instanceof Document,d.childNodes.length].map(String).join('|');";
    assert_eq!(eval(source, "r"), "9|null|null|null|true|0");
}

#[test]
fn new_document_refuses_a_call_without_new() {
    let source = "const r=(()=>{try{Document();return 'ok'}catch(e){return e.name}})();";
    assert_eq!(eval(source, "r"), "TypeError");
}

#[test]
fn create_document_type_keeps_its_names_and_page_owner() {
    let source = "const t=document.implementation.createDocumentType('svg:root','-//x','http://y');\
        const r=[t.name,t.publicId,t.systemId,t.nodeName,t.nodeType,t.ownerDocument===document].map(String).join('|');";
    assert_eq!(eval(source, "r"), "svg:root|-//x|http://y|svg:root|10|true");
}

#[test]
fn create_document_type_rejects_whitespace_and_angle_bracket() {
    let source = "const r=[' a','a>','a\\t'].map(n=>{try{document.implementation.createDocumentType(n,'','');return 'ok'}catch(e){return e.name}}).join('|');";
    assert_eq!(
        eval(source, "r"),
        "InvalidCharacterError|InvalidCharacterError|InvalidCharacterError"
    );
}

#[test]
fn implementation_belongs_to_the_document_it_came_from() {
    let source = "const h=document.implementation.createHTMLDocument('t');\
        const t=h.implementation.createDocumentType('a','','');\
        const r=[t.ownerDocument===h,h.implementation===h.implementation,h.implementation===document.implementation].map(String).join('|');";
    assert_eq!(eval(source, "r"), "true|true|false");
}

#[test]
fn create_document_builds_root_and_doctype_together() {
    let source = "const t=document.implementation.createDocumentType('html','','');\
        const d=document.implementation.createDocument(null,'root',t);\
        const r=[d.documentElement.localName,d.doctype===t,t.ownerDocument===d,d.childNodes.length].map(String).join('|');";
    assert_eq!(eval(source, "r"), "root|true|true|2");
}

#[test]
fn create_document_with_empty_name_has_no_root() {
    let source = "const d=document.implementation.createDocument(null,'',null);\
        const r=[String(d.documentElement),d.childNodes.length].join('|');";
    assert_eq!(eval(source, "r"), "null|0");
}

#[test]
fn create_document_needs_its_two_required_arguments() {
    let source = "const r=[()=>document.implementation.createDocument(),()=>document.implementation.createDocument('')].map(f=>{try{f();return 'ok'}catch(e){return e.name}}).join('|');";
    assert_eq!(eval(source, "r"), "TypeError|TypeError");
}

#[test]
fn create_document_refuses_a_doctype_argument_that_is_not_one() {
    let source = "const r=(()=>{try{document.implementation.createDocument(null,'a',{});return 'ok'}catch(e){return e.name}})();";
    assert_eq!(eval(source, "r"), "TypeError");
}

#[test]
fn processing_instruction_has_target_data_and_type_seven() {
    let source = "const p=document.createProcessingInstruction('xml-x','a b');\
        const r=[p.nodeType,p.nodeName,p.target,p.data,p.nodeValue,p.ownerDocument===document,p instanceof ProcessingInstruction].map(String).join('|');";
    assert_eq!(eval(source, "r"), "7|xml-x|xml-x|a b|a b|true|true");
}

#[test]
fn processing_instruction_refuses_a_bad_target_or_close_in_data() {
    let source = "const r=[['1a','x'],['a','x?>y'],['','x']].map(([t,d])=>{try{document.createProcessingInstruction(t,d);return 'ok'}catch(e){return e.name}}).join('|');";
    assert_eq!(
        eval(source, "r"),
        "InvalidCharacterError|InvalidCharacterError|InvalidCharacterError"
    );
}
