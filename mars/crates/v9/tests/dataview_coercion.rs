/** DataView operations take their final backing witness after all observable numeric conversions. */
use crate::common;

/** Growing a buffer from the offset conversion can make a previously invalid view accessible again. */
#[test]
fn offset_conversion_can_restore_an_out_of_bounds_view() {
    assert_eq!(
        common::eval(
            "const b=new ArrayBuffer(4,{maxByteLength:8});const v=new DataView(b,0,4);b.resize(0);const n=v.getUint8({valueOf(){b.resize(4);return 0}});const r=[n,v.byteLength].join();",
            "r"
        ),
        "0,4"
    );
}

/** A setter converts its value before taking the bounds witness and must reject newly detached storage. */
#[test]
fn setter_conversion_precedes_live_buffer_validation() {
    assert_eq!(
        common::eval(
            "const b=new ArrayBuffer(4,{maxByteLength:8});const v=new DataView(b);let converted=0,type=false;try{v.setUint32(0,{valueOf(){converted++;b.transfer();return 7}})}catch(e){type=e instanceof TypeError}const r=[converted,type,b.detached].join();",
            "r"
        ),
        "1,true,true"
    );
}

/** A large offset must still convert the value before throwing RangeError for the final access. */
#[test]
fn setter_value_conversion_precedes_range_errors() {
    assert_eq!(
        common::eval(
            "const v=new DataView(new ArrayBuffer(1));let converted=0,range=false;try{v.setUint16(10,{valueOf(){converted++;return 7}})}catch(e){range=e instanceof RangeError}const r=[converted,range].join();",
            "r"
        ),
        "1,true"
    );
}

/** Wrong receiver diagnostics identify the actual method and do not execute author conversion hooks. */
#[test]
fn receiver_brand_checks_precede_argument_coercion() {
    assert_eq!(
        common::eval(
            "let calls=0,type=false;const receiver={toString(){calls++;throw 7}};const offset={valueOf(){calls++;throw 8}};try{DataView.prototype.getInt32.call(receiver,offset)}catch(e){type=e instanceof TypeError}const r=[calls,type].join();",
            "r"
        ),
        "0,true"
    );
}
