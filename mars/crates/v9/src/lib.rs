/** V9 executes bounded JavaScript over explicit data without a renderer, browser or service process. */
pub use mars_core::{constants, dom as document, protocol};

/// The owned interpreter preserves its domain facade for existing Rust callers.
pub mod v9;

pub use v9::*;
