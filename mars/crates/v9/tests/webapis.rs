/**
 * Web APIs ported from Blink's core/fileapi and core/html/forms: Blob and File construction,
 * type normalization, line endings, slicing and reads, and FormData's entry list. Each test pins
 * one rule through page script.
 */
mod common;

use common::eval;

/** Blob parts concatenate strings (UTF-8), BufferSources and Blobs in order. */
#[test]
fn blob_concatenates_parts_in_order() {
    let r = eval(
        "const inner = new Blob(['cd']);
         const b = new Blob(['ab', inner, new Uint8Array([101, 102]), new Uint8Array([103]).buffer, '\\u00e9']);
         const r = [b.size, b.type === '', String(b), b instanceof Blob, Blob.length].join('|');",
        "r",
    );
    assert_eq!(r, "9|true|[object Blob]|true|0");
}

/** Blink's NormalizeType: non-printable-ASCII types are dropped, others lowercased. */
#[test]
fn blob_type_is_normalized() {
    let r = eval(
        "const r = [new Blob([], {type: 'Text/HTML'}).type, new Blob([], {type: 'a\\u00e9'}).type,
           new Blob([], {type: 'a\\nb'}).type, new Blob(['x']).slice(0, 1, 'IMAGE/PNG').type].join('|');",
        "r",
    );
    assert_eq!(r, "text/html|||image/png");
}

/** `endings: 'native'` turns CR and CRLF into LF; a bad enum value is a TypeError. */
#[test]
fn blob_native_endings_and_bad_enum() {
    let r = eval(
        "let bad = 'none'; try { new Blob([], {endings: 'other'}); } catch (e) { bad = e.name; }
         let plain = 'none'; try { new Blob('abc'); } catch (e) { plain = e.name; }
         const r = [new Blob(['a\\r\\nb\\rc'], {endings: 'native'}).size,
           new Blob(['a\\r\\nb'], {endings: 'transparent'}).size, bad, plain].join('|');",
        "r",
    );
    assert_eq!(r, "5|4|TypeError|TypeError");
}

/** slice clamps negative and oversized offsets relative to the size. */
#[test]
fn blob_slice_clamps_offsets() {
    let r = eval(
        "const b = new Blob(['abcdef']);
         const r = [b.slice().size, b.slice(-2).size, b.slice(2, -1).size, b.slice(4, 2).size,
           b.slice(-100, 100).size].join('|');",
        "r",
    );
    assert_eq!(r, "6|2|3|0|6");
}

/** text, arrayBuffer and bytes fulfil with the content. */
#[test]
fn blob_reads_resolve_with_content() {
    let r = eval(
        "let r = 'pending'; const b = new Blob(['hi', new Uint8Array([33])]);
         Promise.all([b.text(), b.arrayBuffer(), b.bytes()]).then(([t, a, u]) => {
           r = [t, a.byteLength, a instanceof ArrayBuffer, u[2], u instanceof Uint8Array].join('|');
         });",
        "r",
    );
    assert_eq!(r, "hi!|3|true|33|true");
}

/** File inherits Blob and carries a name and an explicit or default lastModified. */
#[test]
fn file_has_name_and_last_modified() {
    let r = eval(
        "const f = new File(['abc'], 'a.txt', {type: 'text/plain', lastModified: 42});
         let short = 'none'; try { new File(['x']); } catch (e) { short = e.name; }
         const r = [f.name, f.size, f.type, f.lastModified, f instanceof Blob, String(f),
           typeof new File([], 'b').lastModified, short].join('|');",
        "r",
    );
    assert_eq!(
        r,
        "a.txt|3|text/plain|42|true|[object File]|number|TypeError"
    );
}

