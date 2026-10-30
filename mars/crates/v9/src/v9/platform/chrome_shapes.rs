/**
 * Chrome's window interfaces, installed with Chrome's shape so detection scripts (Google's bot
 * gate among them) that check interface names on `window` and compare prototypes member by
 * member see a Chrome. Two passes over the table in `chrome_shapes.txt` (generated from Blink's
 * exposed-interface listing, see its header): interfaces Mars lacks are created whole, the
 * interface object inheriting from its parent interface, a prototype with the same members,
 * constants with their values and `Symbol.toStringTag`; interfaces Mars implements get the
 * members they are missing. No member carries behavior. Called on a prototype, as Chrome does
 * without an instance, a member throws `Illegal invocation`; called on an instance it answers
 * `undefined`, and an accessor remembers what a script assigned, so feature tests and
 * round-trips behave while the real implementation is absent.
 */
use super::dom::{illegal_constructor, native_name};
use crate::v9::value::JsValue;
use crate::v9::vm::heap::{CONFIGURABLE, HIDDEN_ATTRS, INTRINSIC_BASE, Key, Slot};
use crate::v9::vm::realm::{Builder, Static, string, symbol};
use crate::v9::vm::{Call, Res, Vm};
use std::collections::HashMap;

/** The shape table: `name|parent|member tokens`, one interface per line. */
const SHAPES: &str = include_str!("chrome_shapes.txt");
/** Lines in the table that are comments. */
const COMMENT: char = '#';
/** The name `Symbol.iterator` methods carry in Chrome (`NodeList.prototype.values`). */
const ITERATOR_NAME: &str = "values";
/** The name async iterator methods carry (`ReadableStream.prototype.values`). */
const ASYNC_ITERATOR_NAME: &str = "values";
/** Event handler attributes start out null rather than undefined. */
const HANDLER_PREFIX: &str = "on";

/** Legacy window aliases: the same interface object under its prefixed name, as Blink's
`LegacyWindowAlias` installs them. */
const ALIASES: [(&str, &str); 8] = [
    ("webkitURL", "URL"),
    ("webkitMediaStream", "MediaStream"),
    ("webkitRTCPeerConnection", "RTCPeerConnection"),
    ("webkitSpeechGrammar", "SpeechGrammar"),
    ("webkitSpeechGrammarList", "SpeechGrammarList"),
    ("webkitSpeechRecognition", "SpeechRecognition"),
    (
        "webkitSpeechRecognitionError",
        "SpeechRecognitionErrorEvent",
    ),
    ("webkitSpeechRecognitionEvent", "SpeechRecognitionEvent"),
];

/** Interface objects by name: the constructor and its prototype. */
pub(super) type Known = HashMap<&'static str, (u32, u32)>;

/** One parsed line of the table. */
struct Shape {
    name: &'static str,
    parent: &'static str,
    members: &'static str,
}

/** Whether a receiver is an instance rather than a prototype: prototypes own `constructor`,
instances do not. Chrome throws for the prototype; an instance gets a placeholder answer. */
fn instance(vm: &mut Vm, this: &JsValue) -> Res<bool> {
    let JsValue::Object(id) = this else {
        return Ok(false);
    };
    Ok(vm.own(*id, &Key::from("constructor"))?.is_none())
}

/** A method or getter with no implementation: `Illegal invocation` on a prototype, the value a
script stored (or the member's initial value) on an instance. */
fn stub_get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if super::canvas::context_node(vm, &call.this).is_some() {
        return Err(vm.dom_exception(
            "NotSupportedError",
            "This Canvas 2D operation is not implemented.",
        ));
    }
    if !instance(vm, &call.this)? {
        return Err(vm.type_error("Illegal invocation"));
    }
    let name = native_name(&call);
    let JsValue::Object(id) = call.this else {
        return Ok(JsValue::Undefined);
    };
    if let Some(prop) = vm.own(id, &Key::from(name))?
        && let Slot::Data(value) = prop.slot
    {
        return Ok(value);
    }
    Ok(if name.starts_with(HANDLER_PREFIX) {
        JsValue::Null
    } else {
        JsValue::Undefined
    })
}

/** A setter with no implementation: the value becomes an own property of the instance, where
the event dispatcher finds handler attributes and where the getter reads it back. */
fn stub_set(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if super::canvas::context_node(vm, &call.this).is_some() {
        return Err(vm.dom_exception(
            "NotSupportedError",
            "This Canvas 2D state is not implemented.",
        ));
    }
    if !instance(vm, &call.this)? {
        return Err(vm.type_error("Illegal invocation"));
    }
    let name = native_name(&call);
    if let JsValue::Object(id) = call.this {
        vm.put(id, name, call.arg(0))?;
    }
    Ok(JsValue::Undefined)
}

/** The interface objects already installed, by name, with their prototypes. */
fn existing(b: &Builder) -> Known {
    let mut known = HashMap::new();
    for (name, value) in &b.globals {
        let JsValue::Object(ctor) = value else {
            continue;
        };
        let Some(def) = b.objects.get((*ctor - INTRINSIC_BASE) as usize) else {
            continue;
        };
        if let Some((_, Static::Value(JsValue::Object(proto)), _)) =
            def.get(&Key::from("prototype"))
        {
            known.insert(*name, (*ctor, *proto));
        }
    }
    known
}

