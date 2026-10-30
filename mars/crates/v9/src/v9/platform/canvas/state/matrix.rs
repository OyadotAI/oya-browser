/** Detached affine DOMMatrix results use private components and WebIDL dictionary conversion. */
use super::*;
use crate::v9::vm::heap::Key;
static MATRIX: OnceLock<u32> = OnceLock::new();
/** Dictionary aliases name the same six affine components. */
const COMPONENTS: [(&str, &str); 6] = [
    ("a", "m11"),
    ("b", "m12"),
    ("c", "m21"),
    ("d", "m22"),
    ("e", "m41"),
    ("f", "m42"),
];
/** Remaining four-dimensional components of an affine matrix are fixed identity values. */
const FIXED: [(&str, f64); 10] = [
    ("m13", 0.0),
    ("m14", 0.0),
    ("m23", 0.0),
    ("m24", 0.0),
    ("m31", 0.0),
    ("m32", 0.0),
    ("m33", 1.0),
    ("m34", 0.0),
    ("m43", 0.0),
    ("m44", 1.0),
];

/** Return matrices have real branded getters; unsupported full DOMMatrix operations remain separate. */
pub(super) fn install(b: &mut Builder) {
    let readonly = b.object(Some(b.ids.object_prototype));
    super::super::super::chrome_env::interface(b, "DOMMatrixReadOnly", readonly);
    for (name, alias) in COMPONENTS {
        for name in [name, alias] {
            b.getter(readonly, name, get);
        }
    }
    for (name, _) in FIXED {
        b.getter(readonly, name, get);
    }
    for name in ["is2D", "isIdentity"] {
        b.getter(readonly, name, get);
    }
    b.host_method(readonly, "toJSON", 0, json);
    let proto = b.object(Some(readonly));
    super::super::super::chrome_env::interface(b, "DOMMatrix", proto);
    for (name, alias) in COMPONENTS {
        for name in [name, alias] {
            b.accessor(proto, name, Some(get), Some(set));
        }
    }
    let _ = MATRIX.set(proto);
}

/** Private storage is distinct from both the context and every prior getTransform result. */
pub(super) fn record(vm: &mut Vm, transform: Affine) -> Res<JsValue> {
    let id = vm.alloc(Object::new(
        Class::Plain,
        Some(*MATRIX.get().expect("matrix interface")),
    ))?;
    for (index, (name, _)) in COMPONENTS.iter().enumerate() {
        encoding::write_slot(
            vm,
            id,
            &format!("canvas.matrix.{name}"),
            JsValue::Number(transform.0[index]),
        );
    }
    Ok(JsValue::Object(id))
}

/** Missing slots fail branding even when an object inherits the correct public prototype. */
fn read(vm: &mut Vm, value: &JsValue) -> Res<Affine> {
    let mut values = [0.0; 6];
    for (index, (name, _)) in COMPONENTS.iter().enumerate() {
        let Some(JsValue::Number(number)) =
            encoding::read_slot(vm, value, &format!("canvas.matrix.{name}"))
        else {
            return Err(vm.type_error("Illegal invocation"));
        };
        values[index] = number;
    }
    Ok(Affine(values))
}

/** Component aliases share one slot; metadata describes the detached result's actual values. */
fn get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let matrix = read(vm, &call.this)?;
    let name = dom::native_name(&call);
    if name == "is2D" {
        return Ok(JsValue::Boolean(true));
    }
    if name == "isIdentity" {
        return Ok(JsValue::Boolean(matrix == Affine::IDENTITY));
    }
    if let Some(index) = COMPONENTS
        .iter()
        .position(|(key, alias)| *key == name || *alias == name)
    {
        return Ok(JsValue::Number(matrix.0[index]));
    }
    Ok(JsValue::Number(
        FIXED
            .iter()
            .find(|(key, _)| *key == name)
            .expect("registered component")
            .1,
    ))
}

