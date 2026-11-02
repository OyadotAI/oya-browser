/* Externalization host API: contents, Latin-1 reporting and argument validation, observed through real scripts. */
#![cfg(feature = "conformance")]
use crate::common;
use mars_v9::PageEnvironment;

/** Run one synchronous script and return its console lines; no timers or host tasks are involved. */
fn run(source: &str) -> Vec<String> {
    let sources = vec![source.to_string()];
    let host = common::host();
    let environment = PageEnvironment {
        conformance_d8: true,
        conformance_expose_externalize_string: true,
        ..common::environment()
    };
    let execution = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    common::console(&execution)
}

/** The extension globals exist only when the host explicitly enables them. */
#[test]
fn external_string_extension_respects_host_flag() {
    let environment = PageEnvironment {
        conformance_d8: true,
        ..common::environment()
    };
    let execution = common::load_with("let r=typeof externalizeString;", &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "undefined");
    assert_eq!(run("console.log(typeof externalizeString)"), ["function"]);
}

/** Young strings are copied to old storage; surviving a full GC permits in-place externalization. */
#[test]
fn externalizable_preparation_respects_string_generation_and_identity() {
    let environment = PageEnvironment {
        conformance_d8: true,
        conformance_expose_externalize_string: true,
        conformance_native_main_source: Some(0),
        ..common::environment()
    };
    let source = "const young=['young',' text'].join('');let rejected=false;try{externalizeString(young)}catch(e){rejected=true}const prepared=createExternalizableString(young);const copied=!%IsSameHeapObject(young,prepared);gc();const old=createExternalizableString(young);let r=[rejected,copied,%IsSameHeapObject(young,old)].join();";
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "true,true,true");
}

/** Shared strings need the forwarding table; preparation makes a genuine local old copy otherwise. */
#[test]
fn shared_string_without_table_prepares_a_local_copy() {
    let environment = PageEnvironment {
        conformance_d8: true,
        conformance_shared_strings: true,
        conformance_expose_externalize_string: true,
        conformance_native_main_source: Some(0),
        ..common::environment()
    };
    let source = "const s=%ShareObject('shared string');let rejected=false;try{externalizeString(s)}catch(e){rejected=true}const local=createExternalizableString(s);externalizeString(local);let r=[rejected,!%IsSharedString(local),local===s].join();";
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "true,true,true");
}

/** A saved isolate preserves string generation so later preparation keeps the same real backing. */
#[test]
fn old_string_generation_survives_state_transport() {
    let sources = vec!["const s=['old',' string'].join(''),alias=s;gc();setTimeout(()=>console.log(%IsSameHeapObject(s,alias),%IsSameHeapObject(s,createExternalizableString(s))),0);".to_string()];
    let host = common::host();
    let environment = PageEnvironment {
        conformance_d8: true,
        conformance_expose_externalize_string: true,
        conformance_native_main_source: Some(0),
        ..common::environment()
    };
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let delivered = mars_v9::run_timers(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        1.0,
        10,
    )
    .unwrap();
    assert!(
        delivered.failed.is_empty(),
        "{:?}",
        common::failure(&delivered)
    );
    assert_eq!(common::console(&delivered), ["true true"]);
}

/** A young string and its alias retain one real cell after a saved-state round trip. */
#[test]
fn young_string_aliases_survive_state_transport() {
    let sources = vec!["const s=['young',' string'].join(''),alias=s;setTimeout(()=>console.log(%IsSameHeapObject(s,alias),!%IsSameHeapObject(s,createExternalizableString(s))),0);".to_string()];
    let host = common::host();
    let environment = PageEnvironment {
        conformance_d8: true,
        conformance_expose_externalize_string: true,
        conformance_native_main_source: Some(0),
        ..common::environment()
    };
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let delivered = mars_v9::run_timers(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        1.0,
        10,
    )
    .unwrap();
    assert!(
        delivered.failed.is_empty(),
        "{:?}",
        common::failure(&delivered)
    );
    assert_eq!(common::console(&delivered), ["true true"]);
}

