/** A value that cannot be iterated is named in the message the way V8 names it: by its source
text at a loop, spread or destructuring, by its callee when it came out of a call, and by its
type when the engine has no source text for it. */
use crate::common;

/** A loop head or array spread prints the iterable as written, nullish or not. */
#[test]
fn loops_and_spreads_name_the_iterable_expression() {
    assert_eq!(
        common::eval(
            "let r=[];const x={};for(const f of [()=>{for(const a of x.y);},()=>[...x.y],()=>[...1],()=>{for(const a of {});}]){try{f()}catch(e){r.push(e.message)}}r=r.join('|')",
            "r"
        ),
        "x.y is not iterable|x.y is not iterable|1 is not iterable|{} is not iterable"
    );
}

/** A value straight out of a call is blamed on the call. */
#[test]
fn call_results_blame_the_callee() {
    assert_eq!(
        common::eval(
            "let r;function f(){}try{for(const a of f());}catch(e){r=e.message}",
            "r"
        ),
        "f is not a function or its return value is not iterable"
    );
}

/** An array pattern prints a name or literal; a member value is described by its type, with
the property the engine could not read. */
#[test]
fn array_patterns_print_names_and_describe_members() {
    assert_eq!(
        common::eval(
            "let r=[];const x={y:1};let v;try{var [a]=v}catch(e){r.push(e.message)}try{var [b]=x.y}catch(e){r.push(e.message)}try{var [c]=x.z}catch(e){r.push(e.message)}r=r.join('|')",
            "r"
        ),
        "v is not iterable|number 1 is not iterable (cannot read property Symbol(Symbol.iterator))|undefined is not iterable (cannot read property Symbol(Symbol.iterator))"
    );
}

/** A spread argument has V8's own two messages, one for a nullish value and one for the rest. */
#[test]
fn spread_arguments_have_their_own_messages() {
    assert_eq!(
        common::eval(
            "let r=[];const x={};function f(){}try{f(...x.y)}catch(e){r.push(e.message)}try{f(...1)}catch(e){r.push(e.message)}r=r.join('|')",
            "r"
        ),
        "x.y is not iterable (cannot read property undefined)|Spread syntax requires ...iterable[Symbol.iterator] to be a function"
    );
}

/** Reading Symbol.iterator from null or undefined is reported as a failed iteration. */
#[test]
fn iterator_symbol_read_from_nullish_is_an_iteration_error() {
    assert_eq!(
        common::eval(
            "let r=[];const x={};try{null[Symbol.iterator]}catch(e){r.push(e.message)}try{x.y[Symbol.iterator]}catch(e){r.push(e.message)}r=r.join('|')",
            "r"
        ),
        "object null is not iterable (cannot read property Symbol(Symbol.iterator))|undefined is not iterable (cannot read property Symbol(Symbol.iterator))"
    );
}

/** A native that iterates a value has no source text and describes the value by its type. */
#[test]
fn native_iteration_describes_the_value_by_type() {
    assert_eq!(
        common::eval("let r;try{new Set(1)}catch(e){r=e.message}", "r"),
        "number 1 is not iterable (cannot read property Symbol(Symbol.iterator))"
    );
}

/** `in` on a non-object names the key and the value as V8 prints them. */
#[test]
fn in_operator_names_both_sides() {
    assert_eq!(
        common::eval(
            "let r=[];const x={};for(const f of [()=>'a' in x.y,()=>Symbol('q') in 1,()=>({}) in 'abc']){try{f()}catch(e){r.push(e.message)}}r=r.join('|')",
            "r"
        ),
        "Cannot use 'in' operator to search for 'a' in undefined|Cannot use 'in' operator to search for 'Symbol(q)' in 1|Cannot use 'in' operator to search for '#<Object>' in abc"
    );
}
