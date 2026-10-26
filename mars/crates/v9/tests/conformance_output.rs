/* Native d8 output preserves real host records, coercion order, timers, stack traces and explicitly exposed CPU marks. */
#![cfg(feature = "conformance")]
mod common;
use mars_v9::{Execution, HostEffects, PageEnvironment};

/** A shell host is explicitly selected; browser pages continue to use their own console contract. */
fn environment(marker: Option<&str>) -> PageEnvironment {
    PageEnvironment {
        conformance_d8: true,
        conformance_cpu_trace_name: marker.map(str::to_owned),
        ..common::environment()
    }
}

/** Every public fixture must finish without an uncaught script failure. */
fn run(source: &str, marker: Option<&str>) -> Execution {
    let execution = common::load_with(source, &environment(marker));
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    execution
}

/** Browser console assertions retain their standard nonthrowing behavior even in a conformance build. */
#[test]
fn browser_assertions_remain_nonthrowing() {
    let execution = common::load(
        "let threw=false;try{console.assert(false,'detail')}catch(error){threw=true}const r=threw;",
    );
    assert!(execution.failed.is_empty());
    assert_eq!(common::global(&execution, "r"), "false");
    assert_eq!(execution.effects.console[0].level, "error");
    assert_eq!(
        execution.effects.console[0].text,
        "Assertion failed: detail"
    );
}

/** Truthy assertions avoid formatting; falsy assertions print the original condition and throw a real Error. */
#[test]
fn d8_assertions_emit_output_then_throw() {
    let execution = run(
        "let conversions=0;console.assert(true,{toString(){conversions++;throw 1}});const out=[];for(const value of [false,'',0,undefined]){try{console.assert(value,'detail')}catch(error){out.push(error.constructor===Error,error.message)}}const r=[conversions,out.join('|')].join(';');",
        None,
    );
    assert_eq!(
        common::global(&execution, "r"),
        "0;true|console.assert failed|true|console.assert failed|true|console.assert failed|true|console.assert failed"
    );
    assert_eq!(
        common::console(&execution),
        [
            "console.assert: false detail",
            "console.assert:  detail",
            "console.assert: 0 detail",
            "console.assert: undefined detail"
        ]
    );
    assert!(
        execution
            .effects
            .console
            .iter()
            .all(|entry| entry.level == "log")
    );
}

/** Native shell output formats all arguments, including Symbol descriptions, on the correct stdout or stderr channel. */
#[test]
fn print_and_print_err_are_native_variadic_host_functions() {
    let execution = run(
        "print('start',1,true,null,undefined,[2,3],Symbol('label'),Symbol(),Symbol(''));printErr('error',4);const r=[typeof print,typeof printErr,print.toString().includes('[native code]'),printErr.toString().includes('[native code]')].join();",
        None,
    );
    assert_eq!(
        common::global(&execution, "r"),
        "function,function,true,true"
    );
    assert_eq!(execution.effects.console[0].level, "log");
    assert_eq!(
        execution.effects.console[0].text,
        "start 1 true null undefined 2,3 label undefined "
    );
    assert_eq!(execution.effects.console[1].level, "error");
    assert_eq!(execution.effects.console[1].text, "error 4");
}

/** D8 console methods use ordered ToString formatting and their own prefixes rather than browser object inspection. */
#[test]
fn d8_console_prefixes_preserve_real_argument_coercion() {
    let execution = run(
        "let conversions=0;const value={toString(){conversions++;return 'converted'}};console.log(value);console.error(value);console.warn(value);console.info(value);console.debug(value);const r=conversions;",
        None,
    );
    assert_eq!(common::global(&execution, "r"), "5");
    assert_eq!(
        common::console(&execution),
        [
            "converted",
            "console.error: converted",
            "console.warn: converted",
            "console.info: converted",
            "console.debug: converted"
        ]
    );
    assert_eq!(
        execution
            .effects
            .console
            .iter()
            .map(|entry| entry.level.as_str())
            .collect::<Vec<_>>(),
        ["log", "error", "warn", "info", "debug"]
    );
}

