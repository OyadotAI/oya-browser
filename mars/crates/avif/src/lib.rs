/**
 * AVIF still-image support for Mars. This crate reads the HEIF container, parses AV1 OBU and frame headers, and reports picture size; AV1 tile decoding is the next stage.
 * It is std only, and untrusted input is bounded so that bad bytes return an Error and never panic or allocate without limit.
 */
mod api;
mod av1;
mod bits;
mod boxes;
// The inter and motion-vector CDFs are generated for the inter-frame syntax, which is not built yet.
#[allow(dead_code)]
mod cdf_tables;
mod constants;
mod container;
// The inter-only CDF fields are read by the inter syntax, which is not built yet.
#[allow(dead_code)]
mod context;
mod error;
mod frame;
mod iloc;
mod items;
mod meta;
mod obu;
mod output;
mod params;
mod post;
mod properties;
mod reader;
mod recon;
mod sequence;
mod stream;
mod symbol;
#[cfg(test)]
mod test_support;
mod tile;

pub use api::{Image, decode, dimensions};
pub use error::{Error, Result};