/** Mutating a result updates its aliases, never the originating canvas transform. */
fn set(vm: &mut Vm, call: Call) -> Res<JsValue> {
    read(vm, &call.this)?;
    let value = vm.to_number(&call.arg(0))?;
    let name = dom::native_name(&call);
    let (name, _) = COMPONENTS
        .iter()
        .find(|(key, alias)| *key == name || *alias == name)
        .expect("registered component");
    encoding::write_slot(
        vm,
        call.this.object().expect("matrix"),
        &format!("canvas.matrix.{name}"),
        JsValue::Number(value),
    );
    Ok(JsValue::Undefined)
}

/** JSON includes standard affine and 4D names rather than exposing private storage. */
fn json(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let matrix = read(vm, &call.this)?;
    let result = vm.alloc(Object::new(
        Class::Plain,
        Some(crate::v9::vm::realm::realm().ids.object_prototype),
    ))?;
    for (index, (name, alias)) in COMPONENTS.iter().enumerate() {
        for name in [name, alias] {
            vm.put(result, name, JsValue::Number(matrix.0[index]))?;
        }
    }
    for (name, value) in FIXED {
        vm.put(result, name, JsValue::Number(value))?;
    }
    vm.put(result, "is2D", JsValue::Boolean(true))?;
    vm.put(
        result,
        "isIdentity",
        JsValue::Boolean(matrix == Affine::IDENTITY),
    )?;
    Ok(JsValue::Object(result))
}

/** WebIDL converts dictionary members in lexical order before checking conflicting aliases. */
pub(super) fn dictionary(vm: &mut Vm, value: &JsValue) -> Res<Affine> {
    dictionary_members(vm, value)?.resolve(vm)
}

/** Converted members defer alias validation until addPath has checked its source path. */
pub(in crate::v9::platform::canvas) struct Dictionary {
    names: [Option<f64>; 6],
    aliases: [Option<f64>; 6],
}

impl Dictionary {
    /** Fixup is separate from conversion because empty addPath sources skip conflicting-alias checks. */
    pub(in crate::v9::platform::canvas) fn resolve(self, vm: &mut Vm) -> Res<Affine> {
        let mut matrix = Affine::IDENTITY;
        for index in 0..COMPONENTS.len() {
            matrix.0[index] = resolve(vm, self.names[index], self.aliases[index], matrix.0[index])?;
        }
        Ok(matrix)
    }
}

/** Even an empty source must run every dictionary getter and numeric conversion before its no-op. */
pub(in crate::v9::platform::canvas) fn dictionary_members(
    vm: &mut Vm,
    value: &JsValue,
) -> Res<Dictionary> {
    let mut result = Dictionary {
        names: [None; 6],
        aliases: [None; 6],
    };
    if value.nullish() {
        return Ok(result);
    }
    if value.object().is_none() {
        return Err(vm.type_error("Canvas transform must be a dictionary."));
    }
    for (index, (name, _)) in COMPONENTS.iter().enumerate() {
        result.names[index] = member(vm, value, name)?;
    }
    for (index, (_, alias)) in COMPONENTS.iter().enumerate() {
        result.aliases[index] = member(vm, value, alias)?;
    }
    Ok(result)
}

/** Undefined means absent, even when the dictionary has an own property. */
fn member(vm: &mut Vm, value: &JsValue, name: &str) -> Res<Option<f64>> {
    let member = vm.get(value, &Key::from(name))?;
    if matches!(member, JsValue::Undefined) {
        return Ok(None);
    }
    vm.to_number(&member).map(Some)
}

/** Matching NaNs are allowed by matrix dictionary fixup, then ignored by Canvas drawing state. */
fn resolve(vm: &mut Vm, name: Option<f64>, alias: Option<f64>, default: f64) -> Res<f64> {
    if let (Some(left), Some(right)) = (name, alias)
        && left != right
        && !(left.is_nan() && right.is_nan())
    {
        return Err(vm.type_error("Conflicting canvas transform matrix components."));
    }
    Ok(name.or(alias).unwrap_or(default))
}
