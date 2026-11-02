/** Parser admission preserves short-circuit grammar, Unicode names and dynamic function argument boundaries. */
use crate::common;

/** Direct and indirect eval expose the parse reason in JavaScript without a host location prefix. */
#[test]
fn eval_syntax_messages_start_with_the_parameter_error() {
    let source = "const out=[];for(const code of ['(a,a)=>1','(a,a)=>{}','function f(a,a){\\\"use strict\\\"}']){try{eval(code)}catch(error){out.push(error instanceof SyntaxError,error.message.startsWith('Duplicate parameter name not allowed'))}try{(0,eval)(code)}catch(error){out.push(error instanceof SyntaxError,error.message.startsWith('Duplicate parameter name not allowed'))}}const r=out.join();";
    assert_eq!(
        common::eval(source, "r"),
        "true,true,true,true,true,true,true,true,true,true,true,true"
    );
}

/** Ordinary sloppy simple functions retain the legacy duplicate allowance while unique parameter lists reject it. */
#[test]
fn duplicate_parameter_early_errors_keep_their_function_rules() {
    let source = "const out=[Function('a,a','return a')(1,2)];for(const code of ['(a,a=1)=>a','({m(a,a){}})','function f(a,...a){}','function f(a,a){\\\"use strict\\\"}']){try{eval(code);out.push(false)}catch(error){out.push(error instanceof SyntaxError)}}const r=out.join();";
    assert_eq!(common::eval(source, "r"), "2,true,true,true,true");
}

/** Nullish coalescing cannot share an unparenthesized grammar branch with logical AND or OR. */
#[test]
fn unparenthesized_nullish_boolean_mixtures_are_early_errors() {
    let source = "const out=[];for(const code of ['true||true??true','true??true||true','true&&true??true','true??true&&true','a??b??c||d','a&&b??c??d','(a??b)||c??d','a??(b&&c)||d']){try{eval(code);out.push(false)}catch(error){out.push(error instanceof SyntaxError)}}const r=out.join();";
    assert_eq!(
        common::eval(source, "r"),
        "true,true,true,true,true,true,true,true"
    );
}

/** Parentheses around the actual mixed operand admit both grammar branches without changing short circuiting. */
#[test]
fn parenthesized_nullish_boolean_mixtures_preserve_values() {
    let source = "let calls=0;function never(){calls++;return 9}const out=[(false||true)??false,null??(false||true),(undefined??false)||true,false||(null??true),(true&&null)??7,null??(true&&8),null??null??(false||true),((null??true))&&true,true??(false||never()),calls];const r=out.join();";
    assert_eq!(
        common::eval(source, "r"),
        "true,true,true,true,7,8,true,true,true,0"
    );
}

/** Function body text cannot escape its own closing delimiter and run statements in the global scope. */
#[test]
fn dynamic_function_body_escape_never_executes() {
    let source = "var escaped=0;const out=[];for(const C of [Function,(function*(){}).constructor,(async function(){}).constructor,(async function*(){}).constructor]){try{C('}), escaped=1, (function() {');out.push(false)}catch(error){out.push(error instanceof SyntaxError)}}out.push(escaped);const r=out.join();";
    assert_eq!(common::eval(source, "r"), "true,true,true,true,0");
}

/** Parameter text is parsed independently even if a combined wrapper could otherwise form a valid program. */
#[test]
fn dynamic_parameter_escape_and_cross_argument_comments_are_rejected() {
    let source = "var escaped=0;const out=[];for(const args of [['a){}), escaped=1, (function(',''],['/*','*/ return 1'],['a//','return 1']]){try{Function(...args);out.push(false)}catch(error){out.push(error instanceof SyntaxError)}}out.push(escaped);const r=out.join();";
    assert_eq!(common::eval(source, "r"), "true,true,false,0");
}

/** ToString still processes every parameter and the body before any syntax failure is reported. */
#[test]
fn dynamic_argument_coercion_finishes_before_parsing() {
    let source = "let log='';const param={toString(){log+='parameter;';return ')'}};const body={toString(){log+='body;';return 'return 1'}};let error;try{Function(param,body)}catch(caught){error=caught.name}const r=log+error;";
    assert_eq!(common::eval(source, "r"), "parameter;body;SyntaxError");
}

/** Each function kind retains its parameter grammar and valid body operators after constructor admission. */
#[test]
fn dynamic_function_kinds_preserve_valid_boundaries_and_operators() {
    let source = "const G=(function*(){}).constructor;const A=(async function(){}).constructor;const AG=(async function*(){}).constructor;const out=[Function('a/*comment*/','return a//tail')(3),G('a','yield a')(4).next().value];for(const C of [G,A,AG]){try{C(C===G?'yield':'await','');out.push(false)}catch(error){out.push(error instanceof SyntaxError)}}A('a','return await a')(5).then(value=>console.log(value));AG('a','yield await a')(6).next().then(result=>console.log(result.value));console.log(out.join());";
    let execution = common::load(source);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    let console = common::console(&execution);
    assert_eq!(
        console.first().map(String::as_str),
        Some("3,4,true,true,true")
    );
    assert!(console.iter().any(|entry| entry == "5"));
    assert!(console.iter().any(|entry| entry == "6"));
}

/** Unmatched surrogate source cannot become an identifier through lossy UTF16 conversion. */
#[test]
fn invalid_surrogates_and_incomplete_unicode_escapes_are_syntax_errors() {
    let source = "const out=[];for(const code of ['var '+String.fromCharCode(0xD801,0xABCD)+';',\"'\\\\u000\"+String.fromCharCode(0xD801,0xABCD)+\"'\",'var �;','var 😀;']){try{eval(code);out.push(false)}catch(error){out.push(error instanceof SyntaxError)}}const r=out.join();";
    assert_eq!(common::eval(source, "r"), "true,true,true,true");
}

/** Unicode letters and astral starts remain valid; marks and joining controls are permitted only in continuations. */
#[test]
fn unicode_identifier_start_and_continue_use_distinct_properties() {
    let source =
        "let π=1;let á=2;let a‌b=3;let a‍c=4;let 𐐀=5;let \\u03C3=6;const r=[π,á,a‌b,a‍c,𐐀,σ].join();";
    assert_eq!(common::eval(source, "r"), "1,2,3,4,5,6");
}

/** Escapes must satisfy the same start and continuation rules as raw identifier characters. */
#[test]
fn invalid_escaped_identifier_characters_are_rejected() {
    let source = "const out=[];for(const code of ['var \\\\u0030;','var \\\\u200D;','var \\\\u0301;','var a\\\\uFFFD;','var \\\\u002D;','var a\\\\u002D;']){try{eval(code);out.push(false)}catch(error){out.push(error instanceof SyntaxError)}}const r=out.join();";
    assert_eq!(common::eval(source, "r"), "true,true,true,true,true,true");
}

/** A validated dynamic closure keeps its original body and parameter bindings after normal state transport. */
#[test]
fn dynamic_function_admission_survives_state_transport() {
    let sources = vec!["const f=Function('π','return π+1');document.getElementById('go').addEventListener('click',()=>console.log(f(7),f.name));".into()];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let clicked = mars_v9::click_page(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        3,
    )
    .unwrap();
    assert!(clicked.failed.is_empty(), "{:?}", common::failure(&clicked));
    assert_eq!(common::console(&clicked), ["8 anonymous"]);
}
