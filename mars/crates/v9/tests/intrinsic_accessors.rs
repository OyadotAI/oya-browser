/** Intrinsic accessor metadata and prototype operations retain the prescribed names, attributes and proxy observation. */
use crate::common;

/** Getter labels include their prefix while native dispatch still reads the correct property. */
#[test]
fn native_getter_labels_are_separate_from_dispatch_keys() {
    assert_eq!(
        common::eval(
            "const get=Object.getOwnPropertyDescriptor(ArrayBuffer.prototype,'byteLength').get;const typed=Object.getPrototypeOf(Uint8Array.prototype);const length=Object.getOwnPropertyDescriptor(typed,'length').get;const r=[get.name,get.call(new ArrayBuffer(3)),length.name,length.call(new Uint8Array(2)),get.toString().includes('get byteLength')].join('|');",
            "r"
        ),
        "get byteLength|3|get length|2|true"
    );
}

/** Function.prototype's intrinsic hasInstance is fixed, while an own method remains possible after removing that prototype. */
#[test]
fn has_instance_is_a_fixed_intrinsic_property() {
    assert_eq!(
        common::eval(
            "function F(){};let failed=false;try{(function(){'use strict';F[Symbol.hasInstance]=()=>true})()}catch(e){failed=e instanceof TypeError}Object.setPrototypeOf(F,null);F[Symbol.hasInstance]=()=>true;const d=Object.getOwnPropertyDescriptor(Function.prototype,Symbol.hasInstance);const r=[failed,1 instanceof F,d.writable,d.configurable].join();",
            "r"
        ),
        "true,true,false,false"
    );
}

/** isPrototypeOf observes each proxy's actual getPrototypeOf trap and propagates its exception. */
#[test]
fn prototype_queries_use_proxy_internal_methods() {
    assert_eq!(
        common::eval(
            "const a={};const p=new Proxy({},{getPrototypeOf(){return a}});const o=Object.create(p);let failed=false;const bad=new Proxy({},{getPrototypeOf(){throw 7}});try{a.isPrototypeOf(bad)}catch(e){failed=e===7}const r=[a.isPrototypeOf(o),p.isPrototypeOf(o),failed].join();",
            "r"
        ),
        "true,true,true"
    );
}

/** Proxy has no own prototype property, so ordinary GetPrototypeFromConstructor can reach an inherited getter. */
#[test]
fn proxy_constructor_has_no_placeholder_prototype() {
    assert_eq!(
        common::eval(
            "let calls=0;const proto={};Object.setPrototypeOf(Proxy,{get prototype(){calls++;return proto}});const a=Reflect.construct(Object,[],Proxy);const r=[Object.hasOwn(Proxy,'prototype'),calls,Object.getPrototypeOf(a)===proto].join();",
            "r"
        ),
        "false,1,true"
    );
}

/** A shell's global tag is an ordinary own property; replacing it with a non-string uses the ordinary Object fallback. */
#[cfg(feature = "conformance")]
#[test]
fn shell_global_tag_is_an_ordinary_property() {
    let environment = mars_v9::PageEnvironment {
        conformance_d8: true,
        ..common::environment()
    };
    let execution = common::load_with(
        "const before=Object.prototype.toString.call(globalThis);Object.defineProperty(globalThis,Symbol.toStringTag,{value:1});const after=Object.prototype.toString.call(globalThis);const r=[before,after,Object.hasOwn(globalThis,Symbol.toStringTag)].join('|');",
        &environment,
    );
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(
        common::global(&execution, "r"),
        "[object global]|[object Object]|true"
    );
}
