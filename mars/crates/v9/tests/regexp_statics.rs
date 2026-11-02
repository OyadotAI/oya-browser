/** Legacy RegExp result accessors retain exact successful matches across string operations. */
use crate::common;

/** Numbered captures, aliases and contexts come from the actual latest match. */
#[test]
fn statics_describe_the_latest_match() {
    assert_eq!(
        common::eval(
            "/((\\d+)\\.(\\d+))/.exec('pre123.45post');const r=[RegExp.input,RegExp.lastMatch,RegExp.lastParen,RegExp.leftContext,RegExp.rightContext,RegExp.$1,RegExp.$2,RegExp.$3,RegExp.$9].join('|');",
            "r"
        ),
        "pre123.45post|123.45|45|pre|post|123.45|123|45|"
    );
}

/** Failed execs preserve the prior successful result instead of resetting it. */
#[test]
fn failed_matches_leave_previous_statics() {
    assert_eq!(
        common::eval(
            "/(a)/.test('abc');/(z)/.test('abc');const r=[RegExp.lastMatch,RegExp.$1,RegExp.rightContext].join();",
            "r"
        ),
        "a,a,bc"
    );
}

/** LastParen describes the last syntactic group, even when that group did not participate. */
#[test]
fn unmatched_last_group_is_empty() {
    assert_eq!(
        common::eval(
            "/(a)(b)?/.test('a');const r=[RegExp.$1,RegExp.$2,RegExp.lastParen].join();",
            "r"
        ),
        "a,,"
    );
}

/** Global replacement discovers its matches before functional replacers read the last one. */
#[test]
fn global_replacers_observe_the_last_discovered_capture() {
    assert_eq!(
        common::eval("const r='abcd'.replace(/(.)/g,()=>RegExp.$1);", "r"),
        "dddd"
    );
}

/** Compatibility setters do not overwrite captures, while input setters perform ToString. */
#[test]
fn setters_preserve_match_facts() {
    assert_eq!(
        common::eval(
            "/(a)/.test('abc');RegExp.$1='x';RegExp.input=123;const r=[RegExp.input,RegExp.$1,RegExp.leftContext,RegExp.rightContext].join();",
            "r"
        ),
        "123,a,,bc"
    );
}

/** Retained match records survive the same validated JSON state boundary as ordinary globals. */
#[test]
fn match_records_survive_request_restoration() {
    let sources = [
        "let observed='';/(a)/.exec('abc');setTimeout(()=>{observed=RegExp.$1},0);const r='loaded';"
            .to_string(),
    ];
    let host = common::host();
    let environment = common::environment();
    let execution = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    let execution = mars_v9::run_timers(
        common::input(&sources, &host, &environment),
        common::round_trip(execution.state),
        0.0,
        1,
    )
    .unwrap();
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "observed"), "a");
}
