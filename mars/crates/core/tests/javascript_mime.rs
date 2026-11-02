/** JavaScript MIME aliases stay consistent across modules, workers and classic scripts. */
use mars_core::http::is_javascript_mime_essence;

#[test]
fn every_standard_alias_is_case_insensitive() {
    for mime in [
        "application/ecmascript",
        "application/javascript",
        "application/x-ecmascript",
        "application/x-javascript",
        "text/ecmascript",
        "text/javascript",
        "text/javascript1.0",
        "text/javascript1.1",
        "text/javascript1.2",
        "text/javascript1.3",
        "text/javascript1.4",
        "text/javascript1.5",
        "text/jscript",
        "text/livescript",
        "text/x-ecmascript",
        "text/x-javascript",
    ] {
        assert!(is_javascript_mime_essence(mime), "{mime}");
        assert!(is_javascript_mime_essence(&mime.to_ascii_uppercase()));
    }
}

#[test]
fn data_and_unparsed_types_never_become_executable() {
    for mime in [
        "text/html",
        "application/json",
        "text/plain",
        "",
        " text/javascript",
        "text/javascript; charset=utf-8",
        "text/javascript1.6",
    ] {
        assert!(!is_javascript_mime_essence(mime), "{mime}");
    }
}
