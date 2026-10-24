/** Charset metadata, http-equiv, script modes and the node budget hold at Eye's public parser. */
use mars_eye::constants::{NODES, RPC_TIMEOUT};
use mars_eye::protocol::EngineError;
use mars_eye::{CSP_POLICIES, ScriptKind, ScriptMode, parse, parse_with_charset, validate_content};
use std::time::Instant;

/** The refusal a conflicting meta charset produces. */
const CONFLICT: &str =
    "The page declares a character encoding that differs from the one used to read it.";

/** Unwraps a refusal so each test can name the message it expects. */
fn refusal(source: &str) -> EngineError {
    parse(source).unwrap_err()
}

/** A meta charset and a content-type http-equiv that agree with UTF-8 decoding parse. */
#[test]
fn utf8_metadata_matches_an_undeclared_transport() {
    parse("<meta charset='utf-8'><meta http-equiv='Content-Type' content='text/html; charset=UTF-8'><p>ok</p>").unwrap();
}

/** Content-type metadata without a charset carries no encoding and stays inert. */
#[test]
fn content_type_without_charset_is_inert() {
    parse("<meta http-equiv='content-type' content='text/html'><p>ok</p>").unwrap();
}

/** Quoted charset values inside content are read up to the closing quote. */
#[test]
fn quoted_content_charset_is_checked() {
    parse("<meta http-equiv='content-type' content='text/html; charset=\"utf-8\"'><p>ok</p>")
        .unwrap();
    let error = refusal(
        "<meta http-equiv='content-type' content='text/html; charset = \"shift_jis\"'><p>ok</p>",
    );
    assert_eq!(error.message, CONFLICT);
}

/** X-UA-Compatible is inert, as in Chromium. */
#[test]
fn x_ua_compatible_is_inert() {
    parse("<meta http-equiv='X-UA-Compatible' content='IE=edge'><p>ok</p>").unwrap();
}

/** windows-1252 labels agree with UTF-8 decoding only when the source is pure ASCII. */
#[test]
fn latin1_meta_needs_an_ascii_source() {
    parse("<meta charset='iso-8859-1'><p>ascii</p>").unwrap();
    let error = refusal("<meta charset='iso-8859-1'><p>caf\u{e9}</p>");
    assert_eq!(error.message, CONFLICT);
}

/** An encoding Mars cannot decode with is a conflict, not a silent misread. */
#[test]
fn unsupported_meta_charset_is_refused() {
    assert_eq!(
        refusal("<meta charset='shift_jis'><p>x</p>").message,
        CONFLICT
    );
}

/** A meta charset that conflicts with the declared transport charset is refused. */
#[test]
fn conflicting_transport_charset_is_refused() {
    for (meta, transport) in [
        ("<meta charset='utf-8'>", "windows-1252"),
        (
            "<meta http-equiv=content-type content='text/html; charset=shift_jis'>",
            "utf-8",
        ),
    ] {
        let source = format!("{meta}<p>caf\u{e9}</p>");
        let error = parse_with_charset(&source, Some(transport)).unwrap_err();
        assert_eq!(error.message, CONFLICT);
    }
}

/** A meta matching the declared transport charset parses, and ASCII lets UTF-8 and Latin-1 agree. */
#[test]
fn agreeing_transport_charset_parses() {
    parse_with_charset(
        "<meta charset='latin1'><p>caf\u{e9}</p>",
        Some("windows-1252"),
    )
    .unwrap();
    parse_with_charset("<meta charset='utf-8'><p>ascii</p>", Some("windows-1252")).unwrap();
}

/** Refresh is noted and not followed, set-cookie is ignored and unknown pragmas are noted. */
#[test]
fn other_http_equiv_values_never_refuse_the_page() {
    for (equiv, diagnostics) in [
        ("refresh", &["A meta refresh was not followed."][..]),
        ("set-cookie", &[][..]),
        (
            "x-other",
            &["An unsupported http-equiv pragma was ignored."][..],
        ),
    ] {
        let document = parse(&format!(
            "<meta http-equiv='{equiv}' content='0;url=/x'><p>x</p>"
        ))
        .unwrap();
        assert_eq!(document.diagnostics, diagnostics, "{equiv}");
        assert_eq!(document.nodes.len(), 1, "{equiv}");
        assert!(document.csp.is_empty(), "{equiv}");
    }
}

