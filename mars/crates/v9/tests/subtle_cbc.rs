/** AES-CBC for crypto.subtle: NIST SP 800-38A vectors for encryption at 128 and 256 bits, PKCS#7 round
trips, the error names Web Crypto gives an IV, key or padding failure, and the key/algorithm match. */
use crate::common;

/** The JavaScript helpers every vector test shares: hex in and out, and the NIST plaintext and IV. */
const HELPERS: &str = "const u=h=>Uint8Array.from(h.match(/../g).map(x=>parseInt(x,16)));\
const hex=b=>Array.from(new Uint8Array(b)).map(x=>x.toString(16).padStart(2,'0')).join('');\
const plain='6bc1bee22e409f96e93d7e117393172aae2d8a571e03ac9c9eb76fac45af8e5130c81c46a35ce411e5fbc1191a0a52eff69f2445df4f9b17ad2b417be66c3710';\
const iv='000102030405060708090a0b0c0d0e0f';";

/** NIST SP 800-38A F.2.1 (CBC-AES128.Encrypt): the four ciphertext blocks, before the padding block. */
const F21_CIPHERTEXT: &str = "7649abac8119b246cee98e9b12e9197d5086cb9b507219ee95db113a917678b273bed6b8e3c1743b7116e69e22229516\
3ff1caa1681fac09120eca307586e1a7";

/** NIST SP 800-38A F.2.5 (CBC-AES256.Encrypt): the four ciphertext blocks, before the padding block. */
const F25_CIPHERTEXT: &str = "f58c4c04d6e5f1ba779eabfb5f7bfbd69cfc4e967edb808d679f777bc6702c7d39f23369a9d9bacfa530e26304231461b2eb05e2c39be9fcda6c19078c6a9d1b";

/** AES-128-CBC over the NIST plaintext: the output starts with F.2.1 and ends with one padding block. */
#[test]
fn aes_cbc_128_encrypts_the_nist_sp800_38a_vectors_then_a_padding_block() {
    let source = format!(
        "{HELPERS}let r='';crypto.subtle.importKey('raw',u('2b7e151628aed2a6abf7158809cf4f3c'),{{name:'AES-CBC'}},false,['encrypt']).then(k=>crypto.subtle.encrypt({{name:'AES-CBC',iv:u(iv)}},k,u(plain))).then(c=>{{const h=hex(c);r=(h.startsWith('{F21_CIPHERTEXT}')&&h.length===160)?'ok':h}},e=>{{r=e.name}});"
    );
    assert_eq!(common::eval(&source, "r"), "ok");
}

/** AES-256-CBC over the NIST plaintext: the output starts with F.2.5 and ends with one padding block. */
#[test]
fn aes_cbc_256_encrypts_the_nist_sp800_38a_vectors_then_a_padding_block() {
    let source = format!(
        "{HELPERS}let r='';crypto.subtle.importKey('raw',u('603deb1015ca71be2b73aef0857d77811f352c073b6108d72d9810a30914dff4'),{{name:'AES-CBC'}},false,['encrypt']).then(k=>crypto.subtle.encrypt({{name:'AES-CBC',iv:u(iv)}},k,u(plain))).then(c=>{{const h=hex(c);r=(h.startsWith('{F25_CIPHERTEXT}')&&h.length===160)?'ok':h}},e=>{{r=e.name}});"
    );
    assert_eq!(common::eval(&source, "r"), "ok");
}

