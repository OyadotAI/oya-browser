/** HPACK (RFC 7541): a bounded decoder for response header blocks and a literal-only encoder. */
use super::table::{LENGTHS, STATIC};
use std::collections::VecDeque;
use std::sync::OnceLock;

/** RFC 7541 section 4.1: each dynamic table entry costs its name and value plus 32 bytes. */
const ENTRY_OVERHEAD: usize = 32;

/** Integers above this are refused; no legitimate index, length or size comes close. */
const INTEGER_MAX: u64 = u32::MAX as u64;

/** Continuation bytes carry 7 bits each; more than five cannot fit `INTEGER_MAX`. */
const INTEGER_SHIFT_MAX: u32 = 28;

/** Why a header block was refused. Either one fails the whole connection (RFC 9113 4.3). */
#[derive(Debug, PartialEq)]
pub(super) enum Error {
    /** Malformed encoding, a bad index or a forbidden table size. */
    Compression,
    /** The decoded list exceeded the advertised SETTINGS_MAX_HEADER_LIST_SIZE. */
    TooLarge,
}

/// One connection's decoding context. Its dynamic table never exceeds `limit`, the
/// SETTINGS_HEADER_TABLE_SIZE Relay advertised, whatever the peer's size updates say.
pub(super) struct Decoder {
    table: VecDeque<(String, String)>,
    size: usize,
    max: usize,
    limit: usize,
}

impl Decoder {
    /// A decoder whose table may grow to `limit` bytes, the value advertised in SETTINGS.
    pub(super) fn new(limit: usize) -> Self {
        Self {
            table: VecDeque::new(),
            size: 0,
            max: limit,
            limit,
        }
    }

    /// Decodes a whole header block. `list_limit` bounds the decoded list (names, values and
    /// 32 bytes each), which also stops an index-heavy block from expanding without bound.
    pub(super) fn decode(
        &mut self,
        block: &[u8],
        list_limit: usize,
    ) -> Result<Vec<(String, String)>, Error> {
        let mut input = Input {
            bytes: block,
            position: 0,
        };
        let mut fields = Vec::new();
        let mut list = 0;
        while let Some(byte) = input.peek() {
            let field = if byte & 0x80 != 0 {
                let index = input.integer(7)?;
                self.get(index)?
            } else if byte & 0x40 != 0 {
                let field = self.literal(&mut input, 6)?;
                self.insert(field.clone());
                field
            } else if byte & 0x20 != 0 {
                self.resize(&mut input, fields.is_empty())?;
                continue;
            } else {
                self.literal(&mut input, 4)?
            };
            list += field.0.len() + field.1.len() + ENTRY_OVERHEAD;
            if list > list_limit {
                return Err(Error::TooLarge);
            }
            fields.push(field);
        }
        Ok(fields)
    }

    /** A literal field: an indexed or literal name, then a literal value. */
    fn literal(&self, input: &mut Input<'_>, prefix: u32) -> Result<(String, String), Error> {
        let index = input.integer(prefix)?;
        let name = if index == 0 {
            input.string()?
        } else {
            self.get(index)?.0
        };
        Ok((name, input.string()?))
    }

    /** Section 6.3: a size update is allowed only before the block's first field. */
    fn resize(&mut self, input: &mut Input<'_>, at_start: bool) -> Result<(), Error> {
        let size = input.integer(5)?;
        if !at_start || size > self.limit {
            return Err(Error::Compression);
        }
        self.max = size;
        self.evict(0);
        Ok(())
    }

    /** Static indices 1 to 61, then the dynamic table newest first; 0 is invalid. */
    fn get(&self, index: usize) -> Result<(String, String), Error> {
        if let Some((name, value)) = index.checked_sub(1).and_then(|i| STATIC.get(i)) {
            return Ok(((*name).to_owned(), (*value).to_owned()));
        }
        index
            .checked_sub(STATIC.len() + 1)
            .and_then(|i| self.table.get(i))
            .cloned()
            .ok_or(Error::Compression)
    }

    /** Section 4.4: an entry larger than the table empties it and is not added. */
    fn insert(&mut self, field: (String, String)) {
        let cost = field.0.len() + field.1.len() + ENTRY_OVERHEAD;
        self.evict(cost);
        if cost <= self.max {
            self.size += cost;
            self.table.push_front(field);
        }
    }

    /** Drops the oldest entries until `room` more bytes fit. */
    fn evict(&mut self, room: usize) {
        while self.size + room > self.max {
            let Some((name, value)) = self.table.pop_back() else {
                self.size = 0;
                return;
            };
            self.size -= name.len() + value.len() + ENTRY_OVERHEAD;
        }
    }
}

