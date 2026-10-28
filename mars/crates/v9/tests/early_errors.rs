/*! Parser early errors (ECMAScript Static Semantics: Early Errors), checked through `check`. */

/** Each source must fail to parse with a SyntaxError. */
fn rejects(sources: &[&str]) {
    for source in sources {
        let error = mars_v9::check(source).expect_err(source);
        assert!(
            format!("{error:?}").contains("yntax"),
            "{source}: expected a syntax error, got {error:?}"
        );
    }
}

/** Each source must parse. */
fn accepts(sources: &[&str]) {
    for source in sources {
        if let Err(error) = mars_v9::check(source) {
            panic!("{source}: {error:?}");
        }
    }
}

/** A `#name` must be declared by an enclosing class body, once (a getter and setter may pair). */
#[test]
fn private_names_need_one_declaration() {
    rejects(&[
        "this.#x",
        "class C { m() { this.#y } }",
        "class C { #x; m() { class D { n() { this.#z } } } }",
        "class C { #x; #x; }",
        "class C { #x; get #x() {} }",
        "class C { get #x() {} get #x() {} }",
        "class C { static get #x() {} set #x(v) {} }",
        "class C { m() { #x in this } }",
        "({ #x: 1 })",
        "class C { #constructor; }",
    ]);
    accepts(&[
        "class C { #x; m() { return this.#x } }",
        "class C { m() { return this.#x } #x = 1; }",
        "class C { get #x() {} set #x(v) {} }",
        "class C { static get #x() {} static set #x(v) {} }",
        "class C { #x; m() { class D { n(o) { return o.#x } } } }",
        "class C { #x; m(o) { return #x in o } }",
    ]);
}

/** `delete` cannot remove a private member, parenthesized or in an optional chain. */
#[test]
fn private_members_cannot_be_deleted() {
    rejects(&[
        "class C { #x; m() { delete this.#x } }",
        "class C { #x; m() { delete (this.#x) } }",
        "class C { #x; m() { delete this?.#x } }",
    ]);
    accepts(&["class C { #x; m() { delete this.x } }"]);
}

/** Field initializers and static blocks cannot see `arguments`; nested functions can. */
#[test]
fn class_fields_reject_arguments() {
    rejects(&[
        "class C { x = arguments; }",
        "class C { x = () => arguments; }",
        "class C { static x = typeof arguments; }",
        "class C { static { arguments; } }",
        "class C { x = { arguments }; }",
    ]);
    accepts(&[
        "class C { x = function() { return arguments }; }",
        "class C { m() { return arguments } }",
        "class C { x = this.arguments; }",
    ]);
}

/** `await` is reserved directly inside a class static block. */
#[test]
fn static_blocks_reserve_await() {
    rejects(&[
        "class C { static { var await; } }",
        "class C { static { await; } }",
    ]);
    accepts(&[
        "class C { static { function f() { var await; } } }",
        "class C { static { (function await(await) {}); (function* await(await) {}); } }",
    ]);
}

/** Class element names: no `constructor` field or accessor, no static `prototype`. */
#[test]
fn class_element_names_are_checked() {
    rejects(&[
        "class C { constructor; }",
        "class C { 'constructor' = 1; }",
        "class C { static constructor; }",
        "class C { static prototype; }",
        "class C { static 'prototype'() {} }",
        "class C { get constructor() {} }",
    ]);
    accepts(&[
        "class C { constructor() {} }",
        "class C { static constructor() {} }",
        "class C { prototype; }",
        "class C { ['constructor'] = 1; }",
    ]);
}

/** A `"use strict"` body needs a simple parameter list and strict-valid parameter names. */
#[test]
fn use_strict_needs_simple_parameters() {
    rejects(&[
        "function f(a = 1) { 'use strict' }",
        "function f({a}) { 'use strict' }",
        "function f(...a) { 'use strict' }",
        "(a = 1) => { 'use strict' }",
        "({ m([a]) { 'use strict' } })",
        "function f(eval) { 'use strict' }",
    ]);
    accepts(&["function f(a, b) { 'use strict' }"]);
}

/** Duplicate parameters are only allowed in a sloppy function with a simple list. */
#[test]
fn duplicate_parameters_need_sloppy_simple_lists() {
    rejects(&[
        "function f(a, a = 1) {}",
        "function f(a, [a]) {}",
        "(a, a) => 1",
        "({ m(a, a) {} })",
        "'use strict'; function f(a, a) {}",
    ]);
    accepts(&["function f(a, a) {}"]);
}

