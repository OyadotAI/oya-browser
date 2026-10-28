/** Script error objects, and how uncaught exceptions become engine errors with a location. */
use super::data::Class;
use super::heap::{HIDDEN_ATTRS, Key, Object, Prop, Slot};
use super::realm::realm;
use super::{Thrown, Vm};
use crate::protocol::{EngineError, SourceLocation};
use crate::v9::value::{JsString, JsValue, ObjectId};
mod trace;
pub(in crate::v9) use trace::{CALLSITE_KEY, FRAMES_KEY, STRUCTURED_KEY};

/** Error constructors scripts can see. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::v9) enum ErrorKind {
    Error,
    Type,
    Range,
    Reference,
    Syntax,
    Eval,
    Uri,
}

/** The private key an error's creation trace lives under; `stack` reads it. A page can neither
name nor forge it. */
pub(in crate::v9) const TRACE_KEY: &str = "errorTrace";
impl Vm<'_> {
    /** Inspect ordinary prototype data for a diagnostic without invoking accessors or proxy traps. */
    pub(in crate::v9) fn inert_constructor_name(&mut self, mut id: ObjectId) -> Option<String> {
        let constructor = Key::constructor();
        for _ in 0..PEEK_DEPTH {
            let object = self.heap.get(id)?;
            if !matches!(object.class, Class::Plain) { return None; }
            let next = object.proto;
            if let Some(prop) = object.props.get(&constructor) {
                let Slot::Data(JsValue::Object(ctor)) = prop.slot else { return None; };
                if self.heap.get(ctor).and_then(|object| object.function()).is_none() { return None; }
                return match self.own(ctor, &Key::from("name")).ok().flatten()?.slot {
                    Slot::Data(JsValue::String(name)) => Some(name.lossy()),
                    _ => None,
                };
            }
            id = next?;
        }
        None
    }
    /** Only the exact thrown unsupported value carries native-failure provenance into callback diagnostics. */
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn note_conformance_native_error(&mut self, value: &JsValue) {
        if self.page.effects.conformance_native_error.is_none()
            && let Some((id, error)) = &self.unsupported
            && value.object() == Some(*id)
        {
            self.page.effects.conformance_native_error =
                Some((error.code.clone(), error.message.clone()));
        }
    }
    /** A new error object of a kind with a message. */
    pub fn make_error(&mut self, kind: ErrorKind, message: &str) -> Result<ObjectId, EngineError> {
        let ids = &realm().ids;
        let proto = match kind {
            ErrorKind::Error => ids.error_prototype,
            ErrorKind::Type => ids.type_error_prototype,
            ErrorKind::Range => ids.range_error_prototype,
            ErrorKind::Reference => ids.reference_error_prototype,
            ErrorKind::Syntax => ids.syntax_error_prototype,
            ErrorKind::Eval => ids.eval_error_prototype,
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
        self.record_trace_with(id, false)
    }
    /** A native that records its own error is the youngest frame of that capture, which V8 never shows. */
    pub fn record_builtin_trace(&mut self, id: ObjectId) -> Result<(), EngineError> {
        self.record_trace_with(id, true)
    }
    /** Shared capture: `hide_top_builtin` drops the recording builtin before the frame limit applies. */
    fn record_trace_with(
        &mut self,
        id: ObjectId,
        hide_top_builtin: bool,
    ) -> Result<(), EngineError> {
        self.record_structured_trace(id, hide_top_builtin)?;
        let trace = self.throw_trace().join("\n");
        self.heap.put(
            id,
            Key::private(TRACE_KEY),
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
        #[cfg(feature = "conformance")]
        let description = if description == "object" || self.is_conformance_assertion(&value) {
            // The runner's lazy message getters are page code; a failing script keeps its own
            // effects, but nothing the diagnostic itself runs may reach the page.
            let saved = crate::v9::snapshot(self);
            let read = self.conformance_description(&value);
            crate::v9::restore(self, saved);
            read.unwrap_or(description)
        } else {
            description
        };
        // One closing period, whether or not the JavaScript message brought its own.
        let description = description.strip_suffix('.').unwrap_or(&description);
        locate(
            EngineError::new(
                "script_execution_error",
                format!(
                    "Script execution failed at line {}, column {}: Uncaught {description}.",
                    location.line, location.column
                ),
                "Correct the script; what it did before the error stays, as in browsers.",
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
                // An error-like object without a `name` (test262's Test262Error) is called
                // after its constructor, as V8's message for it reads.
                let name = name
                    .or_else(|| self.constructor_name(*id).filter(|name| name != "Object"))
                    .unwrap_or_else(|| "Error".into());
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
    /** The opt-in upstream runner may read a lazy assertion message after a terminal throw.
    The test has already failed; getter failures retain the original failure and normal bounds
    still apply. Production diagnostics never execute getters. */
    #[cfg(feature = "conformance")]
    /** File wrappers retain only assertion provenance, without retaining the original thrown graph. */
    pub(in crate::v9) fn is_conformance_assertion(&mut self, value: &JsValue) -> bool {
        value.object().is_some_and(|id| {
            self.heap
                .get(id)
                .and_then(|o| o.props.get(&Key::private("conformance-file-assertion")))
                .is_some()
                || self.constructor_name(id).as_deref() == Some("MjsUnitAssertionError")
        })
    }
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn conformance_description(&mut self, value: &JsValue) -> Option<String> {
        let JsValue::Object(id) = value else {
            return None;
        };
        let JsValue::String(message) = self.get(value, &Key::from("message")).ok()? else {
            return None;
        };
        let name = self
            .constructor_name(*id)
            .unwrap_or_else(|| "object".into());
        let message: String = message.lossy().chars().take(MESSAGE_CHARS).collect();
        let mut description = format!("{name}: {message}");
        if name != "MjsUnitAssertionError" && self.is_conformance_assertion(value) {
            description.push_str(" (caused by MjsUnitAssertionError)");
        }
        if name == "MjsUnitAssertionError" {
            if let Some(stack) = self.peek_data(*id, "stack") {
                let lines: Vec<_> = stack.lines().rev().take(DIAGNOSTIC_FRAMES).collect();
                for line in lines.into_iter().rev() {
                    description.push('\n');
                    description.push_str(line);
                }
            }
        }
        Some(description)
    }
    /** What a value is, for a message that names it (`NodeList`, `undefined`, `number`), read
    without running page code. */
    pub(in crate::v9) fn kind_name(&mut self, value: &JsValue) -> String {
        match value {
            JsValue::Object(id) => self
                .constructor_name(*id)
                .unwrap_or_else(|| "object".into()),
            JsValue::Undefined => "undefined".into(),
            JsValue::Null => "null".into(),
            JsValue::String(_) => "string".into(),
            JsValue::Boolean(_) => "boolean".into(),
            _ => "value".into(),
        }
    }
    /** The `name` of the object's `constructor`, read without running getters. */
    fn constructor_name(&mut self, mut id: ObjectId) -> Option<String> {
        let key = Key::constructor();
        for _ in 0..PEEK_DEPTH {
            if let Ok(Some(prop)) = self.own(id, &key) {
                return match prop.slot {
                    Slot::Data(JsValue::Object(ctor)) => self.peek_data(ctor, "name"),
                    _ => None,
                };
            }
            id = self.proto_of(id)?;
        }
        None
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

/** The final frames identify the upstream assertion caller without retaining an unbounded trace. */
#[cfg(feature = "conformance")]
const DIAGNOSTIC_FRAMES: usize = 3;

/** Attach a location when the error has none. */
pub(in crate::v9) fn locate(mut error: EngineError, location: SourceLocation) -> EngineError {
    if error.location.is_none() {
        error.location = Some(Box::new(location));
        error.phase = Some("execute".into());
        error.input_kind = Some("script".into());
    }
    error
}
