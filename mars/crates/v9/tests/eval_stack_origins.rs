/** Eval CallSites retain real source creation origins, sourceURL distinctions, and normal source/heap lifetimes. */
use crate::common;

/** Hosts supply source identity without modifying the script being evaluated. */
fn eval(source: &str) -> String {
    let mut environment = common::environment();
    environment.scripts = vec!["https://fixtures.example/eval.js".to_owned()];
    let ran = common::load_with(source, &environment);
    assert!(ran.failed.is_empty(), "{:?}", common::failure(&ran));
    common::global(&ran, "r")
}

/** The origin points to the same genuine caller PC exposed by its structured frame. */
#[test]
fn direct_eval_records_its_actual_creation_callsite() {
    assert_eq!(
        eval(
            "Error.prepareStackTrace=(_,s)=>s;function caller(){return eval('new Error()')}const s=caller().stack,p=s[1],frameLocation=p.getFileName()+':'+p.getLineNumber()+':'+p.getColumnNumber();let r=s[0].isEval()+','+(s[0].getFunctionName()==='eval')+','+(s[0].getFileName()===undefined)+','+(s[0].getEvalOrigin()==='eval at caller ('+frameLocation+')')+','+(p.getEvalOrigin()===undefined);"
        ),
        "true,true,true,true,true"
    );
}

/** Provenance belongs to the parsed source, including named functions invoked after eval returns. */
#[test]
fn eval_created_functions_keep_their_source_provenance() {
    assert_eq!(
        eval(
            "Error.prepareStackTrace=(_,s)=>s;function creator(){return eval('(function born(){return new Error()})')}const f=creator(),s=f().stack;let r=(s[0].getFunctionName()==='born')+','+s[0].isEval()+','+(s[0].getFileName()===undefined)+','+s[0].getEvalOrigin().startsWith('eval at creator (https://fixtures.example/eval.js:')+','+!s[1].isEval();"
        ),
        "true,true,true,true,true"
    );
}

/** Nested eval uses the creator's immutable origin instead of attributing it to the later error call. */
#[test]
fn nested_eval_records_the_real_creation_chain() {
    assert_eq!(
        eval(
            "Error.prepareStackTrace=(_,s)=>s;function outer(){return eval(\"function inner(){return eval('new Error()')}inner()\")}const s=outer().stack;let r=(s[0].getEvalOrigin()==='eval at inner ('+s[1].getEvalOrigin()+')')+','+(s[1].getEvalOrigin()===s[2].getEvalOrigin())+','+(s[1].getEvalOrigin().startsWith('eval at outer (https://fixtures.example/eval.js:'))+','+s[1].isEval();"
        ),
        "true,true,true,true"
    );
}

/** Indirect eval captures its actual caller despite executing in global scope. */
#[test]
fn indirect_eval_records_the_actual_calling_function() {
    assert_eq!(
        eval(
            "Error.prepareStackTrace=(_,s)=>s;const indirect=eval;function caller(){return indirect('new Error()')}const s=caller().stack;let r=s[0].isEval()+','+s[0].getEvalOrigin().startsWith('eval at caller (https://fixtures.example/eval.js:');"
        ),
        "true,true"
    );
}

/** SourceURL is scanner metadata; the filename remains absent while named-source accessors and rendering use the URL. */
#[test]
fn genuine_source_url_does_not_replace_the_eval_filename() {
    assert_eq!(
        eval(
            "Error.prepareStackTrace=(_,s)=>s;const f=eval('(function named(){return new Error()})\\n//# sourceURL=owned-eval.js'),s=f().stack[0];let r=(s.getFileName()===undefined)+','+(s.getScriptNameOrSourceURL()==='owned-eval.js')+','+(s.getEvalOrigin()==='owned-eval.js')+','+s.toString().startsWith('named (owned-eval.js:');"
        ),
        "true,true,true,true"
    );
}

/** Text resembling magic comments inside a literal cannot supply source metadata. */
#[test]
fn source_url_text_inside_literals_is_not_a_directive() {
    assert_eq!(
        eval(
            "Error.prepareStackTrace=(_,s)=>s;const f=eval('(function(){const text=\"//# sourceURL=forged.js\";return new Error()})'),s=f().stack[0];let r=(s.getScriptNameOrSourceURL()===undefined)+','+s.getEvalOrigin().startsWith('eval at <anonymous> (https://fixtures.example/eval.js:');"
        ),
        "true,true"
    );
}