/** A cursor over one header block. */
struct Input<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl Input<'_> {
    /** The next byte without consuming it. */
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.position).copied()
    }

    /** Consumes one byte; running out mid-field is malformed. */
    fn byte(&mut self) -> Result<u8, Error> {
        let byte = self.peek().ok_or(Error::Compression)?;
        self.position += 1;
        Ok(byte)
    }

    /** Section 5.1 prefix integers, refused above `INTEGER_MAX` instead of overflowing. */
    fn integer(&mut self, prefix: u32) -> Result<usize, Error> {
        let mask = (1u8 << prefix) - 1;
        let mut value = u64::from(self.byte()? & mask);
        if value < u64::from(mask) {
            return Ok(value as usize);
        }
        let mut shift = 0;
        loop {
            let byte = self.byte()?;
            value += u64::from(byte & 0x7f) << shift;
            if value > INTEGER_MAX {
                return Err(Error::Compression);
            }
            if byte & 0x80 == 0 {
                return usize::try_from(value).map_err(|_| Error::Compression);
            }
            shift += 7;
            if shift > INTEGER_SHIFT_MAX {
                return Err(Error::Compression);
            }
        }
    }

    /** Section 5.2 strings, Huffman-coded or raw; non-UTF-8 bytes are replaced, not trusted. */
    fn string(&mut self) -> Result<String, Error> {
        let huffman = self.peek().ok_or(Error::Compression)? & 0x80 != 0;
        let length = self.integer(7)?;
        let end = self
            .position
            .checked_add(length)
            .filter(|end| *end <= self.bytes.len());
        let raw = &self.bytes[self.position..end.ok_or(Error::Compression)?];
        self.position += length;
        let bytes = if huffman {
            huffman_decode(raw)?
        } else {
            raw.to_vec()
        };
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }
}

/** The canonical code rebuilt from `LENGTHS`: per length, the first code and its symbols. */
struct Canonical {
    /** First code of each bit length. */
    first: [u32; 31],
    /** How many symbols have each bit length. */
    count: [u32; 31],
    /** Index into `symbols` of each length's first symbol. */
    offset: [usize; 31],
    /** Symbols ordered by (length, symbol), the canonical order. */
    symbols: Vec<u16>,
}

/** Built once; the table is fixed by the RFC. */
fn canonical() -> &'static Canonical {
    static CANONICAL: OnceLock<Canonical> = OnceLock::new();
    CANONICAL.get_or_init(|| {
        let mut symbols: Vec<u16> = (0..=256).collect();
        symbols.sort_by_key(|symbol| (LENGTHS[*symbol as usize], *symbol));
        let mut table = Canonical {
            first: [0; 31],
            count: [0; 31],
            offset: [0; 31],
            symbols,
        };
        let mut code = 0u32;
        let mut previous = 0u8;
        for (index, symbol) in table.symbols.iter().enumerate() {
            let length = LENGTHS[*symbol as usize];
            if index > 0 {
                code = (code + 1) << (length - previous);
            }
            if table.count[length as usize] == 0 {
                table.first[length as usize] = code;
                table.offset[length as usize] = index;
            }
            table.count[length as usize] += 1;
            previous = length;
        }
        table
    })
}

/** Section 5.2: EOS inside the string, or padding that is long or not all ones, is malformed. */
fn huffman_decode(bytes: &[u8]) -> Result<Vec<u8>, Error> {
    let table = canonical();
    let mut output = Vec::with_capacity(bytes.len() * 8 / 5);
    let (mut code, mut length) = (0u32, 0usize);
    for bit in bytes
        .iter()
        .flat_map(|byte| (0..8).rev().map(move |shift| (byte >> shift) & 1))
    {
        code = (code << 1) | u32::from(bit);
        length += 1;
        if length >= table.count.len() {
            return Err(Error::Compression);
        }
        let start = table.first[length];
        if table.count[length] > 0 && code >= start && code - start < table.count[length] {
            let symbol = table.symbols[table.offset[length] + (code - start) as usize];
            output.push(u8::try_from(symbol).map_err(|_| Error::Compression)?);
            (code, length) = (0, 0);
        }
    }
    if length > 7 || code != (1 << length) - 1 {
        return Err(Error::Compression);
    }
    Ok(output)
}

