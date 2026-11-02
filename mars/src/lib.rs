/** Mars composes independently reusable Eye, V9 and Relay engines with native agent control. */
extern crate self as mars;
pub use mars_core::{constants, protocol};
pub use mars_eye as document;
pub use mars_eye::platform;
pub use mars_relay as network;
pub use mars_v9 as v9;
pub mod desktop;
pub mod mcp;
/** Secure desktop profile persistence remains outside engine libraries and agent protocols. */
pub mod profile;
pub mod runtime;
