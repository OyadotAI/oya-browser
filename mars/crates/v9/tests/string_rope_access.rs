/** Indexed and iterated UTF-16 access preserves rope contents without forcing flattening. */
use crate::common;

/** Repeated short-rope searches retain their positions after the search materializes the rope. */
#[test]
fn short_rope_searches_reuse_contiguous_units() {
    let source = "const s='alpha '.repeat(200)+'omega';let ok=true;for(let i=0;i<200;i++){const part=s.substring(i*6,i*6+5);ok=ok&&s.indexOf(part,i*6)===i*6&&s.includes(part,i*6)}const r=ok+','+s.indexOf('omega');";
    assert_eq!(common::eval(source, "r"), "true,1200");
}

/** A surrogate pair split across rope nodes remains one code point while indexed reads stay code units. */
#[test]
fn string_rope_reads_surrogate_pairs_across_nodes() {
    let source = "const s='x'.repeat(1100)+'\\ud83d'+'\\ude00';const tail=[...s].slice(-1)[0];const r=[s.at(-1).charCodeAt(0),s.charCodeAt(1100),s.codePointAt(1100),s[1101].charCodeAt(0),tail.length,tail.codePointAt(0),s.isWellFormed(),(s+'\\ud800').isWellFormed(),(s+'\\ud800').toWellFormed().endsWith('\\ufffd')].join();";
    assert_eq!(
        common::eval(source, "r"),
        "56832,55357,128512,56832,2,128512,true,false,true"
    );
}

/** Repeated ropes preserve indexed contents, concatenation, iteration and UTF-16 surrogate boundaries. */
#[test]
fn repeated_strings_preserve_contents_across_access_paths() {
    let source = "const s='a\\ud83d\\ude00'.repeat(2000),t=s+'!';const r=[s.length,s.charAt(0),s.codePointAt(1),s.at(-1).charCodeAt(0),t.endsWith('!'),[...s].length].join();";
    assert_eq!(common::eval(source, "r"), "6000,a,128512,56832,true,4000");
}

/** A repeated string crosses JSON state transport as a compact validated repeat descriptor. */
#[test]
fn repeated_strings_stay_compact_across_state_transport() {
    let sources = vec!["const s='ab'.repeat(100000);if(s.slice(2,4)!=='ab'||s.substring(3,5)!=='ba'||s.substr(4,2)!=='ab'||!s.startsWith('ab')||!s.endsWith('ab')||!s.includes('ba',4)||s.indexOf('ba',4)!==5||s.lastIndexOf('ba',10)!==9||s.indexOf('')!==0||s.lastIndexOf('')!==s.length||!s.isWellFormed()||s.toWellFormed().length!==s.length||s.trim().length!==s.length)throw Error('string search');document.getElementById('go').addEventListener('click',()=>console.log(s.length,s.charAt(199999)));".into()];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let encoded = serde_json::to_string(&loaded.state).unwrap();
    assert!(
        encoded.len() < common::empty_state_bytes() + 100_000,
        "repeat flattened into snapshot"
    );
    let restored = serde_json::from_str(&encoded).unwrap();
    let clicked =
        mars_v9::click_page(common::input(&sources, &host, &environment), restored, 3).unwrap();
    assert!(clicked.failed.is_empty(), "{:?}", common::failure(&clicked));
    assert_eq!(common::console(&clicked), ["200000 b"]);
}

/** Padded strings preserve compact repeated and non-Latin-1 children through the state wire. */
#[test]
fn padded_rope_stays_compact_across_state_transport() {
    let sources = vec!["const s='\\u03a9'.padStart(200000,'ab'),t=s.slice(10001);document.getElementById('go').addEventListener('click',()=>console.log(s.length,s.at(-1),s.slice(0,3),t.length,t.at(-1)));".into()];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let encoded = serde_json::to_string(&loaded.state).unwrap();
    assert!(
        encoded.len() < common::empty_state_bytes() + 100_000,
        "padding flattened into snapshot"
    );
    let clicked = mars_v9::click_page(
        common::input(&sources, &host, &environment),
        serde_json::from_str(&encoded).unwrap(),
        3,
    )
    .unwrap();
    assert!(clicked.failed.is_empty(), "{:?}", common::failure(&clicked));
    assert_eq!(common::console(&clicked), ["200000 Ω aba 189999 Ω"]);
}

