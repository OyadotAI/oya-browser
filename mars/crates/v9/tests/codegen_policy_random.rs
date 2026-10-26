/** Host string-compilation admission and explicit shell RNG seeds obey real policy, context and persistence boundaries. */
mod common;

/** Every dynamic Function family and both eval forms reject strings through the intrinsic EvalError brand. */
#[test]
fn disallowed_string_compilation_uses_eval_error_for_every_entrypoint() {
    let environment = mars_v9::PageEnvironment {
        disallow_code_generation_from_strings: true,
        ..common::environment()
    };
    let execution = common::load_with(
        "const E=EvalError;globalThis.EvalError=function(){throw 'author constructor'};let n=0;for(const f of [()=>eval('1+1'),()=>(0,eval)('1+1'),()=>Function('return 1'),()=>new Function('return 1'),()=>(function*(){}).constructor('yield 1'),()=>(async function(){}).constructor('return 1'),()=>(async function*(){}).constructor('yield 1')]){try{f()}catch(e){if(e instanceof E)n++}}const r=n;",
        &environment,
    );
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "7");
}

/** Policy checks neither coerce non-string eval arguments nor interfere with a shadowing ordinary function. */
#[test]
fn non_string_eval_and_ordinary_eval_named_calls_remain_allowed() {
    let environment = mars_v9::PageEnvironment {
        disallow_code_generation_from_strings: true,
        ..common::environment()
    };
    let execution = common::load_with(
        "let touched=0;const a={toString(){touched++;throw 1}},b=new String('throw 1');const direct=eval(a)===a&&eval(b)===b;const indirect=(0,eval)(a)===a&&(0,eval)(b)===b;const shadow=(function(eval){return eval('text')})(x=>'ordinary '+x);const r=[direct,indirect,touched,shadow,eval(3)].join();",
        &environment,
    );
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(
        common::global(&execution, "r"),
        "true,true,0,ordinary text,3"
    );
}

/** Function source arguments convert in order before policy validation; coercion exceptions keep their identity. */
#[test]
fn function_policy_runs_after_argument_conversion_but_before_parse() {
    let environment = mars_v9::PageEnvironment {
        disallow_code_generation_from_strings: true,
        ..common::environment()
    };
    let execution = common::load_with(
        "const order=[];let denied=false,same=false;const marker={};try{Function({toString(){order.push('param');return '!bad'}},{toString(){order.push('body');return '???'}})}catch(e){denied=e instanceof EvalError}try{Function({toString(){throw marker}},'return 1')}catch(e){same=e===marker}const r=[denied,same,order.join(':')].join();",
        &environment,
    );
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "true,true,param:body");
}

/** Restricting a later request blocks new compilation while already compiled functions and callbacks still execute. */
#[test]
fn request_policy_does_not_revoke_already_compiled_dynamic_code() {
    let sources = vec!["const f=Function('return 7');let r;setTimeout(()=>{let denied=false;try{eval('1')}catch(e){denied=e instanceof EvalError}r=[f(),denied].join()},0);".into()];
    let host = common::host();
    let mut environment = common::environment();
    let execution =
        mars_v9::load_page(common::input(&sources, &host, &environment)).expect("page loads");
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    environment.disallow_code_generation_from_strings = true;
    let execution = mars_v9::run_timers(
        common::input(&sources, &host, &environment),
        common::round_trip(execution.state),
        0.0,
        1,
    )
    .expect("restored request");
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "7,true");
}

/** Published pinned-V8 sequence points cover initialization and continued draws beyond former cache boundaries. */
#[cfg(feature = "conformance")]
#[test]
fn explicit_seed_matches_native_sequence_without_a_lookup_table() {
    let environment = seeded(12345);
    let execution = common::load_with(
        "const samples=[];for(let i=0;i<500;i++){const value=Math.random();if(i%100===0)samples.push(value)}const r=samples.join();",
        &environment,
    );
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(
        common::global(&execution, "r"),
        "0.07740188933548275,0.42849607869962214,0.11546230417400627,0.25526001410747157,0.15944684713606938"
    );
}

/** Creation contexts restart a requested seed independently and consuming one cannot advance another. */
#[cfg(feature = "conformance")]
#[test]
fn seeded_streams_are_separate_for_realms_and_fresh_pages() {
    let environment = seeded(-31);
    let execution = common::load_with(
        "const child=Realm.create();const first=Math.random();for(let i=0;i<10;i++)Math.random();const foreign=Realm.eval(child,'Math.random()');const second=Realm.eval(child,'Math.random()');const fresh=Realm.create();const repeated=Realm.eval(fresh,'Math.random()');const r=[first===foreign,first===repeated,second!==foreign,first>=0&&first<1].join();",
        &environment,
    );
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "true,true,true,true");
    let first = common::load_with("const r=Math.random();", &environment);
    let again = common::load_with("const r=Math.random();", &environment);
    assert_eq!(common::global(&first, "r"), common::global(&again, "r"));
}

/** JSON persistence resumes the next stream word instead of replaying the seed on every request. */
#[cfg(feature = "conformance")]
#[test]
fn seeded_rng_state_survives_json_and_callback_boundaries() {
    let environment = seeded(12345);
    let host = common::host();
    let sources = vec![
        "for(let i=0;i<100;i++)Math.random();let r;setTimeout(()=>{r=Math.random()},0);".into(),
    ];
    let execution =
        mars_v9::load_page(common::input(&sources, &host, &environment)).expect("initial request");
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    let execution = mars_v9::run_timers(
        common::input(&sources, &host, &environment),
        common::round_trip(execution.state),
        0.0,
        1,
    )
    .expect("continued request");
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "0.42849607869962214");
}

/** Saved stream records cannot introduce all-zero absorbing state or an unallocated creation context. */
#[cfg(feature = "conformance")]
#[test]
fn seeded_rng_snapshot_rejects_invalid_state_and_context_ids() {
    let environment = seeded(9);
    let host = common::host();
    let sources = vec!["Math.random();setTimeout(()=>{},0);".into()];
    let execution =
        mars_v9::load_page(common::input(&sources, &host, &environment)).expect("initial request");
    let wire = serde_json::to_value(execution.state).expect("state encodes");
    for key in ["0", "4294967295"] {
        let mut malformed = wire.clone();
        assert!(forge_rng(&mut malformed, key));
        let state = serde_json::from_value(malformed).expect("inert state decodes");
        assert!(
            mars_v9::run_timers(common::input(&sources, &host, &environment), state, 0.0, 1)
                .is_err()
        );
    }
}

/** All tests use the actual public shell seed option and preserve default production configuration. */
#[cfg(feature = "conformance")]
fn seeded(seed: i32) -> mars_v9::PageEnvironment {
    mars_v9::PageEnvironment {
        conformance_d8: true,
        conformance_random_seed: Some(seed),
        ..common::environment()
    }
}

/** Locate the serialized public state field rather than depending on its surrounding private layout. */
#[cfg(feature = "conformance")]
fn forge_rng(value: &mut serde_json::Value, key: &str) -> bool {
    match value {
        serde_json::Value::Object(fields) => {
            if fields.contains_key("math_random") {
                let word = if key == "0" { 0 } else { 1 };
                let mut states = serde_json::Map::new();
                states.insert(key.into(), serde_json::json!({"first":word,"second":word}));
                fields.insert("math_random".into(), serde_json::Value::Object(states));
                return true;
            }
            fields.values_mut().any(|field| forge_rng(field, key))
        }
        serde_json::Value::Array(fields) => fields.iter_mut().any(|field| forge_rng(field, key)),
        _ => false,
    }
}
