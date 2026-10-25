/** Script error objects, and how uncaught exceptions become engine errors with a location. */
use super::data::Class;
use super::heap::{HIDDEN_ATTRS, Key, Object, Prop, Slot};
use super::realm::realm;
use super::{Thrown, Vm};
use crate::protocol::{EngineError, SourceLocation};
use crate::v9::value::{JsString, JsValue, ObjectId};

/** Error constructors scripts can see. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::v9) enum ErrorKind {
    Error,
    Type,
    Range,
    Reference,
    Syntax,
    Uri,
}

/** The hidden own property an error's creation trace lives under; `stack` reads it. The leading
control character keeps it clear of any name a page would use. */
pub(in crate::v9) const TRACE_KEY: &str = "\u{1}trace";
impl Vm<'_> {
    /** A new error object of a kind with a message. */
    pub fn make_error(&mut self, kind: ErrorKind, message: &str) -> Result<ObjectId, EngineError> {
        let ids = &realm().ids;
        let proto = match kind {
            ErrorKind::Error => ids.error_prototype,
            ErrorKind::Type => ids.type_error_prototype,
            ErrorKind::Range => ids.range_error_prototype,
            ErrorKind::Reference => ids.reference_error_prototype,
            ErrorKind::Syntax => ids.syntax_error_prototype,
            ErrorKind::Uri => ids.uri_error_prototype,
        };
        self.make_error_from(proto, message)
    }
    /** An error object under `proto` carrying `message` and the current trace. */
    fn make_error_from(&mut self, proto: ObjectId, message: &str) -> Result<ObjectId, EngineError> {
        let id = self.heap.insert(Object::new(Class::Error, Some(proto)))?;
        if self.pinning {
            self.pins.push(JsValue::Object(id));
        }
        let text: String = message.chars().take(MESSAGE_CHARS).collect();
        self.heap.put(
            id,
            Key::from("message"),
            Prop {
                slot: Slot::Data(JsValue::String(JsString::from_utf8(&text)?)),
                attrs: HIDDEN_ATTRS,
            },
        );
        self.record_trace(id)?;
        Ok(id)
    }
    /** Keep the activations an error was made in, hidden on it, for its `stack` property. */
    pub fn record_trace(&mut self, id: ObjectId) -> Result<(), EngineError> {
        let trace = self.throw_trace().join("\n");
        self.heap.put(
            id,
            Key::from(TRACE_KEY),
            Prop {
                slot: Slot::Data(JsValue::String(JsString::from_utf8(&trace)?)),
                attrs: HIDDEN_ATTRS,
            },
        );
        Ok(())
    }
    /** A thrown error of a kind; allocation failures become engine errors. */
    pub fn throw(&mut self, kind: ErrorKind, message: &str) -> Thrown {
        match self.make_error(kind, message) {
            Ok(id) => Thrown::Value(JsValue::Object(id)),
            Err(error) => Thrown::Fatal(error),
        }
    }
    /** A thrown DOMException (QuotaExceededError, DataCloneError): `instanceof DOMException`
    and `instanceof Error` both hold, and `name` and `code` are what pages test for. */
    pub fn dom_exception(&mut self, name: &str, message: &str) -> Thrown {
        let id = match self.make_error_from(realm().ids.dom_exception_prototype, message) {
            Ok(id) => id,
            Err(error) => return Thrown::Fatal(error),
        };
        let code = f64::from(crate::v9::builtins::legacy_code(name));
        match JsString::from_utf8(name) {
            Ok(text) => {
                for (property, value) in [
                    ("name", JsValue::String(text)),
                    ("code", JsValue::Number(code)),
                ] {
                    self.heap.put(
                        id,
                        Key::from(property),
                        Prop {
                            slot: Slot::Data(value),
                            attrs: HIDDEN_ATTRS,
                        },
                    );
                }
                Thrown::Value(JsValue::Object(id))
            }
            Err(error) => Thrown::Fatal(error),
        }
    }
    /** A TypeError. */
    pub fn type_error(&mut self, message: &str) -> Thrown {
        self.throw(ErrorKind::Type, message)
    }
    /** A RangeError. */
    pub fn range_error(&mut self, message: &str) -> Thrown {
        self.throw(ErrorKind::Range, message)
    }
    /** A ReferenceError. */
    pub fn reference_error(&mut self, message: &str) -> Thrown {
        self.throw(ErrorKind::Reference, message)
    }
    /** A SyntaxError. */
    pub fn syntax_error(&mut self, message: &str) -> Thrown {
        self.throw(ErrorKind::Syntax, message)
    }
    /**
     * A feature V9 does not have yet. Scripts can catch it as a TypeError (feature detection keeps
     * working); if nothing catches it, the script fails as unsupported_feature.
     */
    pub fn unsupported_error(&mut self, feature: &str) -> Thrown {
        let message = format!("This JavaScript feature is not supported yet: {feature}");
        let thrown = self.type_error(&message);
        if let Thrown::Value(JsValue::Object(id)) = &thrown {
            self.unsupported = Some((*id, EngineError::unsupported(format!("{message}."))));
            self.page.note_unsupported(feature);
        }
        thrown
    }
    /** Turn an uncaught exception into the engine error a failed script reports. */
    pub fn uncaught(&mut self, thrown: Thrown, location: SourceLocation) -> EngineError {
        let value = match thrown {
            Thrown::Fatal(error) => return locate(error, location),
            Thrown::Value(value) => value,
        };
        if let (JsValue::Object(id), Some((unsupported, error))) = (&value, &self.unsupported)
            && id == unsupported
        {
            return locate(error.clone(), location);
        }
        let description = self.describe(&value);
        // One closing period, whether or not the JavaScript message brought its own.
        let description = description.strip_suffix('.').unwrap_or(&description);
        locate(
            EngineError::new(
                "script_execution_error",
                format!(
                    "Script execution failed at line {}, column {}: Uncaught {description}.",
                    location.line, location.column
                ),
                "Correct the script; that script's changes were not applied.",
            ),
            location,
        )
    }
    /** `Name: message` for errors, a short form for other values; never runs script code. */
    pub fn describe(&mut self, value: &JsValue) -> String {
        if let JsValue::Object(id) = value {
            let name = self.peek_data(*id, "name");
            let message = self.peek_data(*id, "message");
            if let Some(message) = message {
                let name = name.unwrap_or_else(|| "Error".into());
                return if message.is_empty() {
                    name
                } else {
                    format!("{name}: {message}")
                };
            }
            return "object".into();
        }
        match value {
            JsValue::String(text) => text.lossy().chars().take(MESSAGE_CHARS).collect(),
            other => format!("{other:?}"),
        }
    }
    /** A data property along the chain, read without running getters. */
    fn peek_data(&mut self, mut id: ObjectId, name: &str) -> Option<String> {
        let key = Key::from(name);
        for _ in 0..PEEK_DEPTH {
            if let Ok(Some(prop)) = self.own(id, &key) {
                return match prop.slot {
                    Slot::Data(JsValue::String(text)) => Some(text.lossy()),
                    _ => None,
                };
            }
            id = self.proto_of(id)?;
        }
        None
    }
}

/** Characters of an error message kept in diagnostics. */
const MESSAGE_CHARS: usize = 300;
/** Prototype depth searched for an error's name and message. */
const PEEK_DEPTH: usize = 8;

/** Attach a location when the error has none. */
pub(in crate::v9) fn locate(mut error: EngineError, location: SourceLocation) -> EngineError {
    if error.location.is_none() {
        error.location = Some(Box::new(location));
        error.phase = Some("execute".into());
        error.input_kind = Some("script".into());
    }
    error
}
