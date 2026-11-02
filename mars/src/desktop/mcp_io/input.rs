/** Physical newline framing rejects oversized and unfinished MCP input before execution. */
use super::*;
use crate::constants::{MCP_FRAME_BYTES, NETWORK_CHUNK_BYTES};
use std::io;

/** Partial frames remain private and bounded until their delimiter arrives. */
pub(super) fn read(input: Descriptor, connection: &Connection) {
    let mut pending = Vec::new();
    let mut bytes = [0_u8; NETWORK_CHUNK_BYTES];
    while connection.active() {
        match super::super::io::poll_ready(input.fd, libc::POLLIN) {
            Ok(true) => {}
            Ok(false) => continue,
            Err(_) => {
                connection.disconnect("MCP input descriptor failed.");
                return;
            }
        }
        let count = unsafe { libc::read(input.fd, bytes.as_mut_ptr().cast(), bytes.len()) };
        if count < 0
            && [io::ErrorKind::WouldBlock, io::ErrorKind::Interrupted]
                .contains(&io::Error::last_os_error().kind())
        {
            continue;
        }
        if count <= 0 {
            connection.disconnect("MCP input closed.");
            return;
        }
        for byte in &bytes[..count as usize] {
            if *byte == b'\n' {
                let actions =
                    connection.session(|session| session.receive(&pending, Instant::now()));
                pending.clear();
                connection.actions(actions);
                if !connection.active() {
                    return;
                }
            } else if pending.len() < MCP_FRAME_BYTES.saturating_sub(1) {
                pending.push(*byte);
            } else {
                connection.disconnect("MCP input frame is too large.");
                return;
            }
        }
    }
}
