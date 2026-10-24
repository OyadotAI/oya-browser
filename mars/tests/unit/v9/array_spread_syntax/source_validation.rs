/** Full-source walks retain strict early errors, closure sites and existing admission budgets. */
use super::*;

/** Unreachable spread operands still participate in strict assignment-target validation. */
#[test]
fn strict_validation_visits_every_spread_operand() {
    for source in [
        "'use strict';if(false){const a=[...(eval=[])];}",
        "'use strict';function unused(){return [...(arguments=[])];}",
        "if(false){const a=[...(()=>{ 'use strict';eval=[];return []; })()];}",
    ] {
        let error = load(source, &[]).unwrap_err();
        assert_eq!(error.code, "script_syntax_error");
        assert!(error.message.contains("eval or arguments in strict mode"));
    }
}

/** Functions nested inside spread operands retain source-owned lexical capture across events. */
#[test]
fn nested_spread_operand_closures_survive_wire_and_source_registry_validation() {
    let source = "const callbacks=[...((value)=>[()=>value])(7)];function show(){document.getElementById('result').textContent=callbacks[0]();}if(false){const never=[...(()=>[()=>9])()];}";
    let loaded = load(source, &[]).unwrap();
    let state = serde_json::from_slice(&serde_json::to_vec(&loaded.state).unwrap()).unwrap();
    assert_eq!(
        event(source, "show()", state, &[]).unwrap().changes[0]
            .text()
            .unwrap(),
        "7"
    );
}

/** Spread wrappers cannot conceal operand AST depth from the existing parser recursion bound. */
#[test]
fn spread_operands_retain_expression_depth_limits() {
    let expression = format!(
        "{}[]{}",
        "[...".repeat(mars::constants::EXPRESSION_DEPTH),
        "]".repeat(mars::constants::EXPRESSION_DEPTH)
    );
    let error = execute(&format!("const a={expression};")).unwrap_err();
    assert_eq!(error.code, "limit_exceeded");
    assert!(error.message.contains("expression nesting"));
}

/** Single-source scanning and ordered-script admission retain their existing End-token distinction. */
#[test]
fn ellipsis_uses_one_token_without_bypassing_the_exact_token_limit() {
    // The frame is 11 tokens and each `...[],` adds 4, so this source is one token under TOKENS.
    let spreads = (mars::constants::TOKENS - 12) / 4;
    let source = format!("function f(){{return[{}];}};", "...[],".repeat(spreads));
    mars::v9::load_scripts(std::slice::from_ref(&source), &[]).unwrap();
    let scanner_boundary = format!("{source};");
    load(&scanner_boundary, &[]).unwrap();
    let error = load(&format!("{scanner_boundary};"), &[]).unwrap_err();
    assert_eq!(error.code, "limit_exceeded");
    assert!(error.message.contains("tokens"));
    assert_eq!(error.phase.as_deref(), Some("scan"));
}

/** Spread syntax neither raises the existing source-byte allowance nor bypasses complete scanning. */
#[test]
fn spread_sources_retain_the_exact_script_byte_limit() {
    let prefix = "const a=[...[]];/*";
    let source = format!(
        "{prefix}{}*/",
        "x".repeat(mars::constants::SCRIPT_BYTES - prefix.len() - 2)
    );
    assert_eq!(source.len(), mars::constants::SCRIPT_BYTES);
    load(&source, &[]).unwrap();
    let error = load(&format!("{source} "), &[]).unwrap_err();
    assert_eq!(error.code, "limit_exceeded");
    assert!(error.message.contains("script bytes"));
}
