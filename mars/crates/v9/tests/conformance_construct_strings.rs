/* V8 string construction intrinsics create real retained cons and slice backing. */
#![cfg(feature = "conformance")]
use crate::common;

/** Invalid types, lengths and arities retain V8's undefined result and argument evaluation. */
#[test]
fn constructor_intrinsics_validate_inputs_and_evaluate_extra_arguments() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_fuzzing = true;
    environment.conformance_native_main_source = Some(0);
    let source = "let n=0;const a=%ConstructConsString('Concatenated','String'),b=%ConstructSlicedString('StringLargeEnoughForSlicing',6);const r=a==='ConcatenatedString'&&b==='LargeEnoughForSlicing'&&%ConstructConsString('Concatenated','String',n++)===undefined&&%ConstructSlicedString('StringLargeEnoughForSlicing',6,n++)===undefined&&n===2&&%ConstructConsString('x')===undefined&&%ConstructConsString('a','b')===undefined&&%ConstructSlicedString('abcd',2)===undefined&&%ConstructSlicedString('StringLargeEnoughForSlicing',-1)===undefined&&%ConstructSlicedString('StringLargeEnoughForSlicing',-0)===undefined&&%ConstructSlicedString('StringLargeEnoughForSlicing',1.5)===undefined;";
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "true");
}

/** Invalid V8 CHECK preconditions remain explicit unsupported outside fuzzing mode. */
#[test]
fn invalid_constructor_preconditions_require_fuzzing() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    for source in [
        "%ConstructConsString('a','b');",
        "%ConstructConsString('a');",
        "%ConstructSlicedString('StringLargeEnoughForSlicing',-0);",
    ] {
        let execution = common::load_with(source, &environment);
        assert!(!execution.failed.is_empty());
    }
}

/** Flattening a cons returns its stable flat child and preserves its text. */
#[test]
fn flatten_string_returns_stable_flat_child() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let source = "const s=%ConstructConsString('Concatenated','String'),a=%FlattenString(s),b=%FlattenString(s);const r=a==='ConcatenatedString'&&%IsSameHeapObject(a,b)&&!%IsSameHeapObject(s,a);";
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "true");
}

/** String generation reflects the tracked backing before and after an actual full collection. */
#[test]
fn young_generation_uses_real_string_gc_state() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let source = "const s=['fresh',' string'].join(''),o={},a=[];const before=%InYoungGeneration(s)&&%InYoungGeneration(o)&&%InYoungGeneration(a)&&!%InYoungGeneration(3)&&!%InYoungGeneration(true)&&!%InYoungGeneration(null);gc();const r=before&&!%InYoungGeneration(s)&&!%InYoungGeneration(o)&&!%InYoungGeneration(a);";
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "true");
    let tracked = common::load_with("const r=%InYoungGeneration(1.5);", &environment);
    assert_eq!(common::global(&tracked, "r"), "false");
    let unsupported = common::load_with("%InYoungGeneration(Math.sqrt(2));", &environment);
    assert!(!unsupported.failed.is_empty());
}

/** Native grammar remains unavailable unless this source has the explicit conformance capability. */
#[test]
fn constructor_intrinsics_require_native_grammar_capability() {
    let execution = common::load_with(
        "%ConstructConsString('abcdefgh','ijklmnop');",
        &common::environment(),
    );
    assert!(!execution.failed.is_empty());
}