/** Meta CSP texts are recorded in order for the coordinator, within the policy count bound. */
#[test]
fn meta_content_security_policies_are_recorded_and_bounded() {
    let meta =
        |policy: &str| format!("<meta http-equiv=Content-Security-Policy content=\"{policy}\">");
    let document = parse(&(meta("script-src 'self'") + &meta("img-src *") + "<p>x</p>")).unwrap();
    assert_eq!(document.csp, ["script-src 'self'", "img-src *"]);
    assert_eq!(
        document.diagnostics,
        ["A meta Content-Security-Policy was recorded for enforcement."]
    );
    let many = meta("default-src 'none'").repeat(CSP_POLICIES + 1);
    let document = parse(&many).unwrap();
    assert_eq!(document.csp.len(), CSP_POLICIES);
    assert!(
        document
            .diagnostics
            .contains(&"A Content-Security-Policy past the policy bounds was dropped.".to_owned())
    );
    let mut forged = parse("<p>x</p>").unwrap();
    forged.csp = vec![String::new(); CSP_POLICIES + 1];
    assert_eq!(
        validate_content(&forged).unwrap_err().code,
        "limit_exceeded"
    );
}

/** External defer and async scripts record their phase; async wins; inline ones block. */
#[test]
fn scripts_record_their_execution_mode() {
    let document = parse(
        "<script src='a.js' defer></script><script src='b.js' async defer></script><script defer>1</script><script src='c.js'></script>",
    )
    .unwrap();
    let modes: Vec<_> = document.scripts.iter().map(|s| s.mode).collect();
    assert_eq!(
        modes,
        [
            ScriptMode::Defer,
            ScriptMode::Async,
            ScriptMode::Blocking,
            ScriptMode::Blocking
        ]
    );
}

/** Module scripts and import maps are recorded for the module loader; since modules run, their
nomodule fallbacks do not, and an external import map is ignored. */
#[test]
fn module_scripts_and_import_maps_are_recorded_and_nomodule_is_not() {
    let document = parse(
        "<script type=importmap>{}</script><script type=importmap src=m.json></script>\
         <script type='module'>1</script><script nomodule>2</script>\
         <script type=MODULE async src='https://example.com/m.js' integrity=sha384-x></script>",
    )
    .unwrap();
    let scripts: Vec<_> = document
        .scripts
        .iter()
        .map(|script| {
            (
                script.kind,
                script.mode,
                script.source.as_str(),
                script.src.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        scripts,
        [
            (ScriptKind::ImportMap, ScriptMode::Blocking, "{}", None),
            (ScriptKind::Module, ScriptMode::Defer, "1", None),
            (
                ScriptKind::Module,
                ScriptMode::Async,
                "",
                Some("https://example.com/m.js")
            ),
        ]
    );
    assert_eq!(document.scripts[2].integrity.as_deref(), Some("sha384-x"));
    assert_eq!(
        document.diagnostics,
        ["An external import map was ignored; import maps must be inline."]
    );
}

/** Integrity metadata is carried for the coordinator to verify; crossorigin alone is inert. */
#[test]
fn integrity_is_recorded_for_verification_and_crossorigin_is_inert() {
    let document = parse(
        "<script src='a.js' integrity='sha384-x'></script><script src='b.js' crossorigin></script>",
    )
    .unwrap();
    let integrity: Vec<_> = document
        .scripts
        .iter()
        .map(|s| s.integrity.as_deref())
        .collect();
    assert_eq!(integrity, [Some("sha384-x"), None]);
}

/** Exactly NODES elements parse; one more is refused with the node limit. */
#[test]
fn node_budget_holds_at_the_limit() {
    assert_eq!(
        parse(&"<div></div>".repeat(NODES)).unwrap().nodes.len(),
        NODES
    );
    let error = refusal(&"<div></div>".repeat(NODES + 1));
    assert_eq!(
        error.message,
        format!("document nodes exceeds the limit of {NODES}.")
    );
}

/** A maximum-size page with text in every element parses well inside one private call. */
#[test]
fn maximum_document_parses_in_linear_time() {
    let source = "<div>text</div>".repeat(NODES);
    let start = Instant::now();
    let document = parse(&source).unwrap();
    assert!(start.elapsed() < RPC_TIMEOUT / 3, "{:?}", start.elapsed());
    assert_eq!(document.nodes.len(), NODES);
    assert!(document.nodes.iter().all(|node| node.text == "text"));
}
