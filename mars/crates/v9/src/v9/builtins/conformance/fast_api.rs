/** Native d8 FastCAPI slow callbacks execute real operations and count only actual callback invocations. */
use super::*;
use crate::v9::value::ObjectId;
use crate::v9::vm::data::Class;
use crate::v9::vm::heap::{CONFIGURABLE, DEFAULT_ATTRS, ENUMERABLE, Key, Object, Prop, Slot};
use crate::v9::vm::realm::{NativeFn, Static, banks};
mod numbers;
mod operations;
mod state;
/** Native identities are immutable; all counters and receiver brands live in the actual isolate heap. */
struct Natives {
    namespace: ObjectId,
    constructor: ObjectId,
    leaf: ObjectId,
    prototype: ObjectId,
    leaf_prototype: ObjectId,
    supports: ObjectId,
}
static NATIVES: OnceLock<Natives> = OnceLock::new();
/** The namespace always exists, but FastCAPI constructors are exposed only by actual host metadata. */
pub(super) fn install(builder: &mut Builder, d8: ObjectId) {
    let namespace = builder.object(Some(builder.ids.object_prototype));
    builder.alias(d8, "test", namespace);
    let prototype = builder.object(Some(builder.ids.object_prototype));
    let constructor = builder.constructor("FastCAPI", 0, construct, prototype, None);
    let leaf_proto = builder.object(Some(builder.ids.object_prototype));
    let leaf = builder.constructor("LeafInterfaceType", 0, leaf, leaf_proto, None);
    let getter_proto = builder.object(Some(builder.ids.object_prototype));
    let supports = builder.constructor("", 0, supports, getter_proto, None);
    install_methods(builder, prototype);
    builder.accessor(
        prototype,
        "fast_attribute",
        Some(attribute_get),
        Some(attribute_set),
    );
    let _ = NATIVES.set(Natives {
        namespace,
        constructor,
        leaf,
        prototype,
        leaf_prototype: leaf_proto,
        supports,
    });
}
/** Method descriptors mirror FunctionTemplate prototype properties; callbacks themselves cannot be constructed. */
fn install_methods(builder: &mut Builder, prototype: ObjectId) {
    let methods: &[(&'static str, NativeFn)] = &[
        ("fast_call_count", fast_count),
        ("slow_call_count", slow_count),
        ("reset_counts", reset),
        ("is_fast_c_api_object", is_object),
        ("call_to_number", to_number),
        ("add_all", operations::add_all),
        ("add_all_no_options", operations::add_all),
        ("overloaded_add_all_32bit_int", operations::add_ints),
        ("overloaded_add_all_32bit_int_no_sig", operations::add_ints),
        ("sum_int64_as_number", numbers::sum_signed),
        ("sum_uint64_as_number", numbers::sum_unsigned),
        ("sum_int64_as_bigint", numbers::sum_signed_bigint),
        ("sum_uint64_as_bigint", numbers::sum_unsigned_bigint),
        ("clamp_compare_i32", numbers::clamp_i32),
        ("clamp_compare_u32", numbers::clamp_u32),
        ("clamp_compare_i64", numbers::clamp_i64),
        ("clamp_compare_u64", numbers::clamp_u64),
        ("enforce_range_compare_i32", numbers::enforce_i32),
        ("enforce_range_compare_u32", numbers::enforce_u32),
        ("enforce_range_compare_i64", numbers::enforce_i64),
        ("enforce_range_compare_u64", numbers::enforce_u64),
        ("get_pointer", operations::pointer),
        ("get_null_pointer", operations::null_pointer),
        ("compare_pointers", operations::compare_pointers),
        ("pass_pointer", operations::pass_pointer),
        ("assert_is_external", operations::assert_external),
        ("throw_no_fallback", operations::throw),
        ("test_wasm_memory", operations::memory),
    ];
    for (name, function) in methods {
        let function = builder.function(name, 1, *function);
        builder.put(
            prototype,
            Key::from(*name),
            Static::Value(JsValue::Object(function)),
            DEFAULT_ATTRS,
        );
    }
}
/** Every fresh context gets its own intrinsic constructor objects without sharing its visible namespace mutations. */
pub(super) fn expose(vm: &mut Vm, bank: u32) {
    if !vm.page.environment.conformance_expose_fast_api {
        return;
    }
    let natives = NATIVES.get().unwrap();
    let delta = bank * banks::BANK_STRIDE;
    vm.materialize(natives.namespace + delta)
        .expect("FastCAPI namespace is an installed intrinsic");
    vm.heap.put(
        natives.namespace + delta,
        Key::from("FastCAPI"),
        Prop::data(JsValue::Object(natives.constructor + delta)),
    );
    vm.heap.put(
        natives.namespace + delta,
        Key::from("LeafInterfaceType"),
        Prop::data(JsValue::Object(natives.leaf + delta)),
    );
}
/** Ordinary Error objects report the actual native callback's argument/receiver contract. */
fn error(vm: &mut Vm, message: &str) -> crate::v9::vm::Thrown {
    match vm.make_error(crate::v9::vm::error::ErrorKind::Error, message) {
        Ok(id) => crate::v9::vm::Thrown::Value(JsValue::Object(id)),
        Err(error) => error.into(),
    }
}
/** New targets select the real prototype; branding never relies on authored inherited properties. */
fn construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(target) = call.new_target else {
        return Err(error(vm, "FastCAPI helper must be constructed with new."));
    };
    let counter = state::root(vm)?;
    let prototype = vm.prototype_for(&target, vm.intrinsic_id(NATIVES.get().unwrap().prototype))?;
    let object = vm.alloc(Object::new(Class::Plain, Some(prototype)))?;
    state::put(vm, object, "fast-api-brand", JsValue::Object(counter));
    let getter = vm.intrinsic_id(NATIVES.get().unwrap().supports);
    vm.heap.put(
        object,
        Key::from("supports_fp_params"),
        Prop {
            slot: Slot::Accessor(Some(getter), None),
            attrs: CONFIGURABLE | ENUMERABLE,
        },
    );
    Ok(JsValue::Object(object))
}
/** The leaf interface has no native operations; its independent prototype remains distinct from FastCAPI. */
fn leaf(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(target) = call.new_target else {
        return Err(error(
            vm,
            "LeafInterfaceType helper must be constructed with new.",
        ));
    };
    let prototype = vm.prototype_for(
        &target,
        vm.intrinsic_id(NATIVES.get().unwrap().leaf_prototype),
    )?;
    vm.alloc(Object::new(Class::Plain, Some(prototype)))
        .map(JsValue::Object)
}
/** V9 invokes real slow native callbacks and has no optimized C-linkage floating-point path. */
fn supports(vm: &mut Vm, call: Call) -> Res<JsValue> {
    state::receiver(vm, &call.this)?;
    Ok(JsValue::Boolean(false))
}
fn fast_count(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let state = state::receiver(vm, &call.this)?;
    Ok(JsValue::Number(state::number(vm, state, "fast")?))
}
fn slow_count(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let state = state::receiver(vm, &call.this)?;
    Ok(JsValue::Number(state::number(vm, state, "slow")?))
}
fn reset(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let state = state::receiver(vm, &call.this)?;
    state::put(vm, state, "fast", JsValue::Number(0.0));
    state::put(vm, state, "slow", JsValue::Number(0.0));
    Ok(JsValue::Undefined)
}
/** This callback examines only its first actual argument and invokes no user conversion hooks. */
fn is_object(vm: &mut Vm, call: Call) -> Res<JsValue> {
    state::enter(vm, &call.this)?;
    if call.args.is_empty() {
        return Err(error(
            vm,
            "is_valid_api_object should be called with an argument",
        ));
    }
    Ok(JsValue::Boolean(state::branded(vm, &call.arg(0))))
}
/** The real native slow callback performs Number conversion and propagates its original thrown value. */
fn to_number(vm: &mut Vm, call: Call) -> Res<JsValue> {
    state::enter(vm, &call.this)?;
    if call.args.is_empty() {
        return Err(error(
            vm,
            "is_valid_api_object should be called with an argument",
        ));
    }
    vm.to_number(&call.arg(0))?;
    Ok(JsValue::Undefined)
}
fn attribute_get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let state = state::enter(vm, &call.this)?;
    Ok(JsValue::Number(state::number(vm, state, "attribute")?))
}
/** Attribute writes require primitive Numbers in int32 range and truncate only after successful validation. */
fn attribute_set(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let state = state::enter(vm, &call.this)?;
    let JsValue::Number(value) = call.arg(0) else {
        return Err(error(vm, "The attribute requires a number as a new value"));
    };
    if !numbers::in_range(value, true, 32) {
        return Err(error(
            vm,
            "New value of attribute is not within int32 range",
        ));
    }
    state::put(vm, state, "attribute", JsValue::Number(value.trunc()));
    Ok(JsValue::Undefined)
}
