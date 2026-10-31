/**
 * The DOM namespace model in V9: createElementNS and createElement namespaces, the validate and
 * extract rules that raise InvalidCharacterError and NamespaceError, the namespace-aware
 * attribute methods, lookupNamespaceURI and isDefaultNamespace, and the namespace records that
 * clone and isEqualNode read. Each test pins one rule and, where a rule rejects input, the
 * refusal as well as the acceptance.
 */
use crate::common;
use common::{environment, global, host, page};
use mars_v9::{DomLevel, Execution, PageInput, StorageState};

/** The HTML namespace, spelled as the tests compare it. */
const HTML: &str = "http://www.w3.org/1999/xhtml";
/** The SVG namespace. */
const SVG: &str = "http://www.w3.org/2000/svg";
/** The XLink namespace, used for namespaced attribute cases. */
const XLINK: &str = "http://www.w3.org/1999/xlink";
/** The XMLNS namespace. */
const XMLNS: &str = "http://www.w3.org/2000/xmlns/";

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

/** A script expression that yields the DOM error name a call throws, or "ok" when it does not. */
fn outcome(call: &str) -> String {
    format!("(()=>{{try{{{call};return 'ok'}}catch(e){{return e.name}}}})()")
}

#[test]
fn create_element_ns_sets_namespace_prefix_and_local_name() {
    let source = format!(
        "const e=document.createElementNS('{SVG}','svg:rect');\
         const r=[e.namespaceURI,e.prefix,e.localName,e.tagName,e.nodeName].join('|');"
    );
    assert_eq!(
        eval(&source, "r"),
        format!("{SVG}|svg|rect|svg:rect|svg:rect")
    );
}

#[test]
fn create_element_ns_keeps_the_case_of_a_null_namespace_name() {
    let source = "const e=document.createElementNS(null,'Foo');\
         const r=[e.namespaceURI,e.prefix,e.localName,e.tagName].map(String).join('|');";
    assert_eq!(eval(source, "r"), "null|null|Foo|Foo");
}

#[test]
fn create_element_ns_puts_an_html_name_in_the_html_namespace_upper_cased() {
    let source = format!(
        "const e=document.createElementNS('{HTML}','span');\
         const r=[e.namespaceURI,e.localName,e.tagName].join('|');"
    );
    assert_eq!(eval(&source, "r"), format!("{HTML}|span|SPAN"));
}

#[test]
fn create_element_ns_refuses_a_name_that_starts_with_a_digit() {
    let source = format!(
        "const r={};",
        outcome("document.createElementNS(null,'1foo')")
    );
    assert_eq!(eval(&source, "r"), "InvalidCharacterError");
}

#[test]
fn create_element_ns_refuses_an_empty_qualified_name() {
    let source = format!("const r={};", outcome("document.createElementNS('', '')"));
    assert_eq!(eval(&source, "r"), "InvalidCharacterError");
}

#[test]
fn create_element_ns_accepts_a_name_with_non_ascii_and_brace_characters() {
    let source = "const r=[['f}oo'],['\\u0BC6foo'],['\\uFFFFfoo'],['foo}'],['f<oo']]\
         .map(([n])=>{try{document.createElementNS(null,n);return 'ok'}catch(e){return e.name}}).join('|');";
    assert_eq!(eval(source, "r"), "ok|ok|ok|ok|ok");
}

#[test]
fn create_element_ns_refuses_a_prefix_without_a_namespace() {
    let source = format!(
        "const r={};",
        outcome("document.createElementNS(null,'f:oo')")
    );
    assert_eq!(eval(&source, "r"), "NamespaceError");
}

#[test]
fn create_element_ns_reserves_the_xml_prefix_for_the_xml_namespace() {
    let source = format!(
        "const r=[{}, {}].join('|');",
        outcome(&format!("document.createElementNS('{HTML}','xml:foo')")),
        outcome(&format!("document.createElementNS('{XML_NS}','xml:foo')")),
    );
    assert_eq!(eval(&source, "r"), "NamespaceError|ok");
}

/** The XML namespace, which only the xml prefix may use. */
const XML_NS: &str = "http://www.w3.org/XML/1998/namespace";

#[test]
fn create_element_ns_reserves_xmlns_for_the_xmlns_namespace() {
    let source = format!(
        "const r=[{}, {}, {}].join('|');",
        outcome("document.createElementNS(null,'xmlns')"),
        outcome(&format!("document.createElementNS('{XMLNS}','foo')")),
        outcome(&format!("document.createElementNS('{XMLNS}','xmlns:foo')")),
    );
    assert_eq!(eval(&source, "r"), "NamespaceError|NamespaceError|ok");
}

#[test]
fn create_element_lowercases_the_name_in_an_html_document() {
    let source = "const e=document.createElement('DIV');\
         const r=[e.localName,e.namespaceURI,e.tagName].join('|');";
    assert_eq!(eval(source, "r"), format!("div|{HTML}|DIV"));
}

#[test]
fn create_element_refuses_a_name_that_starts_with_a_digit() {
    let source = format!("const r={};", outcome("document.createElement('1foo')"));
    assert_eq!(eval(&source, "r"), "InvalidCharacterError");
}

