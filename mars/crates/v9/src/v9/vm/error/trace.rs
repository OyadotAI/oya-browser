/** Error creation records actual JS activations as traced CallSite objects with inert private slots. */
use super::*;
use crate::v9::vm::Frame;

/** Error-owned callsites stay private until the realm's prepareStackTrace hook receives them. */
pub(in crate::v9) const FRAMES_KEY: &str = "errorFrames";
/** The brand and capture context accompany every structured frame across state transport. */
pub(in crate::v9) const CALLSITE_KEY: &str = "callSiteRealm";

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
    asynchronous: bool,
    native: bool,
}
impl Captured {
    /** Source coordinates and function identities come from the actual activation, never parsing text. */
    fn frame(vm: &Vm, frame: &Frame, strict: bool) -> Self {
        let pos = frame.code.position(frame.pc.saturating_sub(1));
        Self {
            function: frame.callee.clone(),
            receiver: frame.this.clone(),
            name: frame.code.name.clone(),
            file: vm.modules.source_name(pos.offset as usize),
            line: pos.line,
            column: pos.column,
            position: pos.offset,
            realm: frame.realm,
            strict,
            construct: frame.construct,
            eval: eval_source(vm, frame.code.site),
            asynchronous: frame.code.flags.asynchronous,
            native: false,
        }
    }
}

/** Dynamic Script sites, rather than ordinary dynamic Function bodies, identify eval activations. */
fn eval_source(vm: &Vm, site: u32) -> bool {
    use crate::v9::syntax::{SITE_SCRIPT, SITE_TAGS};
    if site % SITE_TAGS != SITE_SCRIPT {
        return false;
    }
    let mut base = crate::v9::DYNAMIC_OFFSET;
    vm.dynamic.iter().any(|source| {
        let matches = (base..=base + source.source.len()).contains(&((site / SITE_TAGS) as usize))
            && source.context.is_some();
        base += source.source.len() + 1;
        matches
    })
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
    /** StackTraceLimit is an inert own numeric setting, so capturing an Error cannot run a getter. */
    fn trace_limit(&self) -> usize {
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
            Some(JsValue::Number(limit)) if !limit.is_nan() && *limit > 0.0 => {
                limit.trunc().min(self.limits.frames as f64) as usize
            }
            _ => 0,
        }
    }
    /** Actual callsites are ordinary graph edges, with the same object and retained-value quotas. */
    pub(super) fn record_structured_trace(&mut self, error: ObjectId) -> Result<(), EngineError> {
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
        let mut strict = false;
        let frames: Vec<_> = self
            .frames
            .iter()
            .enumerate()
            .rev()
            .flat_map(|(_index, frame)| {
                strict |= frame.code.strict;
                let mut captures = Vec::new();
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
                            asynchronous: false,
                            native: true,
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
        let mut values = Vec::with_capacity(frames.len());
        for frame in frames {
            values.push(JsValue::Object(self.make_callsite(frame)?));
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
        self.install_stack_accessor(error);
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
    fn make_callsite(&mut self, frame: Captured) -> Result<ObjectId, EngineError> {
        let mut object = Object::new(Class::Plain, Some(realm().ids.call_site_prototype));
        let values = [
            (CALLSITE_KEY, JsValue::Number(frame.realm as f64)),
            ("callSiteFunction", frame.function),
            ("callSiteThis", frame.receiver),
            ("callSiteName", JsValue::String(frame.name)),
            ("callSiteFile", JsValue::string(&frame.file)?),
            ("callSiteLine", JsValue::Number(frame.line as f64)),
            ("callSiteColumn", JsValue::Number(frame.column as f64)),
            ("callSitePosition", JsValue::Number(frame.position as f64)),
            ("callSiteStrict", JsValue::Boolean(frame.strict)),
            ("callSiteConstruct", JsValue::Boolean(frame.construct)),
            ("callSiteEval", JsValue::Boolean(frame.eval)),
            ("callSiteAsync", JsValue::Boolean(frame.asynchronous)),
            ("callSiteNative", JsValue::Boolean(frame.native)),
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
