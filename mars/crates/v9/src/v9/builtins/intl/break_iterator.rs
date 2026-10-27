/**
 * `Intl.v8BreakIterator`, V8's non-standard break iterator. It exists only in conformance builds,
 * so the upstream mjsunit tests that construct it can run, and never in a browser build. It cuts
 * text with the same splitter as `Intl.Segmenter`; the `line` type reuses word cuts, a known
 * simplification. The boundaries are computed once by `adoptText`, so every step is an index move.
 * The step methods are getters that return a closure over the iterator, as V8 shapes them, so
 * `const next = it.next; next()` still works.
 */
use super::break_cursor;
use super::segmentation::split;
use super::{LOCALE, locale, supported_locales};
use crate::v9::value::{JsValue, ObjectId};
use crate::v9::vm::data::{BreakIteratorData, Class, HostData};
use crate::v9::vm::heap::{HIDDEN_ATTRS, Key, Object};
use crate::v9::vm::realm::{Builder, realm};
use crate::v9::vm::{Call, Res, Vm};
use std::sync::OnceLock;

/** The constructor's name on `Intl`. */
const NAME: &str = "v8BreakIterator";
/** The `type` option values, each with the segmentation granularity it cuts by. */
const TYPES: [(&str, &str); 4] = [
    ("character", "grapheme"),
    ("word", "word"),
    ("sentence", "sentence"),
    ("line", "word"),
];
/** The type a constructor uses when the page names none. */
const DEFAULT_TYPE: &str = "word";
/** Native closures the step getters hand back, one per method, set once at install. */
struct Natives {
    /** `adoptText(text)`. */
    adopt_text: u32,
    /** `first()`. */
    first: u32,
    /** `next()`. */
    next: u32,
    /** `current()`. */
    current: u32,
    /** `breakType()`. */
    break_type: u32,
}

/** The closure natives, built once when `Intl` is installed. */
static NATIVES: OnceLock<Natives> = OnceLock::new();

/** The closure natives. */
fn natives() -> &'static Natives {
    NATIVES.get().expect("v8BreakIterator natives installed")
}

/** Install `Intl.v8BreakIterator`: its constructor, `supportedLocalesOf`, the getters and
`resolvedOptions`. */
pub(super) fn install(b: &mut Builder, intl: ObjectId) {
    let proto = b.object(Some(b.ids.object_prototype));
    b.method(proto, "resolvedOptions", 0, resolved_options);
    b.builtin_accessor(proto, "adoptText", Some(get_adopt_text), None);
    b.builtin_accessor(proto, "first", Some(get_first), None);
    b.builtin_accessor(proto, "next", Some(get_next), None);
    b.builtin_accessor(proto, "current", Some(get_current), None);
    b.builtin_accessor(proto, "breakType", Some(get_break_type), None);
    let ctor = b.constructor(NAME, 0, construct, proto, None);
    b.method(ctor, "supportedLocalesOf", 1, supported_locales);
    b.value(intl, NAME, JsValue::Object(ctor), HIDDEN_ATTRS);
    let _ = NATIVES.set(Natives {
        adopt_text: b.native("adoptText", 1, adopt_text),
        first: b.native("first", 0, first),
        next: b.native("next", 0, next),
        current: b.native("current", 0, current),
        break_type: b.native("breakType", 0, break_type),
    });
}

/** `new Intl.v8BreakIterator(locales, options)`, also callable without `new`. The `type` option
must name a known cut; an unknown one is a RangeError rather than a silent default. */
fn construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let locale = locale::canonical_list(vm, &call.arg(0))?
        .into_iter()
        .next()
        .filter(|tag| !tag.is_empty())
        .unwrap_or_else(|| LOCALE.into());
    let kind = option_type(vm, &call.arg(1))?;
    let prototype = vm.get(&JsValue::Object(call.callee), &Key::from("prototype"))?;
    let proto = match prototype {
        JsValue::Object(id) => id,
        _ => realm().ids.object_prototype,
    };
    let state = BreakIteratorData {
        locale,
        kind,
        boundaries: vec![0],
        break_types: Vec::new(),
        cursor: 0,
    };
    let id = vm.alloc(Object::new(
        Class::Host(Box::new(HostData::BreakIterator(Box::new(state)))),
        Some(proto),
    ))?;
    Ok(JsValue::Object(id))
}

/** The `type` option, read from the options argument and checked against the known cuts. */
fn option_type(vm: &mut Vm, init: &JsValue) -> Res<String> {
    if !matches!(init, JsValue::Object(_)) {
        return Ok(DEFAULT_TYPE.into());
    }
    let value = vm.get(init, &Key::from("type"))?;
    if value == JsValue::Undefined {
        return Ok(DEFAULT_TYPE.into());
    }
    let kind = vm.to_text(&value)?;
    if granularity_of(&kind).is_none() {
        return Err(vm.range_error(&format!(
            "Value {kind} out of range for Intl.v8BreakIterator options property type"
        )));
    }
    Ok(kind)
}

