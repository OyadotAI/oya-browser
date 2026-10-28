/**
 * ProcessingInstruction pseudo-attributes in V9: the attribute methods read and write the
 * `name="value"` pairs in the node's data, keep the data in write order, and refuse names the
 * grammar cannot hold. Each rule has one test, and the failure paths are covered too.
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
fn pseudo_attributes_read_back_in_data_order() {
    let source = "const p=document.createProcessingInstruction('t','a=\"b\" x=\"yy\"');\
        const r=[p.hasAttributes(),p.getAttributeNames().join(),p.getAttribute('a'),p.getAttribute('x'),p.getAttribute('X')].map(String).join('|');";
    assert_eq!(eval(source, "r"), "true|a,x|b|yy|null");
}

#[test]
fn unquoted_pseudo_attribute_drops_it_and_the_rest() {
    let source = "const p=document.createProcessingInstruction('t','a=b x=\"yy\"');\
        const r=[p.hasAttributes(),p.getAttribute('x')].map(String).join('|');";
    assert_eq!(eval(source, "r"), "false|null");
}

#[test]
fn set_attribute_rewrites_data_in_place() {
    let source = "const p=document.createProcessingInstruction('t','a=\"b\" x=\"yy\"');\
        p.setAttribute('x','yy2');const r=p.data;";
    assert_eq!(eval(source, "r"), "a=\"b\" x=\"yy2\"");
}

#[test]
fn set_attribute_appends_new_names_last() {
    let source = "const p=document.createProcessingInstruction('t','a=\"b\"');\
        p.setAttribute('z','1');p.setAttribute('c','2');const r=p.data;";
    assert_eq!(eval(source, "r"), "a=\"b\" z=\"1\" c=\"2\"");
}

#[test]
fn remove_attribute_drops_the_pair_from_data() {
    let source = "const p=document.createProcessingInstruction('t','a=\"b\" x=\"yy\"');\
        p.removeAttribute('x');const r=p.data;";
    assert_eq!(eval(source, "r"), "a=\"b\"");
}

#[test]
fn remove_attribute_of_absent_name_leaves_data_alone() {
    let source = "const p=document.createProcessingInstruction('t','a=\"b\"');\
        p.removeAttribute('nope');const r=p.data;";
    assert_eq!(eval(source, "r"), "a=\"b\"");
}

#[test]
fn attribute_value_is_escaped_into_data() {
    let source = "const p=document.createProcessingInstruction('t','');\
        p.setAttribute('attr','axx\"');const r=p.data;";
    assert_eq!(eval(source, "r"), "attr=\"axx&quot;\"");
}

#[test]
fn invalid_attribute_name_throws_invalid_character() {
    let source = "const p=document.createProcessingInstruction('t','');\
        const r=['a=','/x','x\\t',''].map(n=>{try{p.setAttribute(n,'v');return 'ok'}catch(e){return e.name}}).join('|');";
    assert_eq!(
        eval(source, "r"),
        "InvalidCharacterError|InvalidCharacterError|InvalidCharacterError|InvalidCharacterError"
    );
}

#[test]
fn punctuation_names_are_valid_pseudo_attributes() {
    let source = "const p=document.createProcessingInstruction('t','');\
        p.setAttribute('$','1');p.setAttribute('_bar','2');const r=p.getAttributeNames().join();";
    assert_eq!(eval(source, "r"), "$,_bar");
}

#[test]
fn toggle_attribute_flips_then_respects_force() {
    let source = "const p=document.createProcessingInstruction('t','');\
        const a=p.toggleAttribute('k');const b=p.toggleAttribute('k',true);const c=p.toggleAttribute('k',false);\
        const r=[a,b,c,p.hasAttribute('k'),p.data].map(String).join('|');";
    assert_eq!(eval(source, "r"), "true|true|false|false|");
}

#[test]
fn toggle_attribute_rejects_invalid_name_even_when_removing() {
    let source = "const p=document.createProcessingInstruction('t','');\
        const r=(()=>{try{p.toggleAttribute('a=',false);return 'ok'}catch(e){return e.name}})();";
    assert_eq!(eval(source, "r"), "InvalidCharacterError");
}

#[test]
fn assigning_data_rereads_the_pseudo_attributes() {
    let source = "const p=document.createProcessingInstruction('t','');\
        p.data='blabla=\"\"';const r=[p.getAttribute('blabla'),p.getAttribute('BLABLA')].map(String).join('|');";
    assert_eq!(eval(source, "r"), "|null");
}

#[test]
fn attribute_changes_queue_one_character_data_record_each() {
    let source = "const p=document.createProcessingInstruction('t','a=\"b\"');\
        const m=new MutationObserver(()=>{});m.observe(p,{characterData:true});\
        p.setAttribute('x','1');p.removeAttribute('x');const r=m.takeRecords().map(x=>x.type).join();";
    assert_eq!(eval(source, "r"), "characterData,characterData");
}

#[test]
fn html_head_element_has_its_own_interface() {
    let source = "const r=[document.createElement('head') instanceof HTMLHeadElement, typeof HTMLHeadElement].map(String).join('|');";
    assert_eq!(eval(source, "r"), "true|function");
}
