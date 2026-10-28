/** Node document-position constants: each bit has its value, and a detached pair reports disconnected, implementation-specific and an order. */
mod common;
use common::eval;

/** The implementation-specific bit is 32, as the DOM standard fixes it. */
#[test]
fn implementation_specific_position_constant_is_32() {
    assert_eq!(eval("const r=String(Node.DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC);", "r"), "32");
}

/** Two detached nodes report disconnected, implementation-specific and following: 1 + 32 + 4. */
#[test]
fn detached_pair_reports_disconnected_implementation_specific_and_following() {
    let source = "const a=document.createElement('a');const b=document.createElement('b');\
        const r=String(a.compareDocumentPosition(b));";
    assert_eq!(eval(source, "r"), "37");
}
