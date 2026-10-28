/** Join converts supported primitives while preserving separators for every snapshot position. */
use super::*;

/** Holes and nullish values contribute empty fields; other primitives use ECMAScript formatting. */
#[test]
fn join_preserves_sparse_nullish_fields_and_formats_supported_primitives() {
    let source = "const a=[];a.length=6;a[1]=undefined;a[2]=null;a[3]=true;a[4]=7;a[5]='Rust 🐙';";
    assert_eq!(result(source, "a.join()"), ",,,true,7,Rust 🐙");
    assert_eq!(result("", "[1,2].join(null)"), "1null2");
    assert_eq!(result("", "[1,2].join('')"), "12");
}

/** Separator conversion occurs even when no positions will be visited. */
#[test]
fn empty_array_join_still_converts_the_separator() {
    assert_eq!(result("", "[].join(undefined)"), "");
    assert_eq!(
        result(
            "let calls=0;const joined=[].join({toString(){calls++;return ',';}});",
            "joined+calls"
        ),
        "1"
    );
}

/** Object, nested-array and function elements join through ToString, as in JavaScript. */
#[test]
#[ignore = "known V9 gap: Function.prototype.toString prints [native code]; V9 keeps no function source spans"]
fn join_converts_object_nested_array_and_function_elements() {
    assert_eq!(result("", "[{}].join()"), "[object Object]");
    assert_eq!(result("", "[[1,[2]]].join()"), "1,2");
    assert_eq!(result("", "[()=>1].join()"), "()=>1");
}

/** A joined value may reach the exact UTF-16 cap, while the next append fails before growing it further. */
#[test]
fn join_checks_utf16_output_budget_before_appending() {
    let half = JS_STRING_UNITS / 2;
    assert_eq!(
        result(
            &format!("const s='x'.repeat({half});"),
            "[s,s].join('').length"
        ),
        JS_STRING_UNITS.to_string()
    );
    // Past the bound is the catchable RangeError browsers throw (crates/v9/tests/bounds.rs).
    assert_eq!(
        result(
            &format!(
                "const s='x'.repeat({half});let caught='';try{{[s,s].join(',')}}catch(e){{caught=e.constructor.name+':'+e.message}}"
            ),
            "caught"
        ),
        "RangeError:Invalid string length"
    );
}
