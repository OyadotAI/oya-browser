/*! Standard padded base64, for the WebSocket handshake and screenshot data URLs; a few lines, no dependency. */

/// The standard alphabet.
const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// `bytes` as padded base64.
pub fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, b)| n | u32::from(*b) << (16 - 8 * i));
        for i in 0..4 {
            let symbol = if i <= chunk.len() {
                ALPHABET[(n >> (18 - 6 * i) & 63) as usize]
            } else {
                b'='
            };
            out.push(char::from(symbol));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn matches_rfc_4648_vectors() {
        let cases = [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foobar", "Zm9vYmFy"),
        ];
        for (input, output) in cases {
            assert_eq!(super::encode(input.as_bytes()), output);
        }
    }
}