/** The table's interfaces, parsed. */
fn shapes() -> Vec<Shape> {
    SHAPES
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with(COMMENT))
        .filter_map(|line| {
            let mut parts = line.splitn(3, '|');
            Some(Shape {
                name: parts.next()?,
                parent: parts.next()?,
                members: parts.next().unwrap_or(""),
            })
        })
        .collect()
}

/** Install every interface Mars lacks, parents before children so prototype chains link (an
interface whose parent never resolves is left out rather than mislinked), then the aliases,
then Navigator's member objects, then the members Mars's own interfaces are missing. */
pub(super) fn install(b: &mut Builder) {
    let mut known = existing(b);
    let all = shapes();
    let mut pending: Vec<&Shape> = all
        .iter()
        .filter(|shape| !known.contains_key(shape.name))
        .filter(|shape| ALIASES.iter().all(|(alias, _)| *alias != shape.name))
        .collect();
    loop {
        let before = pending.len();
        pending.retain(|shape| {
            let parent = if shape.parent.is_empty() {
                Some((None, b.ids.object_prototype))
            } else {
                known
                    .get(shape.parent)
                    .map(|(ctor, proto)| (Some(*ctor), *proto))
            };
            let Some((parent_ctor, parent_proto)) = parent else {
                return true;
            };
            let ids = interface(b, shape, parent_ctor, parent_proto);
            known.insert(shape.name, ids);
            false
        });
        if pending.len() == before {
            break;
        }
    }
    for (alias, target) in ALIASES {
        if let Some((ctor, _)) = known.get(target) {
            b.global(alias, *ctor);
        }
    }
    super::chrome_env::navigator_objects(b, &known);
    for shape in &all {
        if let Some((ctor, proto)) = known.get(shape.name) {
            complete(b, *ctor, *proto, shape);
        }
    }
}

/** One interface object and prototype with the members the table lists. */
fn interface(
    b: &mut Builder,
    shape: &Shape,
    parent_ctor: Option<u32>,
    parent_proto: u32,
) -> (u32, u32) {
    let proto = b.object(Some(parent_proto));
    b.put(
        proto,
        Key::Sym(symbol::TO_STRING_TAG),
        Static::Value(string(shape.name)),
        CONFIGURABLE,
    );
    let ctor = b.constructor(shape.name, 0, illegal_constructor, proto, parent_ctor);
    b.global(shape.name, ctor);
    complete(b, ctor, proto, shape);
    (ctor, proto)
}

/** Add every listed member the interface does not have yet. */
fn complete(b: &mut Builder, ctor: u32, proto: u32, shape: &Shape) {
    for token in shape.members.split(' ').filter(|token| !token.is_empty()) {
        member(b, ctor, proto, token);
    }
}

/** Whether an intrinsic already has a property. */
fn has(b: &Builder, target: u32, key: &Key) -> bool {
    b.objects[(target - INTRINSIC_BASE) as usize]
        .get(key)
        .is_some()
}

/** Whether an interface or any inherited prototype already owns a member. */
fn has_member(b: &Builder, target: u32, key: &Key) -> bool {
    let mut current = Some(target);
    while let Some(id) = current {
        if has(b, id, key) {
            return true;
        }
        current = b.objects[(id - INTRINSIC_BASE) as usize].proto;
    }
    false
}

/** One member token (see the table header for the letters), unless it is already there. */
fn member(b: &mut Builder, ctor: u32, proto: u32, token: &'static str) {
    match token {
        "i" => return iterator(b, proto, symbol::ITERATOR, ITERATOR_NAME),
        "y" => return iterator(b, proto, symbol::ASYNC_ITERATOR, ASYNC_ITERATOR_NAME),
        _ => {}
    }
    let Some((kind, rest)) = token.split_once(':') else {
        return;
    };
    if kind == "c" {
        return constant(b, ctor, proto, rest);
    }
    let target = if kind.chars().all(char::is_uppercase) {
        ctor
    } else {
        proto
    };
    if has_member(b, target, &Key::from(rest)) {
        return;
    }
    match kind {
        "m" | "M" => {
            b.host_method(target, rest, 0, stub_get);
        }
        "g" | "G" => b.accessor(target, rest, Some(stub_get), None),
        "a" | "A" => b.accessor(target, rest, Some(stub_get), Some(stub_set)),
        "s" | "S" => b.accessor(target, rest, None, Some(stub_set)),
        _ => {}
    }
}

/** `NAME=value` on the interface object and its prototype, as WebIDL places constants. */
fn constant(b: &mut Builder, ctor: u32, proto: u32, spec: &str) {
    let Some((name, value)) = spec.split_once('=') else {
        return;
    };
    let Ok(value) = value.parse::<f64>() else {
        return;
    };
    for target in [ctor, proto] {
        if !has(b, target, &Key::from(name)) {
            b.host_constant(target, name, value);
        }
    }
}

/** `Symbol.iterator` is the same function object as the `values` method when the interface
has one, as in Chrome; otherwise a function of that name. */
fn iterator(b: &mut Builder, proto: u32, sym: u32, name: &'static str) {
    if has(b, proto, &Key::Sym(sym)) {
        return;
    }
    let existing = b.objects[(proto - INTRINSIC_BASE) as usize]
        .get(&Key::from(name))
        .and_then(|(_, value, _)| match value {
            Static::Value(JsValue::Object(id)) => Some(*id),
            _ => None,
        });
    let function = existing.unwrap_or_else(|| b.function(name, 0, stub_get));
    b.put(
        proto,
        Key::Sym(sym),
        Static::Value(JsValue::Object(function)),
        HIDDEN_ATTRS,
    );
}
