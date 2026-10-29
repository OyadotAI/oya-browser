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

/** The destructured value prints as the source wrote it. */
#[test]
fn object_pattern_names_the_value_expression() {
    assert_eq!(
        common::eval(
            "let r=[];const x={};function f(){}try{var {a}=x.y}catch(e){r.push(e.message)}try{var {b}=f()}catch(e){r.push(e.message)}try{var {[1]:c}=x.y}catch(e){r.push(e.message)}r=r.join('|')",
            "r"
        ),
        "Cannot destructure property 'a' of 'x.y' as it is undefined.|Cannot destructure property 'b' of 'f(...)' as it is undefined.|Cannot destructure 'x.y' as it is undefined."
    );
}

/** A parameter has no written value: V8 describes it by its type. */
#[test]
fn parameter_pattern_describes_the_value_by_type() {
    assert_eq!(
        common::eval(
            "let r=[];function f({a}){}try{f()}catch(e){r.push(e.message)}try{f(null)}catch(e){r.push(e.message)}r=r.join('|')",
            "r"
        ),
        "Cannot destructure property 'a' of 'undefined' as it is undefined.|Cannot destructure property 'a' of 'object null' as it is null."
    );
}

/** A nested pattern fails on the read of its first key, as V8 reports it. */
#[test]
fn nested_pattern_fails_as_a_property_read() {
    assert_eq!(
        common::eval(
            "let r;const x={};try{var {a:{b}}=x}catch(e){r=e.message}",
            "r"
        ),
        "Cannot read properties of undefined (reading 'b')"
    );
}

/** A loop head or catch clause destructures a hidden binding V8 names `.for` and `.catch`. */
#[test]
fn hidden_bindings_are_named_as_v8_names_them() {
    assert_eq!(
        common::eval(
            "let r=[];try{for(const {a} of [undefined]);}catch(e){r.push(e.message)}try{try{throw null}catch({a}){}}catch(e){r.push(e.message)}r=r.join('|')",
            "r"
        ),
        "Cannot destructure property 'a' of '.for' as it is undefined.|Cannot destructure property 'a' of '.catch' as it is null."
    );
}
