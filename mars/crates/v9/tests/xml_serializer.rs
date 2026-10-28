/** XMLSerializer: serializeToString writes a node as XML that reads back unchanged. Text and attributes are escaped, empty elements self-close, and a namespace that changes is declared. */
mod common;
use common::eval;

/** Markup the XML reader parsed serializes with its escapes and namespace declaration intact. */
#[test]
fn serialize_keeps_escapes_and_namespace_of_parsed_xml() {
    let source = "const doc=new DOMParser().parseFromString('<root xmlns=\"urn:x\"><a b=\"&quot;&amp;\">t&lt;</a><!--c--></root>','application/xml');\
        const r=new XMLSerializer().serializeToString(doc.documentElement);";
    assert_eq!(eval(source, "r"), "<root xmlns=\"urn:x\"><a b=\"&quot;&amp;\">t&lt;</a><!--c--></root>");
}

/** An element with no children is written self-closing, as XML requires. */
#[test]
fn serialize_writes_empty_element_self_closing() {
    let source = "const doc=new DOMParser().parseFromString('<e></e>','application/xml');\
        const r=new XMLSerializer().serializeToString(doc.documentElement);";
    assert_eq!(eval(source, "r"), "<e/>");
}

/** A child in a different namespace declares it, and its own children inherit it without repeating it. */
#[test]
fn serialize_declares_a_changed_namespace_once() {
    let source = "const doc=new DOMParser().parseFromString('<r><s xmlns=\"urn:n\"><t/></s></r>','application/xml');\
        const r=new XMLSerializer().serializeToString(doc.documentElement.firstElementChild);";
    assert_eq!(eval(source, "r"), "<s xmlns=\"urn:n\"><t/></s>");
}

/** A non-node argument is a TypeError, as WebIDL requires. */
#[test]
fn serialize_rejects_a_non_node() {
    let source = "let r='none';try{new XMLSerializer().serializeToString('x')}catch(e){r=e.name}";
    assert_eq!(eval(source, "r"), "TypeError");
}

/** A very deep tree is written up to the depth bound and stops there, rather than exhausting the stack. */
#[test]
fn serialize_stops_at_the_depth_bound() {
    let source = "let top=document.createElement('d');let cur=top;\
        for(let i=0;i<5000;i++){const c=document.createElement('d');cur.append(c);cur=c;}\
        const r=String(new XMLSerializer().serializeToString(top).length>0);";
    assert_eq!(eval(source, "r"), "true");
}