/** Aliases count once per minor collection; two nursery survivals promote the original cell. */
#[test]
fn repeated_minor_collections_promote_a_string_once_per_cycle() {
    let environment = PageEnvironment {
        conformance_d8: true,
        conformance_expose_externalize_string: true,
        conformance_native_main_source: Some(0),
        ..common::environment()
    };
    let source = "const s=['minor',' string'].join(''),alias=s;gc({type:'minor'});let r=!%IsSameHeapObject(s,createExternalizableString(s));gc({type:'minor'});r+=','+%IsSameHeapObject(s,createExternalizableString(s));";
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "true,true");
}

/** Non-Latin-1 single units are not V8's cached read-only single-character strings. */
#[test]
fn wide_single_units_can_be_externalized() {
    assert_eq!(
        run(
            "const wide=createExternalizableString('\\u1234');const lone=createExternalizableString('\\uD83D');externalizeString(wide);externalizeString(lone);console.log([wide.charCodeAt(0),lone.charCodeAt(0)].join());"
        ),
        ["4660,55357"]
    );
}

#[test]
/** Externalizing leaves length, code units, well-formedness and concatenation exactly as they were. */
fn externalized_string_keeps_contents_and_semantics() {
    assert_eq!(
        run(
            "const s=createExternalizableString('abc\\uD83D');const before=[s.length,s.charCodeAt(3),s.isWellFormed()].join(':');externalizeString(s);console.log([before,[s.length,s.charCodeAt(3),s.isWellFormed()].join(':'),s+'x'===createExternalizableString('abc\\uD83Dx')].join('|'));"
        ),
        ["4:55357:false|4:55357:false|true"]
    );
}

/** Two-byte preparation rejects an actual one-byte cons but accepts a two-byte child. */
#[test]
fn two_byte_preparation_checks_real_cons_encoding() {
    assert_eq!(
        run(
            "let rejected=false;try{createExternalizableTwoByteString('foobarconstest'+42)}catch(e){rejected=true}const s=createExternalizableTwoByteString(createExternalizableTwoByteString('foobarconstest')+42);console.log([rejected,isOneByteString(s)].join());"
        ),
        ["true,false"]
    );
}

#[test]
/** Externalizing a deferred concatenation flattens it without altering its code units. */
fn externalized_concatenation_keeps_its_units() {
    assert_eq!(
        run(
            "let s=createExternalizableString('a'.repeat(2000)+'\\u1234');externalizeString(s);console.log([s.length,s.charCodeAt(2000),s.slice(1999)==='a\\u1234'].join(':'));"
        ),
        ["2001:4660:true"]
    );
}

#[test]
/** Repeated externalization of one string is accepted and changes nothing observable. */
fn repeated_externalization_is_a_no_op() {
    assert_eq!(
        run(
            "const s=createExternalizableString('xyz');console.log([externalizeString(s),externalizeString(s),s].join(':'));"
        ),
        ["::xyz"]
    );
}

#[test]
/** Only Latin-1 content reports one-byte; a single wider unit, however the string was built, does not. */
fn is_one_byte_reports_latin1_content() {
    assert_eq!(
        run(
            "console.log([isOneByteString('abc'),isOneByteString('\\u00e9\\u00ff'),isOneByteString('\\u0100'),isOneByteString('ab\\u1234'.slice(0,2)),isOneByteString('a'+'\\u1234'),isOneByteString(''),isOneByteString('\\uD83D')].join(':'));"
        ),
        ["true:true:false:true:false:true:false"]
    );
}

#[test]
/** A two-byte source whose wide part is sliced away is Latin-1 content and reports as one-byte. */
fn is_one_byte_follows_content_not_storage() {
    assert_eq!(
        run(
            "const wide=createExternalizableString('\\u1234'+'ab'.repeat(3));console.log([isOneByteString(wide),isOneByteString(wide.slice(1))].join(':'));"
        ),
        ["false:true"]
    );
}

