/** decodeURI and decodeURIComponent decode one UTF-8 sequence per escape run and reject malformed input. */
use crate::common;

/** Malformed escapes throw URIError: signs, cut sequences, stray continuations, overlongs, surrogates. */
#[test]
fn malformed_escapes_throw_uri_error() {
    let source = "const r=['%+1','%1','%G0','%C3','%C3a','%80','%C0%80','%ED%A0%80','%F8%80%80%80%80']\
        .map(s=>{try{decodeURI(s);return 'ok'}catch(e){return e.name}}).join();";
    assert_eq!(
        common::eval(source, "r"),
        "URIError,URIError,URIError,URIError,URIError,URIError,URIError,URIError,URIError"
    );
}

/** A lone surrogate outside any escape passes through unchanged instead of becoming U+FFFD. */
#[test]
fn lone_surrogates_pass_through() {
    let source =
        "const d=decodeURI('a\\uD800b');const r=[d.length,d.charCodeAt(1).toString(16)].join();";
    assert_eq!(common::eval(source, "r"), "3,d800");
}

/** Multi-byte sequences decode, and decodeURI keeps reserved escapes that the component form decodes. */
#[test]
fn sequences_decode_and_reserved_escapes_stay() {
    let source = "const r=[decodeURI('%C3%A9%F0%9F%98%80')==='\\u00e9\\u{1F600}',decodeURI('%3B%2f'),\
        decodeURIComponent('%3B%2f'),decodeURI('%41%20')].join();";
    assert_eq!(common::eval(source, "r"), "true,%3B%2f,;/,A ");
}
