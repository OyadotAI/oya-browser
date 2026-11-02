/*! The encoding a parse settles: a meta charset that agrees with the decoding names the document,
and that name is what V9's `document.characterSet` reports through the projection. */
use mars_eye::{parse, parse_with_charset, script_document};

/** A meta charset that agrees with UTF-8 on ASCII names the document windows-1252. */
#[test]
fn meta_charset_names_the_document() {
    let doc = parse(r#"<!doctype html><meta charset="windows-1252"><p>x</p>"#).expect("parses");
    assert_eq!(doc.charset, "windows-1252");
}

/** With no declaration, the default decoding names the page, spelled as the Encoding Standard spells it. */
#[test]
fn undeclared_page_reports_utf8() {
    let doc = parse("<p>x</p>").expect("parses");
    assert_eq!(doc.charset, "UTF-8");
}

/** A transport charset names the page when the markup declares none. */
#[test]
fn transport_charset_names_undeclared_page() {
    let doc = parse_with_charset("<p>x</p>", Some("windows-1252")).expect("parses");
    assert_eq!(doc.charset, "windows-1252");
}

/** A legacy transport charset names the page, and a meta spelling of the same encoding agrees with it. */
#[test]
fn legacy_transport_charset_names_the_page() {
    let doc = parse_with_charset(
        r#"<meta charset="cseucpkdfmtjapanese"><p>x</p>"#,
        Some("euc-jp"),
    )
    .expect("parses");
    assert_eq!(doc.charset, "EUC-JP");
}

/** A meta that names UTF-16 is read as UTF-8, so a UTF-8 page still parses and names UTF-8. */
#[test]
fn meta_utf16_is_read_as_utf8() {
    let doc = parse(r#"<meta charset="utf-16"><p>x</p>"#).expect("parses");
    assert_eq!(doc.charset, "UTF-8");
}

/** The projection V9 receives carries the settled name, so the script side can read it. */
#[test]
fn projection_carries_the_settled_charset() {
    let doc = parse(r#"<meta charset="windows-1252"><p>x</p>"#).expect("parses");
    let projection = script_document(&doc, "page").expect("projects");
    assert_eq!(projection.charset, "windows-1252");
}
