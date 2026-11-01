/** The ways an initialization or media segment can be refused: cut short, structurally wrong, or missing a box it needs. */
use std::fmt;

/// Result of parsing a segment.
pub type Result<T> = std::result::Result<T, Error>;

/** Why a segment was refused. Each variant carries a fixed description, never bytes from the input. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /** A box or field runs past the end of its container. */
    Truncated(&'static str),
    /** A field holds a value the format does not allow, or a count is beyond the sanity limit. */
    Malformed(&'static str),
    /** A box the segment needs is absent; the four bytes name it. */
    Missing([u8; 4]),
}

impl fmt::Display for Error {
    /** A one-line description for logs. */
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated(what) => write!(f, "truncated {what}"),
            Self::Malformed(what) => write!(f, "malformed {what}"),
            Self::Missing(kind) => write!(f, "missing {}", String::from_utf8_lossy(kind)),
        }
    }
}

impl std::error::Error for Error {}
