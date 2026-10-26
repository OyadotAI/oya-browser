/** Untrusted ValueDeserializer input is validated as a bounded graph before any receiving JS allocation. */
use super::*;
mod compound;
/** Object ids are graph-local indices, and every count is bounded before indexing or reserving. */
struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
    version: u32,
    graph: Graph,
    objects: usize,
    depth: usize,
    units: usize,
}
/** A standalone stream returns its first value; trailing values remain permitted as in native d8. */
pub(super) fn parse(bytes: &[u8], objects: usize, depth: usize) -> Result<Graph> {
    let mut reader = Reader {
        bytes,
        position: 0,
        version: 0,
        graph: Graph {
            root: Atom::Primitive(JsValue::Undefined),
            nodes: Vec::new(),
            allowed_detached: Vec::new(),
            worker: false,
        },
        objects,
        depth,
        units: 0,
    };
    if reader.peek() == Some(0xff) {
        reader.tag()?;
        reader.version = reader.var32().map_err(|_| Failure::Version)?;
        if !(13..=VERSION).contains(&reader.version) {
            return Err(Failure::Version);
        }
    } else {
        return Err(Failure::Invalid);
    }
    reader.graph.root = reader.atom(0)?;
    Ok(reader.graph)
}
impl Reader<'_> {
    /** Input positions use checked addition even for lengths decoded from full-width native varints. */
    fn raw(&mut self, length: usize) -> Result<&[u8]> {
        let end = self.position.checked_add(length).ok_or(Failure::Invalid)?;
        let bytes = self.bytes.get(self.position..end).ok_or(Failure::Invalid)?;
        self.position = end;
        Ok(bytes)
    }
    /** Zero is a padding tag only between records; zero bytes inside payloads are never skipped. */
    fn peek(&mut self) -> Option<u8> {
        while self.bytes.get(self.position) == Some(&0) {
            self.position += 1;
        }
        self.bytes.get(self.position).copied()
    }
    fn tag(&mut self) -> Result<u8> {
        self.peek().ok_or(Failure::Invalid)?;
        let tag = self.bytes[self.position];
        self.position += 1;
        Ok(tag)
    }
    /** A native uint64 varint may use at most ten bytes and cannot discard high overflow bits. */
    fn var(&mut self) -> Result<u64> {
        let mut value = 0;
        for shift in (0..70).step_by(7) {
            let byte = self.raw(1)?[0];
            if shift == 63 && byte & 0xfe != 0 {
                return Err(Failure::Invalid);
            }
            value |= ((byte & 0x7f) as u64) << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err(Failure::Invalid)
    }
    fn var32(&mut self) -> Result<u32> {
        u32::try_from(self.var()?).map_err(|_| Failure::Invalid)
    }
    fn length(&mut self) -> Result<usize> {
        usize::try_from(self.var()?).map_err(|_| Failure::Invalid)
    }
    fn double(&mut self) -> Result<f64> {
        Ok(f64::from_le_bytes(
            self.raw(8)?.try_into().map_err(|_| Failure::Invalid)?,
        ))
    }
    /** Every primitive, edge and string contributes to the existing retained graph budget. */
    fn charge(&mut self, units: usize) -> Result<()> {
        self.units = self.units.checked_add(units).ok_or(Failure::Limit)?;
        if self.units > MESSAGE_UNITS {
            return Err(Failure::Limit);
        }
        Ok(())
    }
    /** Shell strings preserve Latin-1 and lone UTF-16 surrogates instead of round-tripping through JSON. */
    fn string(&mut self, tag: u8) -> Result<JsString> {
        let length = self.length()?;
        if length > MAX_BYTES {
            return Err(Failure::Invalid);
        }
        let text = match tag {
            b'"' => {
                self.charge(length)?;
                self.raw(length)?.iter().map(|byte| *byte as u16).collect()
            }
            b'c' => {
                if length % 2 != 0 || self.position % 2 != 0 {
                    return Err(Failure::Invalid);
                }
                self.charge(length / 2)?;
                self.raw(length)?
                    .chunks_exact(2)
                    .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
                    .collect()
            }
            b'S' => {
                self.charge(length)?;
                std::str::from_utf8(self.raw(length)?)
                    .map_err(|_| Failure::Invalid)?
                    .encode_utf16()
                    .collect()
            }
            _ => return Err(Failure::Invalid),
        };
        JsString::from_units(text).map_err(|_| Failure::Limit)
    }
    /** BigInt magnitudes are bounded before allocating limb vectors and normalize through the real arithmetic core. */
    fn bigint(&mut self) -> Result<JsValue> {
        let flags = self.var32()?;
        let length = (flags >> 1) as usize;
        let words = length.div_ceil(4);
        if words > crate::v9::value::BigInteger::word_limit() {
            return Err(Failure::Limit);
        }
        self.charge(words * 2)?;
        let bytes = self.raw(length)?;
        let mut limbs = Vec::with_capacity(words);
        for chunk in bytes.chunks(4) {
            let mut word = [0; 4];
            word[..chunk.len()].copy_from_slice(chunk);
            limbs.push(u32::from_le_bytes(word));
        }
        let value = crate::v9::value::BigInteger::from_words(flags & 1 != 0, limbs)
            .ok_or(Failure::Limit)?;
        Ok(JsValue::bigint(value))
    }
    /** Reserve a shell before reading child edges so self references can only name already declared ids. */
    fn node(&mut self, kind: Kind) -> Result<usize> {
        if self.graph.nodes.len() >= self.objects {
            return Err(Failure::Limit);
        }
        let id = self.graph.nodes.len();
        self.graph.nodes.push(Node {
            kind,
            fields: Vec::new(),
        });
        Ok(id)
    }
    /** A parsed buffer can be followed by exactly one view record, consuming that preceding value as V8 specifies. */
    fn atom(&mut self, depth: usize) -> Result<Atom> {
        if depth >= self.depth {
            return Err(Failure::Limit);
        }
        self.charge(1)?;
        let tag = self.tag()?;
        if tag == b'?' {
            self.var32()?;
            return self.atom(depth + 1);
        }
        let atom = self.value(tag, depth + 1)?;
        if let Atom::Reference(id) = atom {
            if matches!(self.graph.nodes[id].kind, Kind::Buffer(_, _, _))
                && self.peek() == Some(b'V')
            {
                self.tag()?;
                return self.view(id);
            }
        }
        Ok(atom)
    }
    /** Each native tag has a complete payload rule; unknown capabilities and host delegates are refused. */
    fn value(&mut self, tag: u8, depth: usize) -> Result<Atom> {
        let primitive = match tag {
            b'_' => Some(JsValue::Undefined),
            b'0' => Some(JsValue::Null),
            b'T' => Some(JsValue::Boolean(true)),
            b'F' => Some(JsValue::Boolean(false)),
            b'I' => {
                let zig = self.var32()?;
                Some(JsValue::Number(
                    ((zig >> 1) as i32 ^ -((zig & 1) as i32)) as f64,
                ))
            }
            b'U' => Some(JsValue::Number(self.var32()? as f64)),
            b'N' => Some(JsValue::Number(self.double()?)),
            b'Z' => Some(self.bigint()?),
            b'"' | b'c' | b'S' => Some(JsValue::String(self.string(tag)?)),
            _ => None,
        };
        if let Some(value) = primitive {
            return Ok(Atom::Primitive(value));
        }
        if tag == b'^' {
            let id = self.var32()? as usize;
            if id >= self.graph.nodes.len() {
                return Err(Failure::Invalid);
            }
            return Ok(Atom::Reference(id));
        }
        self.composite(tag, depth).map(Atom::Reference)
    }
    /** Data-view offsets and flags must agree with the actual preceding buffer before exposing a JS view. */
    fn view(&mut self, buffer: usize) -> Result<Atom> {
        let kind = match self.var32()? {
            v if v == b'b' as u32 => ElementKind::Int8,
            v if v == b'B' as u32 => ElementKind::Uint8,
            v if v == b'C' as u32 => ElementKind::Uint8Clamped,
            v if v == b'w' as u32 => ElementKind::Int16,
            v if v == b'W' as u32 => ElementKind::Uint16,
            v if v == b'd' as u32 => ElementKind::Int32,
            v if v == b'D' as u32 => ElementKind::Uint32,
            v if v == b'h' as u32 => ElementKind::Float16,
            v if v == b'f' as u32 => ElementKind::Float32,
            v if v == b'F' as u32 => ElementKind::Float64,
            v if v == b'q' as u32 => ElementKind::BigInt64,
            v if v == b'Q' as u32 => ElementKind::BigUint64,
            v if v == b'?' as u32 => ElementKind::DataView,
            _ => return Err(Failure::Invalid),
        };
        let offset = self.length()?;
        let bytes = self.length()?;
        let flags = self.var32()?;
        let Kind::Buffer(data, maximum, _) = &self.graph.nodes[buffer].kind else {
            return Err(Failure::Invalid);
        };
        if flags & !3 != 0
            || offset > data.len()
            || bytes > data.len() - offset
            || offset % width(kind) != 0
            || bytes % width(kind) != 0
            || ((flags & 2 != 0) != maximum.is_some())
            || (flags & 1 != 0 && maximum.is_none())
        {
            return Err(Failure::Invalid);
        }
        let id = self.node(Kind::View(
            kind,
            Atom::Reference(buffer),
            offset,
            bytes / width(kind),
            flags & 1 != 0,
        ))?;
        Ok(Atom::Reference(id))
    }
}
