/** User Timing 3 options: Google's inline bootstrap calls `performance.mark(name, {startTime})`,
and when V9 refused it, the scripts after it failed on the helpers it defines. */
use crate::common;

/** A mark takes its startTime and detail from the options; detail defaults to null. */
#[test]
fn mark_options_set_start_time_and_detail() {
    let source = "const a=performance.mark('a',{startTime:12,detail:{k:1}});const b=performance.mark('b');\
        const r=[a.startTime,a.detail.k,b.detail].join();";
    assert_eq!(common::eval(source, "r"), "12,1,");
}

/** A negative startTime is a TypeError. */
#[test]
fn a_negative_mark_start_time_throws() {
    let source = "let r='';try{performance.mark('x',{startTime:-1})}catch(e){r=e.name}";
    assert_eq!(common::eval(source, "r"), "TypeError");
}

/** measure options resolve start and end from marks, numbers and duration. */
#[test]
fn measure_options_resolve_marks_numbers_and_duration() {
    let source = "performance.mark('s',{startTime:10});performance.mark('e',{startTime:30});\
        const a=performance.measure('a',{start:'s',end:'e'});const b=performance.measure('b',{start:5,duration:7});\
        const c=performance.measure('c',{end:40,duration:15,detail:'d'});\
        const r=[a.startTime,a.duration,b.startTime,b.duration,c.startTime,c.duration,c.detail].join();";
    assert_eq!(common::eval(source, "r"), "10,20,5,7,25,15,d");
}

/** A measure may start from `navigationStart`, a PerformanceTiming attribute, as Next.js does: it
resolves to the time origin, so the measure starts at 0 and is recorded. */
#[test]
fn measure_accepts_a_timing_attribute_name_as_its_start() {
    let source = "performance.mark('late');\
        const m=performance.measure('from-navigation','navigationStart','late');\
        const r=[m.startTime,m.entryType,performance.getEntriesByName('from-navigation').length].join();";
    assert_eq!(common::eval(source, "r"), "0,measure,1");
}

/** A name that is neither a mark nor a timing attribute still throws, so a missing mark is not hidden. */
#[test]
fn a_measure_from_an_unknown_name_still_throws() {
    let source = "let r='';try{performance.measure('x','no-such-mark')}catch(e){r=e.message}";
    assert_eq!(common::eval(source, "r"), "Unknown performance mark");
}