#[test]
/** Non-string operands are refused with an ordinary Error, not coerced to text first. */
fn externalize_rejects_non_string_operands() {
    assert_eq!(
        run(
            "let coerced=false;try{externalizeString({toString(){coerced=true;return 'x'}})}catch(e){console.log([e instanceof Error,e.message,coerced].join(':'))}"
        ),
        ["true:First parameter to externalizeString() must be a string.:false"]
    );
}

#[test]
/** Calling without an operand fails the same way as a non-string operand. */
fn externalize_without_operand_throws() {
    assert_eq!(
        run("try{externalizeString()}catch(e){console.log(e instanceof Error)}"),
        ["true"]
    );
}

#[test]
/** An empty string is refused, because V8 never externalizes it; the refusal is a catchable Error. */
fn externalize_rejects_the_empty_string() {
    assert_eq!(
        run(
            "try{externalizeString('')}catch(e){console.log([e instanceof Error,e.message].join(':'))}"
        ),
        ["true:String is too short to be externalized."]
    );
}

#[test]
/** isOneByteString refuses non-string operands instead of reporting a guess about them. */
fn is_one_byte_rejects_non_string_operands() {
    assert_eq!(
        run(
            "try{isOneByteString(1)}catch(e){console.log([e instanceof Error,e.message].join(':'))}"
        ),
        ["true:isOneByteString() requires a single string argument."]
    );
}

#[test]
/** The host constants are numbers so padding loops in V8-style tests terminate on real lengths. */
fn externalization_minimums_are_numbers() {
    assert_eq!(
        run(
            "console.log([typeof kExternalStringMinOneByteLength,typeof kExternalStringMinTwoByteLength,'E='.padEnd(kExternalStringMinOneByteLength,'.')].join(':'));"
        ),
        ["number:number:E="]
    );
}

#[test]
/** The Latin-1 flag combines through concatenation in either order, so a wide operand always wins. */
fn is_one_byte_flag_survives_concatenation() {
    assert_eq!(
        run(
            "const a='x'.repeat(2000);const b=a+'\\u1234';console.log([isOneByteString(a),isOneByteString(a+'y'),isOneByteString(b),isOneByteString('\\u1234'+a)].join(':'));"
        ),
        ["true:true:false:false"]
    );
}

/** Native string construction retains real identities and contents through garbage collection. */
#[test]
fn constructed_strings_survive_collection_and_keep_identity() {
    let environment = PageEnvironment {
        conformance_d8: true,
        conformance_native_main_source: Some(0),
        ..common::environment()
    };
    let source = "let parent='abcdefghijklmnopqrstuvwxyz';let slice=%ConstructSlicedString(parent,3);let cons=%ConstructConsString('',slice);let alias=cons;parent=null;slice=null;gc();let r=[cons,%IsSameHeapObject(cons,alias)].join();";
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(
        common::global(&execution, "r"),
        "defghijklmnopqrstuvwxyz,true"
    );
}

/** Collection ages a flat child retained only by its cons parent. */
#[test]
fn collection_ages_retained_flat_children() {
    let environment = PageEnvironment {
        conformance_d8: true,
        conformance_expose_externalize_string: true,
        conformance_native_main_source: Some(0),
        ..common::environment()
    };
    let source = "let cons=%ConstructConsString('abcdefghijklmn','opqrstuvwxyz');let flat=%FlattenString(cons);flat=null;gc();flat=%FlattenString(cons);const prepared=createExternalizableString(flat);let r=%IsSameHeapObject(flat,prepared);";
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "true");
}

/** An empty-first cons follows V8's flat-copy path for explicit two-byte preparation. */
#[test]
fn empty_first_cons_can_be_prepared_as_two_byte() {
    let environment = PageEnvironment {
        conformance_d8: true,
        conformance_expose_externalize_string: true,
        conformance_native_main_source: Some(0),
        ..common::environment()
    };
    let source = "const value=%ConstructConsString('','abcdefghijklmn');const copy=createExternalizableTwoByteString(value);let r=[copy===value,isOneByteString(copy)].join();";
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "true,false");
}

