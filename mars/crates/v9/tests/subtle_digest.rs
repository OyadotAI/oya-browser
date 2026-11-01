/** crypto.subtle.digest hashes BufferSources with SHA-1, SHA-256, SHA-384 and SHA-512, and rejects what it does not implement. */
use crate::common;

/** A script's promise result, as lowercase hex of the ArrayBuffer it settles with. */
const HEX: &str = "Array.from(new Uint8Array(b),x=>x.toString(16).padStart(2,'0')).join('')";

/** digest(SHA-256, "abc") is the FIPS 180-4 test vector, byte for byte. */
#[test]
fn sha256_digest_matches_the_standard_vector() {
    let source = format!(
        "let result='';crypto.subtle.digest('SHA-256',new TextEncoder().encode('abc')).then(b=>{{result={HEX}}});"
    );
    assert_eq!(
        common::eval(&source, "result"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

/** Algorithm names are case-insensitive and the algorithm may be an object with a name, as normalization allows. */
#[test]
fn algorithm_names_normalize_case_and_objects() {
    let source = format!(
        "let result='';crypto.subtle.digest({{name:'sha-1'}},new TextEncoder().encode('abc')).then(b=>{{result=b.byteLength+':'+{HEX}}});"
    );
    assert_eq!(
        common::eval(&source, "result"),
        "20:a9993e364706816aba3e25717850c26c9cd0d89d"
    );
}

/** The longer SHA-2 variants produce digests of their own sizes. */
#[test]
fn sha384_and_sha512_digests_have_their_sizes() {
    let source = "let result=[];const data=new Uint8Array([1,2,3]);Promise.all([crypto.subtle.digest('SHA-384',data),crypto.subtle.digest('SHA-512',data)]).then(([a,b])=>{result=[a.byteLength,b.byteLength].join(',')});";
    assert_eq!(common::eval(source, "result"), "48,64");
}

/** A digest of a view covers only the view's bytes, not the whole buffer behind it. */
#[test]
fn a_view_digests_only_its_own_bytes() {
    let source = format!(
        "let result='';const whole=new Uint8Array([9,97,98,99,9]);crypto.subtle.digest('SHA-256',whole.subarray(1,4)).then(b=>{{result={HEX}}});"
    );
    assert_eq!(
        common::eval(&source, "result"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

/** An algorithm digest does not implement rejects with NotSupportedError, as Web Crypto specifies. */
#[test]
fn unsupported_algorithms_reject_with_not_supported() {
    let source = "let result='';crypto.subtle.digest('MD5',new Uint8Array(1)).then(()=>{result='resolved'},e=>{result=e.name});";
    assert_eq!(common::eval(source, "result"), "NotSupportedError");
}

/** Data that is not a BufferSource rejects with a TypeError. */
#[test]
fn non_buffer_data_rejects_with_a_type_error() {
    let source = "let result='';crypto.subtle.digest('SHA-256','text').then(()=>{result='resolved'},e=>{result=e.name});";
    assert_eq!(common::eval(source, "result"), "TypeError");
}

/** SubtleCrypto has only digest: the other members are absent, so feature tests take their fallbacks. */
#[test]
fn only_digest_is_offered_among_subtle_members() {
    let source = "const s=crypto.subtle;const result=[typeof s.digest,typeof s.encrypt,typeof s.importKey,crypto.subtle===s].join(',');";
    assert_eq!(
        common::eval(source, "result"),
        "function,undefined,undefined,true"
    );
}
