/** Binary16 typed storage, DataView and Math share one exact round-to-even conversion. */
mod common;

/** Half elements share two-byte storage and preserve negative zero, infinities and subnormals. */
#[test]
fn half_elements_share_real_buffer_storage() {
    assert_eq!(
        common::eval(
            "const a=new Float16Array([Math.PI,-0,2**-24,Infinity,NaN]);const words=new Uint16Array(a.buffer);const r=[a.BYTES_PER_ELEMENT,a.byteLength,a[0],Object.is(a[1],-0),a[2],words[0],words[1],Number.isNaN(a[4])].join();",
            "r"
        ),
        "2,10,3.140625,true,5.960464477539063e-8,16968,32768,true"
    );
}

/** Rounding halfway values directly from doubles avoids an intermediate binary32 tie. */
#[test]
fn half_rounding_is_direct_and_ties_to_even() {
    assert_eq!(
        common::eval(
            "const half=1+2**-11;const r=[Math.f16round(half),Math.f16round(half+2**-25),Math.f16round(half-2**-25),Math.f16round(65520),Math.f16round(2**-25),Object.is(Math.f16round(-(2**-26)),-0)].join();",
            "r"
        ),
        "1,1.0009765625,1,Infinity,0,true"
    );
}

/** DataView accepts either byte order and interoperates with typed half arrays. */
#[test]
fn data_view_half_accessors_observe_endianness() {
    assert_eq!(
        common::eval(
            "const buffer=new ArrayBuffer(4);const view=new DataView(buffer);view.setFloat16(0,1.5,true);view.setFloat16(2,-2,false);const r=[view.getFloat16(0,true),view.getFloat16(2,false),Array.from(new Uint8Array(buffer)).join()].join('|');",
            "r"
        ),
        "1.5|-2|0,62,192,0"
    );
}