/** `if`, loop and `with` bodies take no declarations; sloppy `if` alone may take a function. */
#[test]
fn statement_positions_reject_declarations() {
    rejects(&[
        "while (0) function f() {}",
        "for (;;) function f() {}",
        "for (x of []) function f() {}",
        "if (1) function* g() {}",
        "if (1) async function f() {}",
        "'use strict'; if (1) function f() {}",
        "if (1) l: function f() {}",
        "while (0) l: function f() {}",
        "l: function* g() {}",
        "'use strict'; l: function f() {}",
    ]);
    accepts(&[
        "if (1) function f() {}",
        "l: function f() {}",
        "if (1) {} else function f() {}",
    ]);
}

/** Block and case-block names: one lexical declaration each, no `var` reusing it. */
#[test]
fn block_redeclarations_are_rejected() {
    rejects(&[
        "{ function f() {} function* f() {} }",
        "{ async function f() {} function f() {} }",
        "switch (0) { case 1: let f; default: let f; }",
        "switch (0) { case 1: function f() {} default: class f {} }",
        "switch (0) { case 1: const f = 1; default: var f; }",
        "'use strict'; { function f() {} function f() {} }",
    ]);
    accepts(&[
        "{ function f() {} function f() {} }",
        "switch (0) { case 1: function f() {} default: function f() {} }",
        "switch (0) { case 1: var f; default: var f; }",
    ]);
}

/** Lexical bindings cannot be named `let` or repeat a name. */
#[test]
fn lexical_bindings_reject_let_and_duplicates() {
    rejects(&[
        "let let = 1;",
        "let [a, a] = [];",
        "const a = 1, a = 2;",
        "for (let [a, a] of []) ;",
        "for (const let in {}) ;",
    ]);
    accepts(&["var a, a;", "let a = 1, b = 2;"]);
}

/** A for-in/of declaration has no initializer, except the sloppy simple `var` for-in. */
#[test]
fn for_in_of_heads_reject_initializers() {
    rejects(&[
        "for (let x = 1 in {}) ;",
        "for (var x = 1 of []) ;",
        "for (var [x] = 1 of []) ;",
        "'use strict'; for (var x = 1 in {}) ;",
    ]);
    accepts(&["for (var x = 1 in {}) ;"]);
}

/** Getters take no parameters and setters exactly one non-rest parameter. */
#[test]
fn accessor_parameter_counts() {
    rejects(&[
        "({ get x(a) {} })",
        "({ set x() {} })",
        "({ set x(...a) {} })",
        "class C { get x(a = 1) {} }",
    ]);
    accepts(&["({ get x() {}, set x(v) {} })"]);
}

/** An assignment rest element cannot carry an initializer. */
#[test]
fn rest_targets_reject_initializers() {
    rejects(&["[...x = 1] = [];"]);
    accepts(&["[...x] = [];"]);
}

/** `#name in object` is a RelationalExpression: as the operand of a tighter operator the bare
private name is an early error, while the relational forms stay grammatical. */
#[test]
fn private_brand_checks_bind_at_relational_precedence() {
    rejects(&[
        "class C { #x; m() { 0 << #x in {} } }",
        "class C { #x; m() { -#x in {} } }",
        "class C { #x; m() { #x } }",
    ]);
    accepts(&[
        "class C { #x; m() { return #x in {} } }",
        "class C { #x; m() { return (#x in {}) === true } }",
        "class C { #x; m() { return #x in {} && #x in {} } }",
        "class C { #x; m() { return 1 < (#x in {}) } }",
    ]);
}

/** A call is a web-compat assignment target only in sloppy code; strict code gets the early error. */
#[test]
fn call_targets_are_errors_in_strict_code() {
    rejects(&[
        "'use strict'; f() = 1;",
        "'use strict'; (f()) = 1;",
        "'use strict'; f() += 1;",
        "'use strict'; f()++;",
        "'use strict'; ++f();",
        "'use strict'; for (f() in [1]) {}",
        "'use strict'; for (f() of [1]) {}",
        "function g() { 'use strict'; f() = 1; }",
    ]);
    accepts(&[
        "f() = 1;",
        "(f()) = 1;",
        "f() += 1;",
        "f()++;",
        "for (f() in [1]) {}",
    ]);
}