/** A throwing later conversion preserves its already-written prefix and prevents conversion of remaining arguments. */
#[test]
fn shell_output_preserves_partial_writes_and_conversion_errors() {
    let execution = run(
        "let order='';const marker={};const first={toString(){order+='first;';return 'written'}};const second={toString(){order+='second;';throw marker}};const last={toString(){order+='last;';return 'unreachable'}};let same=false;try{printErr(first,second,last)}catch(error){same=error===marker}const r=[same,order].join('|');",
        None,
    );
    assert_eq!(common::global(&execution, "r"), "true|first;second;");
    assert_eq!(execution.effects.console[0].level, "error");
    assert_eq!(execution.effects.console[0].text, "written ");
}

/** Print's WriteAndFlush still contributes an empty line when conversion of its first argument throws. */
#[test]
fn print_flushes_a_line_after_a_first_argument_failure() {
    let execution = run(
        "const marker={};let same=false;try{print({toString(){throw marker}})}catch(error){same=error===marker}const r=same;",
        None,
    );
    assert_eq!(common::global(&execution, "r"), "true");
    assert_eq!(execution.effects.console.len(), 1);
    assert_eq!(execution.effects.console[0].level, "log");
    assert_eq!(execution.effects.console[0].text, "");
}

/** Pinned d8 clears a printing exception before throwing its own assertion Error, retaining the emitted prefix. */
#[test]
fn d8_assertion_keeps_its_own_error_after_printing_fails() {
    let execution = run(
        "let actual;try{console.assert(false,{toString(){throw 7}})}catch(error){actual=error.constructor===Error && error.message==='console.assert failed'}const r=actual;",
        None,
    );
    assert_eq!(common::global(&execution, "r"), "true");
    assert_eq!(common::console(&execution), ["console.assert: false "]);
}

/** Console timers measure actual elapsed host clock time across a public state roundtrip and remove completed labels. */
#[test]
fn console_timers_survive_transport_and_measure_elapsed_time() {
    let source = "console.time('work');setTimeout(()=>{console.timeLog('work');console.timeEnd('work');console.timeEnd('work');console.timeStamp('phase')},25);";
    let loaded = run(source, None);
    let sources = vec![source.to_string()];
    let host = common::host();
    let environment = environment(None);
    let resumed = mars_v9::run_timers(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        25.0,
        1,
    )
    .unwrap();
    assert!(resumed.failed.is_empty(), "{:?}", common::failure(&resumed));
    assert_eq!(
        common::console(&resumed),
        [
            "console.timeLog: work, 25.000000",
            "console.timeEnd: work, 25.000000",
            "console.timeEnd: Timer 'work' does not exist",
            "console.timeStamp: phase, 25.000000"
        ]
    );
}

/** Label conversion has the real d8 TryCatch behavior and distinguishes a missing argument from explicit undefined. */
#[test]
fn console_timer_labels_keep_native_conversion_semantics() {
    let execution = run(
        "let conversions=0;const bad={toString(){conversions++;throw 1}};console.time(bad);console.timeStamp(bad);console.time(Symbol('bad'));console.time();console.time(undefined);console.time();console.timeEnd();console.timeEnd(undefined);const r=conversions;",
        None,
    );
    assert_eq!(common::global(&execution, "r"), "2");
    assert_eq!(
        common::console(&execution),
        [
            "console.time: Timer 'default' already exists",
            "console.timeEnd: default, 0.000000",
            "console.timeEnd: undefined, 0.000000"
        ]
    );
}

/** The shell delegate belongs to the isolate, so a different genuine Realm can end a timer begun by its caller. */
#[test]
fn console_timer_labels_are_shared_across_realms() {
    let execution = run(
        "const realm=Realm.createAllowCrossRealmAccess();console.time('shared');Realm.eval(realm,\"console.timeEnd('shared')\");",
        None,
    );
    assert_eq!(
        common::console(&execution),
        ["console.timeEnd: shared, 0.000000"]
    );
}

/** Console trace records actual active function frames instead of an empty placeholder or fabricated message. */
#[test]
fn console_trace_contains_the_actual_callers() {
    let execution = run(
        "function traceCaller(){console.trace()}function traceOuter(){traceCaller()}traceOuter();",
        None,
    );
    assert_eq!(execution.effects.console.len(), 1);
    assert_eq!(execution.effects.console[0].level, "error");
    assert!(execution.effects.console[0].text.contains("traceCaller ("));
    assert!(execution.effects.console[0].text.contains("traceOuter ("));
}

