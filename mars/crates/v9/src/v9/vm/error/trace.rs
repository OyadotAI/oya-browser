/** Error creation records actual JS activations as traced CallSite objects with inert private slots. */
use super::*;
use crate::v9::vm::Frame;
mod async_frames;
mod builtin;

/** Error-owned callsites stay private until the realm's prepareStackTrace hook receives them. */
pub(in crate::v9) const FRAMES_KEY: &str = "errorFrames";
/** The brand and capture context accompany every structured frame across state transport. */
pub(in crate::v9) const CALLSITE_KEY: &str = "callSiteRealm";
/** Newly captured structured traces are authoritative; raw unwind text is a legacy-only fallback. */
pub(in crate::v9) const STRUCTURED_KEY: &str = "errorStructuredTrace";

/** A temporary copy releases the frame stack borrow before bounded heap allocations begin. */
struct Captured {
    function: JsValue,
    receiver: JsValue,
    name: JsString,
    file: String,
    line: u32,
    column: u32,
    position: u32,
    realm: u32,
    strict: bool,
    construct: bool,
    eval: bool,
    site: u32,
    asynchronous: bool,
    native: bool,
    builtin: bool,
    wasm: bool,
    promise_index: Option<u32>,
}
impl Captured {
    /** Source coordinates and function identities come from the actual activation, never parsing text. */
    fn frame(vm: &Vm, frame: &Frame, strict: bool) -> Self {
        let pos = frame.code.position(frame.pc.saturating_sub(1));
        Self {
            function: if frame.code.site % crate::v9::syntax::SITE_TAGS
                == crate::v9::syntax::SITE_SCRIPT
            {
                JsValue::Undefined
            } else {
                frame.callee.clone()
            },
            receiver: frame.this.clone(),
            name: {
                #[cfg(feature = "conformance")]
                { frame.code.name.isolate_code_constant() }
                #[cfg(not(feature = "conformance"))]
                { frame.code.name.clone() }
            },
            file: vm.modules.source_name(pos.offset as usize),
            line: pos.line,
            column: pos.column,
            position: pos.offset,
            realm: frame.realm,
            strict,
            construct: frame.construct,
            eval: eval_source(vm, frame.code.site),
            site: frame.code.site,
            asynchronous: false,
            native: false,
            builtin: false,
            wasm: false,
            promise_index: None,
        }
    }
}

