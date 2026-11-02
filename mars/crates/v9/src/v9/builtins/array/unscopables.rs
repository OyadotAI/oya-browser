/** Array's standard unscopables table controls real identifier resolution inside with scopes. */
use super::*;
use crate::v9::vm::heap::{CONFIGURABLE, DEFAULT_ATTRS};

/** ES2023 additions stay out of legacy with(array) lexical lookup. `with` is not listed: the
specification leaves it out because the keyword can never resolve as an identifier, and V8
follows it. */
const NAMES: [&str; 16] = [
    "at",
    "copyWithin",
    "entries",
    "fill",
    "find",
    "findIndex",
    "findLast",
    "findLastIndex",
    "flat",
    "flatMap",
    "includes",
    "keys",
    "toReversed",
    "toSorted",
    "toSpliced",
    "values",
];

/** The table has no prototype and the Array.prototype property itself is non-writable. */
pub(super) fn install(builder: &mut Builder, prototype: ObjectId) {
    let object = builder.object(None);
    for name in NAMES {
        builder.value(object, name, JsValue::Boolean(true), DEFAULT_ATTRS);
    }
    builder.put(
        prototype,
        Key::Sym(symbol::UNSCOPABLES),
        crate::v9::vm::realm::Static::Value(JsValue::Object(object)),
        CONFIGURABLE,
    );
}