/** Extension minimums are mutable global lexicals in each realm, not global-object properties. */
#[test]
fn externalization_minimums_are_realm_local_lexicals() {
    let environment = PageEnvironment {
        conformance_d8: true,
        conformance_expose_externalize_string: true,
        ..common::environment()
    };
    let source = "const child=Realm.create();let r=[kExternalStringMinOneByteLength,typeof globalThis.kExternalStringMinOneByteLength,Realm.eval(child,'kExternalStringMinOneByteLength'),Realm.eval(child,'typeof globalThis.kExternalStringMinOneByteLength')].join();kExternalStringMinOneByteLength=123;r+=','+Realm.eval(child,'kExternalStringMinOneByteLength');";
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "1,undefined,1,undefined,1");
}

/** Compiled code caches cannot share mutable string resources between separate isolates. */
#[test]
fn code_literal_externalization_is_isolate_local() {
    let environment = PageEnvironment {
        conformance_d8: true,
        conformance_expose_externalize_string: true,
        ..common::environment()
    };
    let source = "const text='distinct isolate literal';const copy=createExternalizableTwoByteString(text);let r=isOneByteString(copy);externalizeString(text);";
    let first = common::load_with(source, &environment);
    let second = common::load_with(source, &environment);
    for execution in [first, second] {
        assert!(
            execution.failed.is_empty(),
            "{:?}",
            common::failure(&execution)
        );
        assert_eq!(common::global(&execution, "r"), "false");
    }
}

/** Cached function names and template chunks cannot publish external resources into another isolate. */
#[test]
fn cached_function_names_and_templates_are_isolate_local() {
    let environment = PageEnvironment {
        conformance_d8: true,
        conformance_expose_externalize_string: true,
        ..common::environment()
    };
    let source = "function uniquelyNamedFunction(){} function tag(parts){return parts.raw[0]}const text=[uniquelyNamedFunction.name,tag`distinct template string`];let r=text.map(s=>isOneByteString(createExternalizableTwoByteString(s))).join();text.forEach(externalizeString);";
    for execution in [
        common::load_with(source, &environment),
        common::load_with(source, &environment),
    ] {
        assert!(
            execution.failed.is_empty(),
            "{:?}",
            common::failure(&execution)
        );
        assert_eq!(common::global(&execution, "r"), "false,false");
    }
}

/** Literal identity remains stable when a saved VM resumes at a later timer. */
#[test]
fn code_literal_identity_survives_state_transport() {
    let sources = vec!["const text='stable literal backing';let r=false;setTimeout(()=>r=%IsSameHeapObject(text,'stable literal backing'),0);".to_string()];
    let host = common::host();
    let environment = PageEnvironment {
        conformance_d8: true,
        conformance_native_main_source: Some(0),
        ..common::environment()
    };
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let state = common::round_trip(loaded.state);
    let resumed = mars_v9::run_timers(
        common::input(&sources, &host, &environment),
        state,
        loaded.effects.next_timer_ms.unwrap(),
        1,
    )
    .unwrap();
    assert!(resumed.failed.is_empty(), "{:?}", common::failure(&resumed));
    assert_eq!(common::global(&resumed, "r"), "true");
}

/** A saved code-constant cache cannot replace compiled source text with forged contents. */
#[test]
fn forged_code_constant_cache_is_rejected() {
    let sources = vec!["const value='verified literal';setTimeout(()=>value,0);".to_string()];
    let host = common::host();
    let environment = PageEnvironment {
        conformance_d8: true,
        ..common::environment()
    };
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    let mut wire = serde_json::to_value(loaded.state).unwrap();
    let constants = wire["page"]["code_constants"].as_object_mut().unwrap();
    let (_, value) = constants.iter_mut().next().unwrap();
    *value = serde_json::json!("forged code constant");
    let state = serde_json::from_value(wire).unwrap();
    let result = mars_v9::run_timers(
        common::input(&sources, &host, &environment),
        state,
        loaded.effects.next_timer_ms.unwrap(),
        1,
    );
    assert!(result.is_err());
}