/** Script compilation provenance applies to every activation born in that source, including named closures. */
fn eval_source(vm: &Vm, site: u32) -> bool {
    vm.eval_origin(site).is_some()
}
impl Vm<'_> {
    /** d8 host evaluation has a real native activation even though it executes no JS bytecode itself. */
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn with_trace_boundary<T>(
        &mut self,
        callee: ObjectId,
        receiver: JsValue,
        name: &'static str,
        work: impl FnOnce(&mut Self) -> crate::v9::vm::Res<T>,
    ) -> crate::v9::vm::Res<T> {
        self.trace_boundaries
            .push((self.frames.len(), callee, receiver, self.active_realm, name));
        let result = work(self);
        self.trace_boundaries.pop();
        result
    }
    /** The frame count an Error captures: NaN and non-positive limits capture none. */
    fn trace_limit(&self) -> usize {
        match self.stack_limit() {
            Some(limit) if !limit.is_nan() && limit > 0.0 => {
                limit.trunc().min(self.limits.frames as f64) as usize
            }
            _ => 0,
        }
    }
    /** StackTraceLimit is an inert own numeric setting, so capturing an Error cannot run a getter.
    Only a number here enables `stack`; V8 installs no stack property for any other value. */
    fn stack_limit(&self) -> Option<f64> {
        let ctor = realm().ids.error;
        let key = Key::from("stackTraceLimit");
        let value = match self.heap.get(ctor) {
            Some(object) => object.props.get(&key).and_then(|prop| match &prop.slot {
                Slot::Data(value) => Some(value),
                _ => None,
            }),
            None => realm()
                .object(ctor)
                .and_then(|def| def.get(&key))
                .and_then(|(_, value, _)| match value {
                    crate::v9::vm::realm::Static::Value(value) => Some(value),
                    _ => None,
                }),
        };
        match value {
            Some(JsValue::Number(limit)) => Some(*limit),
            _ => None,
        }
    }
    /** Actual callsites are ordinary graph edges, with the same object and retained-value quotas. */
    pub(super) fn record_structured_trace(
        &mut self,
        error: ObjectId,
        hide_top_builtin: bool,
    ) -> Result<(), EngineError> {
        // The recording builtin is the youngest activation of its own capture, so it is dropped first.
        let hidden = if hide_top_builtin {
            self.native_activations.len().checked_sub(1)
        } else {
            None
        };
        if let Some(object) = self.heap.get(error)
            && matches!(object.class, Class::Error)
            && let Some(prop) = object.props.get(&Key::from("message"))
            && let Slot::Data(message) = &prop.slot
        {
            self.heap.put(
                error,
                Key::private("errorOriginalMessage"),
                Prop {
                    slot: Slot::Data(message.clone()),
                    attrs: 0,
                },
            );
        }
        // A non-number StackTraceLimit installs no `stack` at all, so there is nothing to capture.
        if self.stack_limit().is_none() {
            return Ok(());
        }
        let mut strict = false;
        let mut frames: Vec<_> = self
            .frames
            .iter()
            .enumerate()
            .rev()
            .flat_map(|(_index, frame)| {
                let mut captures = Vec::new();
                let visible = |(slot, activation): &(usize, &crate::v9::vm::NativeActivation)| {
                    activation.depth == _index + 1 && Some(*slot) != hidden
                };
                for (_, activation) in self
                    .native_activations
                    .iter()
                    .enumerate()
                    .rev()
                    .filter(visible)
                {
                    if self.trace_frame_visible(activation.realm) {
                        // Real JS builtins have strict SharedFunctionInfo in V8; opacity starts
                        // here, after younger callbacks and before this older script frame. A
                        // builtin with no younger JS frame (a plain `new Error()`) hides nothing.
                        if matches!(&activation.kind, super::super::NativeKind::Builtin { .. })
                            && _index + 1 < self.frames.len()
                        {
                            strict = true;
                        }
                        captures.push(Captured::native(self, activation, strict));
                    }
                }
                strict |= frame.code.strict;
                #[cfg(feature = "conformance")]
                for (_, callee, receiver, bank, name) in self
                    .trace_boundaries
                    .iter()
                    .rev()
                    .filter(|boundary| boundary.0 == _index + 1)
                {
                    if self.trace_frame_visible(*bank) {
                        captures.push(Captured {
                            function: JsValue::Object(*callee),
                            receiver: receiver.clone(),
                            name: JsString::from_utf8(name).expect("native name"),
                            file: String::new(),
                            line: 0,
                            column: 0,
                            position: 0,
                            realm: *bank,
                            strict,
                            construct: false,
                            eval: false,
                            site: 0,
                            asynchronous: false,
                            native: true,
                            builtin: false,
                            wasm: false,
                            promise_index: None,
                        });
                    }
                }
                if self.trace_frame_visible(frame.realm) {
                    captures.push(Captured::frame(self, frame, strict));
                }
                captures
            })
            .take(self.trace_limit())
            .collect();
        // A genuine native timer entry can have no older physical JS caller at depth zero.
        for activation in self
            .native_activations
            .iter()
            .rev()
            .filter(|activation| activation.depth == 0)
        {
            if frames.len() >= self.trace_limit() {
                break;
            }
            if self.trace_frame_visible(activation.realm) {
                if matches!(&activation.kind, super::super::NativeKind::Builtin { .. }) {
                    strict = true;
                }
                frames.push(Captured::native(self, activation, strict));
            }
        }
        self.append_async_trace(&mut frames, &mut strict, self.trace_limit());
        let roots = self.pins.len();
        self.pin(JsValue::Object(error));
        for frame in &frames {
            self.pin(frame.function.clone());
            self.pin(frame.receiver.clone());
        }
        let result = self.store_trace_frames(error, frames);
        self.pins.truncate(roots);
        result
    }
    /** Every newly allocated callsite is pinned until its Error-owned array is installed. */
    fn store_trace_frames(
        &mut self,
        error: ObjectId,
        frames: Vec<Captured>,
    ) -> Result<(), EngineError> {
        let mut values = Vec::with_capacity(frames.len());
        for frame in frames {
            let id = self.make_callsite(frame)?;
            self.pin(JsValue::Object(id));
            values.push(JsValue::Object(id));
        }
        let frames = self.heap.insert(Object::new(
            Class::Array(values.into()),
            Some(realm().ids.array_prototype),
        ))?;
        self.heap.put(
            error,
            Key::private(FRAMES_KEY),
            Prop {
                slot: Slot::Data(JsValue::Object(frames)),
                attrs: 0,
            },
        );
        if self.stack_limit().is_some() {
            self.install_stack_accessor(error);
        }
        self.heap.put(
            error,
            Key::private(STRUCTURED_KEY),
            Prop {
                slot: Slot::Data(JsValue::Boolean(true)),
                attrs: 0,
            },
        );
        Ok(())
    }
    /** Context security tokens filter ordinary realms; ShadowRealm frames remain subject to access checks. */
    fn trace_frame_visible(&self, bank: u32) -> bool {
        #[cfg(feature = "conformance")]
        {
            let current = &self.realms.contexts[self.active_realm as usize];
            let other = &self.realms.contexts[bank as usize];
            return current.shadow || other.shadow || current.token == other.token;
        }
        #[cfg(not(feature = "conformance"))]
        {
            let _ = bank;
            true
        }
    }
    /** Each instance has its own lazy stack descriptor, including captureStackTrace targets. */
    fn install_stack_accessor(&mut self, error: ObjectId) {
        let key = Key::from("stack");
        if let Some((_, crate::v9::vm::realm::Static::Accessor(get, set), attrs)) = realm()
            .object(realm().ids.error_prototype)
            .and_then(|def| def.get(&Key::private("errorStackAccessor")))
        {
            self.heap.put(
                error,
                key,
                Prop {
                    slot: Slot::Accessor(*get, *set),
                    attrs: *attrs,
                },
            );
        }
    }
    /** Frame data is inaccessible to JavaScript except through the checked CallSite API. */
    fn make_callsite(&mut self, mut frame: Captured) -> Result<ObjectId, EngineError> {
        let mut wasm_function = JsValue::Null;
        let mut wasm_module = JsValue::Null;
        if frame.wasm {
            let instance = frame
                .receiver
                .object()
                .ok_or_else(|| EngineError::invalid("Invalid captured Wasm instance."))?;
            let JsValue::Number(index) = &frame.function else {
                return Err(EngineError::invalid(
                    "Invalid captured Wasm function index.",
                ));
            };
            let metadata = crate::v9::builtins::wasm_trace_metadata(self, instance, *index as u32)?;
            frame.name = metadata.function.clone().unwrap_or_else(JsString::empty);
            frame.file = metadata.file;
            wasm_function = metadata.function.map_or(JsValue::Null, JsValue::String);
            wasm_module = metadata.module.map_or(JsValue::Null, JsValue::String);
        }
        let mut object = Object::new(Class::Plain, Some(realm().ids.call_site_prototype));
        let origin = self.eval_origin(frame.site);
        let eval_origin = origin.map_or(Ok(JsValue::Undefined), |origin| {
            JsValue::string(origin.display())
        })?;
        let source_url = match origin {
            Some(origin) => origin
                .source_url
                .as_deref()
                .map_or(Ok(JsValue::Undefined), JsValue::string)?,
            None if frame.builtin => JsValue::Undefined,
            None => JsValue::string(&frame.file)?,
        };
        let values = [
            (CALLSITE_KEY, JsValue::Number(frame.realm as f64)),
            ("callSiteFunction", frame.function),
            ("callSiteThis", frame.receiver),
            ("callSiteName", JsValue::String(frame.name)),
            (
                "callSiteFile",
                if frame.eval || frame.builtin {
                    JsValue::Undefined
                } else {
                    JsValue::string(&frame.file)?
                },
            ),
            ("callSiteSourceURL", source_url),
            ("callSiteEvalOrigin", eval_origin),
            (
                "callSiteLine",
                if frame.builtin {
                    JsValue::Null
                } else {
                    JsValue::Number(frame.line as f64)
                },
            ),
            (
                "callSiteColumn",
                if frame.builtin {
                    JsValue::Null
                } else {
                    JsValue::Number(frame.column as f64)
                },
            ),
            (
                "callSitePosition",
                if frame.builtin {
                    JsValue::Null
                } else {
                    JsValue::Number(frame.position as f64)
                },
            ),
            ("callSiteStrict", JsValue::Boolean(frame.strict)),
            ("callSiteConstruct", JsValue::Boolean(frame.construct)),
            ("callSiteEval", JsValue::Boolean(frame.eval)),
            ("callSiteAsync", JsValue::Boolean(frame.asynchronous)),
            ("callSiteNative", JsValue::Boolean(frame.native)),
            ("callSiteWasm", JsValue::Boolean(frame.wasm)),
            ("callSiteWasmFunctionName", wasm_function),
            ("callSiteWasmModuleName", wasm_module),
            (
                "callSitePromiseAll",
                JsValue::Boolean(frame.promise_index.is_some()),
            ),
            (
                "callSitePromiseIndex",
                frame
                    .promise_index
                    .map_or(JsValue::Null, |index| JsValue::Number(index as f64)),
            ),
        ];
        for (name, value) in values {
            object.props.insert(
                Key::private(name),
                Prop {
                    slot: Slot::Data(value),
                    attrs: 0,
                },
            );
        }
        self.heap.insert(object)
    }
}
