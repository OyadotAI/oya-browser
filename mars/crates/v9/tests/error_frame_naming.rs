/** Error stack text names frames the way V8 does: receiver-qualified methods, builtin frames, constructions, aliases, and synthetic initializers, all from real frame data. */
use crate::common;

/** The first `count` frame lines of a thrown error's default stack, with each location dropped. */
fn frames(body: &str, count: usize) -> String {
    let source = format!(
        "let r = ''; try {{ {body} }} catch (e) {{ r = e.stack.split('\\n').slice(1, {count} + 1).map(l => l.trim().split(' (')[0]).join('|') }}"
    );
    common::eval(&source, "r")
}

/** A method called through a prototype names its receiver's type, not the bare function. */
#[test]
fn method_frames_are_qualified_by_receiver_type() {
    assert_eq!(
        frames(
            "function Foo() {} Foo.prototype.bar = function () { throw new Error('y'); }; new Foo().bar();",
            1
        ),
        "at Foo.bar"
    );
}

/** An anonymous callback reached through a receiver's property reports the type and property name. */
#[test]
fn anonymous_method_frames_use_the_receiver_type_and_key() {
    assert_eq!(
        frames(
            "function Nirk() {} Nirk.prototype.valueOf = function () { throw new Error('v'); }; Math.max(new Nirk());",
            1
        ),
        "at Nirk.valueOf"
    );
}

/** A function stored under a second property is spelled with the alias V8 uses. */
#[test]
fn aliased_method_frames_name_the_property_they_were_called_through() {
    assert_eq!(
        frames(
            "function Wookie() {} Wookie.prototype.d = function a() { throw new Error('w'); }; new Wookie().d();",
            1
        ),
        "at Wookie.a [as d]"
    );
}

/** A builtin that runs script shows as its own frame, qualified by its receiver: Array.map. */
#[test]
fn builtin_frames_are_shown_with_their_receiver_type() {
    assert_eq!(
        frames("[1].map(function cb() { throw new Error('m'); });", 2),
        "at cb|at Array.map"
    );
}

/** Math.max is a builtin frame of the Math namespace, which carries its own toStringTag. */
#[test]
fn namespace_builtin_frames_use_the_tag_as_their_type() {
    assert_eq!(
        frames("Math.max({ valueOf() { throw new Error('z'); } });", 2),
        "at Object.valueOf|at Math.max"
    );
}

/** A native constructor that runs script reports as a construction, not a plain call. */
#[test]
fn native_constructor_frames_read_as_constructions() {
    assert_eq!(
        frames(
            "new ReferenceError({ toString() { throw new Error('t'); } });",
            2
        ),
        "at Object.toString|at new ReferenceError"
    );
}

/** The frame of the Error constructor itself is never part of its own capture, even under a limit of one. */
#[test]
fn error_constructor_frame_is_not_part_of_its_own_stack() {
    assert_eq!(
        common::eval(
            "Error.stackTraceLimit = 1; Error.prepareStackTrace = (_, s) => s.map(f => f.getFunctionName()).join(','); function caller() { return new Error('x').stack; } let r = caller();",
            "r"
        ),
        "caller"
    );
}

/** A function whose own name property was rewritten reports that debug name, as V8 does. */
#[test]
fn rewritten_own_name_is_the_frame_name() {
    assert_eq!(
        frames(
            "var f = function foo() { throw new Error('n'); }; Object.defineProperty(f, 'name', { value: 'boo' }); f();",
            1
        ),
        "at boo"
    );
}

/** A class's instance field initializer shows as a bare synthetic frame, not as a method of the instance. */
#[test]
fn instance_initializer_frames_are_synthetic_and_bare() {
    assert_eq!(
        frames(
            "function thrower() { throw new Error('f'); } class X { x = thrower(); } new X();",
            2
        ),
        "at thrower|at <instance_members_initializer>"
    );
}

/** A class with an explicit constructor shows that construction around its initializer. */
#[test]
fn explicit_constructor_frames_wrap_the_initializer() {
    assert_eq!(
        frames(
            "function thrower() { throw new Error('f'); } class X { x = thrower(); constructor() {} } new X();",
            3
        ),
        "at thrower|at <instance_members_initializer>|at new X"
    );
}

/** Method frames reached through a class field keep the field's key as the alias. */
#[test]
fn class_field_method_frames_use_type_and_alias() {
    assert_eq!(
        frames(
            "function thrower() { throw new Error('c'); } class X { x = thrower; } let x = new X(); x.x();",
            1
        ),
        "at X.thrower [as x]"
    );
}

/** A sloppy caller stays visible when the builtin it called has no younger script frame. */
#[test]
fn builtin_without_younger_script_hides_nothing() {
    assert_eq!(
        common::eval(
            "Error.prepareStackTrace = (_, s) => s; function callee() { return new Error('q').stack; } function caller() { return callee(); } const s = caller(); let r = s.slice(0, 2).map(f => f.getFunction() === undefined).join(',');",
            "r"
        ),
        "false,false"
    );
}

/** Call and apply forward without their own frame, so only the forwarded-to function shows. */
#[test]
fn call_and_apply_forwarders_add_no_frame() {
    assert_eq!(
        frames(
            "Array.prototype.map.call([1], function cb() { throw new Error('c'); });",
            2
        ),
        "at cb|at Array.map"
    );
}

/** A static method called on a class is qualified by the class name, as V8 prints it. */
#[test]
fn static_method_frames_use_the_class_name() {
    assert_eq!(
        frames(
            "function thrower() { throw new Error('s'); } class X { static x = thrower; } X.x();",
            1
        ),
        "at X.thrower [as x]"
    );
}

/** A non-number Error.stackTraceLimit installs no stack property at all, while a numeric zero keeps a header. */
#[test]
fn non_number_stack_limit_installs_no_stack() {
    assert_eq!(
        common::eval(
            "Error.stackTraceLimit = 'not a number'; let r = String(new Error('x').stack);",
            "r"
        ),
        "undefined"
    );
}

/** A numeric zero limit still installs a stack: just the header, with no frames. */
#[test]
fn numeric_zero_stack_limit_keeps_the_header() {
    assert_eq!(
        common::eval(
            "Error.stackTraceLimit = 0; let r = new Error('x').stack;",
            "r"
        ),
        "Error: x"
    );
}

/** A name getter that deletes `stack` during formatting leaves it deleted for the next read. */
#[test]
fn stack_deleted_during_formatting_stays_deleted() {
    assert_eq!(
        common::eval(
            "const error = new Error('foobar'); error.__defineGetter__('name', function () { delete error.stack; }); let r = String(error.stack.startsWith('Error')) + ',' + String(error.stack);",
            "r"
        ),
        "true,undefined"
    );
}

/** Error.captureStackTrace does not show itself as the top frame of the object's stack. */
#[test]
fn capture_stack_trace_does_not_show_itself() {
    assert_eq!(
        common::eval(
            "function target() { const o = {}; Error.captureStackTrace(o); return o.stack; } let r = target().split('\\n')[1].trim().split(' (')[0];",
            "r"
        ),
        "at target"
    );
}
