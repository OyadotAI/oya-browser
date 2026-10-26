/** Selected real builtin activations enrich Error capture while remaining transient, bounded, and explicitly rooted. */
use super::*;

/** Native entry records retain actual identities and their position between physical JS frames. */
pub(in crate::v9) struct NativeActivation {
    /// The native call already counts against the existing shared frame limit.
    pub depth: usize,
    /// Aliases and bound/call/apply forwarding preserve this genuine native target.
    pub kind: NativeKind,
    /// Visibility uses the same creation realm/security token rules as physical JS frames.
    pub realm: u32,
    /// Only this entry's roots are cleared; native allocations keep their existing owning scope.
    roots: std::ops::Range<usize>,
}

/** The shared order is actual entry order even when native builtins and Wasm nest at one JS depth. */
pub(in crate::v9) enum NativeKind {
    Builtin {
        callee: ObjectId,
        receiver: JsValue,
    },
    /// Actual embedder API functions have sloppy metadata and no source Script.
    #[cfg(feature = "conformance")]
    Api {
        callee: ObjectId,
        receiver: JsValue,
    },
    Wasm {
        instance: ObjectId,
        index: u32,
        position: usize,
    },
}

impl Vm<'_> {
    /** All normal and abrupt completions remove the activation and release its temporary GC roots. */
    pub(in crate::v9) fn with_builtin_activation<T>(
        &mut self,
        callee: ObjectId,
        receiver: JsValue,
        work: impl FnOnce(&mut Self) -> Res<T>,
    ) -> Res<T> {
        self.with_function_activation(
            callee,
            receiver.clone(),
            NativeKind::Builtin { callee, receiver },
            work,
        )
    }

    /** The real shell flag exposes API entries; ordinary execution keeps them hidden as in V8. */
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn with_api_activation<T>(
        &mut self,
        callee: ObjectId,
        receiver: JsValue,
        work: impl FnOnce(&mut Self) -> Res<T>,
    ) -> Res<T> {
        if !self
            .page
            .environment
            .conformance_experimental_stack_trace_frames
        {
            return work(self);
        }
        self.with_function_activation(
            callee,
            receiver.clone(),
            NativeKind::Api { callee, receiver },
            work,
        )
    }

    /** Actual function entry identities share one bounded ordering and their exact temporary root scope. */
    fn with_function_activation<T>(
        &mut self,
        callee: ObjectId,
        receiver: JsValue,
        kind: NativeKind,
        work: impl FnOnce(&mut Self) -> Res<T>,
    ) -> Res<T> {
        let slot = self.native_activations.len();
        let first = self.pins.len();
        self.pins
            .extend([JsValue::Object(callee), receiver.clone()]);
        self.native_activations.push(NativeActivation {
            depth: self.frames.len(),
            kind,
            realm: self.object_realm(callee),
            roots: first..self.pins.len(),
        });
        let result = work(self);
        self.end_native_activation(slot);
        result
    }

    /** An interpreter entry retains only its actual instance and scalar source metadata. */
    pub(in crate::v9) fn begin_wasm_activation(
        &mut self,
        instance: ObjectId,
        index: u32,
        position: usize,
    ) -> usize {
        let slot = self.native_activations.len();
        let first = self.pins.len();
        self.pins.push(JsValue::Object(instance));
        self.native_activations.push(NativeActivation {
            depth: self.frames.len(),
            kind: NativeKind::Wasm {
                instance,
                index,
                position,
            },
            realm: self.object_realm(instance),
            roots: first..self.pins.len(),
        });
        slot
    }

    /** The decoder's Located opcode supplies the position before any callback or trap can capture it. */
    pub(in crate::v9) fn set_wasm_position(&mut self, slot: usize, position: usize) -> Res<()> {
        match self
            .native_activations
            .get_mut(slot)
            .map(|entry| &mut entry.kind)
        {
            Some(NativeKind::Wasm {
                position: current, ..
            }) => {
                *current = position;
                Ok(())
            }
            _ => Err(EngineError::invalid("Invalid active WebAssembly trace position.").into()),
        }
    }

    /** Abrupt completion and tail replacement clear only entries owned by this real activation scope. */
    pub(in crate::v9) fn end_native_activation(&mut self, slot: usize) {
        while self.native_activations.len() > slot {
            let entry = self.native_activations.pop().expect("checked active scope");
            for root in entry.roots.rev() {
                if root + 1 == self.pins.len() {
                    self.pins.pop();
                } else {
                    self.pins[root] = JsValue::Undefined;
                }
            }
        }
    }
}
