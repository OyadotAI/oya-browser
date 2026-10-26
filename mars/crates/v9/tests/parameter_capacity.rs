/** V8-compatible logical arity remains independent of actual heap, frame, code, source and execution budgets. */
mod common;

/** The original regression first exercises thousands of distinct formals with matching and missing actual arguments. */
const LARGE_PARAMETERS: usize = 8_000;
/** The largest ordinary positive case in regress-1122 uses this many formals and actuals. */
const REGRESSION_PARAMETERS: usize = 32_000;

/** Distinct source names expose slot truncation and accidental duplicate-binding reuse. */
fn names(count: usize) -> String {
    (0..count)
        .map(|index| format!("p{index}"))
        .collect::<Vec<_>>()
        .join(",")
}

/** Literal actuals exercise the real static call operand instead of bypassing it with apply. */
fn actuals(count: usize) -> String {
    (0..count)
        .map(|index| index.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

/** The exposed policy states the actual native receiver-inclusive arity ceiling. */
#[test]
fn published_parameter_capacity_matches_the_native_explicit_arity() {
    assert_eq!(mars_v9::constants::FUNCTION_PARAMETERS, 65_525);
}

/** Valid large functions bind every supplied argument and report their full observable length. */
#[test]
fn large_distinct_formals_and_static_actuals_execute() {
    for count in [LARGE_PARAMETERS, REGRESSION_PARAMETERS] {
        let middle = count / 2;
        let source = format!(
            "function f({}){{return [p0,p{middle},p{},f.length].join(':')}}const r=f({});",
            names(count),
            count - 1,
            actuals(count)
        );
        assert_eq!(
            common::eval(&source, "r"),
            format!("0:{middle}:{}:{count}", count - 1)
        );
    }
}

/** Missing actuals still initialize their high formal slots to undefined. */
#[test]
fn underapplication_initializes_large_formal_lists() {
    let source = format!(
        "function f({}){{return [p0,p{},f.length].join(':')}}const r=f({});",
        names(REGRESSION_PARAMETERS),
        REGRESSION_PARAMETERS / 2,
        actuals(LARGE_PARAMETERS)
    );
    assert_eq!(common::eval(&source, "r"), "0::32000");
}

/** The exact highest valid formal and static-argument positions execute without u16 truncation. */
#[test]
fn maximum_native_formal_and_static_argument_arity_executes() {
    let count = mars_v9::constants::FUNCTION_PARAMETERS;
    let source = format!(
        "function f({}){{return [p0,p{},f.length].join(':')}}const r=f({});",
        names(count),
        count - 1,
        actuals(count)
    );
    assert_eq!(
        common::eval(&source, "r"),
        format!("0:{}:{count}", count - 1)
    );
}

/** Oversized logical arity is a SyntaxError before execution, rather than an engine-capacity failure. */
#[test]
fn oversized_parameter_and_static_argument_lists_are_early_errors() {
    let count = mars_v9::constants::FUNCTION_PARAMETERS + 1;
    let parameters = names(count);
    let arguments = actuals(count);
    for source in [
        format!("function f({parameters}){{}}"),
        format!("function* f({parameters}){{}}"),
        format!("async function f({parameters}){{}}"),
        format!("async function* f({parameters}){{}}"),
        format!("({parameters})=>0"),
        format!("f({arguments})"),
        format!("new F({arguments})"),
    ] {
        assert_eq!(
            mars_v9::check(&source).unwrap_err().code,
            "script_syntax_error"
        );
    }
    let code = serde_json::to_string(&format!("observed++;function f({parameters}){{}}")).unwrap();
    let source = format!(
        "let observed=0,syntax=false;try{{eval({code})}}catch(error){{syntax=error instanceof SyntaxError}}const r=[syntax,observed].join();"
    );
    assert_eq!(common::eval(&source, "r"), "true,0");
}

/** A rest parameter contributes to native arity even though it does not contribute to function length. */
#[test]
fn rest_parameters_count_toward_the_native_boundary() {
    let count = mars_v9::constants::FUNCTION_PARAMETERS;
    let source = format!(
        "function f({},...tail){{return [tail[0],tail.length,f.length].join(':')}}const r=f({});",
        names(count - 1),
        actuals(count)
    );
    assert_eq!(
        common::eval(&source, "r"),
        format!("{}:1:{}", count - 1, count - 1)
    );
    let source = format!("function f({},...tail){{}}", names(count));
    assert_eq!(
        mars_v9::check(&source).unwrap_err().code,
        "script_syntax_error"
    );
}

/** Only the final sloppy duplicate formal maps its matching actual argument to the live binding. */
#[test]
fn large_duplicate_lists_preserve_last_occurrence_arguments_aliases() {
    let count = LARGE_PARAMETERS;
    let parameters = vec!["x"; count].join(",");
    let source = format!(
        "const f=function({parameters}){{arguments[0]=50;x=41;const before=[arguments[0],arguments[{}],x];delete arguments[{}];x=42;return before.concat([arguments[{}],x,f.length]).join(':')}};const r=f.apply(null,Array({count}).fill(1));",
        count - 1,
        count - 1,
        count - 1
    );
    assert_eq!(common::eval(&source, "r"), format!("50:41:41::42:{count}"));
}

/** Defaults retain TDZ initialization and unmapped arguments while length stops at the first default. */
#[test]
fn large_default_parameter_lists_keep_length_and_unmapped_arguments() {
    let count = LARGE_PARAMETERS;
    let source = format!(
        "function f({},tail=7){{arguments[0]=50;return [p0,tail,arguments[0],f.length].join(':')}}const r=f.apply(null,Array({}).fill(1));",
        names(count - 1),
        count - 1
    );
    assert_eq!(common::eval(&source, "r"), format!("1:7:50:{}", count - 1));
    let strict = format!(
        "function f({}){{'use strict';return p{}}}const r=f({});",
        names(count),
        count - 1,
        actuals(count)
    );
    assert_eq!(common::eval(&strict, "r"), (count - 1).to_string());
}

/** Apply may supply a body argument beyond the formal count; duplicate generator formals retain their full length. */
#[test]
fn maximum_generator_constructor_formals_are_real_and_callable() {
    let count = mars_v9::constants::FUNCTION_PARAMETERS;
    let source = format!(
        "const G=(function*(){{}}).constructor;const args=Array({count}+1);args[{count}]='yield 7';const f=G.apply(null,args);const first=f(),second=f();const r=[f.length,first.next().value,second.next().value,Object.getPrototypeOf(f)===G.prototype].join();"
    );
    assert_eq!(common::eval(&source, "r"), format!("{count},7,7,true"));
}

/** Exact source and ordinary constructor prototypes remain observable for large authored functions. */
#[test]
fn large_formals_preserve_source_and_function_prototypes() {
    let function = format!(
        "function large({}){{/*body*/return p0}}",
        names(LARGE_PARAMETERS)
    );
    let source_text = serde_json::to_string(&function).unwrap();
    let source = format!(
        "const f=({function});const r=[f.toString()==={source_text},f.length,Object.getPrototypeOf(f)===Function.prototype,new f() instanceof f].join();"
    );
    assert_eq!(
        common::eval(&source, "r"),
        format!("true,{LARGE_PARAMETERS},true,true")
    );
}

/** Closures and direct eval retain all large-scope names through ordinary public state transport. */
#[test]
fn large_parameter_environments_remain_visible_to_eval_after_transport() {
    let count = LARGE_PARAMETERS;
    let source = format!(
        "function f({}){{return function(){{return eval('p{}')}}}}const saved=f({});document.getElementById('go').addEventListener('click',()=>console.log(saved()));",
        names(count),
        count / 2,
        actuals(count)
    );
    let sources = vec![source];
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
    assert_eq!(common::console(&clicked), [(count / 2).to_string()]);
}

/** Maximum-arity generator frames preserve high formal slots, incoming arguments and mapped environments across JSON. */
#[test]
fn large_generator_parameters_and_arguments_survive_transport() {
    let count = mars_v9::constants::FUNCTION_PARAMETERS;
    let source = format!(
        "function* f({}){{yield p0;return [p{},arguments.length,f.length].join(':')}}const gen=f({});gen.next();document.getElementById('go').addEventListener('click',()=>console.log(gen.next().value));",
        names(count),
        count / 2,
        actuals(count)
    );
    let sources = vec![source];
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
    assert_eq!(
        common::console(&clicked),
        [format!("{}:{count}:{count}", count / 2)]
    );
}
