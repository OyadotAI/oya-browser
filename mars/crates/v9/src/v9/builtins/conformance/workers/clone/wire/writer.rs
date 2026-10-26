/** Native V8 tags, little-endian payloads and object-id memoization produce portable inert byte records. */
use super::*;
/** One stream assigns ids in serialization order, which differs from graph order for typed views' buffers. */
struct Writer<'a> {
    graph: &'a Graph,
    bytes: Vec<u8>,
    ids: BTreeMap<usize, u32>,
}
/** Every argument shares one object-id table and the one native stream header. */
pub(super) fn encode(graph: &Graph, roots: &[Atom]) -> Result<Vec<u8>> {
    let mut writer = Writer {
        graph,
        bytes: vec![0xff],
        ids: BTreeMap::new(),
    };
    writer.var(VERSION as u64)?;
    for root in roots {
        writer.atom(root, 0)?;
    }
    Ok(writer.bytes)
}
impl Writer<'_> {
    /** Bounds are checked before reserving a byte payload, never after growing an untrusted length. */
    fn raw(&mut self, bytes: &[u8]) -> Result<()> {
        if bytes.len() > MAX_BYTES.saturating_sub(self.bytes.len()) {
            return Err(Failure::Limit);
        }
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    fn tag(&mut self, tag: u8) -> Result<()> {
        self.raw(&[tag])
    }
    /** Unsigned base-128 varints are emitted without signed overflow or platform-size truncation. */
    fn var(&mut self, mut value: u64) -> Result<()> {
        loop {
            let byte = (value as u8 & 0x7f) | if value >= 128 { 0x80 } else { 0 };
            self.tag(byte)?;
            value >>= 7;
            if value == 0 {
                return Ok(());
            }
        }
    }
    /** Strings select a valid compact Latin-1 wire encoding independently of the engine's UTF-16 backing. */
    fn string(&mut self, text: &JsString) -> Result<()> {
        let units = text.as_units();
        if units.iter().all(|unit| *unit <= 255) {
            self.tag(b'"')?;
            self.var(units.len() as u64)?;
            for unit in units {
                self.tag(*unit as u8)?;
            }
            return Ok(());
        }
        let length = units.len() * 2;
        let varbytes = (usize::BITS - length.max(1).leading_zeros()).div_ceil(7) as usize;
        if (self.bytes.len() + 1 + varbytes) % 2 != 0 {
            self.tag(0)?;
        }
        self.tag(b'c')?;
        self.var(length as u64)?;
        for unit in units {
            self.raw(&unit.to_le_bytes())?;
        }
        Ok(())
    }
    /** Number tags preserve negative zero and nonfinite payloads while compact exact int32 values use ZigZag. */
    fn number(&mut self, value: f64) -> Result<()> {
        if value.is_finite()
            && value.fract() == 0.0
            && value >= i32::MIN as f64
            && value <= i32::MAX as f64
            && !(value == 0.0 && value.is_sign_negative())
        {
            self.tag(b'I')?;
            let value = value as i32;
            self.var(((value as u32).wrapping_shl(1) ^ ((value >> 31) as u32)) as u64)
        } else {
            self.tag(b'N')?;
            self.raw(&value.to_le_bytes())
        }
    }
    /** BigInt byte-length/sign bitfields carry exact normalized limbs with the native 64-bit digit padding. */
    fn bigint(&mut self, value: &crate::v9::value::BigInteger) -> Result<()> {
        let length = value.words().len().div_ceil(2) * 8;
        self.var(((length as u64) << 1) | u64::from(value.negative()))?;
        for index in 0..length / 4 {
            self.raw(&value.words().get(index).copied().unwrap_or(0).to_le_bytes())?;
        }
        Ok(())
    }
    /** Primitive values never enter the object-id table. */
    fn primitive(&mut self, value: &JsValue) -> Result<()> {
        match value {
            JsValue::Undefined => self.tag(b'_'),
            JsValue::Null => self.tag(b'0'),
            JsValue::Boolean(true) => self.tag(b'T'),
            JsValue::Boolean(false) => self.tag(b'F'),
            JsValue::Number(value) => self.number(*value),
            JsValue::String(value) => self.string(value),
            JsValue::BigInt(value) => {
                self.tag(b'Z')?;
                self.bigint(&crate::v9::value::BigInteger::from_i128(*value))
            }
            JsValue::BigIntWide(value) => {
                self.tag(b'Z')?;
                self.bigint(value)
            }
            _ => Err(Failure::Refused),
        }
    }
    /** Graph depth is independently bounded even if a malicious native caller bypassed the JS encoder. */
    fn atom(&mut self, atom: &Atom, depth: usize) -> Result<()> {
        if depth > crate::constants::VM_CALL_DEPTH {
            return Err(Failure::Limit);
        }
        match atom {
            Atom::Primitive(value) => self.primitive(value),
            Atom::Reference(index) => self.object(*index, depth + 1),
        }
    }
    /** Property keys are data strings, so native decoding can define them without invoking author setters. */
    fn fields(&mut self, fields: &[(Key, Atom)], depth: usize) -> Result<()> {
        for (key, value) in fields {
            let Key::Str(text) = key else {
                return Err(Failure::Invalid);
            };
            self.string(text)?;
            self.atom(value, depth)?;
        }
        Ok(())
    }
    /** View records consume their preceding buffer and receive their own id only after that buffer's id. */
    fn object(&mut self, index: usize, depth: usize) -> Result<()> {
        if let Some(id) = self.ids.get(&index).copied() {
            self.tag(b'^')?;
            return self.var(id as u64);
        }
        let node = self
            .graph
            .nodes
            .get(index)
            .cloned()
            .ok_or(Failure::Invalid)?;
        if let Kind::View(kind, buffer, offset, length, tracking) = node.kind {
            self.atom(&buffer, depth)?;
            self.ids.insert(index, self.ids.len() as u32);
            self.tag(b'V')?;
            self.var(view_tag(kind) as u64)?;
            self.var(offset as u64)?;
            self.var((length * width(kind)) as u64)?;
            let rab = matches!(buffer,Atom::Reference(i)if matches!(self.graph.nodes[i].kind,Kind::Buffer(_,Some(_))));
            return self.var(u64::from(tracking) | (u64::from(rab) << 1));
        }
        self.ids.insert(index, self.ids.len() as u32);
        self.kind(node, depth)
    }
    /** Composite end tags include native property counts, preventing corrupted records from being accepted. */
    fn kind(&mut self, node: Node, depth: usize) -> Result<()> {
        match node.kind {
            Kind::Plain => {
                self.tag(b'o')?;
                self.fields(&node.fields, depth)?;
                self.tag(b'{')?;
                self.var(node.fields.len() as u64)
            }
            Kind::Array(length) => {
                self.tag(b'a')?;
                self.var(length as u64)?;
                self.fields(&node.fields, depth)?;
                self.tag(b'@')?;
                self.var(node.fields.len() as u64)?;
                self.var(length as u64)
            }
            Kind::Date(value) => {
                self.tag(b'D')?;
                self.raw(&value.to_le_bytes())
            }
            Kind::RegExp(source, flags) => {
                self.tag(b'R')?;
                self.string(&source)?;
                self.var(regexp_flags(&flags)? as u64)
            }
            Kind::Map(entries) => {
                self.tag(b';')?;
                for (key, value) in &entries {
                    self.atom(key, depth)?;
                    self.atom(value, depth)?;
                }
                self.tag(b':')?;
                self.var((entries.len() * 2) as u64)
            }
            Kind::Set(entries) => {
                self.tag(b'\'')?;
                for value in &entries {
                    self.atom(value, depth)?;
                }
                self.tag(b',')?;
                self.var(entries.len() as u64)
            }
            Kind::Buffer(bytes, maximum) => {
                self.tag(if maximum.is_some() { b'~' } else { b'B' })?;
                self.var(bytes.len() as u64)?;
                if let Some(maximum) = maximum {
                    self.var(maximum as u64)?;
                }
                self.raw(&bytes)
            }
            Kind::Wrapper(value) => self.wrapper(&value),
            Kind::Error(name, fields) => self.error(&name, &fields, depth),
            _ => Err(Failure::Refused),
        }
    }
    /** Boxed primitive wrappers have distinct native tags and retain their actual primitive payload. */
    fn wrapper(&mut self, value: &JsValue) -> Result<()> {
        match value {
            JsValue::Boolean(value) => self.tag(if *value { b'y' } else { b'x' }),
            JsValue::Number(value) => {
                self.tag(b'n')?;
                self.raw(&value.to_le_bytes())
            }
            JsValue::String(value) => {
                self.tag(b's')?;
                self.string(value)
            }
            JsValue::BigInt(value) => {
                self.tag(b'z')?;
                self.bigint(&crate::v9::value::BigInteger::from_i128(*value))
            }
            JsValue::BigIntWide(value) => {
                self.tag(b'z')?;
                self.bigint(value)
            }
            _ => Err(Failure::Invalid),
        }
    }
    /** Error causes are emitted last so a cause can reference the already registered Error itself. */
    fn error(&mut self, name: &str, fields: &[(String, Atom)], depth: usize) -> Result<()> {
        self.tag(b'r')?;
        if let Some(tag) = match name {
            "EvalError" => Some(b'E'),
            "RangeError" => Some(b'R'),
            "ReferenceError" => Some(b'F'),
            "SyntaxError" => Some(b'S'),
            "TypeError" => Some(b'T'),
            "URIError" => Some(b'U'),
            _ => None,
        } {
            self.var(tag as u64)?;
        }
        for (name, value) in fields {
            let tag = match name.as_str() {
                "message" => b'm',
                "stack" => b's',
                "cause" => b'c',
                _ => return Err(Failure::Invalid),
            };
            self.var(tag as u64)?;
            self.atom(value, depth)?;
        }
        self.var(b'.' as u64)
    }
}
