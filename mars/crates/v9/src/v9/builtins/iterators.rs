/** Iterator prototypes and generator objects' next, return and throw. */
use super::*;
use crate::v9::vm::Resume;
use crate::v9::vm::data::Class;
use crate::v9::vm::heap::CONFIGURABLE;
use crate::v9::vm::realm::{Static, string, symbol};

/** %IteratorPrototype%, the native iterator prototypes, and generator prototypes. */
pub(super) fn install(b: &mut Builder) {
    let object_prototype = b.ids.object_prototype;
    let iterator = b.object(Some(object_prototype));
    b.ids.iterator_prototype = iterator;
    b.symbol_method(iterator, symbol::ITERATOR, "[Symbol.iterator]", 0, identity);
    for (tag, slot) in [
        ("Array Iterator", 0),
        ("String Iterator", 1),
        ("Map Iterator", 2),
        ("Set Iterator", 3),
        ("RegExp String Iterator", 4),
    ] {
        let proto = b.object(Some(iterator));
        b.method(proto, "next", 0, next);
        b.put(
            proto,
            Key::Sym(symbol::TO_STRING_TAG),
            Static::Value(string(tag)),
            CONFIGURABLE,
        );
        let ids = &mut b.ids;
        match slot {
            0 => ids.array_iterator_prototype = proto,
            1 => ids.string_iterator_prototype = proto,
            2 => ids.map_iterator_prototype = proto,
            3 => ids.set_iterator_prototype = proto,
            _ => ids.regexp_iterator_prototype = proto,
        }
    }
    let generator = b.object(Some(iterator));
    b.ids.generator_prototype = generator;
    b.method(generator, "next", 1, generator_next);
    b.method(generator, "return", 1, generator_return);
    b.method(generator, "throw", 1, generator_throw);
    b.put(
        generator,
        Key::Sym(symbol::TO_STRING_TAG),
        Static::Value(string("Generator")),
        CONFIGURABLE,
    );
    let function_prototype = b.ids.function_prototype;
    let generator_function = b.object(Some(function_prototype));
    b.ids.generator_function_prototype = generator_function;
    b.value(
        generator_function,
        "prototype",
        JsValue::Object(generator),
        CONFIGURABLE,
    );
    b.value(
        generator,
        "constructor",
        JsValue::Object(generator_function),
        CONFIGURABLE,
    );
    let async_function = b.object(Some(function_prototype));
    b.ids.async_function_prototype = async_function;
    b.put(
        async_function,
        Key::Sym(symbol::TO_STRING_TAG),
        Static::Value(string("AsyncFunction")),
        CONFIGURABLE,
    );
}

/** `[Symbol.iterator]() { return this }`. */
fn identity(_: &mut Vm, call: Call) -> Res<JsValue> {
    Ok(call.this)
}

/** next() of native iterators. */
fn next(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let JsValue::Object(id) = call.this else {
        return Err(vm.type_error("next method called on incompatible receiver"));
    };
    if !matches!(vm.heap.get(id).map(|o| &o.class), Some(Class::Iter(_))) {
        return Err(vm.type_error("next method called on incompatible receiver"));
    }
    match vm.iter_step(id)? {
        Some(value) => vm.iter_result(value, false),
        None => vm.iter_result(JsValue::Undefined, true),
    }
}

/** Resume a generator receiver. */
fn resume(vm: &mut Vm, call: Call, mode: Resume) -> Res<JsValue> {
    let JsValue::Object(id) = call.this else {
        return Err(vm.type_error("next method called on incompatible receiver"));
    };
    let (value, done) = vm.resume_generator(id, mode, call.arg(0))?;
    vm.iter_result(value, done)
}

/** Generator.prototype.next. */
fn generator_next(vm: &mut Vm, call: Call) -> Res<JsValue> {
    resume(vm, call, Resume::Next)
}

/** Generator.prototype.return. */
fn generator_return(vm: &mut Vm, call: Call) -> Res<JsValue> {
    resume(vm, call, Resume::Return)
}

/** Generator.prototype.throw. */
fn generator_throw(vm: &mut Vm, call: Call) -> Res<JsValue> {
    resume(vm, call, Resume::Throw)
}
