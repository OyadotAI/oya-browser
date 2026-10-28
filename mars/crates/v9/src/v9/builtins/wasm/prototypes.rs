/** js-prototypes imports decode actual setup bytes and install real receiver/constructor wrappers. */
use super::*;
/** V8 bounds property dictionaries before allocating them from untrusted setup bytes. */
const MAX_CONFIGURE_PROPERTIES: u32 = 100_000;

/** Method and constructor dispatchers retain only traced Wasm function captures. */
pub(super) fn install(b: &mut Builder) -> (u32, u32) {
    let method = b.native("", 0, invoke_method);
    let constructor = b.native("", 0, invoke_constructor);
    b.natives[constructor as usize].constructor = true;
    (method, constructor)
}
/** Methods prepend their actual JS receiver before ordinary typed Wasm argument conversion. */
fn invoke_method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let function = vm
        .slots(call.callee)
        .first()
        .cloned()
        .ok_or_else(|| vm.type_error("invalid Wasm method capture"))?;
    let mut args = vec![call.this];
    args.extend(call.args.iter().cloned());
    vm.call(function, JsValue::Undefined, args)
}
/** Constructor calls use actual object results, allocating a newTarget object only for primitive results. */
fn invoke_constructor(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let function = vm
        .slots(call.callee)
        .first()
        .cloned()
        .ok_or_else(|| vm.type_error("invalid Wasm constructor capture"))?;
    let value = vm.call(function, JsValue::Undefined, call.args.to_vec())?;
    if call.new_target.is_none() || value.object().is_some() {
        return Ok(value);
    }
    let proto = vm.prototype_for(
        call.new_target.as_ref().unwrap(),
        vm.intrinsic_id(realm().ids.object_prototype),
    )?;
    vm.alloc(Object::new(Class::Plain, Some(proto)))
        .map(JsValue::Object)
}
/** The data reader stays over an immutable copy while author proxy traps may mutate the input arrays. */
struct Setup<'a> {
    data: binary::Reader<'a>,
    prototypes: JsValue,
    functions: JsValue,
    prototype_index: usize,
    function_index: usize,
}
impl Setup<'_> {
    /** Invalid setup bytecode is a runtime trap, rather than a binary-module compile error. */
    fn integer(&mut self, vm: &mut Vm) -> Res<u32> {
        self.data.u32().map_err(|e| trap(vm, &e.message))
    }
    /** Property names are strict UTF8 byte strings with no authored conversion or property access. */
    fn name(&mut self, vm: &mut Vm) -> Res<String> {
        self.data.name().map_err(|e| trap(vm, &e.message))
    }
    /** Function reads are live so earlier author callbacks can replace subsequent array entries. */
    fn function(&mut self, vm: &mut Vm) -> Res<JsValue> {
        let function = gc::interop_read(vm, &self.functions, self.function_index)?;
        self.function_index += 1;
        if function == JsValue::Null {
            return Err(trap(vm, "null function"));
        }
        if !instance::is_function(vm, &function) {
            return Err(trap(vm, "illegal cast"));
        }
        vm.pin(function.clone());
        Ok(function)
    }
    /** Null is allowed until a requested operation actually needs an object receiver. */
    fn prototype(&mut self, vm: &mut Vm) -> Res<JsValue> {
        let value = gc::interop_read(vm, &self.prototypes, self.prototype_index)?;
        self.prototype_index += 1;
        #[cfg(feature = "conformance")]
        if value.object().is_some_and(|id| crate::v9::builtins::is_shared_object(vm, id)) {
            return Err(trap(vm, "illegal cast"));
        }
        if value.object().is_none() && value != JsValue::Null {
            return Err(prototype_error(vm, &value));
        }
        vm.pin(value.clone());
        Ok(value)
    }
    /** The same descriptor machinery as Object.defineProperty preserves Proxy traps and attributes. */
    fn member(&mut self, vm: &mut Vm, receiver: &JsValue, is_static: bool) -> Res<()> {
        let kind = self.data.byte().map_err(|e| trap(vm, &e.message))?;
        if kind > 2 {
            return Err(trap(vm, "invalid prototype method kind"));
        }
        let name = self.name(vm)?;
        let function = self.function(vm)?;
        let function = if is_static {
            function
        } else {
            JsValue::Object(vm.native_closure(ids(vm).prototype_method, vec![function])?)
        };
        let mut descriptor = Descriptor {
            enumerable: Some(false),
            configurable: Some(true),
            ..Descriptor::default()
        };
        match kind {
            0 => {
                descriptor.value = Some(function);
                descriptor.writable = Some(true)
            }
            1 => descriptor.get = Some(function.object()),
            _ => descriptor.set = Some(function.object()),
        };
        let receiver = receiver
            .object()
            .ok_or_else(|| vm.type_error("Cannot define a property on null"))?;
        define(vm, receiver, key(&name), descriptor)
    }
    /** A constructor is installed on both its prototype and the supplied constructor namespace. */
    fn constructor(
        &mut self,
        vm: &mut Vm,
        prototype: &JsValue,
        constructors: &JsValue,
    ) -> Res<JsValue> {
        if prototype == &JsValue::Null {
            return Err(vm.type_error("Cannot define constructor on null"));
        }
        let function = self.function(vm)?;
        let name = self.name(vm)?;
        let namespace = constructors
            .object()
            .ok_or_else(|| vm.type_error("constructor namespace must be an object"))?;
        let (_, m, index) = instance::target(vm, &function)?;
        let length = m
            .signature(index)
            .map_err(|e| compile_failure(vm, e))?
            .params
            .len();
        let wrapper = vm.native_closure(ids(vm).prototype_constructor, vec![function])?;
        define(
            vm,
            wrapper,
            key("name"),
            Descriptor {
                value: Some(JsValue::string(&name)?),
                configurable: Some(true),
                ..Descriptor::default()
            },
        )?;
        define(
            vm,
            wrapper,
            key("length"),
            Descriptor {
                value: Some(JsValue::Number(length as f64)),
                configurable: Some(true),
                ..Descriptor::default()
            },
        )?;
        define(
            vm,
            wrapper,
            key("prototype"),
            Descriptor {
                value: Some(prototype.clone()),
                writable: Some(false),
                configurable: Some(false),
                enumerable: Some(false),
                ..Descriptor::default()
            },
        )?;
        let value = JsValue::Object(wrapper);
        define(
            vm,
            prototype.object().unwrap(),
            key("constructor"),
            Descriptor {
                value: Some(value.clone()),
                writable: Some(true),
                configurable: Some(true),
                enumerable: Some(false),
                ..Descriptor::default()
            },
        )?;
        define(vm, namespace, key(&name), Descriptor::data(value.clone()))?;
        Ok(value)
    }
    /** Parent references are read after callbacks, because those may overwrite previously visited entries. */
    fn parent(&mut self, vm: &mut Vm, prototype: &JsValue, current: usize) -> Res<()> {
        let parent = self.data.signed(32).map_err(|e| trap(vm, &e.message))?;
        if parent == -1 {
            return Ok(());
        }
        if parent < 0 || parent as usize >= current {
            return Err(trap(vm, "invalid prototype parent index"));
        }
        let parent = gc::interop_read(vm, &self.prototypes, parent as usize)?;
        let parent = if parent == JsValue::Null {
            None
        } else {
            Some(
                parent
                    .object()
                    .ok_or_else(|| prototype_error(vm, &parent))?,
            )
        };
        let receiver = prototype
            .object()
            .ok_or_else(|| vm.type_error("Cannot set prototype on null"))?;
        if !super::super::object_set_prototype(vm, receiver, parent)? {
            return Err(vm.type_error("Cannot set prototype"));
        }
        Ok(())
    }
}
/** Standard invalid-prototype errors preserve the primitive value after authored mutations. */
fn prototype_error(vm: &mut Vm, value: &JsValue) -> Thrown {
    let value = match value {
        JsValue::Undefined => "undefined".into(),
        JsValue::Null => "null".into(),
        JsValue::String(s) => s.lossy(),
        other => format!("{other:?}"),
    };
    vm.type_error(&format!(
        "Object prototype may only be an Object or null: {value}"
    ))
}
/** A setup applies methods, accessors, constructors and inheritance in the decoder's actual order. */
pub(super) fn configure(vm: &mut Vm, call: Call) -> Res<JsValue> {
    gc::interop_array(vm, &call.arg(0), Type::Extern, None)?;
    gc::interop_array(vm, &call.arg(1), Type::Func, None)?;
    gc::interop_array(vm, &call.arg(2), Type::I32, Some(8))?;
    let length = gc::interop_length(vm, &call.arg(2))?;
    let data = gc::string_array(vm, &call.arg(2), 0, length, 8)?
        .into_iter()
        .map(|n| n as u8)
        .collect::<Vec<_>>();
    let mut setup = Setup {
        data: binary::Reader::new(&data),
        prototypes: call.arg(0),
        functions: call.arg(1),
        prototype_index: 0,
        function_index: 0,
    };
    let count = setup.integer(vm)? as usize;
    for index in 0..count {
        vm.charge(1)?;
        let prototype = setup.prototype(vm)?;
        let has_constructor = setup.integer(vm)?;
        if has_constructor > 1 {
            return Err(trap(vm, "invalid prototype constructor count"));
        }
        if has_constructor == 1 {
            let constructor = setup.constructor(vm, &prototype, &call.arg(3))?;
            let statics = setup.integer(vm)?;
            if statics > MAX_CONFIGURE_PROPERTIES {
                return Err(vm.range_error("Out of memory: too many constructor properties in configureAll"));
            }
            for _ in 0..statics {
                vm.charge(1)?;
                setup.member(vm, &constructor, true)?
            }
        }
        let methods = setup.integer(vm)?;
        if methods > MAX_CONFIGURE_PROPERTIES {
            return Err(vm.range_error("Out of memory: too many methods in configureAll"));
        }
        for _ in 0..methods {
            vm.charge(1)?;
            setup.member(vm, &prototype, false)?
        }
        setup.parent(vm, &prototype, index)?;
    }
    if setup.prototype_index != gc::interop_length(vm, &setup.prototypes)? {
        return Err(trap(vm, "unconsumed prototypes"));
    }
    if setup.function_index != gc::interop_length(vm, &setup.functions)? {
        return Err(trap(vm, "unconsumed functions"));
    }
    if setup.data.at != data.len() {
        return Err(trap(vm, "unconsumed data"));
    }
    Ok(JsValue::Undefined)
}

/** DefineOwnProperty failures are visible instead of silently dropping requested prototype members. */
fn define(vm: &mut Vm, object: ObjectId, name: Key, descriptor: Descriptor) -> Res<()> {
    if !vm.define(object, name, descriptor)? {
        return Err(vm.type_error("Cannot define property on target object"));
    }
    Ok(())
}
