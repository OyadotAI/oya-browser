/** Media session metadata stores normalized text and immutable artwork independently of playback. */
use super::dom::native_name;
use crate::v9::builtins::freeze_object;
use crate::v9::value::{JsValue, ObjectId};
use crate::v9::vm::data::Class;
use crate::v9::vm::heap::{HIDDEN_ATTRS, Key, Object, Prop, Slot};
use crate::v9::vm::realm::Builder;
use crate::v9::vm::{Call, Res, Vm};
use std::sync::OnceLock;

/** The installed prototype also supplies the fallback for subclasses. */
static PROTOTYPE: OnceLock<ObjectId> = OnceLock::new();
/** Writable metadata fields, converted before their previous values are replaced. */
const FIELDS: [&str; 4] = ["title", "artist", "album", "artwork"];
/** A native brand prevents prototype methods from accepting arbitrary objects. */
const BRAND: &str = "mediaMetadata.brand";

/** Install the real constructor before the interface shape table fills optional members. */
pub(super) fn install(builder: &mut Builder) {
    let prototype = builder.object(Some(builder.ids.object_prototype));
    for field in FIELDS {
        builder.accessor(prototype, field, Some(get), Some(set));
    }
    let constructor = builder.constructor("MediaMetadata", 0, construct, prototype, None);
    builder.global("MediaMetadata", constructor);
    let _ = PROTOTYPE.set(prototype);
}

/** One private metadata value, inaccessible through author-owned properties. */
fn store(vm: &mut Vm, id: ObjectId, name: &str, value: JsValue) {
    vm.heap.put(
        id,
        Key::private(name),
        Prop {
            slot: Slot::Data(value),
            attrs: HIDDEN_ATTRS,
        },
    );
}

/** Read a native value without invoking a getter. */
fn read(vm: &Vm, id: ObjectId, name: &str) -> Option<JsValue> {
    match &vm.heap.get(id)?.props.get(&Key::private(name))?.slot {
        Slot::Data(value) => Some(value.clone()),
        _ => None,
    }
}

/** WebIDL dictionaries accept null and undefined as empty, but reject other primitives. */
fn member(vm: &mut Vm, dictionary: &JsValue, name: &str) -> Res<JsValue> {
    if dictionary.nullish() {
        return Ok(JsValue::Undefined);
    }
    if dictionary.object().is_none() {
        return Err(vm.type_error("Media metadata must be an object"));
    }
    vm.get(dictionary, &Key::from(name))
}

/** Construction initializes every field before the metadata becomes visible. */
fn construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(target) = &call.new_target else {
        return Err(vm.type_error("MediaMetadata requires new"));
    };
    let prototype = vm.prototype_for(target, *PROTOTYPE.get().expect("installed"))?;
    let id = vm.alloc(Object::new(Class::Plain, Some(prototype)))?;
    vm.pin(JsValue::Object(id));
    store(vm, id, BRAND, JsValue::Boolean(true));
    for field in FIELDS {
        let value = member(vm, &call.arg(0), field)?;
        let value = convert(vm, field, value, true)?;
        store(vm, id, field, value);
    }
    Ok(JsValue::Object(id))
}

/** Accessors require a constructed metadata instance even when borrowed from its prototype. */
fn receiver(vm: &mut Vm, call: &Call) -> Res<ObjectId> {
    call.this
        .object()
        .filter(|id| read(vm, *id, BRAND) == Some(JsValue::Boolean(true)))
        .ok_or_else(|| vm.type_error("Illegal MediaMetadata invocation"))
}

/** Stored fields cannot be redirected through a same-named public property. */
fn get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = receiver(vm, &call)?;
    Ok(read(vm, id, native_name(&call)).unwrap_or(JsValue::Undefined))
}

/** Conversion finishes before a setter replaces the previous valid value. */
fn set(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = receiver(vm, &call)?;
    let name = native_name(&call);
    let value = convert(vm, name, call.arg(0), false)?;
    store(vm, id, name, value);
    Ok(JsValue::Undefined)
}

/** Text uses DOMString conversion; missing constructor fields use their declared defaults. */
fn convert(vm: &mut Vm, name: &str, value: JsValue, defaults: bool) -> Res<JsValue> {
    if name == "artwork" {
        return artwork(vm, value, defaults);
    }
    if defaults && value == JsValue::Undefined {
        return JsValue::string("").map_err(Into::into);
    }
    vm.to_string(&value).map(JsValue::String)
}

/** Artwork is copied and frozen, so callers cannot mutate metadata through an input alias. */
fn artwork(vm: &mut Vm, value: JsValue, defaults: bool) -> Res<JsValue> {
    let values = if defaults && value == JsValue::Undefined {
        Vec::new()
    } else {
        vm.iterate_to_vec(&value)?
    };
    let mut images = Vec::new();
    for value in values {
        let image = image(vm, value)?;
        vm.pin(image.clone());
        images.push(image);
    }
    let array = vm.new_array(images)?;
    freeze_object(vm, array)?;
    Ok(JsValue::Object(array))
}

/** Artwork sources resolve against the current document and require a valid URL. */
fn image(vm: &mut Vm, value: JsValue) -> Res<JsValue> {
    let source = member(vm, &value, "src")?;
    if source == JsValue::Undefined {
        return Err(vm.type_error("Media artwork requires src"));
    }
    let source = vm.to_text(&source)?;
    let source = super::net::resolve(vm, &source)
        .ok_or_else(|| vm.type_error("Invalid media artwork URL"))?;
    let id = vm.new_object()?;
    vm.pin(JsValue::Object(id));
    vm.put(id, "src", JsValue::string(&source)?)?;
    for name in ["sizes", "type"] {
        let field = member(vm, &value, name)?;
        let field = convert(vm, name, field, true)?;
        vm.put(id, name, field)?;
    }
    freeze_object(vm, id)?;
    Ok(JsValue::Object(id))
}
