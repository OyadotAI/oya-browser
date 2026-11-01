/** innerHTML and RCDATA text decode the WHATWG named and numeric references, not a handful of names. */
use crate::common;
use common::eval;

/** Named references in text decode, including a legacy name without its semicolon. */
#[test]
fn inner_html_text_decodes_named_references() {
    let source = "const p=document.createElement('p');p.innerHTML='&eacute;&hellip;&nbsp;&notin;&amp;amp;&copy 2&copy=3&copyright;';\
        const r=p.textContent;";
    assert_eq!(eval(source, "r"), "é…\u{a0}∉&amp;© 2©=3©right;");
}

/** In an attribute, a legacy name followed by `=` stays literal, so query strings survive. */
#[test]
fn inner_html_attributes_keep_query_strings() {
    let source = "const p=document.createElement('p');p.innerHTML='<a title=\"&copy=2&eacute;&copy;\">x</a>';\
        const r=p.firstChild.getAttribute('title');";
    assert_eq!(eval(source, "r"), "&copy=2é©");
}

/** Numeric references: C1 code points map through Windows-1252, bad scalars become U+FFFD. */
#[test]
fn inner_html_numeric_references_follow_the_spec() {
    let source = "const p=document.createElement('p');p.innerHTML='&#x80;&#xD800;&#0;&#65';\
        const r=p.textContent;";
    assert_eq!(eval(source, "r"), "€\u{fffd}\u{fffd}A");
}

/** A textarea's innerHTML is RCDATA: its references decode, its tags stay text. */
#[test]
fn textarea_inner_html_decodes_references_as_text() {
    let source = "const t=document.createElement('textarea');t.innerHTML='&eacute;&lt;b&gt;&hellip;';\
        const r=t.textContent;";
    assert_eq!(eval(source, "r"), "é<b>…");
}
