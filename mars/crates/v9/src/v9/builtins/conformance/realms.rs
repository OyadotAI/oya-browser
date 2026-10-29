/** d8 Realm operations use real context-owned globals, intrinsics and creation identities. */
use crate::protocol::EngineError;
use crate::v9::builtins::dynamic_base;
use crate::v9::value::JsValue;
use crate::v9::vm::data::Class;
use crate::v9::vm::heap::Object;
use crate::v9::vm::realm::{Builder, NativeFn, banks};
use crate::v9::vm::realms::Context;
use crate::v9::vm::{Call, Res, Vm};

/** The host object is distinct in each context; its shared channel is intentionally isolate-wide. */
pub(super) fn install(builder: &mut Builder) {
    let host = builder.object(Some(builder.ids.object_prototype));
    let methods: [(&'static str, u32, NativeFn); 11] = [
        ("current", 0, current),
        ("owner", 1, owner),
        ("global", 1, global),
        ("create", 0, create),
        ("createAllowCrossRealmAccess", 0, create_shared),
        ("eval", 2, eval),
        ("dispose", 1, dispose),
        ("detachGlobal", 1, detach),
        ("navigate", 1, navigate),
        ("navigateSameOrigin", 1, navigate_same),
        ("switch", 1, switch),
    ];
    for (name, length, method) in methods {
        builder.method(host, name, length, method);
    }
    builder.builtin_accessor(host, "shared", Some(shared_get), Some(shared_set));
    builder.global("Realm", host);
}
/** Read an exact live public Realm index without coercing arbitrary objects. */
fn index(vm: &mut Vm, call: &Call) -> Res<usize> {
    let Some(number) = call.arg(0).number_value() else {
        return Err(vm.type_error("Invalid realm index"));
    };
    if !number.is_finite()
        || number < 0.0
        || number.fract() != 0.0
        || vm
            .realms
            .slots
            .get(number as usize)
            .copied()
            .flatten()
            .is_none()
    {
        return Err(vm.type_error("Invalid realm index"));
    }
    Ok(number as usize)
}
/** Explicitly entered contexts, rather than imported functions, determine Realm.current. */
fn current(vm: &mut Vm, _: Call) -> Res<JsValue> {
    let bank = *vm.realm_entries.last().unwrap_or(&0);
    Ok(vm
        .realms
        .slots
        .iter()
        .position(|entry| *entry == Some(bank))
        .map_or(JsValue::Undefined, |index| JsValue::Number(index as f64)))
}
/** Creation identity follows both ordinary objects and immutable intrinsic banks. */
fn owner(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(id) = call.arg(0).object() else {
        return Err(vm.type_error("Invalid argument"));
    };
    let bank = vm.object_realm(id);
    if vm
        .realms
        .contexts
        .get(bank as usize)
        .is_some_and(|context| context.global == Some(id) && context.detached)
    {
        return Ok(JsValue::Undefined);
    }
    Ok(vm
        .realms
        .slots
        .iter()
        .position(|entry| *entry == Some(bank))
        .map_or(JsValue::Undefined, |index| JsValue::Number(index as f64)))
}
/** A returned global is a context-owned identity; cross-origin property operations still refuse. */
fn global(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let index = index(vm, &call)?;
    let bank = vm.realms.slots[index].expect("validated");
    Ok(JsValue::Object(
        vm.realms.contexts[bank as usize].global.expect("live"),
    ))
}
/** A fresh context receives a distinct security token. */
fn create(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let options = call.arg(0);
    let own_queue = if matches!(options, JsValue::Object(_)) {
        vm.get(
            &options,
            &crate::v9::vm::heap::Key::from("create_own_microtask_queue"),
        )?
        .truthy()
    } else {
        false
    };
    let index = create_context(vm, None, None)?;
    let bank = vm.realms.slots[index].expect("new realm");
    vm.realms.contexts[bank as usize].own_microtask_queue = own_queue;
    Ok(JsValue::Number(index as f64))
}
/** A same-origin context shares the entered context's security token, never its globals. */
fn create_shared(vm: &mut Vm, _: Call) -> Res<JsValue> {
    let bank = *vm.realm_entries.last().unwrap_or(&0);
    create_context(vm, Some(vm.realms.contexts[bank as usize].token), None)
        .map(|index| JsValue::Number(index as f64))
}
/** Each creation gets a new intrinsic bank even when replacing a public navigation slot. */
fn create_context(vm: &mut Vm, token: Option<u32>, slot: Option<usize>) -> Res<usize> {
    let bank = create_bank(vm, token)?;
    let slot = slot.unwrap_or(vm.realms.slots.len());
    if slot == vm.realms.slots.len() {
        vm.realms.slots.push(Some(bank));
    } else {
        vm.realms.slots[slot] = Some(bank);
    }
    Ok(slot)
}
/** Private ShadowRealm contexts share the exact isolation and quota path without public d8 slots. */
pub(super) fn create_bank(vm: &mut Vm, token: Option<u32>) -> Res<u32> {
    let bank = vm.realms.contexts.len() as u32;
    if bank >= banks::REALM_BANKS {
        return Err(EngineError::limit("V9 realm contexts", banks::REALM_BANKS as usize).into());
    }
    let mut object = Object::new(
        Class::Plain,
        Some(banks::bank(bank).ids.shell_global_prototype),
    );
    object.realm = bank;
    object.global = true;
    let global = vm.alloc(object)?;
    vm.realms.contexts.push(Context {
        global: Some(global),
        globals: Default::default(),
        own_microtask_queue: false,
        microtask_queue_closed: false,
        custom_jobs: Vec::new(),
        modules: Default::default(),
        token: token.unwrap_or(bank),
        detached: false,
        shadow: false,
    });
    vm.seed_realm_global(global, bank);
    Ok(bank)
}
/** Parse a global Script outside the target context, then run it inside the explicit entry. */
fn eval(vm: &mut Vm, call: Call) -> Res<JsValue> {
    eval_in_context(vm, call)
}
/** Compilation still precedes the explicit target entry; the API function retains its own creation realm. */
fn eval_in_context(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let slot = index(vm, &call)?;
    let source = read_source(vm, &call)?;
    let _ = index(vm, &call)?;
    let bank = vm.realms.slots[slot].expect("validated");
    let code = script(vm, source, bank)?;
    vm.realm_entries.push(bank);
    let result = vm.with_realm(bank, |vm| vm.run_script(code));
    vm.realm_entries.pop();
    result
}
/** The d8 options bag is read before validating the index again, so getters may revoke a realm. */
fn read_source(vm: &mut Vm, call: &Call) -> Res<String> {
    let mut valid = true;
    if let JsValue::Object(_) = call.arg(2) {
        let options = call.arg(2);
        valid = matches!(vm.get(&options,&crate::v9::vm::heap::Key::from("type"))?,JsValue::String(kind) if kind.lossy()=="string");
        if valid {
            let _ = vm.get(&options, &crate::v9::vm::heap::Key::from("arguments"))?;
        }
    }
    let _ = index(vm, call)?;
    match call.arg(1) {
        JsValue::String(source) if valid => Ok(source.lossy()),
        _ => Err(vm.type_error("Invalid argument")),
    }
}
/** Retained Script text uses the same dynamic-source quotas and registry validation as eval. */
pub(super) fn script(
    vm: &mut Vm,
    source: String,
    bank: u32,
) -> Res<std::sync::Arc<crate::v9::compile::Code>> {
    vm.prepare_dynamic(source.len())?;
    let native = vm.native_dynamic();
    let base = dynamic_base(&vm.dynamic);
    let code = crate::v9::compile_dynamic(&mut vm.registry, &source, base, None, native).map_err(
        |error| {
            if error.code == "script_syntax_error" {
                vm.syntax_error(&error.message)
            } else {
                error.into()
            }
        },
    )?;
    vm.realms.scripts.push((vm.dynamic.len(), bank));
    vm.dynamic.push(crate::v9::vm::DynamicSource {
        source,
        context: None,
        base: Some(base),
        origin: None,
    });
    Ok(code)
}
/** Disposing or detaching an entered, main or pending interactive context is forbidden. */
fn restricted(vm: &mut Vm, call: &Call) -> Res<(usize, u32)> {
    let slot = index(vm, call)?;
    let bank = vm.realms.slots[slot].expect("validated");
    if slot == 0 || slot == vm.realms.next as usize || vm.realm_entries.contains(&bank) {
        return Err(vm.type_error("Invalid realm index"));
    }
    Ok((slot, bank))
}
/** Disposal removes the public root; existing object references keep their old context alive. */
fn dispose(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (slot, bank) = restricted(vm, &call)?;
    crate::v9::builtins::cancel_finalizers_for_realm(vm, bank);
    vm.realms.slots[slot] = None;
    vm.realms.contexts[bank as usize].detached = true;
    close_custom_queue(vm, bank);
    Ok(JsValue::Undefined)
}
/** Close and release only an independently owned microtask queue. */
fn close_custom_queue(vm: &mut Vm, bank: u32) {
    let context = &mut vm.realms.contexts[bank as usize];
    if context.own_microtask_queue {
        context.custom_jobs.clear();
        context.microtask_queue_closed = true;
    }
}
/** Detaching removes global-owner exposure without discarding the creation context. */
fn detach(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (_, bank) = restricted(vm, &call)?;
    vm.realms.contexts[bank as usize].detached = true;
    Ok(JsValue::Undefined)
}
/** Navigation creates fresh globals and intrinsics while old references retain their detached context. */
fn navigate(vm: &mut Vm, call: Call) -> Res<JsValue> {
    navigate_context(vm, call, false)
}
/** Same-origin navigation keeps the security token while replacing the executing context. */
fn navigate_same(vm: &mut Vm, call: Call) -> Res<JsValue> {
    navigate_context(vm, call, true)
}
/** The public index survives replacement; context bank identities never get reused. */
fn navigate_context(vm: &mut Vm, call: Call, same: bool) -> Res<JsValue> {
    let (slot, bank) = restricted(vm, &call)?;
    let token = same.then_some(vm.realms.contexts[bank as usize].token);
    vm.realms.contexts[bank as usize].detached = true;
    close_custom_queue(vm, bank);
    create_context(vm, token, Some(slot))?;
    Ok(JsValue::Undefined)
}
/** Interactive selection is saved separately and cannot mutate the currently running Script. */
fn switch(vm: &mut Vm, call: Call) -> Res<JsValue> {
    vm.realms.next = index(vm, &call)? as u32;
    Ok(JsValue::Undefined)
}
/** The isolate's explicit sharing channel is the same data value from every context. */
fn shared_get(vm: &mut Vm, _: Call) -> Res<JsValue> {
    Ok(vm.realms.shared.clone())
}
/** Shared references participate in ordinary graph validation and precise collection. */
fn shared_set(vm: &mut Vm, call: Call) -> Res<JsValue> {
    vm.realms.shared = call.arg(0);
    Ok(JsValue::Undefined)
}
