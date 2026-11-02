/** Legacy RegExp accessors use their actual creation-context match record independently of the supplied receiver. */
use crate::common;

/** Borrowing each capture/context getter neither requires nor consults the intrinsic constructor receiver. */
#[test]
fn borrowed_getters_accept_arbitrary_receivers() {
    assert_eq!(
        common::eval(
            "/(a)(b)/.exec('preabpost');const names=['input','$_','lastMatch','$&','lastParen','$+','leftContext','$`','rightContext',\"$'\",'$1','$2','$9'];let n=0;for(const name of names){const d=Object.getOwnPropertyDescriptor(RegExp,name);const expected=d.get.call(RegExp);for(const receiver of [undefined,null,1,'text',{},new Proxy({}, {get(){throw 1}})]){if(d.get.call(receiver)===expected)n++}}const r=n;",
            "r"
        ),
        "78"
    );
}

/** Ordinary inherited property reads retain the original receiver while accessing the same native match facts. */
#[test]
fn inherited_capture_accessors_do_not_throw_on_the_inheriting_object() {
    assert_eq!(
        common::eval(
            "const inherited=Object.create(RegExp);/(x)/.test('beforexafter');const values=[inherited.$1,inherited.lastMatch,inherited.leftContext,inherited.rightContext];inherited.$1={toString(){throw 1}};const r=[values.join(':'),inherited.$1,Object.hasOwn(inherited,'$1')].join(';');",
            "r"
        ),
        "x:x:before:after;x;false"
    );
}

/** The input setter performs exactly one ToString and updates only the retained input override. */
#[test]
fn borrowed_input_setter_converts_the_value_without_reading_the_receiver() {
    assert_eq!(
        common::eval(
            "/(a)/.test('abc');const d=Object.getOwnPropertyDescriptor(RegExp,'input');let calls=0;const receiver=new Proxy({}, {get(){throw 1}});const result=d.set.call(receiver,{toString(){calls++;return 'changed'}});const r=[result===undefined,calls,RegExp.input,RegExp.$_,RegExp.$1,RegExp.rightContext].join();",
            "r"
        ),
        "true,1,changed,changed,a,bc"
    );
}

/** Readonly legacy setters are inert compatibility functions, including when their argument is hostile. */
#[test]
fn borrowed_readonly_setters_do_not_coerce_or_modify_match_records() {
    assert_eq!(
        common::eval(
            "/(a)(b)/.test('preabpost');let n=0;const value={toString(){throw 1},valueOf(){throw 2}};for(const name of ['lastMatch','$&','lastParen','$+','leftContext','$`','rightContext',\"$'\",'$1','$2','$3','$4','$5','$6','$7','$8','$9']){const d=Object.getOwnPropertyDescriptor(RegExp,name),before=d.get.call(null);if(d.set.call(null,value)===undefined&&d.get.call(undefined)===before)n++}const r=n;",
            "r"
        ),
        "17"
    );
}

/** A failed input conversion propagates its original exception and leaves the previous override unchanged. */
#[test]
fn borrowed_input_setter_preserves_state_when_conversion_throws() {
    assert_eq!(
        common::eval(
            "RegExp.input='saved';const marker={};let same=false;const set=Object.getOwnPropertyDescriptor(RegExp,'$_').set;try{set.call(undefined,{toString(){throw marker}})}catch(e){same=e===marker}const r=[same,RegExp.input,RegExp.$_].join();",
            "r"
        ),
        "true,saved,saved"
    );
}

/** Entering a native getter or setter selects its creation realm even when the receiver belongs elsewhere. */
#[cfg(feature = "conformance")]
#[test]
fn borrowed_accessors_preserve_cross_realm_match_and_input_isolation() {
    let environment = mars_v9::PageEnvironment {
        conformance_d8: true,
        ..common::environment()
    };
    let execution = common::load_with(
        "const child=Realm.create();const getter=Realm.eval(child,\"/(b)/.exec('foreignbafter');Object.getOwnPropertyDescriptor(RegExp,'$1').get\");const setter=Realm.eval(child,\"Object.getOwnPropertyDescriptor(RegExp,'input').set\");const foreign=Realm.eval(child,'RegExp');/(a)/.exec('rootaafter');const root=Object.getOwnPropertyDescriptor(RegExp,'$1').get;setter.call(RegExp,'foreign override');const r=[getter.call(RegExp),root.call(foreign),RegExp.input,Realm.eval(child,'RegExp.input'),RegExp.$1].join(';');",
        &environment,
    );
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(
        common::global(&execution, "r"),
        "b;a;rootaafter;foreign override;a"
    );
}