/** A parenthesized assignment is an expression with no assignment target, not a default. */
#[test]
fn parenthesized_assignments_are_not_targets() {
    rejects(&[
        "var a, b; (a = b) = 1;",
        "(x = 1) = 1;",
        "((x = y)) = 1;",
        "function f() { var a, b = 2; (a = b) = 1; }",
        "[(x = 1)] = [];",
        "({ a: (b = 1) } = {});",
    ]);
    accepts(&[
        "[x = 1] = [];",
        "({ a: b = 1 } = {});",
        "(x) = 1;",
        "(x.y) = 1;",
        "(a = b);",
    ]);
}

/** An assignment rest element ends its list: no comma, elision or element may follow it. */
#[test]
fn rest_elements_take_no_trailing_comma() {
    rejects(&[
        "[...x,] = [];",
        "0, [...x,] = [];",
        "({...rest,} = {});",
        "for ([...x,] in [[]]) ;",
        "for ({...rest,} of [{}]) ;",
        "[...x, ,] = [];",
    ]);
    accepts(&[
        "[...x] = [];",
        "({...rest} = {});",
        "[...x,];",
        "({...rest,});",
        "[a,] = [];",
    ]);
}

/** Arrow parameters keep the enclosing `yield` and `await` grammar; only the body gets the arrow's own. */
#[test]
fn arrow_parameters_keep_the_enclosing_yield_and_await() {
    rejects(&[
        "function *g() { (x = yield) => {}; }",
        "function *g() { yield => 1; }",
        "function *g() { (yield) => 1; }",
        "async function f() { (x = await 1) => x; }",
        "async function f() { (await) => 1; }",
        "async(a = await => {}) => {};",
        "async(a = (await) => {}) => {};",
        "async(a = (...await) => {}) => {};",
        "class C { static { (await => 0); } }",
        "class C { static { ((x = await) => 0); } }",
        "async await => 1;",
    ]);
    accepts(&[
        "(yield) => 1;",
        "yield => 1;",
        "(await) => 1;",
        "await => 1;",
        "function *g() { () => yield; }",
        "async function f() { async (x) => await x; }",
        "async function f() { (x) => { var await; }; }",
        "class C { static { (x) => { var await; }; } }",
    ]);
}

/** `super.#x` has no brand to check: SuperProperty names public members only. */
#[test]
fn super_cannot_reach_private_names() {
    rejects(&[
        "class C extends B { #x() {} method() { super.#x(); } }",
        "class C { #m; Child = class extends C { access() { return super.#m; } } }",
    ]);
    accepts(&[
        "class C extends B { #x() {} method() { super.x(); this.#x(); } }",
        "class C extends B { #x; m() { return super[this.#x]; } }",
    ]);
}

/** A label may carry a plain sloppy function, never an async function or async generator. */
#[test]
fn labels_reject_async_functions() {
    rejects(&[
        "label: async function f() {}",
        "label: async function* g() {}",
        "a: b: async function f() {}",
    ]);
    accepts(&["label: function f() {}", "label: async\nfunction f() {}"]);
}

/** `new.target` needs an enclosing non-arrow function or class element, spelled without escapes. */
#[test]
fn new_target_needs_a_non_arrow_function() {
    rejects(&[
        "new.target;",
        "() => { new.target; };",
        "() => new.target;",
        "function f() { new.t\\u0061rget; }",
        "function f() { new.other; }",
    ]);
    accepts(&[
        "function f() { new.target; }",
        "function f() { () => new.target; }",
        "class C { static { new.target; } }",
        "class C { x = new.target; }",
        "class C { static { () => new.target; } }",
        "({ m() { return new.target; } });",
    ]);
}

/** A `\\k<name>` must name a group once the pattern has named groups or the `u`/`v` flag. */
#[test]
fn named_group_references_need_a_group() {
    rejects(&[
        "/(?<a>a)\\k<ab>/;",
        "/(?<a>a)\\k<ab>/u;",
        "/(?<ab>a)\\k<a>/;",
        "/\\k<a>(?<ab>a)/;",
        "/\\k<a>/u;",
        "/\\k<a>/v;",
        "/(?<a>.)\\k<b>/;",
        "/[\\k<a>]/u;",
    ]);
    accepts(&[
        "/(?<a>a)\\k<a>/;",
        "/\\k<a>(?<a>x)/u;",
        "/(?<a>x)|(?<a>y)\\k<a>/;",
        "/\\k<a>/;",
        "/\\\\k<a>/u;",
        "/(?<\\u0061>x)\\k<\\u0061>/u;",
        "/(?<a>x)\\k<\\u0061>/u;",
    ]);
}