/// Encodes request fields without touching the peer's dynamic table: an exact static match is
/// one indexed byte, anything else a raw literal "without indexing" (section 6.2.2), with the
/// name taken from the static table when it is there. Nothing the encoder does depends on the
/// peer's SETTINGS_HEADER_TABLE_SIZE, so no table state can desynchronize.
pub(super) fn encode(fields: &[(&str, &str)]) -> Vec<u8> {
    let mut output = Vec::new();
    for (name, value) in fields {
        if let Some(index) = STATIC.iter().position(|field| field == &(*name, *value)) {
            integer(&mut output, 0x80, 7, index + 1);
            continue;
        }
        match STATIC.iter().position(|field| field.0 == *name) {
            Some(index) => integer(&mut output, 0x00, 4, index + 1),
            None => {
                output.push(0x00);
                string(&mut output, name);
            }
        }
        string(&mut output, value);
    }
    output
}

/** Section 5.1 integer with `flags` in the bits above the prefix. */
fn integer(output: &mut Vec<u8>, flags: u8, prefix: u32, value: usize) {
    let mask = (1usize << prefix) - 1;
    if value < mask {
        output.push(flags | value as u8);
        return;
    }
    output.push(flags | mask as u8);
    let mut rest = value - mask;
    while rest >= 0x80 {
        output.push((rest & 0x7f) as u8 | 0x80);
        rest >>= 7;
    }
    output.push(rest as u8);
}

/** A string literal, Huffman-coded when that is shorter, as Chrome writes every header string;
raw strings made Mars's header blocks longer than any browser's (mars/docs/D1-fingerprint.md).
ponytail: no incremental indexing, so repeated headers are not shared across requests. */
fn string(output: &mut Vec<u8>, value: &str) {
    let coded = huffman_encode(value.as_bytes());
    if coded.len() < value.len() {
        integer(output, HUFFMAN_FLAG, 7, coded.len());
        output.extend_from_slice(&coded);
    } else {
        integer(output, 0x00, 7, value.len());
        output.extend_from_slice(value.as_bytes());
    }
}

/** The high bit of a string's length byte marks Huffman coding (RFC 7541 section 5.2). */
const HUFFMAN_FLAG: u8 = 0x80;

/** Each byte's canonical Huffman code and bit length, from the same table the decoder uses. */
fn codes() -> &'static [(u32, u8); 256] {
    static CODES: OnceLock<[(u32, u8); 256]> = OnceLock::new();
    CODES.get_or_init(|| {
        let table = canonical();
        let mut codes = [(0u32, 0u8); 256];
        for (index, symbol) in table.symbols.iter().enumerate() {
            let length = LENGTHS[*symbol as usize];
            let rank = (index - table.offset[length as usize]) as u32;
            if let Some(slot) = codes.get_mut(*symbol as usize) {
                *slot = (table.first[length as usize] + rank, length);
            }
        }
        codes
    })
}

