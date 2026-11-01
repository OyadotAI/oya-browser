/** A definition the target refuses names the missing property when the target is not extensible, and reports a redefinition otherwise; proxies answer for their target. */
use crate::common;

/** Object.defineProperty on a non-extensible object names the property it could not add. */
#[test]
fn define_property_on_non_extensible_object_names_missing_property() {
    assert_eq!(
        common::eval(
            "let r;try{Object.defineProperty(Object.preventExtensions({}),'x',{value:1});r='none'}catch(e){r=e.message}",
            "r"
        ),
        "Cannot define property x, object is not extensible"
    );
}

/** A property that already exists and is non-configurable is a redefinition, even on a non-extensible object. */
#[test]
fn define_property_on_existing_non_configurable_property_is_a_redefinition() {
    assert_eq!(
        common::eval(
            "let r;const o=Object.preventExtensions(Object.defineProperty({},'x',{value:1}));try{Object.defineProperty(o,'x',{value:2});r='none'}catch(e){r=e.message}",
            "r"
        ),
        "Cannot redefine property: x"
    );
}

/** A proxy's refusal comes from its target, so a missing property on a non-extensible target is named. */
#[test]
fn proxy_definition_refusal_names_missing_property_of_target() {
    assert_eq!(
        common::eval(
            "let r;const p=new Proxy(Object.preventExtensions({}),{});try{Object.defineProperty(p,'y',{value:1});r='none'}catch(e){r=e.message}",
            "r"
        ),
        "Cannot define property y, object is not extensible"
    );
}

/** A class field defined on an object that was made non-extensible names the field. */
#[test]
fn class_field_on_non_extensible_instance_names_field() {
    assert_eq!(
        common::eval(
            "let r;class X{f=Object.preventExtensions(this)}try{new X();r='none'}catch(e){r=e.message}",
            "r"
        ),
        "Cannot define property f, object is not extensible"
    );
}
