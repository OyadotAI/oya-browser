/** Errors for AVIF reading and decoding: each says whether the input is broken, too large, or uses a feature Mars does not decode yet. */
use std::fmt;

/** Why an AVIF file could not be read or decoded. */
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /** The input ended before a field or payload it declared. */
    Truncated,
    /** The bytes break the format; the text names the structure that is wrong. */
    Malformed(&'static str),
    /** A valid feature Mars does not implement yet, so the file is refused rather than guessed at. */
    Unsupported(&'static str),
    /** A declared count or size passes a bound that protects memory from untrusted input. */
    Limit(&'static str),
}

/** The result type every fallible AVIF operation returns. */
pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    /** Render the error for logs: the kind, then the detail when there is one. */
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Truncated => write!(f, "avif: input truncated"),
            Error::Malformed(what) => write!(f, "avif: malformed {what}"),
            Error::Unsupported(what) => write!(f, "avif: unsupported {what}"),
            Error::Limit(what) => write!(f, "avif: {what} exceeds limit"),
        }
    }
}

impl std::error::Error for Error {}
