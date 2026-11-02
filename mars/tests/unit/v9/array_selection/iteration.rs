/** Fixed-length selection preserves the distinct sparse rules of some and find. */
use super::*;

/** Callability is checked even when no source element can invoke the callback. */
#[test]
fn empty_arrays_validate_callbacks_and_return_the_standard_result_types() {
    assert_eq!(
        result("", "[].some(()=>true)+' '+([].find(()=>true)===undefined)"),
        "false true"
    );
    for method in ["some", "find"] {
        for callback in ["", "undefined", "null", "0", "{}"] {
            assert_eq!(
                execute(&format!("[].{method}({callback});"))
                    .unwrap_err()
                    .code,
                "script_execution_error"
            );
        }
    }
}

/** A present undefined slot is visited by both methods while only find visits absent indices. */
#[test]
fn sparse_some_skips_holes_and_find_visits_them_as_undefined() {
    let source = "const a=[];a.length=4;a[1]=undefined;a[3]=7;let some='';let find='';const s=a.some((value,index)=>{some=some+index;return false;});const f=a.find((value,index)=>{find=find+index+(value===undefined);return false;});";
    assert_eq!(
        result(source, "some+' '+find+' '+s+' '+(f===undefined)"),
        "13 0true1true2true3false false true"
    );
}

/** Matching an absent value still terminates find without visiting a later defined slot. */
#[test]
fn find_returns_undefined_for_a_matching_hole_and_stops_immediately() {
    let source = "const a=[];a.length=3;a[2]=7;let trail='';const f=a.find((value,index)=>{trail=trail+index;return true;});";
    assert_eq!(result(source, "(f===undefined)+' '+trail"), "true 0");
}

/** Find retains the original selected reference even if callback code replaces the source slot. */
#[test]
fn find_returns_the_precallback_value_and_preserves_object_identity() {
    let source = "const original={n:7};const a=[original];const found=a.find((value,index,array)=>{array[index]={n:99};return true;});";
    assert_eq!(
        result(source, "(found===original)+' '+found.n+' '+a[0].n"),
        "true 7 99"
    );
}

/** Future edits and filled holes are read fresh while appends beyond the original range are ignored. */
#[test]
fn predicates_observe_future_changes_but_ignore_new_indices_beyond_snapshot_length() {
    for method in ["some", "find"] {
        let source = format!(
            "const a=[];a.length=3;a[0]=1;a[2]=3;let trail='';a.{method}((value,index)=>{{trail=trail+index+':'+value+';';if(index===0){{a[1]=20;a[2]=30;a[3]=40;}}return false;}});"
        );
        assert_eq!(result(&source, "trail+' '+a.length"), "0:1;1:20;2:30; 4");
    }
}

/** A smaller current length changes presence but cannot reduce find's snapshotted traversal range. */
#[test]
fn shrinking_length_skips_some_positions_but_find_still_visits_them() {
    let source = "const a=[1,2,3];const b=[1,2,3];let s='';let f='';a.some((v,i)=>{s=s+i;if(i===0){a.length=1;}return false;});b.find((v,i)=>{f=f+i+':'+v+';';if(i===0){b.length=1;}return false;});";
    assert_eq!(result(source, "s+' '+f"), "0 0:1;1:undefined;2:undefined;");
}

/** Success avoids all later callbacks, including one that would otherwise reject the candidate. */
#[test]
fn both_predicates_stop_before_later_failing_callbacks() {
    let source = "let s='';let f='';const some=[1,2,3].some((v,i)=>{s=s+i;if(i===1){return 'yes';}if(i===2){missing();}return 0;});const found=[4,5,6].find((v,i)=>{f=f+i;if(i===1){return {};}if(i===2){missing();}return null;});";
    assert_eq!(result(source, "some+' '+found+' '+s+' '+f"), "true 5 01 01");
}

/** Holes consume shared work; endless callbacks stop at the work budget and deep recursion throws a catchable RangeError. */
#[test]
fn sparse_traversal_and_nested_predicates_share_operation_and_depth_limits() {
    assert_eq!(
        result("const a=[];a.length=1024;", "a.some(()=>true)"),
        "false"
    );
    assert_eq!(
        execute("[1].some(()=>{while(true){}});").unwrap_err().code,
        "limit_exceeded"
    );
    let error = execute("function recurse(){return [1].find(recurse);}recurse();").unwrap_err();
    assert_eq!(error.code, "script_execution_error");
    assert!(error.message.contains("RangeError"), "{}", error.message);
}
