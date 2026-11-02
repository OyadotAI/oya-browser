/** Broadcast routing carries UTF-16 names and bounded structured-clone packets, never VM identities. */
use super::*;
use crate::v9::WorkerData;
use serde::{Deserialize, Serialize};

/** Retained wrappers, including still-referenced closed ones, remain bounded per context. */
pub const BROADCAST_CHANNELS: usize = 256;
/** Names preserve all DOMString code units, including unmatched surrogates. */
pub const BROADCAST_NAME_UNITS: usize = 4096;
/** Open, post and explicit close share one ordered bounded command stream. */
pub const BROADCAST_COMMANDS: usize = 256;
/** Inert effects consume at most one quarter of the unchanged encoded VM frame allowance. */
pub const BROADCAST_EFFECT_BYTES: usize = crate::constants::VM_STATE_BYTES / 4;
/** Maximal numeric IDs, punctuation and variant names are independently charged. */
const COMMAND_OVERHEAD: usize = 64;
/** A UTF-16 unit occupies at most five decimal digits and one separator in the wire array. */
const UNIT_WIRE_BYTES: usize = 6;

/** A final live-endpoint snapshot reconciles garbage collection after ordered effects are applied. */
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BroadcastEndpoint {
    /// Never-reused identity local to one fenced page or worker incarnation.
    pub id: u64,
    /// Exact DOMString identity; the coordinator compares these code units without normalization.
    pub name: Vec<u16>,
}

/** Native publication resolves authoritative origin and storage partition from the owning context. */
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum BroadcastCommand {
    /// Register before subsequent posts select their recipients.
    Open { id: u64, name: Vec<u16> },
    /// Structured clone was captured synchronously; delivery is always a later native task.
    Post { id: u64, data: WorkerData },
    /// Stop future delivery to this endpoint, including already queued messages.
    Close { id: u64 },
}

impl BroadcastCommand {
    /// Validate and charge wire storage before the coordinator retains an untrusted command.
    pub fn bytes(&self) -> EngineResult<usize> {
        let bytes = match self {
            Self::Open { id, name } => {
                if *id == 0 || name.len() > BROADCAST_NAME_UNITS {
                    return Err(EngineError::invalid(
                        "Invalid BroadcastChannel registration.",
                    ));
                }
                name.len().saturating_mul(UNIT_WIRE_BYTES)
            }
            Self::Post { id, data } => {
                if *id == 0 {
                    return Err(EngineError::invalid("Invalid BroadcastChannel sender."));
                }
                data.bytes()?
            }
            Self::Close { id } => {
                if *id == 0 {
                    return Err(EngineError::invalid("Invalid BroadcastChannel closure."));
                }
                0
            }
        };
        Ok(bytes.saturating_add(COMMAND_OVERHEAD))
    }
}
