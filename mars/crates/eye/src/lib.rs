/** Eye parses documents and produces bounded layout without launching Mars or sibling services. */
pub use mars_core::{constants, protocol};
pub mod document;
pub mod platform;
pub use document::*;
