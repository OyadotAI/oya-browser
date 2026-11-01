/**
 * Attr objects and DOMTokenList in V9: createAttribute(NS), the Attr properties, identity of the
 * Attr an element hands out, attaching, detaching and the errors each method raises, and the
 * classList and relList rules (value setter, update steps, validation, toggle, replace, supports).
 * Each test pins one rule, and where a rule refuses input, the refusal as well as the acceptance.
 */
use crate::common;
use common::{environment, global, host, page};
use mars_v9::{DomLevel, Execution, PageInput, StorageState};

/** The XLink namespace, used for the namespaced attribute cases. */
const XLINK: &str = "http://www.w3.org/1999/xlink";

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

/** A script expression that yields the error name a call throws, or "ok" when it does not. */
fn outcome(call: &str) -> String {
    format!("(()=>{{try{{{call};return 'ok'}}catch(e){{return e.name}}}})()")
}

#[test]
fn create_attribute_makes_an_attr_with_a_lower_cased_name_and_empty_value() {
    let source = "const a=document.createAttribute('Data-X');\
        const r=[a.nodeType,a.name,a.localName,a.namespaceURI,a.prefix,a.value,a.ownerElement,a.specified].map(String).join('|');";
    assert_eq!(eval(source, "r"), "2|data-x|data-x|null|null||null|true");
}

#[test]
fn create_attribute_refuses_an_empty_name_with_invalid_character() {
    let source = format!("const r={};", outcome("document.createAttribute('')"));
    assert_eq!(eval(&source, "r"), "InvalidCharacterError");
}

#[test]
fn create_attribute_ns_splits_the_prefix_and_keeps_the_namespace() {
    let source = format!(
        "const a=document.createAttributeNS('{XLINK}','xlink:href');\
         const r=[a.namespaceURI,a.prefix,a.localName,a.name].join('|');"
    );
    assert_eq!(eval(&source, "r"), format!("{XLINK}|xlink|href|xlink:href"));
}

#[test]
fn create_attribute_ns_refuses_a_prefix_without_a_namespace() {
    let source = format!(
        "const r={};",
        outcome("document.createAttributeNS(null,'a:b')")
    );
    assert_eq!(eval(&source, "r"), "NamespaceError");
}

#[test]
fn get_attribute_node_returns_the_same_attr_on_every_read() {
    let source = "const e=document.createElement('div');e.setAttribute('id','x');\
        const a=e.getAttributeNode('id');\
        const r=[a===e.getAttributeNode('id'),a===e.attributes[0],a.ownerElement===e,a.value,\
        e.getAttributeNode('nope')].map(String).join('|');";
    assert_eq!(eval(source, "r"), "true|true|true|x|null");
}

#[test]
fn attr_value_reads_the_element_while_attached_and_writes_through() {
    let source = "const e=document.createElement('p');e.setAttribute('title','one');\
        const a=e.getAttributeNode('title');e.setAttribute('title','two');\
        const before=a.value;a.value='three';\
        const r=[before,e.getAttribute('title')].join('|');";
    assert_eq!(eval(source, "r"), "two|three");
}

#[test]
fn remove_attribute_node_detaches_the_attr_and_keeps_its_value() {
    let source = "const e=document.createElement('p');e.setAttribute('lang','en');\
        const a=e.getAttributeNode('lang');\
        const removed=e.removeAttributeNode(a);\
        const r=[removed===a,a.ownerElement,a.value,e.hasAttribute('lang')].map(String).join('|');";
    assert_eq!(eval(source, "r"), "true|null|en|false");
}

#[test]
fn remove_attribute_detaches_a_held_attr_keeping_its_value() {
    let source = "const e=document.createElement('p');e.setAttribute('lang','fr');\
        const a=e.getAttributeNode('lang');e.removeAttribute('lang');\
        const r=[a.ownerElement,a.value].map(String).join('|');";
    assert_eq!(eval(source, "r"), "null|fr");
}

#[test]
fn remove_attribute_node_refuses_an_attr_the_element_does_not_hold() {
    let source = format!(
        "const e=document.createElement('p');const a=document.createAttribute('x');const r={};",
        outcome("e.removeAttributeNode(a)")
    );
    assert_eq!(eval(&source, "r"), "NotFoundError");
}

#[test]
fn set_attribute_node_refuses_an_attr_another_element_holds() {
    let source = format!(
        "const e1=document.createElement('p'),e2=document.createElement('p');\
         e1.setAttribute('id','k');const a=e1.getAttributeNode('id');const r={};",
        outcome("e2.setAttributeNode(a)")
    );
    assert_eq!(eval(&source, "r"), "InUseAttributeError");
}

