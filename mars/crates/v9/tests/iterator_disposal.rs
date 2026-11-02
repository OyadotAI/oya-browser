/** Iterator disposal preserves optional return semantics and intrinsic async continuation behavior. */
use crate::common;

/** A return method is optional and its primitive result is accepted by iterator disposal. */
#[test]
fn synchronous_disposal_discards_primitive_return_values() {
    assert_eq!(
        common::eval(
            "let n=0;const dispose=Iterator.prototype[Symbol.dispose];const source={return(){n++;return 7}};const r=[dispose.call(source)===undefined,n,dispose.call({return:null})===undefined].join();",
            "r"
        ),
        "true,1,true"
    );
}

/** The async prototype's method awaits a return thenable and resolves undefined, bypassing species getters. */
#[test]
fn asynchronous_disposal_awaits_without_species() {
    assert_eq!(
        common::eval(
            "const proto=Object.getPrototypeOf(Object.getPrototypeOf(Object.getPrototypeOf((async function*(){})())));const dispose=proto[Symbol.asyncDispose];let then=0;let r='pending';Object.defineProperty(Promise,Symbol.species,{get(){throw 8},configurable:true});const p=dispose.call({return(){return {then(resolve){then++;resolve(7)}}}});Object.defineProperty(Promise,Symbol.species,{value:Promise,configurable:true});Promise.prototype.then.call(p,v=>{r=[v===undefined,then].join()});",
            "r"
        ),
        "true,1"
    );
}

/** Error branding is independent of prototypes and refuses proxies without invoking any traps. */
#[test]
fn error_branding_does_not_follow_prototypes_or_proxies() {
    assert_eq!(
        common::eval(
            "const e=new TypeError();Object.setPrototypeOf(e,null);const rev=Proxy.revocable(new Error(),{});rev.revoke();const r=[Error.isError(e),Error.isError(Object.create(Error.prototype)),Error.isError(new Proxy(new Error(),{})),Error.isError(rev.proxy),Error.isError(new SuppressedError(1,2))].join();",
            "r"
        ),
        "true,false,false,false,true"
    );
}