/** Only the actual requested extension name is exposed, with immutable installed source metadata separate from mutable name. */
#[test]
fn cpu_markers_require_the_actual_exposure_name() {
    let absent = run("const r=typeof cputracemark;", None);
    assert_eq!(common::global(&absent, "r"), "undefined");
    let execution = run(
        "const r=[typeof cputracemark,typeof markHere,markHere.name,markHere.toString().includes('function markHere()')].join();Object.defineProperty(markHere,'name',{value:'changed'});markHere(100,'ignored');",
        Some("markHere"),
    );
    assert_eq!(
        common::global(&execution, "r"),
        "undefined,function,markHere,true"
    );
    assert_eq!(execution.effects.conformance_trace_marks[0].marker, 100);
    assert_eq!(execution.effects.conformance_trace_marks[0].time_ms, 0.0);
}

/** Uint32 admission accepts exact numeric boundaries without consulting object, string or BigInt coercion hooks. */
#[test]
fn cpu_marker_arguments_use_uint32_brand_without_coercion() {
    let execution = run(
        "let conversions=0;for(const value of [0,-0,4294967295,100])mark(value,{toString(){throw 1}});const bad={valueOf(){conversions++;return 1}};const out=[];for(const value of [-1,null,'',undefined,0.5,NaN,Infinity,4294967296,1n,Object(1),bad]){try{mark(value);out.push(false)}catch(error){out.push(error.constructor===Error && error.message==='First parameter to cputracemark() must be a unsigned int32.')}}const r=[conversions,out.every(Boolean)].join();",
        Some("mark"),
    );
    assert_eq!(common::global(&execution, "r"), "0,true");
    assert_eq!(
        execution
            .effects
            .conformance_trace_marks
            .iter()
            .map(|mark| mark.marker)
            .collect::<Vec<_>>(),
        [0, 0, u32::MAX, 100]
    );
}

/** CPU records and console records share the existing diagnostic bound while native calls and coercions still occur. */
#[test]
fn cpu_marker_records_share_the_console_budget() {
    let limit = mars_v9::constants::VM_CONSOLE_ENTRIES;
    let source = format!(
        "for(let i=0;i<10;i++)print(i);for(let i=0;i<{};i++)mark(i);let conversions=0;print({{toString(){{conversions++;return 'dropped'}}}});const r=conversions;",
        limit + 1
    );
    let execution = run(&source, Some("mark"));
    assert_eq!(common::global(&execution, "r"), "1");
    assert_eq!(
        execution.effects.console.len() + execution.effects.conformance_trace_marks.len(),
        limit
    );
    assert_eq!(execution.effects.conformance_trace_marks_dropped, 11);
    assert_eq!(execution.effects.console_dropped, 1);
    let restored: HostEffects =
        serde_json::from_value(serde_json::to_value(&execution.effects).unwrap()).unwrap();
    assert_eq!(
        restored.conformance_trace_marks,
        execution.effects.conformance_trace_marks
    );
}

/** CPU marker timestamps use actual resumed host time, and the native function remains valid after state transport. */
#[test]
fn cpu_marker_timestamps_follow_the_host_clock() {
    let source = "setTimeout(()=>mark(7),12);";
    let loaded = run(source, Some("mark"));
    let sources = vec![source.to_string()];
    let host = common::host();
    let environment = environment(Some("mark"));
    let resumed = mars_v9::run_timers(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        12.0,
        1,
    )
    .unwrap();
    assert!(resumed.failed.is_empty(), "{:?}", common::failure(&resumed));
    assert_eq!(resumed.effects.conformance_trace_marks[0].marker, 7);
    assert_eq!(resumed.effects.conformance_trace_marks[0].time_ms, 12.0);
}

/** An empty exposure flag remains disabled and malformed native declaration text is rejected before scripts execute. */
#[test]
fn malformed_cpu_marker_names_cannot_inject_source() {
    let absent = run("const r=typeof cputracemark;", Some(""));
    assert_eq!(common::global(&absent, "r"), "undefined");
    let sources = vec!["throw 'must not run';".to_string()];
    let host = common::host();
    let environment = environment(Some("mark;injected()"));
    assert!(mars_v9::load_page(common::input(&sources, &host, &environment)).is_err());
}