/** Content equality and ordering do not flatten equivalent differently shaped ropes. */
#[test]
fn comparing_compact_strings_preserves_their_storage() {
    let sources = vec!["const a='ab'.repeat(100000),b='abab'.repeat(50000);if(a!==b||a<b||a>b||!(a+'c'>b)||[a+'c',b].sort()[0]!==a||a.localeCompare(b)!==0)throw Error('compare');document.getElementById('go').addEventListener('click',()=>console.log(a.length,b.length));".into()];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let encoded = serde_json::to_string(&loaded.state).unwrap();
    assert!(
        encoded.len() < common::empty_state_bytes() + 100_000,
        "comparison flattened compact strings"
    );
}

/** Repeated substrings retain compact backing across aligned and unaligned boundaries. */
#[test]
fn repeated_substrings_stay_compact() {
    let sources = vec!["const s='ab'.repeat(100000),a=s.slice(10001,190001),b=s.substring(20000,180000),c=s.substr(30000,140000),pieces=(s+'z').split('z'),unchanged=s.replace('z','x');if(!s.replace('ab','xy').startsWith('xy'))throw Error('replace');document.getElementById('go').addEventListener('click',()=>console.log(a.length,b.length,c.length,a.startsWith('ba'),a.endsWith('ba'),pieces[0].length,pieces[1].length,unchanged.length));".into()];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let encoded = serde_json::to_string(&loaded.state).unwrap();
    assert!(
        encoded.len() < common::empty_state_bytes() + 100_000,
        "repeated substring flattened: {}",
        encoded.len()
    );
    let clicked = mars_v9::click_page(
        common::input(&sources, &host, &environment),
        serde_json::from_str(&encoded).unwrap(),
        3,
    )
    .unwrap();
    assert_eq!(
        common::console(&clicked),
        ["180000 160000 140000 true true 200000 0 200000"]
    );
}

/** Forged repeat descriptors cannot bypass the existing UTF-16 string length bound. */
#[test]
fn forged_repeat_snapshot_cannot_exceed_the_string_limit() {
    let text = format!(
        "{{\"r\":[\"a\",{}]}}",
        mars_v9::constants::JS_STRING_UNITS + 1
    );
    assert!(serde_json::from_str::<mars_v9::JsString>(&text).is_err());
}

/** A compact concatenation cannot combine individually valid strings past the limit. */
#[test]
fn forged_concat_snapshot_cannot_exceed_the_string_limit() {
    let text = format!(
        "{{\"c\":[{{\"r\":[\"a\",{}]}},\"b\"]}}",
        mars_v9::constants::JS_STRING_UNITS
    );
    assert!(serde_json::from_str::<mars_v9::JsString>(&text).is_err());
}

/** Forged retained-slice offsets and lengths cannot escape the parent's actual range. */
#[cfg(feature = "conformance")]
#[test]
fn forged_slice_snapshot_cannot_escape_parent() {
    for wire in [
        "{\"v\":[\"abcdefghijklmnop\",5,20]}",
        "{\"v\":[\"abcdefghijklmnop\",18446744073709551615,20]}",
    ] {
        assert!(serde_json::from_str::<mars_v9::JsString>(wire).is_err());
    }
    assert!(serde_json::from_str::<mars_v9::JsString>("{\"R\":1}").is_err());
}

/** A compact malicious JSON array cannot expand the temporary snapshot tree without bound. */
#[cfg(feature = "conformance")]
#[test]
fn snapshot_decode_rejects_excessive_buffered_entries() {
    let count =
        mars_v9::constants::VM_STATE_BYTES / (2 * std::mem::size_of::<serde_json::Value>()) + 1;
    let wire = format!(
        "{{\"heap\":[{}null],\"$slice_parents\":[]}}",
        "null,".repeat(count)
    );
    let error = serde_json::from_str::<mars_v9::VmState>(&wire).unwrap_err();
    assert!(error.to_string().contains("snapshot decoding buffer limit"));
}

