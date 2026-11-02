/** Serializable lexical cells preserve declaration kind and initialization independently of values. */
use super::JsValue;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
/// A lexical declaration's mutability survives every stateless V9 event request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BindingKind {
    /// Declaration-scope cells initialize before execution and permit redeclaration.
    Var,
    Let,
    Const,
}
/// An uninitialized lexical cell is distinct from an initialized JavaScript undefined value.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    /// Var and let cells admit reassignment after initialization.
    pub kind: BindingKind,
    /// None represents the temporal dead zone, never a committed successful global. On the wire
    /// the dead zone is an absent field, so an initialized `null` (JSON null) stays `Some(Null)`.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "initialized"
    )]
    pub value: Option<JsValue>,
}

/** A present value field is always initialized, whatever it holds, including null. */
fn initialized<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<JsValue>, D::Error> {
    JsValue::deserialize(deserializer).map(Some)
}
/// The coordinator owns this private, bounded per-page state; agents cannot supply it.
pub type Globals = BTreeMap<String, Binding>;
