/** Concatenation preserves immutable UTF-16 values, conversion order and the existing admission boundary. */
mod common;

/** Concatenation retains lone surrogates and pairs adjacent halves without rewriting either input. */
#[test]
fn concatenation_preserves_exact_code_units_and_input_aliases() {
    let source = "const high='\\ud800',low='\\udc00';const a=high+'x'+low;const pair=high+low;const r=[a.length,a.charCodeAt(0),a.charCodeAt(2),pair.length,[...pair].length,high.charCodeAt(0),low.charCodeAt(0)].join();";
    assert_eq!(common::eval(source, "r"), "3,55296,56320,2,1,55296,56320");
}

/** Building later strings cannot mutate earlier primitive snapshots or their property-key identities. */
#[test]
fn repeated_concatenation_keeps_old_values_and_keys_immutable() {
    let source = "let text='';const saved=[],object={};for(let i=0;i<1000;i++){if(i%250===0){saved.push(text);object[text]=i}text+='a'}const r=[saved.map(s=>s.length).join(':'),text.length,object[''],object['a'.repeat(250)],object[text]===undefined].join();";
    assert_eq!(common::eval(source, "r"), "0:250:500:750,1000,0,250,true");
}

/** The operator evaluates both operands before invoking primitive conversions, then converts left before right. */
#[test]
fn concatenation_keeps_user_conversion_order_and_exceptions() {
    let source = "const log=[];function value(label){log.push('eval '+label);return {[Symbol.toPrimitive](hint){log.push(label+' '+hint);return label}}}const text=value('left')+value('right');const marker={};let same=false;try{({[Symbol.toPrimitive](){throw marker}})+{[Symbol.toPrimitive](){log.push('unreachable');return 'x'}}}catch(error){same=error===marker}const r=[text,log.join(':'),same].join();";
    assert_eq!(
        common::eval(source, "r"),
        "leftright,eval left:eval right:left default:right default,true"
    );
}

/** Empty operands retain the full code-unit value without treating an unmatched surrogate as invalid text. */
#[test]
fn empty_concatenations_keep_unmatched_surrogates() {
    let source = "const text='a\\udfff';const r=[(''+text)===text,(text+'')===text,(''+text+'').length,(''+text+'').charCodeAt(1)].join();";
    assert_eq!(common::eval(source, "r"), "true,true,2,57343");
}

/** The public value API accepts the exact existing UTF-16 ceiling and rejects the next unit before producing a value. */
#[test]
fn concatenation_preserves_the_exact_string_unit_guard() {
    let limit = mars_v9::constants::JS_STRING_UNITS;
    let left = mars_v9::JsString::from_units(vec![u16::from(b'x'); limit - 2]).unwrap();
    let right = mars_v9::JsString::from_utf8("🧪").unwrap();
    let result = left.concat(&right).unwrap();
    assert_eq!(result.len(), limit);
    assert_eq!(&result.as_units()[limit - 2..], right.as_units());
    assert_eq!(left.len(), limit - 2);
    let extra = mars_v9::JsString::from_utf8("x").unwrap();
    assert_eq!(result.concat(&extra).unwrap_err().code, "limit_exceeded");
    assert_eq!(result.len(), limit);
    assert_eq!(left.as_units()[0], u16::from(b'x'));
}

/** Committed aliases and surrogate contents remain immutable when concatenation resumes on another request. */
#[test]
fn concatenation_survives_state_transport_without_mutating_aliases() {
    let sources=vec!["let text='a\\ud800';const old=text;document.getElementById('go').addEventListener('click',()=>{text+='\\udc00';console.log(old.length,old.charCodeAt(1),text.length,text.codePointAt(1),old!==text)});".into()];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let clicked = mars_v9::click_page(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        3,
    )
    .unwrap();
    assert!(clicked.failed.is_empty(), "{:?}", common::failure(&clicked));
    assert_eq!(common::console(&clicked), ["2 55296 3 65536 true"]);
}