/** Sealing pads to whole blocks and opening strips the padding: five bytes and sixteen bytes both round trip. */
#[test]
fn aes_cbc_round_trips_with_pkcs7_padding() {
    let source: String = [HELPERS, "let r='';const key=u('000102030405060708090a0b0c0d0e0f');const zero=new Uint8Array(16);\
        const text=new TextEncoder();crypto.subtle.importKey('raw',key,{name:'AES-CBC'},true,['encrypt','decrypt']).then(k=>Promise.all([\
        crypto.subtle.encrypt({name:'AES-CBC',iv:zero},k,text.encode('hello')),crypto.subtle.encrypt({name:'AES-CBC',iv:zero},k,text.encode('0123456789abcdef'))])\
        .then(([a,b])=>Promise.all([crypto.subtle.decrypt({name:'AES-CBC',iv:zero},k,a),crypto.subtle.decrypt({name:'AES-CBC',iv:zero},k,b),a.byteLength,b.byteLength]))\
        .then(([p,q,la,lb])=>{r=new TextDecoder().decode(p)+'|'+new TextDecoder().decode(q)+'|'+la+'|'+lb})).catch(e=>{r='error '+e.name});"].concat();
    assert_eq!(common::eval(&source, "r"), "hello|0123456789abcdef|16|32");
}

/** Opening under the wrong key fails the padding check: OperationError, the one name every padding failure gets. */
#[test]
fn bad_padding_rejects_with_operation_error() {
    let source: String = [HELPERS, "let r='';const zero=new Uint8Array(16);const good=u('000102030405060708090a0b0c0d0e0f');const bad=u('ffeeddccbbaa99887766554433221100');\
        crypto.subtle.importKey('raw',good,{name:'AES-CBC'},false,['encrypt']).then(k=>crypto.subtle.encrypt({name:'AES-CBC',iv:zero},k,new Uint8Array(20))).then(c=>\
        crypto.subtle.importKey('raw',bad,{name:'AES-CBC'},false,['decrypt']).then(k2=>crypto.subtle.decrypt({name:'AES-CBC',iv:zero},k2,c))).then(()=>{r='resolved'},e=>{r=e.name});"].concat();
    assert_eq!(common::eval(&source, "r"), "OperationError");
}

/** A ciphertext that is not whole blocks, or is empty, cannot open: OperationError. */
#[test]
fn ciphertext_that_is_not_whole_blocks_rejects_with_operation_error() {
    let source = "let r='';const iv=new Uint8Array(16);crypto.subtle.importKey('raw',new Uint8Array(16),{name:'AES-CBC'},false,['decrypt']).then(k=>crypto.subtle.decrypt({name:'AES-CBC',iv},k,new Uint8Array(17))).then(()=>{r='resolved'},e=>{r=e.name});";
    assert_eq!(common::eval(source, "r"), "OperationError");
}

/** An IV that is not one block long is refused with OperationError, as Web Crypto names it. */
#[test]
fn wrong_iv_length_rejects_with_operation_error() {
    let source = "let r='';crypto.subtle.importKey('raw',new Uint8Array(16),{name:'AES-CBC'},false,['encrypt']).then(k=>crypto.subtle.encrypt({name:'AES-CBC',iv:new Uint8Array(12)},k,new Uint8Array(4))).then(()=>{r='resolved'},e=>{r=e.name});";
    assert_eq!(common::eval(source, "r"), "OperationError");
}

/** A key of 128, 192 or 256 bits imports; any other length is a DataError. */
#[test]
fn aes_cbc_key_lengths_are_128_192_or_256_bits() {
    let source = "let r='';const import_=n=>crypto.subtle.importKey('raw',new Uint8Array(n),{name:'AES-CBC'},false,['encrypt']).then(()=>'ok',e=>e.name);\
        Promise.all([16,24,32].map(import_)).then(a=>Promise.all([20].map(import_)).then(b=>{r=a.join()+'|'+b.join()}));";
    assert_eq!(common::eval(source, "r"), "ok,ok,ok|DataError");
}

/** An AES-CBC key used for AES-GCM (or the reverse) is InvalidAccessError, as the spec requires. */
#[test]
fn cbc_key_for_gcm_is_invalid_access() {
    let source = "let r='';crypto.subtle.importKey('raw',new Uint8Array(16),{name:'AES-CBC'},false,['encrypt']).then(k=>crypto.subtle.encrypt({name:'AES-GCM',iv:new Uint8Array(12)},k,new Uint8Array(4))).then(()=>{r='resolved'},e=>{r=e.name});";
    assert_eq!(common::eval(source, "r"), "InvalidAccessError");
}
