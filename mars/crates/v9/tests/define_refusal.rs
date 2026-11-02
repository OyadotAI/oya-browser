/** Refused property definitions name V8's cause: a closed target for a new key, a redefinition otherwise. */
use crate::common;
use common::eval;

/** A new key on a non-extensible object is refused with the closed-target message, through Object.defineProperty. */
#[test]
fn define_property_on_closed_object_names_the_closed_target() {
    let source = "let r;try{Object.defineProperty(Object.preventExtensions({}),'zz',{value:1})}catch(e){r=e.message}";
    assert_eq!(
        eval(source, "r"),
        "Cannot define property zz, object is not extensible"
    );
}

/** An existing non-configurable property refuses a new descriptor with the redefinition message even on a closed object. */
#[test]
fn define_property_on_existing_locked_key_names_the_redefinition() {
    let source = "let r;const o=Object.preventExtensions(Object.defineProperty({},'k',{value:1}));try{Object.defineProperty(o,'k',{value:2})}catch(e){r=e.message}";
    assert_eq!(eval(source, "r"), "Cannot redefine property: k");
}

/** Accessor definitions through __defineGetter__ on a closed object use the closed-target message. */
#[test]
fn define_getter_on_closed_object_names_the_closed_target() {
    let source =
        "let r;try{Object.preventExtensions({}).__defineGetter__('g',()=>1)}catch(e){r=e.message}";
    assert_eq!(
        eval(source, "r"),
        "Cannot define property g, object is not extensible"
    );
}

/** A class field on a closed instance is refused with the closed-target message, not a redefinition. */
#[test]
fn class_field_on_closed_instance_names_the_closed_target() {
    let source =
        "let r;class X{['field']=Object.preventExtensions(this)}try{new X()}catch(e){r=e.message}";
    // Computed-key field: the same refusal as the named-key form, reported by the field's define step.
    assert_eq!(
        eval(source, "r"),
        "Cannot define property field, object is not extensible"
    );
}

/** A field defined on a proxy whose target is closed reports the target's refusal, following the proxy chain. */
#[test]
fn class_field_through_closed_proxy_names_the_closed_target() {
    let source = "let r;const p=new Proxy(Object.preventExtensions({}),{});\
        class Base{constructor(){return p}}class X extends Base{['field']=1}try{new X()}catch(e){r=e.message}";
    assert_eq!(
        eval(source, "r"),
        "Cannot define property field, object is not extensible"
    );
}