/** Many retained substrings keep their shared parent across page state transport. */
#[cfg(feature = "conformance")]
#[test]
fn many_substrings_share_parent_across_state_transport() {
    let environment = common::environment();
    let source = "let s='0123456789abcdef';for(let i=0;i<8;i++)s+=s;const slices=[];for(let i=0;i<1000;i++)slices.push(s.substring(i%100));document.getElementById('go').addEventListener('click',()=>console.log(slices.length,slices[37].length));";
    let sources = vec![source.into()];
    let host = common::host();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let encoded = serde_json::to_string(&loaded.state).unwrap();
    assert!(
        encoded.len() < 1_000_000,
        "slice snapshot exceeded its bounded transport: {}",
        encoded.len()
    );
    let reordered: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    let round_trip: mars_v9::VmState = serde_json::from_value(reordered.clone()).unwrap();
    assert_eq!(serde_json::to_string(&round_trip).unwrap(), encoded);
    let mut fields = reordered.as_object().unwrap().clone();
    let parents = fields.remove("$slice_parents").unwrap();
    let mut moved = serde_json::to_string(&fields).unwrap();
    moved.pop();
    moved.push_str(&format!(",\"$slice_parents\":{parents}}}"));
    let _: mars_v9::VmState = serde_json::from_str(&moved).unwrap();
    let mut missing = reordered.clone();
    missing["$slice_parents"] = serde_json::json!([]);
    assert!(serde_json::from_value::<mars_v9::VmState>(missing).is_err());
    let mut duplicate = reordered;
    let parents = duplicate["$slice_parents"].as_array_mut().unwrap();
    parents.push(parents[0].clone());
    assert!(serde_json::from_value::<mars_v9::VmState>(duplicate).is_err());
    let clicked = mars_v9::click_page(
        common::input(&sources, &host, &environment),
        serde_json::from_str(&encoded).unwrap(),
        3,
    )
    .unwrap();
    assert_eq!(common::console(&clicked), ["1000 4059"]);
}

/** Allocation accounting observes alias sharing and newly materialized backing. */
#[test]
fn resident_accounting_tracks_materialization_and_aliases() {
    let text: mars_v9::JsString = serde_json::from_str("{\"r\":[\"ab\",100000]}").unwrap();
    let alias = text.clone();
    let compact = text.resident_units();
    assert!(compact < 1000);
    assert_eq!(alias.resident_units(), compact);
    assert_eq!(
        text.code_units().take(4).collect::<Vec<_>>(),
        "abab".encode_utf16().collect::<Vec<_>>()
    );
    assert_eq!(text.resident_units(), compact);
    assert!(text.is(&"ab".repeat(100000)));
    assert_eq!(text.resident_units(), compact);
    assert_eq!(text.to_dom().unwrap().len(), 200000);
    assert_eq!(text.resident_units(), compact);
    assert_eq!(text.lossy().len(), 200000);
    assert_eq!(text.resident_units(), compact);
    assert_eq!(text.try_as_units().unwrap().len(), 200000);
    assert!(text.resident_units() >= 200000);
    assert_eq!(alias.resident_units(), text.resident_units());
    let restored: mars_v9::JsString =
        serde_json::from_str(&serde_json::to_string(&text).unwrap()).unwrap();
    assert_eq!(
        restored.try_as_units().unwrap(),
        text.try_as_units().unwrap()
    );
}

/** Hash lookups match flat and compact strings without expanding the compact backing. */
#[test]
fn hash_lookup_preserves_compact_backing() {
    let compact: mars_v9::JsString = serde_json::from_str("{\"r\":[\"ab\",10000]}").unwrap();
    let flat = mars_v9::JsString::from_utf8(&"ab".repeat(10000)).unwrap();
    let before = compact.resident_units();
    // Keys compare by value and nothing writes them while the set lives.
    #[allow(clippy::mutable_key_type)]
    let mut keys = std::collections::HashSet::new();
    keys.insert(compact.clone());
    assert!(keys.contains(&flat));
    assert_eq!(compact.resident_units(), before);
}

/** Public primitive-number conversion reads compact input without changing its backing. */
#[test]
fn public_numeric_conversion_preserves_compact_input() {
    let text: mars_v9::JsString = serde_json::from_str("{\"r\":[\"0\",100000]}").unwrap();
    let before = text.resident_units();
    assert_eq!(mars_v9::JsValue::String(text.clone()).to_number(), 0.0);
    assert_eq!(text.resident_units(), before);
}

