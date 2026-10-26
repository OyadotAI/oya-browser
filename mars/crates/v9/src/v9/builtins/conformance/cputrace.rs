/** The explicitly named CPU marker validates native uint32 values and executes the pinned architecture's trace instruction. */
use super::*;
use crate::constants::VM_CONSOLE_ENTRIES;
use crate::protocol::{EngineError, EngineResult};
use crate::v9::platform::{CpuTraceMark, PageEnvironment};
use crate::v9::value::{JsString, ObjectId};
use crate::v9::vm::Thrown;
use crate::v9::vm::error::ErrorKind;
use crate::v9::vm::heap::{CONFIGURABLE, Key, Prop};
use crate::v9::vm::realm::banks;

/** Immutable function identities are banked; mutable exposure names stay in each owning VM heap. */
static NATIVE: OnceLock<ObjectId> = OnceLock::new();
/** Pinned CpuTraceMarkExtension encodes the marker in the high sixteen EAX bits. */
const MARKER_SHIFT: u32 = 16;
/** Tracers recognize this low EAX signature at the actual CPUID instruction. */
const MARKER_SIGNATURE: u32 = 0x4711;

/** Register the native callback without exposing it in globals unless the actual shell flag requested a name. */
pub(super) fn install(builder: &mut Builder) {
    let _ = NATIVE.set(builder.function("cputracemark", 0, mark));
}

/** Host metadata names exactly one valid native-function identifier, never a fragment that executes JavaScript. */
fn requested_name(environment: &PageEnvironment) -> EngineResult<Option<JsString>> {
    let Some(name) = environment
        .conformance_cpu_trace_name
        .as_deref()
        .filter(|name| !name.is_empty())
    else {
        return Ok(None);
    };
    let tokens = crate::v9::lexer::tokenize(name)?;
    let [
        crate::v9::lexer::Token {
            kind: crate::v9::lexer::Kind::Name(decoded),
            ..
        },
        crate::v9::lexer::Token {
            kind: crate::v9::lexer::Kind::End,
            ..
        },
    ] = tokens.as_slice()
    else {
        return Err(EngineError::invalid(
            "Invalid CPU trace marker function name.",
        ));
    };
    crate::v9::syntax::parse(crate::v9::lexer::tokenize(&format!(
        "function {name}(){{}}"
    ))?)?;
    JsString::from_utf8(decoded).map(Some)
}

/** Public request admission rejects malformed native extension metadata before any script can execute. */
pub(in crate::v9) fn validate(environment: &PageEnvironment) -> EngineResult<()> {
    requested_name(environment).map(|_| ())
}

/** Each fresh realm receives its own native function object under the requested decoded identifier. */
pub(super) fn expose(vm: &mut Vm, global: ObjectId, bank: u32) {
    let Ok(Some(name)) = requested_name(&vm.page.environment) else {
        return;
    };
    let id = *NATIVE.get().unwrap() + bank * banks::BANK_STRIDE;
    vm.materialize(id)
        .expect("installed CPU trace marker intrinsic");
    vm.heap.put(
        id,
        Key::from("name"),
        Prop {
            slot: crate::v9::vm::heap::Slot::Data(JsValue::String(name.clone())),
            attrs: CONFIGURABLE,
        },
    );
    vm.heap.put(
        id,
        Key::private("native-function-source-name"),
        Prop::data(JsValue::String(name.clone())),
    );
    vm.heap
        .put(global, Key::Str(name), Prop::data(JsValue::Object(id)));
}

/** IsUint32 does not coerce strings, boxed numbers, BigInts or objects; extra arguments are ignored. */
fn mark(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let JsValue::Number(marker) = call.arg(0) else {
        return Err(argument_error(vm));
    };
    if !marker.is_finite() || marker < 0.0 || marker > f64::from(u32::MAX) || marker.fract() != 0.0
    {
        return Err(argument_error(vm));
    }
    if !vm.page.state.clock.is_finite() {
        return Err(EngineError::invalid("Invalid CPU trace marker clock.").into());
    }
    vm.charge(1)?;
    let marker = marker as u32;
    instruction(marker);
    if vm.page.effects.console.len() + vm.page.effects.conformance_trace_marks.len()
        < VM_CONSOLE_ENTRIES
    {
        vm.page.effects.conformance_trace_marks.push(CpuTraceMark {
            marker,
            time_ms: vm.page.state.clock,
        });
    } else {
        vm.page.effects.conformance_trace_marks_dropped = vm
            .page
            .effects
            .conformance_trace_marks_dropped
            .saturating_add(1);
    }
    Ok(JsValue::Undefined)
}

/** The native extension throws an ordinary Error rather than a coercion TypeError. */
fn argument_error(vm: &mut Vm) -> Thrown {
    match vm.make_error(
        ErrorKind::Error,
        "First parameter to cputracemark() must be a unsigned int32.",
    ) {
        Ok(id) => Thrown::Value(JsValue::Object(id)),
        Err(error) => error.into(),
    }
}

/** X86 executes the actual CPUID trace marker; other architectures match the primary extension's validation-only path. */
fn instruction(marker: u32) {
    let leaf = MARKER_SIGNATURE | marker.wrapping_shl(MARKER_SHIFT);
    #[cfg(target_arch = "x86_64")]
    // SAFETY: CPUID is available on the x86_64 baseline and reads no process memory or pointers.
    unsafe {
        let _ = std::arch::x86_64::__cpuid(leaf);
    }
    #[cfg(target_arch = "x86")]
    // SAFETY: Rust's supported x86 baseline supplies CPUID; the intrinsic preserves the ABI registers.
    unsafe {
        let _ = std::arch::x86::__cpuid(leaf);
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "x86")))]
    let _ = leaf;
}
