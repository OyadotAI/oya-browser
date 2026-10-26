/** The opt-in d8 CPU profiler records real cooperative samples and exports bounded actual call trees. */
mod model;
mod report;
mod sampling;
mod validation;
use crate::protocol::EngineError;
use crate::v9::value::JsValue;
use crate::v9::vm::heap::{DEFAULT_ATTRS, WRITABLE};
use crate::v9::vm::{Call, Res, Thrown, Vm, realm::Builder};
pub(in crate::v9) use model::State;
pub(in crate::v9) use sampling::tick;
use std::time::Instant;
pub(in crate::v9) use validation::{validate, validate_state};

/** Native anchors are transient; serialized records contain inert measured timestamps only. */
pub(in crate::v9) struct Clock {
    anchor: Instant,
    epoch: u64,
}

/** Native methods share isolate state across realms, without granting a filesystem or browser capability. */
pub(super) fn install(builder: &mut Builder, d8: crate::v9::value::ObjectId) {
    let profiler = builder.object(Some(builder.ids.object_prototype));
    let methods: [(&str, crate::v9::vm::realm::NativeFn); 2] = [
        ("setOnProfileEndListener", set_listener),
        ("triggerSample", trigger),
    ];
    for (name, method) in methods {
        let prototype = builder.object(Some(builder.ids.object_prototype));
        let function = builder.constructor(name, 0, method, prototype, None);
        builder.value(function, "prototype", JsValue::Object(prototype), WRITABLE);
        builder.value(profiler, name, JsValue::Object(function), DEFAULT_ATTRS);
    }
    builder.value(d8, "profiler", JsValue::Object(profiler), DEFAULT_ATTRS);
}

/** The shell ignores profile labels, including getters and conversions on passed arguments. */
pub(in crate::v9) fn console(vm: &mut Vm, method: &str) -> Res<JsValue> {
    if method == "profile" {
        if vm.page.state.cpu_profile.session.is_none() {
            vm.page.state.cpu_profile.session = Some(model::Profile::new(vm.profile_clock.now()));
        }
        sampling::sample(vm)?;
    } else {
        end(vm)?;
    }
    Ok(JsValue::Undefined)
}

/** A real function and its creation realm survive replacement, GC and committed-state transport. */
fn set_listener(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let receiver = super::files::receiver(vm, call.new_target.clone())?;
    let listener = call.arg(0);
    if !vm.is_callable(&listener) {
        let error = vm.make_error(
            crate::v9::vm::error::ErrorKind::Error,
            "The OnProfileEnd listener has to be a function",
        )?;
        return Err(Thrown::Value(JsValue::Object(error)));
    }
    vm.page.state.cpu_profile.listener = listener.object();
    vm.page.state.cpu_profile.context = vm.realms.contexts[vm.active_realm as usize].global;
    Ok(receiver)
}

/** Explicit samples have no effect before the first profiling session, as with d8's absent profiler. */
fn trigger(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let receiver = super::files::receiver(vm, call.new_target.clone())?;
    sampling::sample(vm)?;
    Ok(receiver)
}

/** Session disposal precedes callbacks, so recursive start/stop is real work under the existing call budget. */
fn end(vm: &mut Vm) -> Res<()> {
    let Some(mut profile) = vm.page.state.cpu_profile.session.take() else {
        return Ok(());
    };
    profile.end_time = vm.profile_clock.now().max(profile.end_time);
    let json = serde_json::to_string(&profile)
        .map_err(|_| EngineError::invalid("Cannot encode CPU profile."))?;
    if json.len() > vm.limits.state_bytes {
        return Err(EngineError::limit("V9 CPU profile bytes", vm.limits.state_bytes).into());
    }
    if let Some(listener) = vm.page.state.cpu_profile.listener {
        let value = crate::v9::builtins::text(vm, &json)?;
        let context = vm
            .page
            .state
            .cpu_profile
            .context
            .ok_or_else(|| EngineError::invalid("Missing profiler listener context."))?;
        let bank = vm.object_realm(context);
        vm.with_realm(bank, |vm| {
            match vm.call(JsValue::Object(listener), JsValue::Undefined, vec![value]) {
                Ok(_) => Ok(()),
                Err(Thrown::Value(error)) => report::exception(vm, error),
                Err(fatal) => Err(fatal),
            }
        })?;
    } else {
        if let Some(host) = &vm.page.environment.conformance_workers {
            host.record_cpu_profile(vm.page.environment.conformance_worker_id, json)?;
            return Ok(());
        }
        let used: usize = vm
            .page
            .effects
            .conformance_cpu_profiles
            .iter()
            .map(String::len)
            .sum();
        if used.saturating_add(json.len()) > vm.limits.state_bytes
            || vm.page.effects.conformance_cpu_profiles.len()
                >= crate::constants::VM_CONSOLE_ENTRIES
        {
            return Err(EngineError::limit("V9 CPU profile bytes", vm.limits.state_bytes).into());
        }
        vm.page.effects.conformance_cpu_profiles.push(json);
        bounds(vm)?;
    }
    Ok(())
}

/** Safepoint collection never silently truncates data or widens the engine's existing retained-data quota. */
fn bounds(vm: &Vm) -> Res<()> {
    let state = &vm.page.state.cpu_profile;
    let output_units = vm
        .page
        .effects
        .conformance_cpu_profiles
        .iter()
        .map(|json| json.capacity().div_ceil(std::mem::size_of::<u16>()))
        .sum::<usize>();
    if state
        .units()
        .saturating_add(vm.heap.counts.units)
        .saturating_add(output_units)
        > vm.limits.units
    {
        return Err(EngineError::limit("V9 environment UTF-16 units", vm.limits.units).into());
    }
    if let Some(profile) = &state.session {
        if profile.samples.len() > vm.limits.array_length {
            return Err(
                EngineError::limit("V9 resident CPU samples", vm.limits.array_length).into(),
            );
        }
        if profile.nodes.len() > vm.limits.properties {
            return Err(EngineError::limit("V9 CPU profile nodes", vm.limits.properties).into());
        }
    }
    Ok(())
}
