/*!
 * The query and collection contract in V9: a bad selector is a DOMException named SyntaxError
 * with code 12, digit-led identifiers are rejected, a zero escape is U+FFFD, a document's
 * getElementsByTagName and getElementsByClassName include the root element, the Node type
 * constants and TreeWalker's tag exist, createCDATASection refuses HTML documents, and an empty
 * name never matches in namedItem.
 */
mod common;
use common::{environment, failure, global, load_with};
use mars_v9::{DomLevel, Execution};

/** Run a script with the extended change log, which innerHTML writes need, and return a global,
failing on any script failure. */
fn eval(source: &str, name: &str) -> String {
    let mut environment = environment();
    environment.dom_level = DomLevel::Extended;
    let execution: Execution = load_with(source, &environment);
    assert!(execution.failed.is_empty(), "{:?}", failure(&execution));
    global(&execution, name)
}

/** A bad selector in querySelector, matches and closest throws the DOM SyntaxError, which is
a DOMException with code 12 and not a JavaScript SyntaxError. */
#[test]
fn bad_selectors_throw_the_dom_syntax_error_in_every_method() {
    let source = "const el = document.createElement('div'); const out = [];\
        for (const call of [() => document.querySelector('::before'), () => document.querySelectorAll('a['), () => el.matches('a['), () => el.closest('::after')]) {\
          try { call(); out.push('none'); } catch (e) { out.push([e instanceof DOMException, e.name, e.code].join(':')); }\
        }\
        const r = out.join('|');";
    assert_eq!(
        eval(source, "r"),
        "true:SyntaxError:12|true:SyntaxError:12|true:SyntaxError:12|true:SyntaxError:12"
    );
}

/** An identifier may not start with a digit, or a hyphen then a digit, so `#-123` and `.1a` are
rejected while `#a1` still matches. */
#[test]
fn identifiers_may_not_start_with_a_digit_or_hyphen_digit() {
    let source = "const bad = ['#-123', '.1a', '#1']; const out = [];\
        for (const s of bad) { try { document.querySelector(s); out.push('ok'); } catch (e) { out.push(e.name); } }\
        document.body.innerHTML = '<p id=\"a1\"></p>';\
        out.push(document.querySelector('#a1') ? 'found' : 'missing');\
        const r = out.join('|');";
    assert_eq!(
        eval(source, "r"),
        "SyntaxError|SyntaxError|SyntaxError|found"
    );
}

/** A zero escape in an identifier names U+FFFD, so `#zero\0` finds the element whose id holds it. */
#[test]
fn zero_escape_in_an_identifier_is_the_replacement_character() {
    let source = "const s = document.createElement('span'); s.id = 'zero\\uFFFD'; document.body.appendChild(s);\
        const r = document.querySelector('#zero\\\\0') === s ? 'same' : 'different';";
    assert_eq!(eval(source, "r"), "same");
}

/** document.getElementsByTagName('html') finds the root element: the document searches its whole
tree, where an element's search starts below itself. */
#[test]
fn document_get_elements_by_tag_name_includes_the_root_element() {
    let source = "const r = String(document.getElementsByTagName('html').length) + ':' + \
        (document.getElementsByTagName('html')[0] === document.documentElement ? 'root' : 'other');";
    assert_eq!(eval(source, "r"), "1:root");
}

/** document.getElementsByClassName finds the root element carrying the class. */
#[test]
fn document_get_elements_by_class_name_includes_the_root_element() {
    let source = "document.documentElement.className = 'marked'; \
        const r = document.getElementsByClassName('marked')[0] === document.documentElement ? 'root' : 'missing';";
    assert_eq!(eval(source, "r"), "root");
}

/** The node type constants the DOM defines beyond the common few are on the Node constructor. */
#[test]
fn node_type_constants_include_cdata_and_processing_instruction() {
    let source = "const r = [Node.CDATA_SECTION_NODE, Node.ENTITY_REFERENCE_NODE, Node.ENTITY_NODE, \
        Node.PROCESSING_INSTRUCTION_NODE, Node.NOTATION_NODE].join(',');";
    assert_eq!(eval(source, "r"), "4,5,6,7,12");
}

/** createCDATASection throws NotSupportedError in an HTML document, where CDATA is not allowed. */
#[test]
fn create_cdata_section_refuses_html_documents() {
    let source = "let r; try { document.createCDATASection('x'); r = 'created'; } \
        catch (e) { r = e.name; }";
    assert_eq!(eval(source, "r"), "NotSupportedError");
}

/** TreeWalker and NodeIterator carry their interface name as their string tag. */
#[test]
fn tree_walker_stringifies_as_its_interface() {
    let source = "const r = [String(document.createTreeWalker(document.body)), \
        String(document.createNodeIterator(document.body))].join('|');";
    assert_eq!(
        eval(source, "r"),
        "[object TreeWalker]|[object NodeIterator]"
    );
}

/** namedItem('') matches nothing, even an element whose id is the empty string. */
#[test]
fn named_item_with_an_empty_name_is_null() {
    let source = "document.body.innerHTML = '<div id=\"\"></div>'; \
        const r = document.getElementsByTagName('div').namedItem('') === null ? 'null' : 'matched';";
    assert_eq!(eval(source, "r"), "null");
}
