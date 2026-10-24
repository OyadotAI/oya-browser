/** HTTP/2 frame layout (RFC 9113 section 4): parsing with size checks, and encoding. */
use crate::network::constants::{
    H2_CONNECTION_INCREMENT, H2_FRAME_BYTES, H2_HEADER_LIST_BYTES, H2_HEADER_TABLE_BYTES,
    H2_STREAM_WINDOW,
};

/// Frame types (section 6).
pub(super) const DATA: u8 = 0x0;
pub(super) const HEADERS: u8 = 0x1;
pub(super) const PRIORITY: u8 = 0x2;
pub(super) const RST_STREAM: u8 = 0x3;
pub(super) const SETTINGS: u8 = 0x4;
pub(super) const PUSH_PROMISE: u8 = 0x5;
pub(super) const PING: u8 = 0x6;
pub(super) const GOAWAY: u8 = 0x7;
pub(super) const WINDOW_UPDATE: u8 = 0x8;
pub(super) const CONTINUATION: u8 = 0x9;

/// Flags. END_STREAM and ACK share a bit on different frame types.
pub(super) const END_STREAM: u8 = 0x1;
pub(super) const ACK: u8 = 0x1;
pub(super) const END_HEADERS: u8 = 0x4;
pub(super) const PADDED: u8 = 0x8;
pub(super) const PRIORITY_FLAG: u8 = 0x20;

/// Error codes (section 7).
pub(super) const PROTOCOL_ERROR: u32 = 0x1;
pub(super) const FLOW_CONTROL_ERROR: u32 = 0x3;
pub(super) const FRAME_SIZE_ERROR: u32 = 0x6;
pub(super) const REFUSED_STREAM: u32 = 0x7;
pub(super) const CANCEL: u32 = 0x8;
pub(super) const COMPRESSION_ERROR: u32 = 0x9;
pub(super) const ENHANCE_YOUR_CALM: u32 = 0xb;

/// SETTINGS identifiers (section 6.5.2).
pub(super) const SETTINGS_HEADER_TABLE_SIZE: u16 = 0x1;
pub(super) const SETTINGS_ENABLE_PUSH: u16 = 0x2;
pub(super) const SETTINGS_MAX_CONCURRENT_STREAMS: u16 = 0x3;
pub(super) const SETTINGS_INITIAL_WINDOW_SIZE: u16 = 0x4;
pub(super) const SETTINGS_MAX_FRAME_SIZE: u16 = 0x5;
pub(super) const SETTINGS_MAX_HEADER_LIST_SIZE: u16 = 0x6;

/// Frame header bytes: 24-bit length, type, flags, 31-bit stream identifier.
pub(super) const HEADER_BYTES: usize = 9;

/// Section 6.9.1: windows and stream identifiers are 31-bit.
pub(super) const MAX_WINDOW: i64 = 0x7fff_ffff;

/// Section 6.5.2: the largest SETTINGS_MAX_FRAME_SIZE a peer may announce.
pub(super) const MAX_FRAME_LIMIT: u32 = 0xff_ffff;

/// Section 6.9.2: every window starts at 65535 until SETTINGS change it.
pub(super) const DEFAULT_WINDOW: i64 = 65_535;

/// Section 3.4: the client connection preface.
const PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";

/// One received frame, borrowing its payload.
pub(super) struct Frame<'a> {
    pub kind: u8,
    pub flags: u8,
    pub stream: u32,
    pub payload: &'a [u8],
}

/// The next complete frame and its total size, None while more bytes are needed, or the
/// FRAME_SIZE_ERROR a payload above `H2_FRAME_BYTES` (the size Relay allows) earns.
pub(super) fn parse(bytes: &[u8]) -> Result<Option<(Frame<'_>, usize)>, u32> {
    if bytes.len() < HEADER_BYTES {
        return Ok(None);
    }
    let length = usize::from(bytes[0]) << 16 | usize::from(bytes[1]) << 8 | usize::from(bytes[2]);
    if length > H2_FRAME_BYTES {
        return Err(FRAME_SIZE_ERROR);
    }
    let total = HEADER_BYTES + length;
    if bytes.len() < total {
        return Ok(None);
    }
    let frame = Frame {
        kind: bytes[3],
        flags: bytes[4],
        stream: u32_at(bytes, 5) & 0x7fff_ffff,
        payload: &bytes[HEADER_BYTES..total],
    };
    Ok(Some((frame, total)))
}

/// Appends one frame. Callers keep payloads within the peer's maximum frame size.
pub(super) fn write(output: &mut Vec<u8>, kind: u8, flags: u8, stream: u32, payload: &[u8]) {
    let length = payload.len() as u32;
    output.extend_from_slice(&length.to_be_bytes()[1..]);
    output.push(kind);
    output.push(flags);
    output.extend_from_slice(&(stream & 0x7fff_ffff).to_be_bytes());
    output.extend_from_slice(payload);
}