/** The segmentation granularity a `type` option names, if it names one. */
fn granularity_of(kind: &str) -> Option<&'static str> {
    TYPES
        .iter()
        .find(|(name, _)| *name == kind)
        .map(|(_, granularity)| *granularity)
}

/** Whether a value is a break iterator, checked without copying its state. */
fn is_iterator(vm: &Vm, value: &JsValue) -> bool {
    let JsValue::Object(id) = value else {
        return false;
    };
    matches!(
        vm.heap.get(*id).map(|object| &object.class),
        Some(Class::Host(host)) if matches!(**host, HostData::BreakIterator(_))
    )
}

/** Runs `edit` on the state behind `receiver`, or throws if the receiver is not an iterator. */
fn with_state<T>(
    vm: &mut Vm,
    receiver: &JsValue,
    edit: impl FnOnce(&mut BreakIteratorData) -> T,
) -> Res<T> {
    let JsValue::Object(id) = receiver else {
        return Err(vm.type_error("Illegal invocation"));
    };
    let edited = match vm.heap.get_mut(*id).map(|object| &mut object.class) {
        Some(Class::Host(host)) => match host.as_mut() {
            HostData::BreakIterator(state) => Some(edit(state)),
            _ => None,
        },
        _ => None,
    };
    edited.ok_or_else(|| vm.type_error("Illegal invocation"))
}

/** The receiver a step closure captured in its one slot. */
fn captured(vm: &Vm, call: &Call) -> JsValue {
    vm.slots(call.callee)
        .into_iter()
        .next()
        .unwrap_or(JsValue::Undefined)
}

/** A step getter: validates the receiver now and returns the step as a closure over it. */
fn method_for(vm: &mut Vm, call: &Call, pick: fn(&Natives) -> u32) -> Res<JsValue> {
    if !is_iterator(vm, &call.this) {
        return Err(vm.type_error("Illegal invocation"));
    }
    let closure = vm.native_closure(pick(natives()), vec![call.this.clone()])?;
    Ok(JsValue::Object(closure))
}

/** The getter for `adoptText`. */
fn get_adopt_text(vm: &mut Vm, call: Call) -> Res<JsValue> {
    method_for(vm, &call, |n| n.adopt_text)
}

/** The getter for `first`. */
fn get_first(vm: &mut Vm, call: Call) -> Res<JsValue> {
    method_for(vm, &call, |n| n.first)
}

/** The getter for `next`. */
fn get_next(vm: &mut Vm, call: Call) -> Res<JsValue> {
    method_for(vm, &call, |n| n.next)
}

/** The getter for `current`. */
fn get_current(vm: &mut Vm, call: Call) -> Res<JsValue> {
    method_for(vm, &call, |n| n.current)
}

/** The getter for `breakType`. */
fn get_break_type(vm: &mut Vm, call: Call) -> Res<JsValue> {
    method_for(vm, &call, |n| n.break_type)
}

/** `adoptText(text)`: cuts the text by the iterator's type and rewinds to the first boundary. */
fn adopt_text(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let receiver = captured(vm, &call);
    let text = vm.to_text(&call.arg(0))?;
    let kind = with_state(vm, &receiver, |state| state.kind.clone())?;
    let granularity = granularity_of(&kind).unwrap_or("grapheme");
    let pieces = split(&text, granularity);
    let total = text.encode_utf16().count();
    with_state(vm, &receiver, |state| {
        break_cursor::load(state, &pieces, total)
    })?;
    Ok(JsValue::Undefined)
}

/** `first()`: rewinds to the first boundary and returns it. */
fn first(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let receiver = captured(vm, &call);
    let at = with_state(vm, &receiver, break_cursor::rewind)?;
    Ok(JsValue::Number(at))
}

/** `next()`: moves to the following boundary and returns it, or DONE past the last one. */
fn next(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let receiver = captured(vm, &call);
    let at = with_state(vm, &receiver, break_cursor::advance)?;
    Ok(JsValue::Number(at))
}

/** `current()`: the boundary under the cursor, or DONE. */
fn current(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let receiver = captured(vm, &call);
    let at = with_state(vm, &receiver, |state| break_cursor::position(state))?;
    Ok(JsValue::Number(at))
}

/** `breakType()`: the type of the segment the cursor last passed, as ICU names it. */
fn break_type(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let receiver = captured(vm, &call);
    let name = with_state(vm, &receiver, |state| break_cursor::type_name(state))?;
    JsValue::string(name).map_err(Into::into)
}

/** `resolvedOptions()`: the locale the constructor resolved and the type it read. */
fn resolved_options(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (locale, kind) = with_state(vm, &call.this, |state| {
        (state.locale.clone(), state.kind.clone())
    })?;
    let id = vm.new_object()?;
    vm.put(id, "locale", JsValue::string(&locale)?)?;
    vm.put(id, "type", JsValue::string(&kind)?)?;
    Ok(JsValue::Object(id))
}
