/** HTML partial updates: the Unsafe methods insert parsed markup at a position beside or inside an element. */
use crate::common;
use common::eval;

/** appendHTMLUnsafe adds parsed markup as the element's last children. */
#[test]
fn append_html_unsafe_adds_children_last() {
    let source = "const d=document.createElement('div');d.innerHTML='<i>a</i>';\
        d.appendHTMLUnsafe('<b>x</b>');const r=d.innerHTML;";
    assert_eq!(eval(source, "r"), "<i>a</i><b>x</b>");
}

/** prependHTMLUnsafe adds parsed markup before the element's first child. */
#[test]
fn prepend_html_unsafe_adds_children_first() {
    let source = "const d=document.createElement('div');d.innerHTML='<i>a</i>';\
        d.prependHTMLUnsafe('<b>x</b>');const r=d.innerHTML;";
    assert_eq!(eval(source, "r"), "<b>x</b><i>a</i>");
}

/** beforeHTMLUnsafe and afterHTMLUnsafe place markup beside the element, not inside it. */
#[test]
fn before_and_after_html_unsafe_place_siblings() {
    let source = "const p=document.createElement('div');const d=document.createElement('i');\
        p.append(d);d.beforeHTMLUnsafe('<b>x</b>');d.afterHTMLUnsafe('<u>y</u>');const r=p.innerHTML;";
    assert_eq!(eval(source, "r"), "<b>x</b><i></i><u>y</u>");
}
