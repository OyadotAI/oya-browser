/** Script bounds: each one is enforced, per script, without panics, hangs or a fatal page. */
use mars::constants::{
    EXPRESSION_DEPTH, FRAME_BYTES, OPERATIONS, RPC_TIMEOUT, SCRIPT_BYTES, SCRIPT_COUNT,
    SCRIPT_DEADLINE_DEFAULT, SCRIPT_TOTAL_BYTES, STATEMENT_DEPTH, STYLE_BYTES, STYLE_SHEETS,
    STYLE_TOTAL_BYTES, SYNTAX_DEPTH, TEXT_BYTES, TOKENS,
};
use mars::v9::{DocumentHost, StorageState, check, load_document_scripts_isolated};

/* Escaped sources and sheets fit the frame; tokens and work admit a large minified bundle. */
const _: () = assert!(SCRIPT_TOTAL_BYTES >= SCRIPT_BYTES && 6 * SCRIPT_TOTAL_BYTES < FRAME_BYTES);
const _: () = assert!(6 * STYLE_TOTAL_BYTES < FRAME_BYTES);
const _: () = assert!(TOKENS >= SCRIPT_BYTES / 4 && OPERATIONS >= 50_000_000);

/** The raised budgets admit real bundles and the time guard fires before the RPC deadline. */
#[test]
fn bounds_admit_real_bundles_and_fit_the_frame() {
    assert_eq!(SCRIPT_BYTES, 16 * 1024 * 1024);
    assert_eq!(SCRIPT_COUNT, 512);
    assert_eq!(STYLE_BYTES, 4 * 1024 * 1024);
    assert_eq!(STYLE_SHEETS, 1024);
    assert_eq!(TEXT_BYTES, 64 * 1024);
    assert!(SCRIPT_DEADLINE_DEFAULT < RPC_TIMEOUT);
}

/** Pathological nesting is a limit error at any depth, never a stack overflow. */
#[test]
fn deep_nesting_is_a_limit_error() {
    for source in [
        "(".repeat(SYNTAX_DEPTH * 4),
        "[".repeat(SYNTAX_DEPTH * 4),
        format!("{}1", "-".repeat(SYNTAX_DEPTH * 4)),
        "{".repeat(SYNTAX_DEPTH * 4),
        format!("x={}", "function(){".repeat(SYNTAX_DEPTH)),
        format!(
            "{}1{}",
            "`${".repeat(SYNTAX_DEPTH * 2),
            "}`".repeat(SYNTAX_DEPTH * 2)
        ),
        format!("{}0", "a?b:".repeat(EXPRESSION_DEPTH + 1)),
        format!("{};", "if(x)".repeat(STATEMENT_DEPTH + 1)),
    ] {
        let error = check(&source).unwrap_err();
        assert!(
            ["limit_exceeded", "script_syntax_error"].contains(&error.code.as_str()),
            "{}: {}",
            &source[..16],
            error.code
        );
    }
}

/** A script over the token bound fails alone. */
#[test]
fn token_bound_applies_per_script() {
    let source = "a,".repeat(TOKENS / 2 + 1);
    assert_eq!(check(&source).unwrap_err().code, "limit_exceeded");
}

/** An empty one-node projection for direct V9 loads. */
fn document() -> mars::v9::document::ScriptDocument {
    let parsed = mars::document::parse("<p id=a>a</p>").unwrap();
    mars::document::script_document(&parsed, "tab-1:document-1").unwrap()
}

/** Over-limit, too-many and failing scripts each fail alone; the others still run. */
#[test]
fn over_limit_scripts_are_skipped_not_fatal() {
    let mut sources = vec![
        "var first = 1;".to_string(),
        " ".repeat(SCRIPT_BYTES + 1),
        "throw 1;".to_string(),
        "var last = 2;".to_string(),
    ];
    sources.resize(SCRIPT_COUNT + 1, "var more = 3;".into());
    let execution = load_document_scripts_isolated(
        &sources,
        document(),
        StorageState::default(),
        &DocumentHost::default(),
    )
    .unwrap();
    let failed = execution
        .failed
        .iter()
        .map(|failure| failure.index)
        .collect::<Vec<_>>();
    assert_eq!(failed, vec![1, 2, SCRIPT_COUNT]);
    assert_eq!(execution.failed[0].error.code, "limit_exceeded");
    assert_eq!(execution.failed[2].error.code, "limit_exceeded");
    let location = execution.failed[1].error.location.as_deref().unwrap();
    assert_eq!(location.script_index, Some(3));
}
