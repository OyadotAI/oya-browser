/** CallSite.getMethodName reports the property that installs an accessor, as V8 does, instead of its `get `/`set ` name. */
mod common;
use common::eval;

/** A getter frame reports the property key it was installed under, even for a numeric key. */
#[test]
fn getter_frame_reports_its_property_key() {
    let source = "let r;Error.prepareStackTrace=(_,s)=>s;\
        const o={get 0(){throw new Error('x')}};\
        try{o[0]}catch(e){r=e.stack[0].getMethodName()+'|'+e.stack[0].getFunctionName()}";
    assert_eq!(eval(source, "r"), "0|get 0");
}

/** A setter frame reports the property key, found on the receiver's prototype chain. */
#[test]
fn setter_frame_reports_the_inherited_property_key() {
    let source = "let r;Error.prepareStackTrace=(_,s)=>s;\
        class Base{set value(v){throw new Error('x')}}\
        class Child extends Base{}\
        try{new Child().value=1}catch(e){r=e.stack[0].getMethodName()}";
    assert_eq!(eval(source, "r"), "value");
}