#[test]
fn set_attribute_node_returns_the_replaced_attr_detached_with_its_value() {
    let source = "const e=document.createElement('p');e.setAttribute('title','old');\
        const old=e.getAttributeNode('title');\
        const fresh=document.createAttribute('title');fresh.value='new';\
        const replaced=e.setAttributeNode(fresh);\
        const r=[replaced===old,old.ownerElement,old.value,fresh.ownerElement===e,\
        e.getAttribute('title')].map(String).join('|');";
    assert_eq!(eval(source, "r"), "true|null|old|true|new");
}

#[test]
fn an_attr_is_a_node_so_it_cannot_be_appended() {
    let source = format!(
        "const e=document.createElement('p');const a=document.createAttribute('x');const r={};",
        outcome("e.appendChild(a)")
    );
    assert_eq!(eval(&source, "r"), "HierarchyRequestError");
}

#[test]
fn named_node_map_items_are_the_element_attr_objects() {
    let source = "const e=document.createElement('p');e.setAttribute('id','i');\
        const r=[e.attributes.getNamedItem('id')===e.getAttributeNode('id'),\
        e.attributes.getNamedItem('id').value].map(String).join('|');";
    assert_eq!(eval(source, "r"), "true|i");
}

#[test]
fn class_list_returns_the_same_object_on_every_read() {
    let source = "const e=document.createElement('p');\
        const r=String(e.classList===e.classList);";
    assert_eq!(eval(source, "r"), "true");
}

#[test]
fn class_list_value_setter_writes_the_literal_class_attribute() {
    let source = "const e=document.createElement('p');e.classList.value='a  b';\
        const r=e.getAttribute('class');";
    assert_eq!(eval(source, "r"), "a  b");
}

#[test]
fn class_list_add_refuses_an_empty_token_and_changes_nothing() {
    let source = format!(
        "const e=document.createElement('p');e.setAttribute('class','x');const r={}+'|'+e.getAttribute('class');",
        outcome("e.classList.add('y','')")
    );
    assert_eq!(eval(&source, "r"), "SyntaxError|x");
}

#[test]
fn class_list_add_refuses_a_token_with_whitespace() {
    let source = format!(
        "const e=document.createElement('p');const r={};",
        outcome("e.classList.add('a b')")
    );
    assert_eq!(eval(&source, "r"), "InvalidCharacterError");
}

#[test]
fn class_list_update_steps_rewrite_the_attribute_in_normal_form() {
    let source = "const e=document.createElement('p');e.setAttribute('class','  a  a b');\
        e.classList.remove('zz');const r=e.getAttribute('class');";
    assert_eq!(eval(source, "r"), "a b");
}

#[test]
fn class_list_read_only_calls_do_not_create_the_class_attribute() {
    let source = "const e=document.createElement('p');e.classList.remove('zz');\
        const r=String(e.hasAttribute('class'));";
    assert_eq!(eval(source, "r"), "false");
}

#[test]
fn class_list_toggle_honours_an_explicit_force() {
    let source = "const e=document.createElement('p');\
        const r=[e.classList.toggle('t',true),e.classList.toggle('t',true),\
        e.classList.toggle('t'),e.getAttribute('class')].map(String).join('|');";
    assert_eq!(eval(source, "r"), "true|true|false|");
}

#[test]
fn class_list_replace_keeps_place_and_reports_a_missing_token() {
    let source = "const e=document.createElement('p');e.setAttribute('class','a b c');\
        const r=[e.classList.replace('b','x'),e.classList.replace('zz','y'),\
        e.getAttribute('class')].map(String).join('|');";
    assert_eq!(eval(source, "r"), "true|false|a x c");
}

#[test]
fn class_list_supports_throws_type_error() {
    let source = format!(
        "const e=document.createElement('p');const r={};",
        outcome("e.classList.supports('x')")
    );
    assert_eq!(eval(&source, "r"), "TypeError");
}

#[test]
fn rel_list_supports_the_link_tokens_without_case() {
    let source = "const l=document.createElement('link');\
        const r=[l.relList.supports('STYLESHEET'),l.relList.supports('noopener')].map(String).join('|');";
    assert_eq!(eval(source, "r"), "true|false");
}

#[test]
fn class_list_reads_the_attribute_live() {
    let source = "const e=document.createElement('p');const list=e.classList;\
        e.setAttribute('class','p q');\
        const r=[list.contains('q'),list.length].map(String).join('|');";
    assert_eq!(eval(source, "r"), "true|2");
}