/** Public literal splitting retains UTF-16 content and does not flatten its input. */
#[test]
fn public_literal_split_preserves_compact_input() {
    let text: mars_v9::JsString = serde_json::from_str("{\"r\":[\"ab\",100000]}").unwrap();
    let parts = text
        .split_literal(Some(&mars_v9::JsString::from_utf8("z").unwrap()), 2)
        .unwrap();
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].len(), 200_000);
    assert!(serde_json::to_string(&text).unwrap().len() < 100);
    let overlapping = mars_v9::JsString::from_utf8("aabaabaa").unwrap();
    let separator = mars_v9::JsString::from_utf8("aabaa").unwrap();
    let parts = overlapping.split_literal(Some(&separator), 2).unwrap();
    assert_eq!(parts.len(), 2);
    assert!(parts[0].is_empty());
    assert_eq!(
        parts[1].as_units(),
        "baa".encode_utf16().collect::<Vec<_>>()
    );
}

/** Repairing lone surrogates does not flatten the original repeated string. */
#[test]
fn well_formed_repair_preserves_original_storage() {
    let sources = vec!["const s='\\ud800'.repeat(100000);if(s.isWellFormed()||s.toWellFormed().length!==s.length)throw Error('repair');document.getElementById('go').addEventListener('click',()=>console.log(s.charCodeAt(0)));".into()];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    assert!(
        serde_json::to_string(&loaded.state).unwrap().len() < common::empty_state_bytes() + 100_000
    );
}

/** Unicode casing reads repeated content without flattening the original string. */
#[test]
fn case_mapping_preserves_original_storage() {
    let sources = vec!["const s='ä'.repeat(100000);if(s.toUpperCase().charAt(0)!=='Ä')throw Error('case');document.getElementById('go').addEventListener('click',()=>console.log(s.length));".into()];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    assert!(
        serde_json::to_string(&loaded.state).unwrap().len() < common::empty_state_bytes() + 100_000
    );
}

/** Normalization preserves a repeated input backing while producing independent output. */
#[test]
fn normalization_preserves_original_storage() {
    let sources = vec!["const s='é'.repeat(20000);if(s.normalize('NFD').length!==40000)throw Error('normalize');document.getElementById('go').addEventListener('click',()=>console.log(s.length));".into()];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    assert!(
        serde_json::to_string(&loaded.state).unwrap().len() < common::empty_state_bytes() + 30_000
    );
}

/** Legacy HTML formatting keeps the source string compact while building the result. */
#[test]
fn html_formatting_preserves_original_storage() {
    let sources = vec!["const s='ab'.repeat(100000);if(s.bold().length!==s.length+7)throw Error('html');document.getElementById('go').addEventListener('click',()=>console.log(s.length));".into()];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    assert!(
        serde_json::to_string(&loaded.state).unwrap().len() < common::empty_state_bytes() + 100_000
    );
}

/** String.raw keeps repeated raw segments compact after all observable coercions. */
#[test]
fn raw_template_preserves_compact_segments() {
    let sources = vec!["const s='ab'.repeat(100000),r=String.raw({raw:[s]});if(r!==s)throw Error('raw');document.getElementById('go').addEventListener('click',()=>console.log(r.length));".into()];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    assert!(
        serde_json::to_string(&loaded.state).unwrap().len() < common::empty_state_bytes() + 100_000
    );
}

/** Raw-template overflow still observes later getters before reporting a limit. */
#[test]
fn raw_template_overflow_preserves_getter_order() {
    let source = format!(
        "const token={{}},raw={{length:3,0:'x'.repeat({}),1:'x',get 2(){{throw token}}}};let r=false;try{{String.raw({{raw}})}}catch(e){{r=e===token}}",
        mars_v9::constants::JS_STRING_UNITS
    );
    assert_eq!(common::eval(&source, "r"), "true");
}

/** RegExp.escape reads code points across compact repetitions without flattening them. */
#[test]
fn regexp_escape_preserves_original_storage() {
    let sources = vec!["const s='😀'.repeat(20000),t='\\ud83d'.repeat(20000)+'\\ude00';if(RegExp.escape(s).length!==s.length||!RegExp.escape(t).endsWith('😀'))throw Error('escape');document.getElementById('go').addEventListener('click',()=>console.log(s.length));".into()];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    assert!(
        serde_json::to_string(&loaded.state).unwrap().len() < common::empty_state_bytes() + 30_000
    );
}