#[test]
fn set_attribute_ns_is_read_back_by_namespace_and_local_name() {
    let source = format!(
        "const e=document.createElement('div');\
         e.setAttributeNS('{XLINK}','xlink:href','v');\
         const r=[e.getAttributeNS('{XLINK}','href'),e.getAttribute('xlink:href'),\
         e.hasAttributeNS('{XLINK}','href')].join('|');"
    );
    assert_eq!(eval(&source, "r"), "v|v|true");
}

#[test]
fn get_attribute_ns_with_a_null_namespace_does_not_find_a_namespaced_attribute() {
    let source = format!(
        "const e=document.createElement('div');\
         e.setAttributeNS('{XLINK}','xlink:href','v');\
         const r=[e.getAttributeNS(null,'href'),e.hasAttributeNS(null,'xlink:href')].map(String).join('|');"
    );
    assert_eq!(eval(&source, "r"), "null|false");
}

#[test]
fn set_attribute_ns_keeps_the_case_of_its_name() {
    let source = "const e=document.createElement('div');\
         e.setAttributeNS(null,'DataX','v');\
         const r=[e.hasAttributeNS(null,'DataX'),e.getAttribute('datax')].map(String).join('|');";
    assert_eq!(eval(source, "r"), "true|null");
}

#[test]
fn remove_attribute_ns_removes_the_namespaced_attribute_and_its_record() {
    let source = format!(
        "const e=document.createElement('div');\
         e.setAttributeNS('{XLINK}','xlink:href','v');\
         e.removeAttributeNS('{XLINK}','href');\
         const r=[e.hasAttributeNS('{XLINK}','href'),e.hasAttribute('xlink:href')].join('|');"
    );
    assert_eq!(eval(&source, "r"), "false|false");
}

#[test]
fn set_attribute_ns_refuses_a_space_in_the_name() {
    let source = format!(
        "const e=document.createElement('div');\
         const r={};",
        outcome("e.setAttributeNS(null,'a b','v')")
    );
    assert_eq!(eval(&source, "r"), "InvalidCharacterError");
}

#[test]
fn set_attribute_ns_refuses_a_prefix_with_the_xml_namespace_missing() {
    let source = format!(
        "const e=document.createElement('div');\
         const r={};",
        outcome("e.setAttributeNS(null,'xml:lang','en')")
    );
    assert_eq!(eval(&source, "r"), "NamespaceError");
}

#[test]
fn lookup_namespace_uri_finds_the_nearest_declaration_of_a_prefix() {
    let source = format!(
        "const outer=document.createElement('div');\
         outer.setAttributeNS('{XMLNS}','xmlns:p','urn:outer');\
         const inner=document.createElement('span');\
         outer.appendChild(inner);\
         const r=[inner.lookupNamespaceURI('p'),inner.lookupNamespaceURI(''),\
         inner.lookupNamespaceURI('q')].map(String).join('|');"
    );
    // The span is in the HTML namespace with no prefix, so its default answer is that namespace.
    assert_eq!(eval(&source, "r"), format!("urn:outer|{HTML}|null"));
}

#[test]
fn lookup_namespace_uri_reads_an_element_in_its_own_namespace() {
    let source = format!(
        "const e=document.createElementNS('{SVG}','svg:rect');\
         const r=[e.lookupNamespaceURI('svg'),e.lookupNamespaceURI(null)].map(String).join('|');"
    );
    assert_eq!(eval(&source, "r"), format!("{SVG}|null"));
}

#[test]
fn is_default_namespace_matches_the_default_declaration() {
    let source = format!(
        "const e=document.createElementNS(null,'div');\
         e.setAttributeNS('{XMLNS}','xmlns','urn:default');\
         const r=[e.isDefaultNamespace('urn:default'),e.isDefaultNamespace('urn:other'),\
         e.isDefaultNamespace('')].join('|');"
    );
    assert_eq!(eval(&source, "r"), "true|false|false");
}

#[test]
fn is_equal_node_compares_namespaces_and_prefixes() {
    let source = format!(
        "const a=document.createElementNS('{SVG}','svg:rect');\
         const b=document.createElementNS(null,'rect');\
         const c=document.createElementNS('{SVG}','svg:rect');\
         const r=[a.isEqualNode(b),a.isEqualNode(c)].join('|');"
    );
    assert_eq!(eval(&source, "r"), "false|true");
}

#[test]
fn clone_node_keeps_the_namespace_prefix_and_attribute_namespaces() {
    let source = format!(
        "const e=document.createElementNS('{SVG}','svg:rect');\
         e.setAttributeNS('{XLINK}','xlink:href','v');\
         const c=e.cloneNode(false);\
         const r=[c.namespaceURI,c.prefix,c.localName,c.getAttributeNS('{XLINK}','href')].join('|');"
    );
    assert_eq!(eval(&source, "r"), format!("{SVG}|svg|rect|v"));
}

#[test]
fn parsed_svg_elements_take_the_svg_namespace_from_their_tag() {
    let source = "const d=document.createElement('div');d.innerHTML='<svg></svg>';\
         const r=d.firstChild.namespaceURI;";
    assert_eq!(eval(source, "r"), SVG);
}
