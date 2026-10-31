/** CallSite.getTypeName reports the receiver's kind: primitives by wrapper name, proxies as Proxy, and objects by the constructor their prototype names. */
use crate::common;

/** Strict callers keep a primitive receiver unboxed, so its wrapper name is what the call site reports. */
#[test]
fn primitive_receivers_report_their_wrapper_names() {
    assert_eq!(
        common::eval(
            "Error.prepareStackTrace=function(e,s){return s};function probe(){'use strict';return new Error().stack}const r=[probe.call(1)[0].getTypeName(),probe.call('')[0].getTypeName(),probe.call(false)[0].getTypeName()].join();",
            "r"
        ),
        "Number,String,Boolean"
    );
}

/** A proxy receiver reports Proxy, whatever its target is. */
#[test]
fn proxy_receiver_reports_proxy() {
    assert_eq!(
        common::eval(
            "Error.prepareStackTrace=function(e,s){return s};function probe(){'use strict';return new Error().stack}const r=probe.call(new Proxy({},{}))[0].getTypeName();",
            "r"
        ),
        "Proxy"
    );
}

/** An instance reports the name of its constructor, read from the prototype without running code. */
#[test]
fn instance_receiver_reports_constructor_name() {
    assert_eq!(
        common::eval(
            "Error.prepareStackTrace=function(e,s){return s};function Custom(){}function probe(){'use strict';return new Error().stack}const r=[probe.call(new Custom())[0].getTypeName(),probe.call({})[0].getTypeName()].join();",
            "r"
        ),
        "Custom,Object"
    );
}
