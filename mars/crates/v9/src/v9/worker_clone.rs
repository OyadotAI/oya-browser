/** Worker messages are bounded inert graphs; object identities never cross an isolate boundary. */
mod decode;
mod encode;
pub(in crate::v9) use decode::decode;
pub(in crate::v9) use encode::encode;

use super::value::{JsString, JsValue, ObjectId};
use super::vm::data::{Bytes, Class, ElementKind, ViewData};
use super::vm::heap::{Key, Object};
use super::vm::realm::realm;
use super::vm::{Res, Vm};
use crate::protocol::{EngineError, EngineResult};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/** One message has a fixed storage budget independent of either VM's heap allowance. */
pub const WORKER_MESSAGE_BYTES: usize = 1024 * 1024;
/** Object count and recursion are independently bounded to prevent tiny-object graph attacks. */
const NODES: usize = 4096;
const DEPTH: usize = 64;

/** The serialized graph preserves cycles, aliases, sparse arrays and overlapping typed views. */
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerData {
    root: Atom,
    nodes: Vec<Node>,
}
/** Primitive variants are validated; references address only this packet's node table. */
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Atom {
    Value(Primitive),
    Reference(usize),
}
/** Number bits retain NaN, infinities and signed zero without JSON number coercion. */
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Primitive {
    Undefined,
    Null,
    Boolean(bool),
    Number(u64),
    String(JsString),
    BigInt(super::value::BigInteger),
}
/** Unsupported host objects and transfer types are refused rather than flattened. */
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Kind {
    Plain,
    Array(u32),
    Buffer(Vec<u8>),
    View(ElementKind, usize, usize, usize),
}
/** Properties retain enumeration order; getters have already been evaluated in the sender. */
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Node {
    kind: Kind,
    fields: Vec<(JsString, Atom)>,
}

/** Only language primitives carry no isolate-local identity or native authority. */
fn primitive(value: &JsValue) -> Option<Primitive> {
    Some(match value {
        JsValue::Undefined => Primitive::Undefined,
        JsValue::Null => Primitive::Null,
        JsValue::Boolean(value) => Primitive::Boolean(*value),
        JsValue::Number(value) => Primitive::Number(value.to_bits()),
        JsValue::String(value) => Primitive::String(value.clone()),
        JsValue::BigInt(_) | JsValue::BigIntWide(_) => {
            Primitive::BigInt(value.bigint_value().expect("BigInt value"))
        }
        #[cfg(feature = "conformance")]
        JsValue::Smi(value) => Primitive::Number(value.to_bits()),
        #[cfg(feature = "conformance")]
        JsValue::LocalNumber(value) => Primitive::Number(value.value.to_bits()),
        _ => return None,
    })
}

impl WorkerData {
    /** Validate before allocation; bounded wire size also covers property names and primitive bytes. */
    pub fn bytes(&self) -> EngineResult<usize> {
        let bytes = serde_json::to_vec(self)
            .map_err(|_| EngineError::invalid("Invalid worker message."))?
            .len();
        let atom = |value: &Atom| match value {
            Atom::Value(_) => true,
            Atom::Reference(index) => *index < self.nodes.len(),
        };
        if bytes > WORKER_MESSAGE_BYTES
            || self.nodes.len() > NODES
            || !atom(&self.root)
            || self
                .nodes
                .iter()
                .any(|node| node.fields.iter().any(|(_, value)| !atom(value)))
        {
            return Err(EngineError::limit(
                "worker message bytes",
                WORKER_MESSAGE_BYTES,
            ));
        }
        for node in &self.nodes {
            if let Kind::View(kind, buffer, offset, length) = node.kind {
                let Some(Node {
                    kind: Kind::Buffer(bytes),
                    ..
                }) = self.nodes.get(buffer)
                else {
                    return Err(EngineError::invalid("Invalid worker buffer reference."));
                };
                let width = super::builtins::typed::element_size(kind);
                if offset % width != 0
                    || length
                        .checked_mul(width)
                        .and_then(|n| offset.checked_add(n))
                        .is_none_or(|end| end > bytes.len())
                {
                    return Err(EngineError::invalid("Invalid worker typed view."));
                }
            }
        }
        Ok(bytes)
    }
}
