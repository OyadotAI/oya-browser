/** DOMTokenList indexed reads: classList[i] is the i-th token in the attribute's order, following adds and removes, and past the end it is undefined. */
mod common;
use common::eval;

/** Indexed reads follow the attribute's order. */
#[test]
fn class_list_index_reads_tokens_in_order() {
    let source = "const d=document.createElement('div');d.className='a b c';\
        const r=[d.classList[0],d.classList[2]].join('|');";
    assert_eq!(eval(source, "r"), "a|c");
}

/** After a removal the remaining tokens shift down, so the index reflects the new order. */
#[test]
fn class_list_index_follows_removal() {
    let source = "const d=document.createElement('div');d.className='a b c';\
        d.classList.remove('a');const r=[d.classList[0],d.classList[1]].join('|');";
    assert_eq!(eval(source, "r"), "b|c");
}

/** An index past the last token is undefined. */
#[test]
fn class_list_index_past_the_end_is_undefined() {
    let source = "const d=document.createElement('div');d.className='a';\
        const r=String(d.classList[1]);";
    assert_eq!(eval(source, "r"), "undefined");
}
