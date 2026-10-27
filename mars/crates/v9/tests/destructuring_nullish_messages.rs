/** Destructuring null or undefined throws a TypeError that names the pattern's first key when it has one, as V8 does. */
mod common;

/** An object pattern with a first key names that key and the nullish value. */
#[test]
fn object_pattern_names_first_key_for_undefined() {
    assert_eq!(
        common::eval(
            "let r;try{({d:r}=undefined);r='none'}catch(e){r=e.message}",
            "r"
        ),
        "Cannot destructure property 'd' of 'undefined' as it is undefined."
    );
}

/** An empty object pattern has no key to name, so the message names only the nullish value. */
#[test]
fn empty_object_pattern_names_only_null_value() {
    assert_eq!(
        common::eval("let r;try{({}=null);r='none'}catch(e){r=e.message}", "r"),
        "Cannot destructure 'null' as it is null."
    );
}