/** FormData keeps ordered entries; set replaces the first and drops the rest; Blobs become
Files named `blob` or the given filename. */
#[test]
fn form_data_entry_list() {
    let r = eval(
        "const fd = new FormData(); fd.append('a', '1'); fd.append('b', '2'); fd.append('a', '3');
         fd.set('a', '4'); fd.append('f', new Blob(['xy'])); fd.append('g', new Blob([]), 'n.bin');
         const names = [...fd.keys()].join(',');
         const r = [names, fd.getAll('a').join(','), fd.get('f').name, fd.get('f') instanceof File,
           fd.get('g').name, fd.has('b'), fd.get('zz'), String(fd)].join('|');
         fd.delete('b');",
        "r",
    );
    assert_eq!(r, "a,b,f,g|4|blob|true|n.bin|true||[object FormData]");
}

/** TextDecoder resolves labels through the Encoding spec's table and decodes UTF-16 and
windows-1252; an unknown label is a RangeError. */
#[test]
fn text_decoder_labels_and_legacy_encodings() {
    let r = eval(
        "let bad = 'none'; try { new TextDecoder('nope'); } catch (e) { bad = e.name; }
         const r = [new TextDecoder(' Latin1 ').encoding, new TextDecoder('utf-16').encoding,
           new TextDecoder('utf-16le').decode(new Uint8Array([0x41, 0, 0x3D, 0xD8, 0x00, 0xDE])) === 'A\\u{1F600}',
           new TextDecoder('utf-16be').decode(new Uint8Array([0, 0x42])),
           new TextDecoder('windows-1252').decode(new Uint8Array([0x80, 0xE9])) === '\\u20AC\\u00E9',
           new TextDecoder('x-user-defined').decode(new Uint8Array([0x80])) === '\\uF780', bad].join('|');",
        "r",
    );
    assert_eq!(r, "windows-1252|utf-16le|true|B|true|true|RangeError");
}

/** Legacy labels decode through their WHATWG tables, report the spec's lowercase name, and
`fatal` turns a truncated lead byte into a TypeError. */
#[test]
fn text_decoder_legacy_families_decode() {
    let r = eval(
        "let fatal = 'none'; try { new TextDecoder('shift_jis', {fatal: true}).decode(new Uint8Array([0x81])); } catch (e) { fatal = e.name; }
         const r = [new TextDecoder('sjis').decode(new Uint8Array([0x82, 0xA0])) === '\\u{3042}',
           new TextDecoder('euc-jp').decode(new Uint8Array([0xA4, 0xA2])) === '\\u{3042}',
           new TextDecoder('big5').decode(new Uint8Array([0xA4, 0x40])) === '\\u{4E00}',
           new TextDecoder('SJIS').encoding, new TextDecoder('gbk').encoding, fatal].join('|');",
        "r",
    );
    assert_eq!(r, "true|true|true|shift_jis|gbk|TypeError");
    let refused = eval(
        "let refused = 0; for (const label of ['csiso2022kr', 'hz-gb-2312', 'iso-2022-cn']) { try { new TextDecoder(label); } catch (e) { refused++; } } String(refused)",
        "refused",
    );
    assert_eq!(refused, "3");
}

/** `fatal` throws on malformed input, `ignoreBOM` keeps the BOM, and `stream` holds a split
sequence for the next call. */
#[test]
fn text_decoder_fatal_bom_and_stream() {
    let r = eval(
        "let fatal = 'none'; try { new TextDecoder('utf-8', {fatal: true}).decode(new Uint8Array([0xFF])); } catch (e) { fatal = e.name; }
         const d = new TextDecoder(); const a = d.decode(new Uint8Array([0xE2, 0x82]), {stream: true});
         const b = d.decode(new Uint8Array([0xAC]));
         const keep = new TextDecoder('utf-8', {ignoreBOM: true}).decode(new Uint8Array([0xEF, 0xBB, 0xBF, 0x41]));
         const drop = new TextDecoder().decode(new Uint8Array([0xEF, 0xBB, 0xBF, 0x41]));
         const r = [fatal, a === '', b === '\\u20AC', keep === '\\uFEFFA', drop,
           new TextDecoder().decode(new Uint8Array([0x61, 0xF0, 0x9F]))  === 'a\\uFFFD'].join('|');",
        "r",
    );
    assert_eq!(r, "TypeError|true|true|true|A|true");
}
