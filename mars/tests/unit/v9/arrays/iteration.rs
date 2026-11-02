/** Callback iteration follows the pinned V8 HasProperty/Get/Call ordering for bounded owned arrays. */
use super::*;

/** Ordinary callbacks receive value, index, original array and the explicit receiver. */
#[test]
fn map_callbacks_receive_current_value_index_array_and_this_argument() {
    let source = "const a=[2,4,6];const b=a.map(function(value,index,original){return value+index+this.offset+(original===a);},{offset:10});";
    assert_eq!(result(source, "b.length+' '+b.join(' ')"), "3 13 16 19");
}

/** Arrow and bound callbacks keep their own receiver semantics rather than accepting the adapter's thisArg. */
#[test]
fn arrow_and_bound_array_callbacks_reuse_lexical_and_bound_receivers() {
    let source = "const receiver={n:4,run:function(){return [1,2].map(value=>value+this.n,{n:99});}};const a=receiver.run();const b=[1,2].map(function(value){return value+this.n;}.bind({n:7}),{n:99});";
    assert_eq!(result(source, "a.join(' ')+':'+b.join(' ')"), "5 6:8 9");
}

/** A missing callback is rejected before an empty source can skip every invocation. */
#[test]
fn map_and_filter_validate_callback_callability_for_empty_arrays() {
    for source in ["[].map();", "[].filter(null);", "[].map({});"] {
        assert_eq!(
            execute(source).unwrap_err().code,
            "script_execution_error",
            "{source}"
        );
    }
}

/** Present undefined elements invoke callbacks while true holes stay absent in map results. */
#[test]
fn map_preserves_snapshot_length_and_holes_and_visits_present_undefined() {
    let source = "const a=[];a.length=4;a[1]=undefined;a[3]=7;let seen='';const b=a.map((value,index)=>{seen=seen+index;return value;});let mapped='';b.map((value,index)=>{mapped=mapped+index;return value;});";
    assert_eq!(
        result(source, "seen+' '+mapped+' '+b.length+' '+b.join(':')"),
        "13 13 4 :::7"
    );
}

/** Every visit rereads its value, but new indices beyond the initial length wait for a future call. */
#[test]
fn map_observes_pending_value_changes_and_ignores_appends_beyond_the_snapshot() {
    let source = "const a=[1,2,3];const b=a.map((value,index)=>{if(index===0){a[1]=20;a[3]=4;}return value;});";
    assert_eq!(
        result(source, "b.join(' ')+' '+b.length+' '+a.length"),
        "1 20 3 3 4"
    );
}

/** Shrinking removes future positions; restoring one before its turn makes that position observable again. */
#[test]
fn map_skips_shrunk_elements_and_visits_positions_restored_before_their_turn() {
    let source = "const a=[1,2,3];let seen='';const b=a.map((value,index)=>{seen=seen+index;if(index===0){a.length=1;a[2]=9;}return value;});";
    assert_eq!(
        result(source, "seen+' '+b.length+' '+b.join(',')"),
        "02 3 1,,9"
    );
}

/** Filter compacts matching sparse values and preserves the value read before its predicate mutates the slot. */
#[test]
fn filter_is_dense_and_keeps_the_value_captured_before_the_predicate() {
    let source = "const a=[];a.length=5;a[1]=2;a[3]=4;const b=a.filter((value,index,original)=>{original[index]=99;return value>2;});";
    assert_eq!(
        result(source, "b.length+' '+b.join(',')+' '+a[3]"),
        "1 4 99"
    );
}

/** Filter uses ordinary JavaScript truthiness rather than requiring a Boolean return. */
#[test]
fn filter_predicates_use_truthiness_and_snapshot_iteration() {
    let source = "const a=[0,1,2];const b=a.filter((value,index)=>{if(index===0){a[2]=8;a[3]=9;}return value;});";
    assert_eq!(result(source, "b.join(' ')+' '+b.length"), "1 8 2");
}

/** Endless callbacks stop at the work budget; recursion past the call depth throws a catchable RangeError. */
#[test]
fn array_callbacks_share_operation_and_call_depth_limits() {
    assert_eq!(
        execute("[1].map(()=>{while(true){}});").unwrap_err().code,
        "limit_exceeded"
    );
    let error = execute("function recurse(){return [1].filter(recurse);}recurse();").unwrap_err();
    assert_eq!(error.code, "script_execution_error");
    assert!(error.message.contains("RangeError"), "{}", error.message);
}
