/** Frame bounds are enforced while reading, before untrusted input can grow. */
use super::{EngineError, EngineResult};
use crate::constants::FRAME_BYTES;
use serde::{Serialize, de::DeserializeOwned};
use std::io::{BufRead, Write};

/// EOF ends an ownership channel; unterminated frames are never accepted.
pub fn read_frame(reader: &mut impl BufRead) -> EngineResult<Option<Vec<u8>>> {
    let mut frame = Vec::new();
    loop {
        let available = reader.fill_buf().map_err(io_error)?;
        if available.is_empty() {
            return end_frame(frame);
        }
        let length = available
            .iter()
            .position(|byte| *byte == b'\n')
            .map(|index| index + 1)
            .unwrap_or(available.len());
        if frame.len() + length > FRAME_BYTES {
            return Err(EngineError::limit("wire frame bytes", FRAME_BYTES));
        }
        let complete = available[length - 1] == b'\n';
        frame.extend_from_slice(&available[..length]);
        reader.consume(length);
        if complete {
            return Ok(Some(frame));
        }
    }
}

/** Distinguishes orderly EOF from a request cut off by disconnect. */
fn end_frame(frame: Vec<u8>) -> EngineResult<Option<Vec<u8>>> {
    if frame.is_empty() {
        Ok(None)
    } else {
        Err(EngineError::invalid("A complete JSON line is required."))
    }
}

/// JSON syntax errors are ordinary input errors, not panics.
pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> EngineResult<T> {
    serde_json::from_slice(bytes)
        .map_err(|_| EngineError::invalid("Expected one valid JSON object per line."))
}

/// Output receives the same bound as input, including the newline.
pub fn encode(value: &impl Serialize) -> EngineResult<Vec<u8>> {
    let mut bytes = serde_json::to_vec(value)
        .map_err(|_| EngineError::invalid("Cannot serialize the response."))?;
    bytes.push(b'\n');
    if bytes.len() > FRAME_BYTES {
        return Err(EngineError::limit("wire frame bytes", FRAME_BYTES));
    }
    Ok(bytes)
}

/// A reply is flushed before the next request is accepted.
pub fn write_message(writer: &mut impl Write, value: &impl Serialize) -> EngineResult<()> {
    writer.write_all(&encode(value)?).map_err(io_error)?;
    writer.flush().map_err(io_error)
}

/** Internal pipe errors are translated by the owning supervisor. */
fn io_error(_: std::io::Error) -> EngineError {
    EngineError::invalid("The transport closed or could not be read or written.")
}
