/** Cooperative safepoints observe the real interpreter stack at real native times, without replaying missed ticks. */
use super::{Clock, model::CallFrame};
use crate::v9::vm::{NativeKind, Res, Vm, data::FunctionKind};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

impl Clock {
    /** A native monotonic anchor prevents wall-clock adjustments during one VM execution. */
    pub(in crate::v9) fn new(floor: u64) -> Self {
        Self {
            anchor: Instant::now(),
            epoch: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_micros()
                .min(u64::MAX as u128) as u64,
        }
        .with_floor(floor)
    }
    /** A restored snapshot cannot make timestamps run backwards after a host wall-clock correction. */
    fn with_floor(mut self, floor: u64) -> Self {
        self.epoch = self.epoch.max(floor);
        self
    }
    /** State transport carries timestamps, never an untrusted native clock or thread handle. */
    pub(super) fn now(&self) -> u64 {
        self.epoch
            .saturating_add(self.anchor.elapsed().as_micros().min(u64::MAX as u128) as u64)
    }
}

/** The interpreter already checks time here; inactive sessions allocate nothing and read no clock. */
pub(in crate::v9) fn tick(vm: &mut Vm) -> Res<()> {
    let Some(profile) = &vm.page.state.cpu_profile.session else {
        return Ok(());
    };
    let interval = vm.page.environment.conformance_cpu_profiler_interval_us;
    let now = vm.profile_clock.now();
    if now.saturating_sub(profile.end_time) >= interval {
        sample(vm)?;
    }
    Ok(())
}

/** Explicit collection and profile start each observe the current stack, even before an interval elapses. */
pub(super) fn sample(vm: &mut Vm) -> Res<()> {
    if vm.page.state.cpu_profile.session.is_none() {
        return Ok(());
    }
    super::bounds(vm)?;
    let frames = capture(vm)?;
    let now = vm.profile_clock.now();
    vm.page
        .state
        .cpu_profile
        .session
        .as_mut()
        .expect("active profiler")
        .record(frames, now);
    super::bounds(vm)
}

/** Oldest-to-youngest physical activation order creates an ordinary rooted profile call tree. */
fn capture(vm: &Vm) -> Res<Vec<CallFrame>> {
    let mut frames = Vec::new();
    for depth in 0..=vm.frames.len() {
        for activation in vm
            .native_activations
            .iter()
            .filter(|entry| entry.depth == depth)
        {
            frames.push(native(vm, &activation.kind)?);
        }
        if let Some(frame) = vm.frames.get(depth) {
            let position = frame.code.position(0);
            frames.push(CallFrame {
                function_name: frame.code.name.lossy(),
                script_id: frame.code.site,
                url: vm.modules.source_name(position.offset as usize),
                line_number: position.line as i32 - 1,
                column_number: position.column as i32 - 1,
            });
        }
    }
    Ok(frames)
}

/** Wasm names and source identities come from the actual instance bytes; absent names retain numeric identity. */
fn native(vm: &Vm, kind: &NativeKind) -> Res<CallFrame> {
    if let NativeKind::Wasm {
        instance,
        index,
        position,
    } = kind
    {
        let metadata = crate::v9::builtins::wasm_trace_metadata(vm, *instance, *index)?;
        return Ok(CallFrame {
            function_name: metadata
                .function
                .map_or_else(|| format!("wasm-function[{index}]"), |name| name.lossy()),
            script_id: *instance,
            url: metadata.file,
            line_number: 0,
            column_number: (*position).min(i32::MAX as usize) as i32,
        });
    }
    let callee = match kind {
        NativeKind::Builtin { callee, .. } | NativeKind::Api { callee, .. } => *callee,
        NativeKind::Wasm { .. } => unreachable!("handled Wasm"),
    };
    let intrinsics = vm.object_intrinsics(callee);
    let definition = intrinsics.native(callee).or_else(|| {
        let FunctionKind::Native { id, .. } = &vm.heap.get(callee)?.function()?.kind else {
            return None;
        };
        intrinsics.natives.get(*id as usize)
    });
    Ok(CallFrame {
        function_name: definition.map_or("", |native| native.name).into(),
        script_id: 0,
        url: String::new(),
        line_number: -1,
        column_number: -1,
    })
}
