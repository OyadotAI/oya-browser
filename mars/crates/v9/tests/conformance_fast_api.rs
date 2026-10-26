/* D8 native callback tests count real slow calls and never claim an optimized C ABI exists. */
#![cfg(feature = "conformance")]
mod common;
use mars_v9::PageEnvironment;
/** Native helper exposure is an explicit owning-environment capability, not a global test override. */
fn environment() -> PageEnvironment {
    PageEnvironment {
        conformance_d8: true,
        conformance_expose_fast_api: true,
        ..common::environment()
    }
}
/** Public page execution checks results outside native callbacks and reports all asynchronous errors. */
fn run(source: &str) -> Vec<String> {
    let execution = common::load_with(source, &environment());
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert!(
        execution.effects.conformance_error.is_none(),
        "{:?}",
        execution.effects.conformance_error
    );
    common::console(&execution)
}
#[test]
/** Ordinary conformance environments retain the d8.test namespace without exposing unrequested constructors. */
fn constructor_exposure_requires_the_actual_flag() {
    let execution = common::load_with(
        "console.log(typeof d8.test+':'+typeof d8.test.FastCAPI)",
        &PageEnvironment {
            conformance_d8: true,
            ..common::environment()
        },
    );
    assert_eq!(common::console(&execution), ["object:undefined"]);
}
#[test]
/** Native construction rejects ordinary calls and honors real subclass prototypes. */
fn constructors_and_receiver_brands_match_the_native_boundary() {
    assert_eq!(
        run(
            "let rejected=false;try{d8.test.FastCAPI()}catch(error){rejected=error.constructor===Error};class Sub extends d8.test.FastCAPI{};const api=new Sub();console.log([rejected,api instanceof Sub,api.is_fast_c_api_object(api),api.is_fast_c_api_object(Object.create(api)),api.is_fast_c_api_object(new d8.test.LeafInterfaceType())].join(':'))"
        ),
        ["true:true:true:false:false"]
    );
}
#[test]
/** Inherited getters and borrowed methods cannot manufacture an embedded native receiver. */
fn inherited_objects_do_not_inherit_native_receiver_brand() {
    assert_eq!(
        run(
            "const api=new d8.test.FastCAPI(),fake=Object.create(api);let caught=0;for(const call of [()=>fake.supports_fp_params,()=>api.fast_call_count.call(fake),()=>api.add_all.call(fake)])try{call()}catch(error){if(error.constructor===Error)caught++}console.log([caught,api.slow_call_count(),api.fast_call_count()].join(':'))"
        ),
        ["3:0:0"]
    );
}
#[test]
/** Every executed native slow operation counts, including failed validation, while queries never fabricate fast calls. */
fn callback_counts_track_actual_invocations_only() {
    assert_eq!(
        run(
            "const a=new d8.test.FastCAPI(),b=new d8.test.FastCAPI();a.reset_counts();const sum=a.add_all(-1,2,3,4,5.5,6.5);try{a.sum_int64_as_number(true,2)}catch(error){}console.log([sum,a.fast_call_count(),b.slow_call_count(),a.supports_fp_params,a.slow_call_count()].join(':'));b.reset_counts();console.log(a.slow_call_count())"
        ),
        ["20:0:2:false:2", "0"]
    );
}
#[test]
/** Numeric slots are strict primitive Numbers and never invoke caller coercion hooks. */
fn integer_range_checks_are_strict_and_ordered() {
    assert_eq!(
        run(
            "const api=new d8.test.FastCAPI();let coercions=0;const object={valueOf(){coercions++;return 1}};const messages=[];for(const args of [[object,0],[0,2**63],[2**63,0]])try{api.sum_int64_as_number(...args)}catch(error){messages.push(error.message)}try{api.sum_uint64_as_number(0,-1)}catch(error){messages.push(error.message)}console.log([coercions,messages.join('|'),api.slow_call_count()].join(':'))"
        ),
        [
            "0:Did not get a number as first parameter.|Second number is out of int64_t range.|First number is out of int64_t range.|Second number is out of uint64_t range.:4"
        ]
    );
}
#[test]
/** Signed/unsigned native sums use actual fixed-width values and exact BigInt low-word conversion. */
fn fixed_width_sums_preserve_native_integer_results() {
    assert_eq!(
        run(
            "const api=new d8.test.FastCAPI();console.log([api.sum_int64_as_number(-2,5),api.sum_uint64_as_number(2,5),api.sum_int64_as_bigint(-1n,2n),api.sum_uint64_as_bigint((1n<<64n)-1n,1n),api.sum_int64_as_bigint((1n<<128n)+1n,2n)].join(':'))"
        ),
        ["3:7:1:0:3"]
    );
}
#[test]
/** Real Number conversion propagates the author's original exception while retaining the completed callback count. */
fn number_conversion_runs_author_code_and_preserves_thrown_identity() {
    assert_eq!(
        run(
            "const api=new d8.test.FastCAPI(),marker={};let calls=0,same=false;try{api.call_to_number({valueOf(){calls++;throw marker}})}catch(error){same=error===marker}console.log([calls,same,api.slow_call_count()].join(':'))"
        ),
        ["1:true:1"]
    );
}
#[test]
/** Native attributes share the isolate backing, validate first, and count real get/set operations. */
fn native_attributes_use_shared_primitive_integer_storage() {
    assert_eq!(
        run(
            "const a=new d8.test.FastCAPI(),b=new d8.test.FastCAPI();a.reset_counts();a.fast_attribute=4.9;const value=b.fast_attribute;try{a.fast_attribute={valueOf(){throw Error('coerced')}}}catch(error){}console.log([value,a.slow_call_count(),Object.getOwnPropertyDescriptor(a,'supports_fp_params').enumerable].join(':'))"
        ),
        ["4:3:true"]
    );
}
#[test]
/** Range and clamp callbacks perform their actual slow-path comparisons and safe-integer saturation. */
fn range_and_clamp_callbacks_compute_real_native_results() {
    assert_eq!(
        run(
            "const api=new d8.test.FastCAPI();console.log([api.enforce_range_compare_i32(1.9,1.1),api.enforce_range_compare_u64(10,-1),api.clamp_compare_i64(1,2**63,2**63),api.clamp_compare_i32(true,1.5,1),api.clamp_compare_u64(false,NaN,0)].join(':'))"
        ),
        ["true:false:9007199254740991:2:0"]
    );
}
#[test]
/** The real slow throw callback reports its own message and leaves the fast counter unchanged. */
fn throwing_callbacks_remain_actual_slow_calls() {
    assert_eq!(
        run(
            "const api=new d8.test.FastCAPI();let message;try{api.throw_no_fallback()}catch(error){message=error.message}console.log([message,api.slow_call_count(),api.fast_call_count()].join(':'))"
        ),
        ["Exception from slow callback:1:0"]
    );
}
#[test]
/** Native null pointers retain null identity without exposing or inventing a JS External representation. */
fn null_pointer_operations_are_genuine_and_nonnull_pointers_are_unsupported() {
    assert_eq!(
        run(
            "const api=new d8.test.FastCAPI();console.log([api.get_null_pointer(),api.pass_pointer(null),api.compare_pointers(null,null)].join(':'))"
        ),
        ["::true"]
    );
    let execution = common::load_with("new d8.test.FastCAPI().get_pointer()", &environment());
    assert!(
        common::failure(&execution)
            .unwrap()
            .contains("external native pointer")
    );
}
#[test]
/** Fuzzing changes only the primary native helper's pointer-return behavior, never assertions or callback counts. */
fn explicit_fuzzing_pointer_mode_returns_actual_null() {
    let env = PageEnvironment {
        conformance_fuzzing: true,
        ..environment()
    };
    let execution = common::load_with(
        "const api=new d8.test.FastCAPI();console.log([api.get_pointer()===null,api.slow_call_count(),api.fast_call_count()].join(':'))",
        &env,
    );
    assert!(execution.failed.is_empty());
    assert_eq!(common::console(&execution), ["true:1:0"]);
}
#[test]
/** Native receiver internal fields cannot be cloned into counterfeit receivers through either clone API. */
fn native_receiver_objects_are_not_ordinary_clonable_data() {
    assert_eq!(
        run(
            "const api=new d8.test.FastCAPI();let rejected=0;for(const clone of [structuredClone,d8.serializer.serialize])try{clone(api)}catch(error){rejected++}console.log(rejected)"
        ),
        ["2"]
    );
}
#[test]
/** Real stored counters and native brands survive snapshot validation and a later task. */
fn counters_survive_saved_state_and_timer_delivery() {
    let env = environment();
    let host = common::host();
    let sources=vec!["const api=new d8.test.FastCAPI();api.sum_int64_as_number(1,2);setTimeout(()=>{api.sum_int64_as_number(3,4);console.log(api.slow_call_count())},0)".into()];
    let execution = mars_v9::load_page(common::input(&sources, &host, &env)).unwrap();
    assert!(execution.failed.is_empty());
    let due = execution.effects.next_timer_ms.unwrap();
    let execution = mars_v9::run_timers(
        common::input(&sources, &host, &env),
        common::round_trip(execution.state),
        due,
        1,
    )
    .unwrap();
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::console(&execution), ["2"]);
}
#[test]
/** Native method descriptors keep the pinned prototype flags and reject function construction before executing callbacks. */
fn native_methods_reject_construction_without_incrementing_counts() {
    assert_eq!(
        run(
            "const api=new d8.test.FastCAPI();let rejected=0;for(const name of ['fast_call_count','slow_call_count','reset_counts'])try{new api[name]()}catch(error){if(error instanceof TypeError)rejected++}const descriptor=Object.getOwnPropertyDescriptor(d8.test.FastCAPI.prototype,'add_all');console.log([rejected,api.slow_call_count(),descriptor.enumerable,descriptor.writable,descriptor.configurable].join(':'))"
        ),
        ["3:0:true:true:true"]
    );
}
#[test]
/** Same-isolate contexts share native callback state while preserving distinct receiver prototypes. */
fn foreign_contexts_share_real_isolate_counters() {
    assert_eq!(
        run(
            "const a=new d8.test.FastCAPI();const r=Realm.create();const b=Realm.eval(r,'new d8.test.FastCAPI()');a.reset_counts();b.sum_int64_as_number(1,2);const branded=a.is_fast_c_api_object(b);console.log([branded,a.slow_call_count(),b.slow_call_count(),b instanceof d8.test.FastCAPI].join(':'))"
        ),
        ["true:2:2:false"]
    );
}
#[test]
/** Child worker isolates execute their own callbacks and cannot increment the parent's native state. */
fn worker_isolates_keep_native_counter_storage_separate() {
    let session = mars_v9::ConformanceWorkerSession::new();
    let env = PageEnvironment {
        conformance_workers: Some(session.host()),
        ..environment()
    };
    let execution = common::load_with(
        "const a=new d8.test.FastCAPI();a.sum_int64_as_number(1,2);const worker=new Worker('const b=new d8.test.FastCAPI();b.sum_int64_as_number(3,4);postMessage(b.slow_call_count())',{type:'string'});console.log([worker.getMessage(),a.slow_call_count()].join(':'));worker.terminateAndWait()",
        &env,
    );
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    session.finish().unwrap();
    assert_eq!(common::console(&execution), ["1:1"]);
}