/// The preface, Chrome's SETTINGS (header table, push off, initial window, header list, in
/// Chrome's order) and Chrome's connection WINDOW_UPDATE: the Akamai HTTP/2 fingerprint
/// `1:65536;2:0;4:6291456;6:262144|15663105|0|m,a,s,p`.
pub(super) fn preface() -> Vec<u8> {
    let mut output = PREFACE.to_vec();
    let mut settings = Vec::new();
    for (id, value) in [
        (SETTINGS_HEADER_TABLE_SIZE, H2_HEADER_TABLE_BYTES as u32),
        (SETTINGS_ENABLE_PUSH, 0),
        (SETTINGS_INITIAL_WINDOW_SIZE, H2_STREAM_WINDOW),
        (SETTINGS_MAX_HEADER_LIST_SIZE, H2_HEADER_LIST_BYTES as u32),
    ] {
        settings.extend_from_slice(&id.to_be_bytes());
        settings.extend_from_slice(&value.to_be_bytes());
    }
    write(&mut output, SETTINGS, 0, 0, &settings);
    write(
        &mut output,
        WINDOW_UPDATE,
        0,
        0,
        &H2_CONNECTION_INCREMENT.to_be_bytes(),
    );
    output
}

/// Strips padding (and, for HEADERS, the priority fields) from a payload. Padding as long as
/// the payload is a PROTOCOL_ERROR (section 6.1).
pub(super) fn content<'a>(frame: &Frame<'a>) -> Result<&'a [u8], u32> {
    let mut payload = frame.payload;
    let mut padding = 0;
    if frame.flags & PADDED != 0 {
        let (&length, rest) = payload.split_first().ok_or(FRAME_SIZE_ERROR)?;
        padding = usize::from(length);
        payload = rest;
    }
    if frame.kind == HEADERS && frame.flags & PRIORITY_FLAG != 0 {
        payload = payload.get(5..).ok_or(FRAME_SIZE_ERROR)?;
    }
    if padding > payload.len() {
        return Err(PROTOCOL_ERROR);
    }
    Ok(&payload[..payload.len() - padding])
}

/// A big-endian u32 at `offset`; callers check the length first.
pub(super) fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_round_trip_and_wait_for_their_whole_payload() {
        let mut bytes = Vec::new();
        write(&mut bytes, PING, ACK, 0, &[7; 8]);
        assert!(
            parse(&bytes[..HEADER_BYTES + 3])
                .expect("partial")
                .is_none()
        );
        let (frame, used) = parse(&bytes).expect("valid").expect("complete");
        assert_eq!(
            (frame.kind, frame.flags, frame.stream, frame.payload, used),
            (PING, ACK, 0, &[7u8; 8][..], 17)
        );
    }

    #[test]
    fn oversized_frames_are_a_frame_size_error() {
        let mut bytes = Vec::new();
        write(&mut bytes, DATA, 0, 1, &vec![0; H2_FRAME_BYTES + 1]);
        assert_eq!(parse(&bytes).err(), Some(FRAME_SIZE_ERROR));
    }

    #[test]
    fn padding_longer_than_the_payload_is_a_protocol_error() {
        let frame = Frame {
            kind: DATA,
            flags: PADDED,
            stream: 1,
            payload: &[5, 1, 2],
        };
        assert_eq!(content(&frame).err(), Some(PROTOCOL_ERROR));
        let frame = Frame {
            kind: DATA,
            flags: PADDED,
            stream: 1,
            payload: &[1, 9, 0],
        };
        assert_eq!(content(&frame), Ok(&[9u8][..]));
        let frame = Frame {
            kind: HEADERS,
            flags: PRIORITY_FLAG,
            stream: 1,
            payload: &[0, 0],
        };
        assert_eq!(content(&frame).err(), Some(FRAME_SIZE_ERROR));
    }

    #[test]
    fn the_preface_carries_chromes_settings_and_window() {
        let bytes = preface();
        assert!(bytes.starts_with(PREFACE));
        let (settings, used) = parse(&bytes[PREFACE.len()..])
            .expect("valid")
            .expect("frame");
        assert_eq!(settings.kind, SETTINGS);
        assert_eq!(
            settings.payload,
            [
                0, 1, 0, 1, 0, 0, 0, 2, 0, 0, 0, 0, 0, 4, 0, 0x60, 0, 0, 0, 6, 0, 4, 0, 0
            ]
        );
        let (update, _) = parse(&bytes[PREFACE.len() + used..])
            .expect("valid")
            .expect("frame");
        assert_eq!(u32_at(update.payload, 0), 15_663_105);
    }
}
