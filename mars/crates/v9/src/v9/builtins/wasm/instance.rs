/** Linking and instance exports retain only typed inert records and traced native captures. */
use super::*;
use types::{Constant, External};

/** A native export's two captures identify an instance and a validated function index. */
pub(super) fn is_function(vm: &Vm, value: &JsValue) -> bool {
    let Some(id) = value.object() else {
        return false;
    };
    matches!(vm.heap.get(id).and_then(|o|o.function()).map(|f|&f.kind),Some(crate::v9::vm::data::FunctionKind::Native{id,slots})if *id==ids(vm).invoke&&slots.len()==2)
}
/** Internal index spaces are ordinary arrays behind inaccessible keys. */
fn store_list(vm: &mut Vm, id: ObjectId, name: &str, values: Vec<JsValue>) -> Res<()> {
    let list = vm.new_array(values)?;
    set(vm, id, name, JsValue::Object(list));
    Ok(())
}
/** Every indexed lookup is checked even though the binary itself has already validated indices. */
pub(super) fn item(vm: &mut Vm, id: ObjectId, name: &str, index: u32) -> Res<JsValue> {
    let storage = get(vm, id, name)
        .and_then(|v| v.object())
        .ok_or_else(|| vm.type_error("invalid WebAssembly instance index space"))?;
    let value = match vm.heap.get(storage).map(|o| &o.class) {
        Some(Class::Array(values)) => values.get(index as usize).cloned(),
        _ => None,
    };
    value.ok_or_else(|| vm.type_error("invalid WebAssembly instance index"))
}
/** Imported getters execute once in declaration order; their own thrown values pass through. */
fn import(vm: &mut Vm, imports: &JsValue, declaration: &types::Import) -> Res<JsValue> {
    if !matches!(imports, JsValue::Object(_)) {
        return Err(vm.type_error("WebAssembly imports must be an object"));
    }
    let namespace = vm.get(imports, &key(&declaration.module))?;
    if !matches!(namespace, JsValue::Object(_)) {
        return Err(link(vm, "WebAssembly import module must be an object"));
    }
    vm.get(&namespace, &key(&declaration.name))
}
/** Linker validates kind, type and bounds before accepting a live imported object. */
fn resolve_imports(
    vm: &mut Vm,
    module_id: ObjectId,
    m: &Module,
    imports: &JsValue,
) -> Res<[Vec<JsValue>; 5]> {
    let mut spaces: [Vec<JsValue>; 5] = std::array::from_fn(|_| Vec::new());
    for declaration in &m.imports {
        let value = match imports::resolve(vm, module_id, m, declaration)? {
            Some(value) => value,
            None => import(vm, imports, declaration)?,
        };
        match declaration.kind {
            External::Function(ty) => {
                if !vm.is_callable(&value) && !jspi::is_suspending(vm, &value) {
                    return Err(link(vm, "WebAssembly function import is not callable"));
                }
                if is_function(vm, &value) {
                    if !function_matches(vm, m, ty, &value)? {
                        return Err(link(vm, "WebAssembly imported function type mismatch"));
                    }
                }
                spaces[0].push(value)
            }
            External::Tag(ty) => {
                if !exceptions::compatible(vm, m, ty, &value)? {
                    return Err(link(vm, "imported tag does not match the expected type"));
                }
                spaces[4].push(value)
            }
            External::Table(ty, limits) => {
                if !table::compatible(vm, &value, ty, limits, m)? {
                    return Err(link(
                        vm,
                        "WebAssembly imported table type or limits mismatch",
                    ));
                }
                spaces[1].push(value)
            }
            External::Memory(limits) => {
                if !memory::compatible(vm, &value, limits)? {
                    return Err(link(vm, "WebAssembly imported memory limits mismatch"));
                }
                spaces[2].push(value)
            }
            External::Global(ty, mutable) => {
                let value = if value
                    .object()
                    .is_some_and(|id| get(vm, id, GLOBAL).is_some())
                {
                    let (_, actual, changeable) = global::ty(vm, &value)?;
                    if changeable != mutable
                        || !type_context::equivalent(vm, value.object().unwrap(), actual, m, ty)?
                    {
                        return Err(link(vm, "WebAssembly imported global type mismatch"));
                    }
                    value
                } else {
                    if mutable {
                        return Err(link(
                            vm,
                            "mutable global import must be a WebAssembly.Global",
                        ));
                    }
                    let global = global::create(vm, ty, false, value, ids(vm).global)?;
                    type_context::bind(vm, global, module_id);
                    let _ = global::read(vm, &JsValue::Object(global))?;
                    JsValue::Object(global)
                };
                spaces[3].push(value)
            }
        }
    }
    Ok(spaces)
}