/** Different concat trees keep the former slice-based hashes, equality and raw UTF-16 ordering. */
#[test]
fn deferred_concatenations_have_content_hash_equality_and_order() {
    eprintln!(
        "CONCAT_LAYOUT JsString={} JsValue={} flat_arc={}",
        std::mem::size_of::<mars_v9::JsString>(),
        std::mem::size_of::<mars_v9::JsValue>(),
        std::mem::size_of::<std::sync::Arc<[u16]>>()
    );
    use std::{
        collections::{BTreeSet, HashSet},
        hash::{DefaultHasher, Hash, Hasher},
    };
    let prefix = vec![u16::from(b'x'); 2_048];
    let left = mars_v9::JsString::from_units(prefix.clone()).unwrap();
    let tail = mars_v9::JsString::from_units(vec![0xd800, 0xdc00, 0xdfff]).unwrap();
    let deferred = left.concat(&tail).unwrap();
    let mut expected = prefix;
    expected.extend_from_slice(tail.as_units());
    let flat = mars_v9::JsString::from_units(expected.clone()).unwrap();
    let mut actual_hash = DefaultHasher::new();
    deferred.hash(&mut actual_hash);
    let mut old_slice_hash = DefaultHasher::new();
    expected.as_slice().hash(&mut old_slice_hash);
    assert_eq!(actual_hash.finish(), old_slice_hash.finish());
    assert_eq!(deferred, flat);
    assert_eq!(HashSet::from([deferred.clone()]).get(&flat), Some(&flat));
    let later = flat
        .concat(&mars_v9::JsString::from_utf8("a").unwrap())
        .unwrap();
    let values = BTreeSet::from([later.clone(), deferred.clone(), flat.clone()]);
    assert_eq!(values.into_iter().collect::<Vec<_>>(), [flat, later]);
}

/** Shared Rust readers publish one stable slice even while aliases materialize child trees in another order. */
#[test]
fn concurrent_flattening_preserves_stable_slices_and_aliases() {
    use std::sync::{Arc, Barrier};
    let mut text = mars_v9::JsString::from_units(vec![u16::from(b'x'); 2_048]).unwrap();
    let mut expected = text.as_units().to_vec();
    let mut aliases = Vec::new();
    for index in 0..24 {
        let units = vec![u16::from(b'a') + index];
        expected.extend_from_slice(&units);
        text = text
            .concat(&mars_v9::JsString::from_units(units).unwrap())
            .unwrap();
        aliases.push(text.clone());
    }
    let barrier = Arc::new(Barrier::new(8));
    let handles = (0..8)
        .map(|index| {
            let text = text.clone();
            let alias = aliases[index * 3].clone();
            let expected = expected.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                if index % 2 == 0 {
                    assert!(!alias.as_units().is_empty())
                }
                let address = text.as_units().as_ptr() as usize;
                for _ in 0..64 {
                    assert_eq!(text.as_units(), expected.as_slice());
                    assert_eq!(text.as_units().as_ptr() as usize, address);
                }
                if index % 2 != 0 {
                    assert!(!alias.as_units().is_empty())
                }
                address
            })
        })
        .collect::<Vec<_>>();
    let addresses = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect::<Vec<_>>();
    assert!(addresses.iter().all(|address| *address == addresses[0]));
    assert_eq!(text.as_units(), expected.as_slice());
}

/** Many unobserved appends stay bounded on destruction and do not alter any retained earlier string. */
#[test]
fn long_concat_chains_keep_snapshots_and_drop_without_recursive_overflow() {
    let mut text = mars_v9::JsString::from_utf8(&"x".repeat(2_048)).unwrap();
    let chunk = mars_v9::JsString::from_utf8("a").unwrap();
    let first = text.clone();
    let mut saved = Vec::new();
    for index in 0..50_000 {
        text = text.concat(&chunk).unwrap();
        if index % 10_000 == 0 {
            saved.push(text.clone())
        }
    }
    assert_eq!(text.len(), 52_048);
    assert_eq!(first.lossy(), "x".repeat(2_048));
    for (index, snapshot) in saved.into_iter().enumerate() {
        assert_eq!(snapshot.len(), 2_049 + index * 10_000);
        assert_eq!(snapshot.as_units()[snapshot.len() - 1], u16::from(b'a'));
    }
    assert!(
        text.as_units()[2_048..]
            .iter()
            .all(|unit| *unit == u16::from(b'a'))
    );
    drop(text);
}

