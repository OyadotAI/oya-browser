/**
 * Link connects Mars to an Oya account the way the Electron Oya Browser does: the control socket
 * (auth, heartbeat, reconnect, the server's command vocabulary run as Mars native operations under
 * Mars's ownership rules), pairing from an `oya://` link with credentials behind a store the app
 * backs with the Keychain, personas loaded as one immutable unit with Oya's deterministic device
 * generator and identity, and the governed-egress host matcher pinned to Oya's by shared vectors.
 */
mod base64;
pub mod client;
pub mod commands;
pub mod constants;
pub mod devices;
pub mod egress;
pub mod identity;
pub mod pairing;
pub mod persona;
pub mod prng;
pub mod profile;
pub mod session;
pub mod ws;

pub use client::{Outcome, run};
pub use commands::{Browser, CommandError, Commands};
pub use egress::allowed_host;
pub use pairing::{CredentialStore, Credentials, MemoryCredentials, pair, parse_link};
pub use persona::{Identity, Persona, Proxy};
pub use session::{Event, Session};

/// A fresh browser id, `oya-` and 16 hex digits, as the Electron browser mints before first connect.
pub fn random_browser_id() -> String {
    let bytes = ws::random::<{ constants::BROWSER_ID_BYTES }>();
    format!(
        "oya-{}",
        bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
    )
}