/** Different direct-eval environments retain their own code constants across saved-state resumption. */
#[test]
fn direct_eval_contexts_survive_constant_cache_and_state_transport() {
    let sources = vec!["function a(){let x=1;return eval('x')}function b(){let y=2;return eval('y')}let r=a()+b();setTimeout(()=>r+=a()+b(),0);".to_string()];
    let host = common::host();
    let environment = PageEnvironment {
        conformance_d8: true,
        ..common::environment()
    };
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let state = common::round_trip(loaded.state);
    let resumed = mars_v9::run_timers(
        common::input(&sources, &host, &environment),
        state,
        loaded.effects.next_timer_ms.unwrap(),
        1,
    )
    .unwrap();
    assert!(resumed.failed.is_empty(), "{:?}", common::failure(&resumed));
    assert_eq!(common::global(&resumed, "r"), "6");
}

#[test]
fn internalization_keeps_weak_identity_through_state() {
    let sources = vec!["let a=['alpha',' beta'].join('');let b=%InternalizeString(a);let r=[a===b,!%IsSameHeapObject(a,b)].join();setTimeout(()=>{let c=%InternalizeString(['alpha',' beta'].join(''));r+=','+%IsSameHeapObject(b,c)},0);".to_string()];
    let host = common::host();
    let environment = PageEnvironment {
        conformance_d8: true,
        conformance_native_main_source: Some(0),
        ..common::environment()
    };
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let state = common::round_trip(loaded.state);
    let resumed = mars_v9::run_timers(
        common::input(&sources, &host, &environment),
        state,
        loaded.effects.next_timer_ms.unwrap(),
        1,
    )
    .unwrap();
    assert!(resumed.failed.is_empty(), "{:?}", common::failure(&resumed));
    assert_eq!(common::global(&resumed, "r"), "true,true,true");
}

#[test]
fn internalizes_old_external_in_place_without_releasing_backing() {
    let source = "const s=createExternalizableString(['old',' external text'].join(''));externalizeString(s);const t=%InternalizeString(s);let r=[%IsSameHeapObject(s,t),isOneByteString(t)].join();";
    let environment = PageEnvironment {
        conformance_d8: true,
        conformance_expose_externalize_string: true,
        conformance_native_main_source: Some(0),
        ..common::environment()
    };
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "true,true");
}

#[test]
fn internalization_uses_real_shared_target() {
    let source = "const local=['shared',' payload'].join('');const target=%InternalizeString(local);let r=[local===target,%IsSharedString(target),!%IsSharedString(local),!%IsSameHeapObject(local,target)].join();";
    let environment = PageEnvironment {
        conformance_d8: true,
        conformance_shared_string_table: true,
        conformance_native_main_source: Some(0),
        ..common::environment()
    };
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "true,true,true,true");
}

#[test]
fn weak_intern_table_does_not_retain_dead_dynamic_strings() {
    let source = "(()=>{let a=%InternalizeString(['unique weak',' dynamic string'].join(''));a=null})();gc();";
    let environment = PageEnvironment {
        conformance_d8: true,
        conformance_native_main_source: Some(0),
        ..common::environment()
    };
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    let state = serde_json::to_string(&execution.state).unwrap();
    assert!(!state.contains("unique weak dynamic string"));
}

#[test]
fn shared_internalization_normalizes_encoding_without_aliasing_source() {
    let sources = vec!["const source=%ShareObject(createExternalizableTwoByteString(['distinct shared',' ascii string'].join('')));const target=%InternalizeString(source);let r=[%IsSharedString(source),%IsSharedString(target),isOneByteString(source),isOneByteString(target),!%IsSameHeapObject(source,target)].join();setTimeout(()=>r+=','+[%IsSharedString(source),isOneByteString(source),isOneByteString(target),!%IsSameHeapObject(source,target)].join(),0);".to_string()];
    let host = common::host();
    let environment = PageEnvironment {
        conformance_d8: true,
        conformance_shared_string_table: true,
        conformance_expose_externalize_string: true,
        conformance_native_main_source: Some(0),
        ..common::environment()
    };
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let state = common::round_trip(loaded.state);
    let resumed = mars_v9::run_timers(
        common::input(&sources, &host, &environment),
        state,
        loaded.effects.next_timer_ms.unwrap(),
        1,
    )
    .unwrap();
    assert!(resumed.failed.is_empty(), "{:?}", common::failure(&resumed));
    assert_eq!(
        common::global(&resumed, "r"),
        "true,true,false,true,true,true,false,true,true"
    );
}

