/** RFC 6455 server framing: the handshake key, masked client frames in, unmasked frames out. */
use std::io::{self, Read, Write};

/// The GUID RFC 6455 appends to the client key.
const GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
/// Largest message a client may send: Runtime.evaluate carries whole automation libraries.
pub const MESSAGE_BYTES: usize = 16 * 1024 * 1024;
/// Frame opcodes this server reads or writes.
const CONTINUATION: u8 = 0x0;
const TEXT: u8 = 0x1;
const CLOSE: u8 = 0x8;
const PING: u8 = 0x9;
const PONG: u8 = 0xA;

/** Sec-WebSocket-Accept for a client's Sec-WebSocket-Key. */
pub fn accept_key(key: &str) -> String {
    let digest = ring::digest::digest(
        &ring::digest::SHA1_FOR_LEGACY_USE_ONLY,
        format!("{}{GUID}", key.trim()).as_bytes(),
    );
    base64(digest.as_ref())
}

/** One whole text message, or None when the peer closed. Pings are answered in passing. */
pub fn read_message(stream: &mut (impl Read + Write)) -> io::Result<Option<String>> {
    let mut message = Vec::new();
    loop {
        let mut head = [0_u8; 2];
        if stream.read_exact(&mut head).is_err() {
            return Ok(None);
        }
        let (fin, opcode, masked) = (head[0] & 0x80 != 0, head[0] & 0x0f, head[1] & 0x80 != 0);
        let length = match head[1] & 0x7f {
            126 => {
                let mut bytes = [0_u8; 2];
                stream.read_exact(&mut bytes)?;
                u64::from(u16::from_be_bytes(bytes))
            }
            127 => {
                let mut bytes = [0_u8; 8];
                stream.read_exact(&mut bytes)?;
                u64::from_be_bytes(bytes)
            }
            short => u64::from(short),
        };
        if !masked || length as usize > MESSAGE_BYTES - message.len().min(MESSAGE_BYTES) {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid frame"));
        }
        let mut mask = [0_u8; 4];
        stream.read_exact(&mut mask)?;
        let mut payload = vec![0_u8; length as usize];
        stream.read_exact(&mut payload)?;
        for (index, byte) in payload.iter_mut().enumerate() {
            *byte ^= mask[index % 4];
        }
        match opcode {
            CLOSE => {
                let _ = write_frame(stream, CLOSE, &[]);
                return Ok(None);
            }
            PING => write_frame(stream, PONG, &payload)?,
            PONG => {}
            TEXT | CONTINUATION => {
                message.extend_from_slice(&payload);
                if fin {
                    return String::from_utf8(message)
                        .map(Some)
                        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "not UTF-8"));
                }
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "binary frames are not used",
                ));
            }
        }
    }
}

/** Write one complete text message. */
pub fn write_text(stream: &mut impl Write, text: &str) -> io::Result<()> {
    write_frame(stream, TEXT, text.as_bytes())
}

/** One final, unmasked frame, as a server sends. */
fn write_frame(stream: &mut impl Write, opcode: u8, payload: &[u8]) -> io::Result<()> {
    let mut frame = vec![0x80 | opcode];
    match payload.len() {
        length if length < 126 => frame.push(length as u8),
        length if length <= usize::from(u16::MAX) => {
            frame.push(126);
            frame.extend_from_slice(&(length as u16).to_be_bytes());
        }
        length => {
            frame.push(127);
            frame.extend_from_slice(&(length as u64).to_be_bytes());
        }
    }
    frame.extend_from_slice(payload);
    stream.write_all(&frame)?;
    stream.flush()
}

/** Standard padded base64. */
pub fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0_u32, |n, (i, b)| n | u32::from(*b) << (16 - 8 * i));
        for i in 0..4 {
            out.push(if i <= chunk.len() {
                ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char
            } else {
                '='
            });
        }
    }
    out
}

/** Standard base64 back to bytes; None for anything that is not base64. */
pub fn decode_base64(text: &str) -> Option<Vec<u8>> {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let digits: Vec<u8> = text
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace() && *byte != b'=')
        .map(|byte| {
            ALPHABET
                .iter()
                .position(|known| *known == byte)
                .map(|index| index as u8)
        })
        .collect::<Option<Vec<u8>>>()?;
    let mut out = Vec::with_capacity(digits.len() / 4 * 3 + 2);
    for chunk in digits.chunks(4) {
        if chunk.len() == 1 {
            return None;
        }
        let n = chunk
            .iter()
            .enumerate()
            .fold(0_u32, |n, (i, d)| n | u32::from(*d) << (18 - 6 * i));
        for i in 0..chunk.len() - 1 {
            out.push((n >> (16 - 8 * i) & 0xff) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::{accept_key, base64, decode_base64};

    /** Decoding inverts encoding, padding or not, and refuses junk. */
    #[test]
    fn base64_round_trips() {
        for sample in [&b""[..], b"f", b"fo", b"foo", b"foob", b"fooba", b"foobar"] {
            assert_eq!(decode_base64(&base64(sample)).unwrap(), sample);
        }
        assert_eq!(decode_base64("Zm9v").unwrap(), b"foo");
        assert!(decode_base64("Zm9v!").is_none());
    }

    /** The accept key matches the worked example in RFC 6455 section 1.3. */
    #[test]
    fn accept_key_matches_rfc_example() {
        assert_eq!(
            accept_key("dGhlIHNhbXBsZSBub25jZQ=="),
            "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
        );
    }
}
