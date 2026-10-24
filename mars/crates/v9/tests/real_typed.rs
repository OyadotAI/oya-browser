/** Typed arrays, DataView, TextEncoder/TextDecoder and getRandomValues as page bundles use them. */
mod common;
use common::{eval, failure, load};

/** Views share their buffer's bytes, convert on write and iterate like arrays. */
#[test]
fn views_share_bytes_and_convert_on_write() {
    let source = "const b=new ArrayBuffer(8),u8=new Uint8Array(b),u32=new Uint32Array(b);u32[0]=0x01020304;u8[4]=300;\
        const c=new Uint8ClampedArray([300,-5,1.5,2.5]),f=Float64Array.from([1,2,3],x=>x*2),sub=u8.subarray(1,3);sub[0]=9;\
        const r=[u8[0],u8[1],u8[4],u8.length,b.byteLength,[...c].join('|'),f.join('|'),u8[1],Array.from(sub).join('|'),\
        u8 instanceof Uint8Array,ArrayBuffer.isView(u8),Object.prototype.toString.call(u8),new Int8Array([255])[0],\
        new Uint8Array([3,1,2]).sort().join(''),new Uint16Array(2).fill(7).map(x=>x+1).join(),u8[99]].join();";
    assert_eq!(
        eval(source, "r"),
        "4,9,44,8,8,255|0|2|2,2|4|6,9,9|2,true,true,[object Uint8Array],-1,123,8,8,"
    );
}

/** DataView reads and writes either endianness, including 64-bit BigInts. */
#[test]
fn data_view_reads_both_endiannesses() {
    let source = "const v=new DataView(new ArrayBuffer(16));v.setUint16(0,0x1234);v.setUint16(2,0x1234,true);v.setBigUint64(8,2n**63n);\
        const r=[v.getUint8(0),v.getUint8(2),v.getUint16(0),v.getBigUint64(8),new BigInt64Array(v.buffer,8,1)[0]].join();";
    assert_eq!(eval(source, "r"), "18,52,4660,9223372036854775808,128");
}

/** UTF-8 round trips through TextEncoder and TextDecoder. */
#[test]
fn text_encoding_round_trips_utf8() {
    let source = "const e=new TextEncoder().encode('h\u{e9}\u{1f600}'),d=new TextDecoder().decode(e);const r=[e.length,d.length,d==='h\u{e9}\u{1f600}'].join();";
    assert_eq!(eval(source, "r"), "7,4,true");
}

/** getRandomValues fills an integer view and returns it; floats are refused. */
#[test]
fn get_random_values_fills_integer_views() {
    let source = "const a=new Uint32Array(8);const same=crypto.getRandomValues(a)===a;let e='';try{crypto.getRandomValues(new Float64Array(1))}catch(x){e=x.name}const r=[same,a.some(x=>x!==0),e].join();";
    assert_eq!(eval(source, "r"), "true,true,TypeError");
}

/** Buffers and views survive the state wire with their bytes. */
#[test]
fn buffers_survive_the_state_wire() {
    let execution = load("var view=new Uint8Array([1,2,250]);");
    assert!(failure(&execution).is_none());
    let wire = serde_json::to_string(&execution.state).unwrap();
    assert!(wire.contains("0102fa"), "{wire}");
    let state: mars_v9::VmState = serde_json::from_str(&wire).unwrap();
    assert_eq!(serde_json::to_string(&state).unwrap(), wire);
}

/** Closure's bidi patterns put lone surrogates in classes built from strings (Google's xjs). */
#[test]
fn lone_surrogates_in_string_built_classes_compile() {
    let source = "const re=RegExp('[\\u0591-\\u06ef\\ud802-\\ud803\\ud83a-\\ud83b]');const r=[re.test('\\u05d0'),re.test('a'),RegExp('\\ud83d').test('\\ud83d')].join();";
    assert_eq!(eval(source, "r"), "true,false,true");
}
