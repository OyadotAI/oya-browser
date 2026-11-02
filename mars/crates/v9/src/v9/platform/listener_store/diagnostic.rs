/** Bounded quota diagnostics identify native listener pressure without disclosing page event names. */
use super::{ListenerStore, Target};
use crate::protocol::EngineError;

/** Existing navigation reasons retain 240 characters; leave room for compact location suffixes. */
const MESSAGE_BYTES: usize = 220;

/** Distinguish executing script registration from restored state or final invariant validation. */
#[derive(Clone, Copy)]
pub(super) enum Stage {
    Add,
    Restore,
    Validate,
}

impl Stage {
    /** Labels come solely from trusted native call sites. */
    fn label(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Restore => "restore",
            Self::Validate => "validate",
        }
    }
}

/** Keep the original code, hint and limit; append only bounded counts from the native registry. */
pub(super) fn annotate(
    store: &ListenerStore,
    mut error: EngineError,
    stage: Stage,
    kind: Option<&String>,
) -> EngineError {
    let mut targets = [0usize; 4];
    for listener in store.iter() {
        let index = match listener.target {
            Target::Window => 0,
            Target::Document => 1,
            Target::Node(_) => 2,
            Target::Object(_) => 3,
        };
        targets[index] += 1;
    }
    let (length, capacity) = kind.map_or((0, 0), |kind| (kind.len(), kind.capacity()));
    error.message = format!(
        "{} [{} n={} used={} kind={}/{} W/D/N/O={}/{}/{}/{}]",
        error.message.trim_end_matches('.'),
        stage.label(),
        store.len(),
        store.bytes,
        length,
        capacity,
        targets[0],
        targets[1],
        targets[2],
        targets[3],
    );
    // Every component is native ASCII; pathological forged sizes cannot expand UI diagnostics.
    error.message.truncate(MESSAGE_BYTES);
    error
}
