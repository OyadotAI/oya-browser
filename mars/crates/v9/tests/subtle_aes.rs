/** crypto.subtle AES-GCM: importKey("raw"), encrypt and decrypt answer as Chrome does, and refuse what ring cannot seal. */
use crate::common;

/** A script's promise result as lowercase hex of the ArrayBuffer it settles with. */
const HEX: &str = "Array.from(new Uint8Array(b),x=>x.toString(16).padStart(2,'0')).join('')";

/** Encrypt with the all-zero key matches NIST GCM test case 2: ciphertext then tag, byte for byte. */
#[test]
fn aes_gcm_seal_matches_the_nist_vector() {
    let source = format!(
        "let r='';crypto.subtle.importKey('raw',new Uint8Array(16),{{name:'AES-GCM'}},false,['encrypt']).then(k=>crypto.subtle.encrypt({{name:'AES-GCM',iv:new Uint8Array(12)}},k,new Uint8Array(16))).then(b=>{{r={HEX}}},e=>{{r=e.name}});"
    );
    assert_eq!(
        common::eval(&source, "r"),
        "0388dace60b6a392f328c2b971b2fe78ab6e47d42cec13bdf53a67b21257bddf"
    );
}

/** A decrypt of what encrypt sealed, under the same IV and additionalData, returns the plaintext. */
#[test]
fn aes_gcm_round_trips_with_additional_data() {
    let source = "let r='';const iv=new Uint8Array(12).fill(7),aad=new TextEncoder().encode('hdr');crypto.subtle.importKey('raw',new Uint8Array(32).fill(1),{name:'AES-GCM'},true,['encrypt','decrypt']).then(k=>crypto.subtle.encrypt({name:'AES-GCM',iv,additionalData:aad},k,new TextEncoder().encode('hello')).then(c=>crypto.subtle.decrypt({name:'AES-GCM',iv,additionalData:aad},k,c))).then(p=>{r=new TextDecoder().decode(p)},e=>{r=e.name});";
    assert_eq!(common::eval(source, "r"), "hello");
}

/** Opening a tampered ciphertext rejects with OperationError, the spec's report for failed authentication. */
#[test]
fn tampered_ciphertext_rejects_with_operation_error() {
    let source = "let r='';const iv=new Uint8Array(12);crypto.subtle.importKey('raw',new Uint8Array(16),{name:'AES-GCM'},false,['encrypt','decrypt']).then(k=>crypto.subtle.decrypt({name:'AES-GCM',iv},k,new Uint8Array(32))).then(()=>{r='resolved'},e=>{r=e.name});";
    assert_eq!(common::eval(source, "r"), "OperationError");
}

/** A key that is neither 128 nor 256 bits is a DataError, as Web Crypto names it. */
#[test]
fn wrong_key_length_rejects_with_data_error() {
    let source = "let r='';crypto.subtle.importKey('raw',new Uint8Array(24),{name:'AES-GCM'},false,['encrypt']).then(()=>{r='resolved'},e=>{r=e.name});";
    assert_eq!(common::eval(source, "r"), "DataError");
}

/** Only the raw format is implemented; another format rejects with NotSupportedError, never a fake key. */
#[test]
fn non_raw_key_format_rejects_with_not_supported() {
    let source = "let r='';crypto.subtle.importKey('jwk',{},{name:'AES-GCM'},false,['encrypt']).then(()=>{r='resolved'},e=>{r=e.name});";
    assert_eq!(common::eval(source, "r"), "NotSupportedError");
}

/** A 16-byte IV is refused with NotSupportedError, since ring seals only 96-bit IVs and Mars never truncates one. */
#[test]
fn non_96_bit_iv_rejects_with_not_supported() {
    let source = "let r='';crypto.subtle.importKey('raw',new Uint8Array(16),{name:'AES-GCM'},false,['encrypt']).then(k=>crypto.subtle.encrypt({name:'AES-GCM',iv:new Uint8Array(16)},k,new Uint8Array(1))).then(()=>{r='resolved'},e=>{r=e.name});";
    assert_eq!(common::eval(source, "r"), "NotSupportedError");
}

/** A tagLength other than 128 bits is refused with NotSupportedError. */
#[test]
fn tag_length_other_than_128_rejects_with_not_supported() {
    let source = "let r='';crypto.subtle.importKey('raw',new Uint8Array(16),{name:'AES-GCM'},false,['encrypt']).then(k=>crypto.subtle.encrypt({name:'AES-GCM',iv:new Uint8Array(12),tagLength:96},k,new Uint8Array(1))).then(()=>{r='resolved'},e=>{r=e.name});";
    assert_eq!(common::eval(source, "r"), "NotSupportedError");
}

/** Encrypt given something other than a CryptoKey rejects with TypeError. */
#[test]
fn encrypt_with_a_non_key_rejects_with_type_error() {
    let source = "let r='';crypto.subtle.encrypt({name:'AES-GCM',iv:new Uint8Array(12)},{},new Uint8Array(1)).then(()=>{r='resolved'},e=>{r=e.name});";
    assert_eq!(common::eval(source, "r"), "TypeError");
}

/** An imported key is a CryptoKey with its type, extractability and AES-GCM algorithm of 128 bits. */
#[test]
fn imported_key_reports_its_type_algorithm_and_extractability() {
    let source = "let r='';crypto.subtle.importKey('raw',new Uint8Array(16),{name:'aes-gcm'},true,['encrypt']).then(k=>{r=[k instanceof CryptoKey,k.type,k.extractable,k.algorithm.name,k.algorithm.length].join('|')});";
    assert_eq!(common::eval(source, "r"), "true|secret|true|AES-GCM|128");
}