/** Function constructors also compile actual eval sources, with provenance from the genuine parse call. */
#[test]
fn dynamic_function_records_its_real_eval_compilation_origin() {
    assert_eq!(
        eval(
            "Error.prepareStackTrace=(_,s)=>s;function create(){return Function('return new Error()')}const s=create()().stack[0];let r=s.isEval()+','+s.getEvalOrigin().startsWith('eval at create (https://fixtures.example/eval.js:');"
        ),
        "true,true"
    );
}

/** Canonical state transport preserves the source-owned origin without keeping the creating activation alive. */
#[test]
fn eval_origin_survives_state_transport() {
    let sources=vec!["Error.prepareStackTrace=(_,s)=>s;const f=eval('(function(){return new Error()})');const expected=f().stack[0].getEvalOrigin();document.getElementById('go').addEventListener('click',()=>console.log(f().stack[0].getEvalOrigin()===expected));".to_owned()];
    let host = common::host();
    let environment = common::environment();
    let input = || common::input(&sources, &host, &environment);
    let ran = mars_v9::load_page(input()).unwrap();
    assert!(ran.failed.is_empty(), "{:?}", common::failure(&ran));
    let clicked = mars_v9::click_page(input(), common::round_trip(ran.state), 3).unwrap();
    assert!(clicked.failed.is_empty(), "{:?}", common::failure(&clicked));
    assert_eq!(common::console(&clicked), ["true"]);
}

/** Equal Function text in a foreign native context must not reuse another context's parsed source origin. */
#[cfg(feature = "conformance")]
#[test]
fn function_creation_origins_are_partitioned_by_the_actual_native_context() {
    let source = "Error.prepareStackTrace=(_,s)=>s;const bank=Realm.createAllowCrossRealmAccess();Realm.eval(bank,'Error.prepareStackTrace=(_,s)=>s');const ForeignFunction=Realm.eval(bank,'Function');function rootCreate(){return Function('return new Error()')}function foreignCreate(){return ForeignFunction('return new Error()')}const a=rootCreate(),b=foreignCreate(),first=a().stack[0].getEvalOrigin(),second=b().stack[0].getEvalOrigin();let r=first.startsWith('eval at rootCreate (')+','+second.startsWith('eval at foreignCreate (')+','+(Object.getPrototypeOf(b)===Realm.eval(bank,'Function.prototype'));";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.scripts = vec!["https://fixtures.example/realm-origin.js".to_owned()];
    let ran = common::load_with(source, &environment);
    assert!(ran.failed.is_empty(), "{:?}", common::failure(&ran));
    assert_eq!(common::global(&ran, "r"), "true,true,true");
}

/** Restored diagnostic ownership cannot claim a nonexistent native context before the next script runs. */
#[test]
fn stored_eval_origins_reject_unknown_compilation_contexts() {
    let sources = vec![
        "const f=Function('return 1');document.getElementById('go').onclick=()=>console.log(f());"
            .to_owned(),
    ];
    let host = common::host();
    let environment = common::environment();
    let input = || common::input(&sources, &host, &environment);
    let loaded = mars_v9::load_page(input()).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let mut state = serde_json::to_value(loaded.state).unwrap();
    state["dynamic"][0][3]["realm"] = serde_json::json!(u32::MAX);
    let state = serde_json::from_value(state).unwrap();
    let error = mars_v9::click_page(input(), state, 3).err().unwrap();
    assert_eq!(error.code, "invalid_request");
}

/** Metadata consumes the existing source allowance even when the source text itself remains small. */
#[test]
fn transported_eval_metadata_cannot_escape_the_dynamic_byte_budget() {
    let sources = vec![
        "const f=Function('return 1');document.getElementById('go').onclick=()=>console.log(f());"
            .to_owned(),
    ];
    let host = common::host();
    let environment = common::environment();
    let input = || common::input(&sources, &host, &environment);
    let loaded = mars_v9::load_page(input()).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let mut state = serde_json::to_value(loaded.state).unwrap();
    state["dynamic"][0][3]["origin"] =
        serde_json::json!("x".repeat(mars_v9::constants::VM_DYNAMIC_BYTES));
    let state = serde_json::from_value(state).unwrap();
    let error = mars_v9::click_page(input(), state, 3).err().unwrap();
    assert_eq!(error.code, "limit_exceeded");
}
