/** Uint8Array codecs preserve exact bytes, option order and observable writes before syntax failures. */
use crate::common;

/** Encoding and decoding use distinct standard and URL alphabets and precise optional padding. */
#[test]
fn alphabets_padding_and_hex_round_trip() {
    assert_eq!(
        common::eval(
            "const a=new Uint8Array([0,255,239,1]);const encoded=a.toBase64();const url=a.toBase64({alphabet:'base64url',omitPadding:true});const r=[encoded,url,Uint8Array.fromBase64(encoded).toHex(),Uint8Array.fromBase64(url,{alphabet:'base64url'}).toHex(),Uint8Array.fromHex('00ffEF01').toHex()].join('|');",
            "r"
        ),
        "AP/vAQ==|AP_vAQ|00ffef01|00ffef01|00ffef01"
    );
}

/** Setters leave an unfittable quantum unread and report source characters including whitespace. */
#[test]
fn bounded_decoding_reports_exact_read_and_written_counts() {
    assert_eq!(
        common::eval(
            "const a=new Uint8Array(5);const x=a.setFromBase64('Zm9vYmFy');const b=new Uint8Array(3);const y=b.setFromBase64('Z g==');const c=new Uint8Array(1);const z=c.setFromHex('aabbzz');const r=[x.read,x.written,a.toHex(),y.read,y.written,b[0],z.read,z.written,c[0]].join();",
            "r"
        ),
        "4,3,666f6f0000,5,1,102,2,1,170"
    );
}

/** Syntax failures expose preceding complete chunks, but never partially commit a bad final chunk. */
#[test]
fn invalid_input_commits_only_preceding_valid_chunks() {
    assert_eq!(
        common::eval(
            "const a=new Uint8Array([255,255,255,255,255]);let b64=false;try{a.setFromBase64('MjYyZg===')}catch(e){b64=e instanceof SyntaxError}const b=new Uint8Array([255,255,255]);let hex=false;try{b.setFromHex('01ffgg')}catch(e){hex=e instanceof SyntaxError}const r=[b64,a.toHex(),hex,b.toHex()].join('|');",
            "r"
        ),
        "true|323632ffff|true|01ffff"
    );
}

/** Strict mode validates overflow bits and padding; stop mode leaves an incomplete tail untouched. */
#[test]
fn final_chunk_modes_preserve_unused_bit_rules() {
    assert_eq!(
        common::eval(
            "let strict=false;try{Uint8Array.fromBase64('Zh==',{lastChunkHandling:'strict'})}catch(e){strict=e instanceof SyntaxError}const a=new Uint8Array(5);const x=a.setFromBase64('Zm9vYmE',{lastChunkHandling:'stop-before-partial'});const r=[strict,Uint8Array.fromBase64('Zh==').toHex(),x.read,x.written,a.toHex()].join('|');",
            "r"
        ),
        "true|66|4|3|666f6f0000"
    );
}

/** Only Uint8Array receivers and primitive strings are accepted without coercing either argument. */
#[test]
fn brands_and_strings_are_checked_without_coercion() {
    assert_eq!(
        common::eval(
            "let n=0,errors=0;const s={toString(){n++;return 'ff'}};for(const f of [()=>Uint8Array.fromHex(s),()=>Uint8Array.prototype.toHex.call(new Uint8ClampedArray(1)),()=>Uint8Array.fromBase64('Zg==',null)])try{f()}catch(e){if(e instanceof TypeError)errors++}const z=new Uint8Array(0).setFromBase64('#');const r=[errors,n,z.read,z.written].join();",
            "r"
        ),
        "3,0,0,0"
    );
}

/** An options getter may resize the buffer, so setFromBase64 must use the updated live view bounds. */
#[test]
fn options_getters_update_the_destination_witness() {
    assert_eq!(
        common::eval(
            "const b=new ArrayBuffer(3,{maxByteLength:6});const a=new Uint8Array(b);const log=[];const x=a.setFromBase64('Zm9vYmFy',{get alphabet(){log.push('alphabet');b.resize(6);return 'base64'},get lastChunkHandling(){log.push('tail');return 'strict'}});const r=[x.read,x.written,a.toHex(),log.join()].join('|');",
            "r"
        ),
        "8|6|666f6f626172|alphabet,tail"
    );
}
