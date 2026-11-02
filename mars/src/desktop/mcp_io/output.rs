/** One writer preserves complete MCP frames and bounds partial progress by an absolute deadline. */
use super::*;
use crate::constants::MCP_WRITE_DEADLINE;
use std::io;

/** A stalled client cannot keep native ownership or the launched process alive. */
pub(super) fn write(output: Descriptor, frames: Receiver<Vec<u8>>, connection: &Connection) {
    while connection.active() {
        let bytes = match frames.recv_timeout(POLL) {
            Ok(bytes) => bytes,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(_) => {
                connection.disconnect("MCP output channel closed.");
                return;
            }
        };
        let deadline = Instant::now() + MCP_WRITE_DEADLINE;
        let mut offset = 0;
        while offset < bytes.len() && connection.active() {
            if Instant::now() >= deadline {
                connection.disconnect("MCP output write timed out.");
                return;
            }
            match super::super::io::poll_ready(output.fd, libc::POLLOUT) {
                Ok(true) => {}
                Ok(false) => continue,
                Err(_) => {
                    connection.disconnect("MCP output descriptor failed.");
                    return;
                }
            }
            let count = unsafe {
                libc::write(
                    output.fd,
                    bytes[offset..].as_ptr().cast(),
                    bytes.len() - offset,
                )
            };
            if count > 0 {
                offset += count as usize;
            } else if count == 0
                || ![io::ErrorKind::WouldBlock, io::ErrorKind::Interrupted]
                    .contains(&io::Error::last_os_error().kind())
            {
                connection.disconnect("MCP output closed.");
                return;
            }
        }
    }
}
