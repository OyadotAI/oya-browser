/** crypto.subtle is a real SubtleCrypto instance with every Chrome member, and the random helpers behave. */
use crate::common;

/** The instance is a SubtleCrypto, tagged as one, and the getter returns the same object each time. */
#[test]
fn subtle_is_a_subtlecrypto_instance_with_its_tag() {
    assert_eq!(
        common::eval(
            "const s=crypto.subtle;const r=[s instanceof SubtleCrypto,Object.prototype.toString.call(s),Object.getPrototypeOf(s)===SubtleCrypto.prototype,crypto.subtle===s].join('|');",
            "r"
        ),
        "true|[object SubtleCrypto]|true|true"
    );
}

/** SubtleCrypto is an illegal constructor, with or without new. */
#[test]
fn subtlecrypto_cannot_be_constructed() {
    assert_eq!(
        common::eval(
            "let e=[];try{new SubtleCrypto()}catch(x){e.push(x.name)}try{SubtleCrypto()}catch(x){e.push(x.name)}const r=e.join(',');",
            "r"
        ),
        "TypeError,TypeError"
    );
}

/** Every member has its WebIDL name and length, so feature tests that read them see Chrome's shape. */
#[test]
fn members_carry_their_webidl_names_and_lengths() {
    assert_eq!(
        common::eval(
            "const p=SubtleCrypto.prototype;const r=[p.importKey.length,p.importKey.name,p.unwrapKey.length,p.digest.length,p.decrypt.length,p.wrapKey.length].join(',');",
            "r"
        ),
        "5,importKey,7,2,3,4"
    );
}

/** Members Mars does not implement reject with NotSupportedError rather than throwing or resolving. */
#[test]
fn unimplemented_members_reject_with_not_supported() {
    assert_eq!(
        common::eval(
            "let r='';crypto.subtle.sign({name:'HMAC'},null,new Uint8Array(1)).then(()=>{r='resolved'},e=>{r=e.name});",
            "r"
        ),
        "NotSupportedError"
    );
}

/** getRandomValues fills and returns the same view; randomUUID is a version-4 UUID. */
#[test]
fn random_helpers_fill_and_format_as_specified() {
    assert_eq!(
        common::eval(
            "const v=new Uint8Array(16);const same=crypto.getRandomValues(v)===v;const id=crypto.randomUUID();const r=[same,v.length,/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(id)].join('|');",
            "r"
        ),
        "true|16|true"
    );
}
