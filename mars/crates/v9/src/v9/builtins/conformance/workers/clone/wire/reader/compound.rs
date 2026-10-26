/** Compound wire records validate native terminators and counts while retaining cycles through reserved shells. */
use super::*;
impl Reader<'_> {
    /** Scalar wrappers reserve exactly one native object id even when their payload is a string or BigInt. */
    pub(super) fn composite(&mut self, tag: u8, depth: usize) -> Result<usize> {
        match tag {
            b'o' => {
                let id = self.node(Kind::Plain)?;
                let fields = self.fields(b'{', depth)?;
                self.graph.nodes[id].fields = fields;
                Ok(id)
            }
            b'a' | b'A' => self.array(tag, depth),
            b';' | b'\'' => self.table(tag, depth),
            b'r' => self.error(depth),
            b'D' => {
                let value = self.double()?;
                let value = if value.is_finite() && value.abs() <= DATE_CLIP_MS {
                    value.trunc() + 0.0
                } else {
                    f64::NAN
                };
                self.node(Kind::Date(value))
            }
            b'y' | b'x' => self.node(Kind::Wrapper(JsValue::Boolean(tag == b'y'))),
            b'n' => {
                let value = self.double()?;
                self.node(Kind::Wrapper(JsValue::Number(value)))
            }
            b'z' => {
                let value = self.bigint()?;
                self.node(Kind::Wrapper(value))
            }
            b's' => {
                let value = self.atom(depth)?;
                let Atom::Primitive(JsValue::String(value)) = value else {
                    return Err(Failure::Invalid);
                };
                self.node(Kind::Wrapper(JsValue::String(value)))
            }
            b'R' => self.regexp(depth),
            b'B' | b'~' => self.buffer(tag),
            b'C' | b'u' | b'p' | b'w' | b'm' | b't' | b'\\' => Err(Failure::Invalid),
            _ => Err(Failure::Invalid),
        }
    }
    /** Only own property keys accepted by the native format become receiving data descriptors. */
    fn key(&mut self, depth: usize) -> Result<Key> {
        match self.atom(depth)? {
            Atom::Primitive(JsValue::String(text)) => Ok(Key::Str(text)),
            Atom::Primitive(JsValue::Number(number)) if number.is_finite() => Ok(Key::Str(
                JsString::from_utf8(&crate::v9::value::number_to_string(number))
                    .map_err(|_| Failure::Limit)?,
            )),
            _ => Err(Failure::Invalid),
        }
    }
    /** Property lists terminate with their exact native count; duplicate keys cannot create hidden live values. */
    fn fields(&mut self, end: u8, depth: usize) -> Result<Vec<(Key, Atom)>> {
        let mut fields = Vec::new();
        let mut keys = std::collections::BTreeSet::new();
        while self.peek() != Some(end) {
            if fields.len() >= MESSAGE_UNITS {
                return Err(Failure::Limit);
            }
            let key = self.key(depth)?;
            if !keys.insert(key.clone()) {
                return Err(Failure::Invalid);
            }
            let value = self.atom(depth)?;
            fields.push((key, value));
        }
        self.tag()?;
        if self.var32()? as usize != fields.len() {
            return Err(Failure::Invalid);
        }
        Ok(fields)
    }
    /** Sparse logical lengths consume no hole vector; dense arrays require one actual input tag per element. */
    fn array(&mut self, tag: u8, depth: usize) -> Result<usize> {
        let length = self.var32()? as usize;
        let id = self.node(Kind::Array(length))?;
        let mut elements = Vec::new();
        if tag == b'A' {
            if length > self.bytes.len().saturating_sub(self.position) {
                return Err(Failure::Invalid);
            }
            for index in 0..length {
                if self.peek() == Some(b'-') {
                    self.tag()?;
                    self.charge(1)?;
                } else {
                    let value = self.atom(depth)?;
                    elements.push((Key::from(index.to_string().as_str()), value));
                }
            }
        }
        let mut fields = self.fields(if tag == b'A' { b'$' } else { b'@' }, depth)?;
        if fields.iter().any(|(key, _)| *key == Key::from("length")) {
            return Err(Failure::Invalid);
        }
        if self.var32()? as usize != length {
            return Err(Failure::Invalid);
        }
        elements.append(&mut fields);
        self.graph.nodes[id].fields = elements;
        Ok(id)
    }
    /** Map counts are twice the number of entries; Set counts name each value once. */
    fn table(&mut self, tag: u8, depth: usize) -> Result<usize> {
        let id = self.node(if tag == b';' {
            Kind::Map(Vec::new())
        } else {
            Kind::Set(Vec::new())
        })?;
        let end = if tag == b';' { b':' } else { b',' };
        let mut entries = Vec::new();
        while self.peek() != Some(end) {
            entries.push(self.atom(depth)?);
        }
        self.tag()?;
        if self.var32()? as usize != entries.len() || tag == b';' && entries.len() % 2 != 0 {
            return Err(Failure::Invalid);
        }
        self.graph.nodes[id].kind = if tag == b';' {
            Kind::Map(
                entries
                    .chunks_exact(2)
                    .map(|pair| (pair[0].clone(), pair[1].clone()))
                    .collect(),
            )
        } else {
            Kind::Set(entries)
        };
        Ok(id)
    }
    /** Byte payload bounds and the declared maximum are checked before copying any backing allocation. */
    fn buffer(&mut self, tag: u8) -> Result<usize> {
        let length = self.var32()? as usize;
        let maximum = if tag == b'~' {
            Some(self.var32()? as usize)
        } else {
            None
        };
        if length > MAX_BYTES
            || maximum.is_some_and(|maximum| {
                maximum < length || maximum > 2 * crate::constants::JS_STRING_UNITS
            })
        {
            return Err(Failure::Invalid);
        }
        self.charge(length.div_ceil(2))?;
        let bytes = self.raw(length)?.to_vec();
        self.node(Kind::Buffer(bytes, maximum))
    }
    /** RegExp flags are native bit positions, with all unsupported bits and Unicode-mode conflicts rejected. */
    fn regexp(&mut self, depth: usize) -> Result<usize> {
        let Atom::Primitive(JsValue::String(source)) = self.atom(depth)? else {
            return Err(Failure::Invalid);
        };
        let bits = self.var32()?;
        if bits & !0x1ff != 0 {
            return Err(Failure::Invalid);
        }
        let mut flags = String::new();
        for flag in ['d', 'g', 'i', 'l', 'm', 's', 'u', 'v', 'y'] {
            if bits & regexp_flags(&flag.to_string())? != 0 {
                flags.push(flag)
            }
        }
        regexp_flags(&flags)?;
        self.node(Kind::RegExp(source, flags))
    }
    /** Error headers precede optional message/stack/cause; the reserved id permits a self-referencing cause. */
    fn error(&mut self, depth: usize) -> Result<usize> {
        let id = self.node(Kind::Plain)?;
        let mut name = "Error";
        let mut fields = Vec::new();
        let mut stage = 0;
        loop {
            let tag = self.var32()?;
            if tag == b'.' as u32 {
                break;
            }
            if fields.is_empty() && stage == 0 {
                let kind = match tag {
                    v if v == b'E' as u32 => Some("EvalError"),
                    v if v == b'R' as u32 => Some("RangeError"),
                    v if v == b'F' as u32 => Some("ReferenceError"),
                    v if v == b'S' as u32 => Some("SyntaxError"),
                    v if v == b'T' as u32 => Some("TypeError"),
                    v if v == b'U' as u32 => Some("URIError"),
                    _ => None,
                };
                if let Some(kind) = kind {
                    name = kind;
                    stage = 1;
                    continue;
                }
            }
            let (key, next) = match tag {
                v if v == b'm' as u32 => ("message", 2),
                v if v == b's' as u32 => ("stack", 3),
                v if v == b'c' as u32 => ("cause", 4),
                _ => return Err(Failure::Invalid),
            };
            if next <= stage {
                return Err(Failure::Invalid);
            }
            stage = next;
            let value = self.atom(depth)?;
            if key != "cause" && !matches!(value, Atom::Primitive(JsValue::String(_))) {
                return Err(Failure::Invalid);
            }
            fields.push((key.to_string(), value));
        }
        self.graph.nodes[id].kind = Kind::Error(name.into(), fields);
        Ok(id)
    }
}