/** Section 5.2 Huffman coding, padded with the most significant bits of EOS (all ones). */
fn huffman_encode(bytes: &[u8]) -> Vec<u8> {
    let codes = codes();
    let (mut output, mut buffer, mut bits) = (Vec::new(), 0u64, 0u32);
    for byte in bytes {
        let (code, length) = codes[*byte as usize];
        buffer = (buffer << length) | u64::from(code);
        bits += u32::from(length);
        while bits >= 8 {
            bits -= 8;
            output.push((buffer >> bits) as u8);
        }
    }
    if bits > 0 {
        let pad = 8 - bits;
        output.push(((buffer << pad) | ((1 << pad) - 1)) as u8);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    /** Hex test vectors from RFC 7541 Appendix C. */
    fn hex(text: &str) -> Vec<u8> {
        let digits: Vec<u8> = text.bytes().filter(u8::is_ascii_hexdigit).collect();
        digits
            .chunks(2)
            .map(|pair| {
                u8::from_str_radix(std::str::from_utf8(pair).expect("hex"), 16).expect("hex")
            })
            .collect()
    }

    /** Owned pairs for comparison. */
    fn pairs(fields: &[(&str, &str)]) -> Vec<(String, String)> {
        fields
            .iter()
            .map(|(n, v)| ((*n).into(), (*v).into()))
            .collect()
    }

    #[test]
    fn rfc_7541_huffman_request_example_decodes() {
        let mut decoder = Decoder::new(4096);
        let block = hex("8286 8441 8cf1 e3c2 e5f2 3a6b a0ab 90f4 ff");
        assert_eq!(
            decoder.decode(&block, 1 << 16),
            Ok(pairs(&[
                (":method", "GET"),
                (":scheme", "http"),
                (":path", "/"),
                (":authority", "www.example.com")
            ]))
        );
        let next = hex("8286 84be 5886 a8eb 1064 9cbf");
        assert_eq!(
            decoder.decode(&next, 1 << 16),
            Ok(pairs(&[
                (":method", "GET"),
                (":scheme", "http"),
                (":path", "/"),
                (":authority", "www.example.com"),
                ("cache-control", "no-cache")
            ]))
        );
    }

    #[test]
    fn encoded_requests_round_trip() {
        let fields = [
            (":method", "GET"),
            (":path", "/x"),
            ("accept-language", "en-US,en;q=0.9"),
            ("sec-fetch-mode", "navigate"),
        ];
        let mut decoder = Decoder::new(4096);
        assert_eq!(
            decoder.decode(&encode(&fields), 1 << 16),
            Ok(pairs(&fields))
        );
        let long = "v".repeat(300);
        let fields = [("x-long", long.as_str())];
        assert_eq!(
            decoder.decode(&encode(&fields), 1 << 16),
            Ok(pairs(&fields))
        );
    }

    #[test]
    fn the_dynamic_table_evicts_and_never_exceeds_the_advertised_size() {
        let mut decoder = Decoder::new(64);
        let mut block = vec![0x40];
        string(&mut block, "a");
        string(&mut block, "b");
        assert!(decoder.decode(&block, 1 << 16).is_ok());
        assert_eq!(decoder.get(62), Ok(("a".into(), "b".into())));
        let mut big = vec![0x40];
        string(&mut big, "n");
        string(&mut big, &"v".repeat(64));
        assert!(decoder.decode(&big, 1 << 16).is_ok());
        assert!(decoder.table.is_empty());
        assert_eq!(
            decoder.decode(&[0x3f, 0x22], 1 << 16),
            Err(Error::Compression),
            "size above limit"
        );
    }

    #[test]
    fn size_updates_after_a_field_are_refused() {
        let mut decoder = Decoder::new(4096);
        assert_eq!(
            decoder.decode(&[0x82, 0x20], 1 << 16),
            Err(Error::Compression)
        );
        assert!(decoder.decode(&[0x20, 0x82], 1 << 16).is_ok());
    }

    #[test]
    fn malformed_blocks_are_refused_without_panicking() {
        let mut decoder = Decoder::new(4096);
        for block in [
            &[0x80][..],
            &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f],
            &[0xbf],
            &[0x40, 0x85, 0x01],
            &[0x00, 0x81, 0x00, 0x00],
            &[0x00, 0x81, 0xff, 0x05],
            &[0xfe],
        ] {
            assert_eq!(
                decoder.decode(block, 1 << 16),
                Err(Error::Compression),
                "{block:?}"
            );
        }
    }

    #[test]
    fn indexed_fields_cannot_expand_past_the_list_limit() {
        let mut decoder = Decoder::new(4096);
        let mut block = vec![0x40];
        string(&mut block, "big");
        string(&mut block, &"v".repeat(1000));
        block.extend(std::iter::repeat_n(0xbe, 1000));
        assert_eq!(decoder.decode(&block, 64 * 1024), Err(Error::TooLarge));
    }

    #[test]
    fn huffman_encoding_round_trips_and_matches_the_rfc() {
        // RFC 7541 C.4.1: "www.example.com" Huffman-codes to these 12 bytes.
        assert_eq!(
            huffman_encode(b"www.example.com"),
            hex("f1e3 c2e5 f23a 6ba0 ab90 f4ff")
        );
        for text in [
            "",
            "a",
            "custom-key",
            "Mozilla/5.0 (Macintosh)",
            "\u{0}\u{ff}",
        ] {
            assert_eq!(
                huffman_decode(&huffman_encode(text.as_bytes())).unwrap(),
                text.as_bytes()
            );
        }
    }

    #[test]
    fn huffman_padding_rules_hold() {
        assert_eq!(
            huffman_decode(&[0x07]),
            Ok(b"0".to_vec()),
            "'0' then three padding ones"
        );
        assert_eq!(
            huffman_decode(&[0xff]),
            Err(Error::Compression),
            "padding longer than 7 bits"
        );
        assert_eq!(
            huffman_decode(&[0x00]),
            Err(Error::Compression),
            "padding of zeros"
        );
        assert_eq!(
            huffman_decode(&[0xff, 0xff, 0xff, 0xff]),
            Err(Error::Compression),
            "EOS"
        );
    }
}