/** Shared children cannot make doubling evade either the logical unit ceiling or bounded tree destruction. */
#[test]
fn repeated_doubling_preserves_the_existing_unit_ceiling() {
    let limit = mars_v9::constants::JS_STRING_UNITS;
    let mut text = mars_v9::JsString::from_utf8("x").unwrap();
    while text.len() < limit {
        text = text.concat(&text).unwrap();
    }
    assert_eq!(text.len(), limit);
    assert_eq!(text.concat(&text).unwrap_err().code, "limit_exceeded");
    assert!(text.as_units().iter().all(|unit| *unit == u16::from(b'x')));
}

/** Deferred strings and equal flat strings remain interchangeable as ordinary and Map keys. */
#[test]
fn deferred_string_property_and_map_keys_use_content_identity() {
    let source = "const prefix='x'.repeat(2048);let key=prefix;for(let i=0;i<24;i++)key+='a';const same=prefix+'a'.repeat(24);const object={[key]:7};const map=new Map([[key,8]]);const r=[key===same,object[same],map.get(same),Object.keys(object)[0]===same,new Set([key,same]).size].join();";
    assert_eq!(common::eval(source, "r"), "true,7,8,true,1");
}

/** The private wire form remains identical to flat strings, including unmatched surrogate tags. */
#[test]
fn deferred_strings_keep_the_exact_existing_wire_form() {
    let prefix = mars_v9::JsString::from_utf8(&"x".repeat(2_048)).unwrap();
    let tail = mars_v9::JsString::from_units(vec![0xd800]).unwrap();
    let deferred = prefix.concat(&tail).unwrap();
    let mut units = prefix.as_units().to_vec();
    units.extend_from_slice(tail.as_units());
    let flat = mars_v9::JsString::from_units(units).unwrap();
    let encoded = serde_json::to_string(&deferred).unwrap();
    assert_eq!(encoded, serde_json::to_string(&flat).unwrap());
    let restored: mars_v9::JsString = serde_json::from_str(&encoded).unwrap();
    assert_eq!(restored, deferred);
}

/** Concurrent observers and appends share only immutable contents while canonicalization reuses published flat children. */
#[test]
fn concurrent_appends_and_flattening_keep_content_hashes() {
    use std::{
        hash::{DefaultHasher, Hash, Hasher},
        sync::{Arc, Barrier},
    };
    let prefix = mars_v9::JsString::from_utf8(&"x".repeat(2_048)).unwrap();
    let base = prefix
        .concat(&mars_v9::JsString::from_utf8("abc").unwrap())
        .unwrap();
    let barrier = Arc::new(Barrier::new(4));
    let handles = (0..4)
        .map(|_| {
            let base = base.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                let mut text = base;
                for _ in 0..128 {
                    text = text
                        .concat(&mars_v9::JsString::from_utf8("y").unwrap())
                        .unwrap();
                    let mut before = DefaultHasher::new();
                    text.hash(&mut before);
                    let units = text.as_units();
                    let mut after = DefaultHasher::new();
                    units.hash(&mut after);
                    assert_eq!(before.finish(), after.finish());
                }
                text
            })
        })
        .collect::<Vec<_>>();
    let values = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect::<Vec<_>>();
    assert!(values.iter().all(|value| *value == values[0]));
    assert_eq!(
        values[0].lossy(),
        format!("{}abc{}", "x".repeat(2_048), "y".repeat(128))
    );
}