/** Instantiate the validated module, allocate index spaces, initialize segments, then start. */
pub(super) fn create(
    vm: &mut Vm,
    value: &JsValue,
    imports: &JsValue,
    target: Option<&JsValue>,
) -> Res<ObjectId> {
    let (module_id, m) = module(vm, value)?;
    let [
        mut functions,
        mut tables,
        mut memories,
        mut globals,
        mut tags,
    ] = resolve_imports(vm, module_id, &m, imports)?;
    let proto = match target {
        Some(t) => vm.prototype_for(t, ids(vm).instance)?,
        None => ids(vm).instance,
    };
    let instance = vm.alloc(Object::new(Class::Plain, Some(proto)))?;
    set(vm, instance, INSTANCE, JsValue::Object(module_id));
    for limits in &m.memories {
        memories.push(JsValue::Object(memory::create(
            vm,
            *limits,
            ids(vm).memory,
        )?))
    }
    for index in &m.tags {
        tags.push(exceptions::create(vm, module_id, *index)?)
    }
    store_list(vm, instance, exceptions::TAGS, tags)?;
    let imported = functions.clone();
    store_list(vm, instance, "wasm.imports", imported.clone())?;
    functions.clear();
    for index in 0..m.function_types().len() {
        if let Some(function) = imported.get(index).filter(|f| is_function(vm, f)) {
            functions.push(function.clone());
            continue;
        }
        let arity = m
            .signature(index as u32)
            .map_err(|e| compile_failure(vm, e))?
            .params
            .len();
        let function = vm.native_closure(
            ids(vm).invoke,
            vec![JsValue::Object(instance), JsValue::Number(index as f64)],
        )?;
        vm.define(
            function,
            key("name"),
            Descriptor {
                value: Some(JsValue::string(&index.to_string())?),
                configurable: Some(true),
                ..Descriptor::default()
            },
        )?;
        vm.define(
            function,
            key("length"),
            Descriptor {
                value: Some(JsValue::Number(arity as f64)),
                configurable: Some(true),
                ..Descriptor::default()
            },
        )?;
        functions.push(JsValue::Object(function))
    }
    store_list(vm, instance, FUNCTIONS, functions)?;
    store_list(vm, instance, TABLES, tables.clone())?;
    store_list(vm, instance, MEMORIES, memories)?;
    store_list(vm, instance, GLOBALS, globals.clone())?;
    for (ty, mutable, constant) in &m.globals {
        let value =
            constant_value(vm, instance, constant).map_err(|e| initialization_error(vm, e))?;
        let global = global::create(vm, *ty, *mutable, value, ids(vm).global)?;
        type_context::bind(vm, global, module_id);
        globals.push(JsValue::Object(global));
        store_list(vm, instance, GLOBALS, globals.clone())?
    }
    for (ty, limits, constant) in &m.tables {
        let value = match constant {
            Some(c) => constant_value(vm, instance, c)?,
            None => global::default(*ty),
        };
        let table = table::create(vm, *ty, *limits, value, ids(vm).table)?;
        type_context::bind(vm, table, module_id);
        tables.push(JsValue::Object(table));
    }
    store_list(vm, instance, TABLES, tables)?;
    segments::initialize(vm, instance, &m)?;
    initialize(vm, instance, &m)?;
    let exports = vm.alloc(Object::new(Class::Plain, None))?;
    for export in &m.exports {
        let space = match export.kind {
            0 => FUNCTIONS,
            1 => TABLES,
            2 => MEMORIES,
            4 => exceptions::TAGS,
            _ => GLOBALS,
        };
        let value = item(vm, instance, space, export.index)?;
        vm.define(
            exports,
            key(&export.name),
            Descriptor {
                value: Some(value),
                writable: Some(false),
                enumerable: Some(true),
                configurable: Some(false),
                ..Descriptor::default()
            },
        )?;
    }
    vm.heap.get_mut(exports).unwrap().extensible = false;
    set(vm, instance, "wasm.exports", JsValue::Object(exports));
    if let Some(start) = m.start {
        execute::call(vm, instance, &m, start, vec![])?;
    }
    Ok(instance)
}
/** JS constructor enforces new and accepts an already validated Module only. */
pub(super) fn construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if call.new_target.is_none() {
        return Err(vm.type_error("WebAssembly.Instance must be invoked with 'new'"));
    }
    create(vm, &call.arg(0), &call.arg(1), call.new_target.as_ref()).map(JsValue::Object)
}
/** Exports have a stable identity and are not reached through author property getters. */
pub(super) fn exports(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = brand(vm, &call.this, INSTANCE)?;
    get(vm, id, "wasm.exports").ok_or_else(|| vm.type_error("invalid WebAssembly exports record"))
}
/** Restore, revalidate and check both native captures before entering the interpreter. */
pub(super) fn target(vm: &mut Vm, value: &JsValue) -> Res<(ObjectId, Module, u32)> {
    if !is_function(vm, value) {
        return Err(vm.type_error("expected a WebAssembly function"));
    }
    let captures = vm.slots(value.object().unwrap());
    let instance = brand(vm, &captures[0], INSTANCE)?;
    let index = match captures[1] {
        JsValue::Number(n)
            if n.is_finite() && n >= 0.0 && n.fract() == 0.0 && n <= f64::from(u32::MAX) =>
        {
            n as u32
        }
        _ => return Err(vm.type_error("invalid WebAssembly function index")),
    };
    let record = get(vm, instance, INSTANCE).unwrap();
    let (_, module) = module(vm, &record)?;
    module
        .signature(index)
        .map_err(|e| compile_failure(vm, e))?;
    Ok((instance, module, index))
}
/** Canonical signatures compare recursive graphs from each function's actual owning module. */
pub(super) fn function_matches(
    vm: &mut Vm,
    expected: &Module,
    ty: u32,
    value: &JsValue,
) -> Res<bool> {
    let (_, actual, index) = target(vm, value)?;
    let actual_type = actual
        .function_type(index)
        .map_err(|e| compile_failure(vm, e))?;
    Ok(gc_types::equivalent(&actual, actual_type, expected, ty))
}