#[test]
fn shared_internalized_and_noncanonical_strings_survive_worker_transport() {
    let session = mars_v9::ConformanceWorkerSession::new();
    let source = "const s=%ShareObject(createExternalizableTwoByteString(['worker shared',' ascii text'].join('')));const t=%InternalizeString(s);const w=new Worker('onmessage=({data})=>postMessage([isOneByteString(data[0]),isOneByteString(data[1]),%IsSameHeapObject(data[0],data[1])].join())',{type:'string'});w.postMessage([s,t]);let r=w.getMessage();w.terminateAndWait();";
    let environment = PageEnvironment {
        conformance_d8: true,
        conformance_shared_string_table: true,
        conformance_expose_externalize_string: true,
        conformance_native_main_source: Some(0),
        conformance_workers: Some(session.host()),
        ..common::environment()
    };
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "false,true,false");
    session.finish().unwrap();
}

#[test]
fn noncanonical_shared_external_resource_survives_snapshot() {
    let sources = vec!["const s=%ShareObject(createExternalizableTwoByteString(['external shared',' ascii payload'].join('')));externalizeString(s);const t=%InternalizeString(s);let r=[isOneByteString(s),isOneByteString(t),!%IsSameHeapObject(s,t)].join();setTimeout(()=>r+=','+[isOneByteString(s),isOneByteString(t),!%IsSameHeapObject(s,t),%IsSameHeapObject(s,createExternalizableTwoByteString(s))].join(),0);".to_string()];
    let host = common::host();
    let environment = PageEnvironment {
        conformance_d8: true,
        conformance_shared_string_table: true,
        conformance_expose_externalize_string: true,
        conformance_native_main_source: Some(0),
        ..common::environment()
    };
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let state = common::round_trip(loaded.state);
    let resumed = mars_v9::run_timers(
        common::input(&sources, &host, &environment),
        state,
        loaded.effects.next_timer_ms.unwrap(),
        1,
    )
    .unwrap();
    assert!(resumed.failed.is_empty(), "{:?}", common::failure(&resumed));
    assert_eq!(
        common::global(&resumed, "r"),
        "false,true,true,false,true,true,true"
    );
}

/** Constructed thin strings keep their own identity and forward to the actual interned target. */
#[test]
fn native_thin_and_internalized_strings_keep_distinct_backings() {
    let source = "let source=['real thin',' payload'].join('');let thin=%ConstructThinString(source);let target=%ConstructInternalizedString(source);let again=%ConstructThinString(thin);let r=[thin===target,!%IsSameHeapObject(thin,target),%IsSameHeapObject(thin,again),%IsSameHeapObject(target,%InternalizeString(thin))].join();";
    let environment = PageEnvironment {
        conformance_d8: true,
        conformance_native_main_source: Some(0),
        ..common::environment()
    };
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "true,true,true,true");
}

/** Fuzzing rejects invalid arity and inputs after evaluating all extra arguments. */
#[test]
fn native_thin_construction_fuzzing_preserves_argument_effects() {
    let source = "let n=0;let a=%ConstructThinString();let b=%ConstructThinString('too short');let c=%ConstructThinString(1);let d=%ConstructThinString('long enough payload',++n);let e=%ConstructInternalizedString(1);let f=%ConstructInternalizedString('valid',++n);let r=[a,b,c,d,e,f,n].join();";
    let environment = PageEnvironment {
        conformance_d8: true,
        conformance_fuzzing: true,
        conformance_native_main_source: Some(0),
        ..common::environment()
    };
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), ",,,,,,2");
}