/** JSON quoting does not flatten its compact source, including split surrogate pairs. */
#[test]
fn json_stringify_preserves_original_storage() {
    let sources = vec!["const s='ab'.repeat(100000)+'\\ud83d'+'\\ude00';if(JSON.stringify(s).length!==s.length+2)throw Error('json');document.getElementById('go').addEventListener('click',()=>console.log(s.length));".into()];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    assert!(
        serde_json::to_string(&loaded.state).unwrap().len() < common::empty_state_bytes() + 100_000
    );
}

/** JSON parsing and reviver source spans read compact UTF-16 input without materializing it. */
#[test]
fn json_parse_preserves_compact_input_and_reviver_sources() {
    let sources = vec!["const s=' '.repeat(100000)+'{\"x\":123}';let source;const v=JSON.parse(s,(k,v,c)=>{if(k==='x')source=c.source;return v});if(v.x!==123||source!=='123')throw Error('parse');document.getElementById('go').addEventListener('click',()=>console.log(s.length,source));".into()];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    assert!(
        serde_json::to_string(&loaded.state).unwrap().len() < common::empty_state_bytes() + 100_000
    );
}

/** Diagnostics retain UTF-16 positions while their source context stays bounded. */
#[test]
fn json_parse_reports_compact_input_positions() {
    let source = "const s=' '.repeat(20000)+'{\"x\" 1}';let message;try{JSON.parse(s)}catch(e){message=e.message}const r=message.includes('position 20005 (line 1 column 20006)');";
    assert_eq!(common::eval(source, "r"), "true");
}

/** URI and legacy escape conversions retain the original compact UTF-16 storage. */
#[test]
fn uri_conversions_preserve_original_storage() {
    let sources = vec!["const s='a'.repeat(100000);if(encodeURI(s).length!==s.length||decodeURI(s).length!==s.length||escape(s).length!==s.length||unescape(s).length!==s.length)throw Error('uri');document.getElementById('go').addEventListener('click',()=>console.log(s.length));".into()];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    assert!(
        serde_json::to_string(&loaded.state).unwrap().len() < common::empty_state_bytes() + 100_000
    );
}

/** Joining arrays reads elements and separators without flattening their compact backing. */
#[test]
fn array_join_preserves_original_storage() {
    let sources = vec!["const s='ab'.repeat(100000);if([s].join().length!==s.length||[s].join('').length!==s.length||['',''].join(s).length!==s.length)throw Error('join');document.getElementById('go').addEventListener('click',()=>console.log(s.length));".into()];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    assert!(
        serde_json::to_string(&loaded.state).unwrap().len() < common::empty_state_bytes() + 100_000
    );
}

/** Hex and base64 decoding leave compact input strings unmaterialized. */
#[test]
fn typed_codecs_preserve_original_storage() {
    let sources = vec!["const h='61'.repeat(100000),b='YWFh'.repeat(20000);if(Uint8Array.fromHex(h).length!==100000||Uint8Array.fromBase64(b).length!==60000)throw Error('codec');document.getElementById('go').addEventListener('click',()=>console.log(h.length,b.length));".into()];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    assert!(
        serde_json::to_string(&loaded.state).unwrap().len() < common::empty_state_bytes() + 100_000
    );
}

/** Window base64 encoding reads Latin-1 units without flattening the original rope. */
#[test]
fn window_base64_preserves_original_storage() {
    let sources = vec!["const s='a'.repeat(100000);if(btoa(s).length!==133336)throw Error('btoa');document.getElementById('go').addEventListener('click',()=>console.log(s.length));".into()];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    assert!(
        serde_json::to_string(&loaded.state).unwrap().len() < common::empty_state_bytes() + 100_000
    );
}

/** Truly impossible d8 lengths throw before allocation; smaller V9 quota breaches remain fatal. */
#[cfg(feature = "conformance")]
#[test]
fn d8_impossible_lengths_do_not_change_resource_limits() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    let source = "let n=0;for(const f of [()=> 'x'.repeat(2**30),()=> 'x'.padStart(2**30),()=> 'x'.padEnd(2**30)])try{f()}catch(e){n+=e instanceof RangeError}const r=n;";
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "3");
    let within_v8 = format!("'x'.repeat({});", mars_v9::constants::JS_STRING_UNITS + 1);
    let limited = common::load_with(&within_v8, &environment);
    assert!(!limited.failed.is_empty());
}
