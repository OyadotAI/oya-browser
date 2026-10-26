/** Error capture follows actual pending promise reactions to suspended callers, without copying an unbounded ancestry chain. */
use super::{Captured, Vm};
use crate::v9::value::ObjectId;
use crate::v9::vm::data::{Class, PromiseState, ReactionHandler, SavedFrame};
use std::collections::HashSet;

/** Awaiters yield frames; native adoption forwards to another genuine pending promise. */
enum Edge {
    Await(ObjectId),
    Forward(ObjectId),
    All(crate::v9::builtins::AllTrace),
}

impl Captured {
    /** Saved PC/site and the callee's creation context identify a real suspended await continuation. */
    fn continuation(vm: &Vm, frame: &SavedFrame, realm: u32, strict: bool) -> Option<Self> {
        let code = vm.registry.get(frame.site)?;
        let pos = code.position((frame.pc as usize).saturating_sub(1));
        Some(Self {
            function: frame.callee.clone(),
            receiver: frame.this.clone(),
            name: code.name.clone(),
            file: vm.modules.source_name(pos.offset as usize),
            line: pos.line,
            column: pos.column,
            position: pos.offset,
            realm,
            strict,
            construct: false,
            eval: super::eval_source(vm, frame.site),
            site: frame.site,
            asynchronous: true,
            native: false,
            builtin: false,
            wasm: false,
            promise_index: None,
        })
    }
}

impl Captured {
    /** V8's combinator frame has intrinsic function/receiver identities and its actual captured input position. */
    fn combinator(vm: &Vm, info: &crate::v9::builtins::AllTrace) -> Self {
        let intrinsics = vm.object_intrinsics(info.function);
        let name = intrinsics
            .native(info.function)
            .expect("owned Promise.all intrinsic")
            .name;
        Self {
            function: crate::v9::value::JsValue::Object(info.function),
            receiver: crate::v9::value::JsValue::Object(info.receiver),
            name: crate::v9::value::JsString::from_utf8(name).expect("native name"),
            file: String::new(),
            line: 0,
            column: 0,
            position: info.index,
            realm: vm.object_realm(info.function),
            strict: false,
            construct: false,
            eval: false,
            site: 0,
            asynchronous: true,
            native: false,
            builtin: false,
            wasm: false,
            promise_index: Some(info.index),
        }
    }
}

impl Vm<'_> {
    /** Physical frames were already captured; append only unambiguous live await continuations within the same bound. */
    pub(super) fn append_async_trace(
        &self,
        frames: &mut Vec<Captured>,
        strict: &mut bool,
        limit: usize,
    ) {
        #[cfg(feature = "conformance")]
        if !self.page.environment.conformance_async_stack_traces {
            return;
        }
        let Some(current) = self.async_drivers.last().copied() else {
            return;
        };
        let Some(mut promise) = self.async_output(current) else {
            return;
        };
        let mut seen = HashSet::new();
        for _ in 0..self.limits.frames {
            if frames.len() >= limit || !seen.insert(promise) {
                break;
            }
            let Some(edge) = self.continuation_edge(promise) else {
                break;
            };
            let holder = match edge {
                Edge::Forward(next) => {
                    promise = next;
                    continue;
                }
                Edge::All(info) => {
                    let realm = self.object_realm(info.function);
                    if self.trace_frame_visible(realm) {
                        frames.push(Captured::combinator(self, &info));
                    }
                    promise = info.promise;
                    continue;
                }
                Edge::Await(holder) => holder,
            };
            let Some(Class::Async(data)) = self.heap.get(holder).map(|object| &object.class) else {
                break;
            };
            let Some(frame) = &data.frame else { break };
            let Some(code) = self.registry.get(frame.site) else {
                break;
            };
            *strict |= code.strict;
            let realm = self.object_realm(holder);
            if self.trace_frame_visible(realm) {
                let Some(capture) = Captured::continuation(self, frame, realm, *strict) else {
                    break;
                };
                frames.push(capture);
            }
            promise = data.promise;
        }
    }

    /** Await's two vector arms are one reaction; extra subscribers make the continuation ambiguous. */
    fn continuation_edge(&self, promise: ObjectId) -> Option<Edge> {
        let Class::Promise(data) = &self.heap.get(promise)?.class else {
            return None;
        };
        if !matches!(data.state, PromiseState::Pending) || data.reactions.len() != 2 {
            return None;
        }
        let fulfill = data.reactions.iter().find(|reaction| !reaction.reject)?;
        let reject = data.reactions.iter().find(|reaction| reaction.reject)?;
        if fulfill.target != reject.target {
            return None;
        }
        match (&fulfill.handler, &reject.handler) {
            (ReactionHandler::Resume(a), ReactionHandler::Resume(b)) if a == b => {
                Some(Edge::Await(*a))
            }
            (ReactionHandler::Function(a), ReactionHandler::Function(b)) => {
                crate::v9::builtins::promise_all_trace(self, a)
                    .map(Edge::All)
                    .or_else(|| {
                        crate::v9::builtins::promise::forwarding_promise(self, a, b)
                            .map(Edge::Forward)
                    })
            }
            _ => None,
        }
    }

    /** The currently executing holder owns the output promise whose awaiting reaction represents its caller. */
    fn async_output(&self, holder: ObjectId) -> Option<ObjectId> {
        match &self.heap.get(holder)?.class {
            Class::Async(data) => Some(data.promise),
            _ => None,
        }
    }
}