/** Wasm-to-Wasm imports and indirect calls retain v128 operands without a JS boundary crossing. */
pub(super) fn call_typed(vm: &mut Vm, value: &JsValue, args: Vec<JsValue>) -> Res<Vec<JsValue>> {
    let (instance, module, index) = target(vm, value)?;
    execute::call(vm, instance, &module, index, args)
}
/** A JS-callable export performs boundary coercions once and returns multi-values as an array. */
pub(super) fn invoke(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (instance, m, index) = target(vm, &JsValue::Object(call.callee))?;
    let sig = m.signature(index).map_err(|e| compile_failure(vm, e))?;
    if sig
        .params
        .iter()
        .chain(&sig.results)
        .any(|t| !t.js_compatible())
    {
        return Err(vm.type_error(
            "type incompatibility when transforming from/to JS: unsupported value type",
        ));
    }
    let mut args = Vec::new();
    for (i, ty) in sig.params.iter().enumerate() {
        let value = global::convert(vm, *ty, &call.arg(i))?;
        if ty.reference().is_some() && !gc::matches(vm, &m, *ty, &value)? {
            return Err(vm.type_error("WebAssembly parameter reference type mismatch"));
        }
        args.push(value)
    }
    let result = execute::call(vm, instance, &m, index, args)?;
    match result.as_slice() {
        [] => Ok(JsValue::Undefined),
        [one] => Ok(one.clone()),
        _ => Ok(JsValue::Object(vm.new_array(result)?)),
    }
}
/** Constant expressions read real imported globals and instantiate real function references. */
pub(super) fn constant_value(vm: &mut Vm, instance: ObjectId, value: &Constant) -> Res<JsValue> {
    Ok(match value {
        Constant::Expression(code) => {
            let source = get(vm, instance, INSTANCE)
                .ok_or_else(|| vm.type_error("invalid Wasm constant instance"))?;
            let (_, m) = module(vm, &source)?;
            let mut stack = Vec::new();
            for instruction in code {
                vm.charge(1)?;
                match instruction {
                    types::Instruction::Constant(c) => stack.push(constant_value(vm, instance, c)?),
                    types::Instruction::Simple(op) => numeric::run(vm, *op, &mut stack)?,
                    types::Instruction::Gc(op, args) => {
                        gc::run(vm, instance, &m, *op, args, &mut stack)?
                    }
                    _ => return Err(vm.type_error("invalid Wasm constant instruction")),
                }
            }
            if stack.len() != 1 {
                return Err(vm.type_error("invalid Wasm constant result"));
            }
            stack.pop().unwrap()
        }
        Constant::I32(n) => JsValue::Number(f64::from(*n)),
        Constant::I64(n) => JsValue::BigInt(i128::from(*n)),
        Constant::F32(n) => JsValue::Number(numeric::promote_f32(n.to_bits())),
        Constant::F64(n) => JsValue::Number(*n),
        Constant::V128(bytes) => simd::value(*bytes)?,
        Constant::Null(_) => JsValue::Null,
        Constant::Function(i) => item(vm, instance, FUNCTIONS, *i)?,
        Constant::Global(i) => {
            let global = item(vm, instance, GLOBALS, *i)?;
            global::read(vm, &global)?
        }
    })
}
/** Active segments are range-checked in full before any bytes or table slots are changed. */
fn initialize(vm: &mut Vm, instance: ObjectId, m: &Module) -> Res<()> {
    let mut data = Vec::new();
    for segment in &m.data {
        if let (Some(memory), Some(offset)) = (segment.memory, &segment.offset) {
            let memory = item(vm, instance, MEMORIES, memory)?;
            let offset = constant_value(vm, instance, offset)?;
            let at = address::effective(vm, &memory, &offset, 0)?;
            if at
                .checked_add(segment.bytes.len())
                .is_none_or(|end| end > memory::length(vm, &memory).unwrap_or(0))
            {
                return Err(trap(vm, "data segment out of bounds"));
            }
            data.push((memory, at, &segment.bytes));
        }
    }
    let mut elements = Vec::new();
    for segment in &m.elements {
        if let (Some(table), Some(offset)) = (segment.table, &segment.offset) {
            let table = item(vm, instance, TABLES, table)?;
            let offset = constant_value(vm, instance, offset)?;
            let at = numeric::i32_value(&offset)? as u32 as usize;
            if at
                .checked_add(segment.values.len())
                .is_none_or(|end| end > table::length(vm, &table).unwrap_or(0))
            {
                return Err(trap(vm, "element segment is out of bounds"));
            }
            elements.push((table, at, &segment.values));
        }
    }
    for (memory, at, bytes) in data {
        vm.charge(bytes.len())?;
        memory::write(vm, &memory, at, bytes)?;
    }
    for (table, at, values) in elements {
        for (i, value) in values.iter().enumerate() {
            let value = constant_value(vm, instance, value)?;
            table::write(vm, &table, (at + i) as u32, &value)?;
        }
    }
    Ok(())
}

/** Native constant-initializer traps identify the construction API without rewriting author exceptions. */
fn initialization_error(vm: &mut Vm, error: Thrown) -> Thrown {
    if let Thrown::Value(JsValue::Object(id)) = &error
        && get(vm, *id, "wasm.trap").is_some()
        && get(vm, *id, "wasm.instance.prefix").is_none()
    {
        if let Some(JsValue::String(message)) = vm
            .heap
            .get(*id)
            .and_then(|o| o.props.get(&key("message")))
            .and_then(|p| match &p.slot {
                Slot::Data(v) => Some(v.clone()),
                _ => None,
            })
            && let Ok(value) =
                JsValue::string(&format!("WebAssembly.Instance(): {}", message.lossy()))
        {
            vm.heap.put(
                *id,
                key("message"),
                Prop {
                    slot: Slot::Data(value),
                    attrs: HIDDEN_ATTRS,
                },
            );
            set(vm, *id, "wasm.instance.prefix", JsValue::Boolean(true));
        }
    }
    error
}
