/** Proxy chains cannot overflow the native stack: trap-less proxies forward in a loop, and a
chain whose traps recurse natively meets the call-depth bound as a RangeError. */
mod common;
use std::sync::Mutex;

/// Proxies in each chain: far past any native stack if every hop were a native call.
const CHAIN: usize = 1_000_000;

/// Building a million proxies takes a good part of the script deadline, so the tests run one at
/// a time instead of competing for the CPU and timing out.
static SERIAL: Mutex<()> = Mutex::new(());

/** Run `source` alone and read `r`. */
fn eval(source: &str, name: &str) -> String {
    let _serial = SERIAL.lock().unwrap_or_else(|poison| poison.into_inner());
    common::eval(source, name)
}

/** A script that wraps `{a:1}` in CHAIN proxies sharing `handler` (one handler keeps the chain
under the heap's object bound), then stores `check`, or the name of what it threw, in `r`. */
fn chain(handler: &str, check: &str) -> String {
    format!(
        "const h={handler};let p={{a:1}};for(let i=0;i<{CHAIN};i++)p=new Proxy(p,h);\
         let r;try{{r=String({check})}}catch(e){{r=e.name}}"
    )
}

/** [[GetPrototypeOf]] forwards through a million trap-less proxies. */
#[test]
fn get_prototype_of_follows_a_trapless_chain() {
    let source = chain("{}", "Object.getPrototypeOf(p)===Object.prototype");
    assert_eq!(eval(&source, "r"), "true");
}

/** [[IsExtensible]] forwards through a million trap-less proxies. */
#[test]
fn is_extensible_follows_a_trapless_chain() {
    assert_eq!(eval(&chain("{}", "Object.isExtensible(p)"), "r"), "true");
}

/** [[Get]] forwards through a million trap-less proxies. */
#[test]
fn get_follows_a_trapless_chain() {
    assert_eq!(eval(&chain("{}", "Reflect.get(p,'a')"), "r"), "1");
}

/** [[HasProperty]] forwards through a million trap-less proxies. */
#[test]
fn has_follows_a_trapless_chain() {
    assert_eq!(eval(&chain("{}", "'a' in p"), "r"), "true");
}

/** [[OwnPropertyKeys]] forwards through a million trap-less proxies. */
#[test]
fn own_keys_follow_a_trapless_chain() {
    assert_eq!(eval(&chain("{}", "Reflect.ownKeys(p).join()"), "r"), "a");
}

/** An isExtensible trap whose invariant check asks the next trap is bounded natively. */
#[test]
fn trapped_is_extensible_chain_throws_a_range_error() {
    let source = chain("{isExtensible(){return true}}", "Object.isExtensible(p)");
    assert_eq!(eval(&source, "r"), "RangeError");
}
