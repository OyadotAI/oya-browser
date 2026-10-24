/** The native MCP front door translates bounded protocol state without owning I/O or browser authority. */
/** Agents attach to the running browser over a private local socket. */
pub mod attach;
pub mod catalog;
mod session;
mod wire;
use crate::protocol::Request;
use serde_json::Value;
pub use session::Session;
pub use wire::encode;
/// Pure session decisions are executed by the existing desktop broker outside the session mutex.
#[derive(Debug)]
pub enum Action {
    /// One bounded JSON-RPC message waits for the sole stdout writer.
    Send(Value),
    /// A validated catalog request enters the same native authorization and command queue.
    Dispatch(Request),
    /// Cancellation carries only exact authority metadata, never retained tool arguments.
    Cancel {
        /// Internal correlation stays separate from client-selected IDs.
        native_id: String,
        /// A targeted tab must not be silently replaced by the selected tab.
        tab_id: Option<String>,
        /// A later human grant must survive cancellation of an earlier epoch.
        control_epoch: Option<u64>,
        /// The existing native policy decides whether cancellation revokes authority.
        mutating: bool,
    },
    /// Terminal reasons are fixed diagnostics without client IDs, arguments or page data.
    Close(&'static str),
}
