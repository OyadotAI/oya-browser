/** A checked WebAssembly binary reader; lengths, indices and LEB encodings never index unchecked. */
use super::{constants::*, types::*};

/** One bounded slice; child section readers cannot consume a neighboring section. */
pub(super) struct Reader<'a> {
    pub bytes: &'a [u8],
    pub at: usize,
    pub base: usize,
    pub(super) stringref_enabled: bool,
    pub(super) custom_descriptors_enabled: bool,
    pub(super) acquire_release_enabled: bool,
    pub(super) fp16_enabled: bool,
    pub(super) wide_arithmetic_enabled: bool,
    pub(super) wasmfx_enabled: bool,
    pub(super) shared_enabled: bool,
}
impl<'a> Reader<'a> {
    /** A fresh section or body reader. */
    pub fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            at: 0,
            base: 0,
            stringref_enabled: true,
            custom_descriptors_enabled: true,
            acquire_release_enabled: true,
            fp16_enabled: true,
            wide_arithmetic_enabled: true,
            wasmfx_enabled: true,
            shared_enabled: true,
        }
    }
    /** Nested readers retain absolute binary offsets without reading outside their own section. */
    fn section(&mut self, count: usize) -> Result<Reader<'a>, Error> {
        let base = self.base + self.at;
        Ok(Reader {
            bytes: self.take(count)?,
            at: 0,
            base,
            stringref_enabled: self.stringref_enabled,
            custom_descriptors_enabled: self.custom_descriptors_enabled,
            acquire_release_enabled: self.acquire_release_enabled,
            fp16_enabled: self.fp16_enabled,
            wide_arithmetic_enabled: self.wide_arithmetic_enabled,
            wasmfx_enabled: self.wasmfx_enabled,
            shared_enabled: self.shared_enabled,
        })
    }
    /** One byte, failing at the actual end of this section. */
    pub fn byte(&mut self) -> Result<u8, Error> {
        let b = *self
            .bytes
            .get(self.at)
            .ok_or_else(|| Error::invalid("unexpected end of WebAssembly binary"))?;
        self.at += 1;
        Ok(b)
    }
    /** A fixed byte range, checked before slicing or allocating. */
    pub fn take(&mut self, count: usize) -> Result<&'a [u8], Error> {
        let end = self
            .at
            .checked_add(count)
            .ok_or_else(|| Error::invalid("section size overflow"))?;
        let out = self.bytes.get(self.at..end).ok_or_else(|| {
            Error::invalid(&format!("expected {count} bytes, fell off end")).at(self.base + self.at)
        })?;
        self.at = end;
        Ok(out)
    }
    /** Unsigned 32-bit LEB128, including the final byte's unused-bit rule. */
    pub fn u32(&mut self) -> Result<u32, Error> {
        let mut out = 0u32;
        for i in 0..5 {
            let b = self.byte()?;
            if i == 4 && b > 15 {
                return Err(Error::invalid("unsigned LEB128 overflow"));
            }
            out |= u32::from(b & 127) << (i * 7);
            if b & 128 == 0 {
                return Ok(out);
            }
        }
        Err(Error::invalid("unterminated unsigned LEB128"))
    }
    /** Signed LEB128 with the exact declared bit width, including signed terminal padding. */
    pub fn signed(&mut self, bits: u32) -> Result<i64, Error> {
        let mut out = 0u64;
        let max = bits.div_ceil(7);
        for i in 0..max {
            let b = self.byte()?;
            let shift = i * 7;
            let payload = u64::from(b & 127);
            if i + 1 == max {
                let used = bits - shift;
                let mask = 127u64 << used;
                let high = payload & mask;
                let expected = if payload & (1 << (used - 1)) != 0 {
                    mask & 127
                } else {
                    0
                };
                if high != expected {
                    return Err(Error::invalid("signed LEB128 overflow"));
                }
            }
            out |= payload << shift;
            if b & 128 == 0 {
                let filled = (shift + 7).min(64);
                if b & 64 != 0 && filled < 64 {
                    out |= u64::MAX << filled
                }
                return Ok(out as i64);
            }
        }
        Err(Error::invalid("unterminated signed LEB128"))
    }
    /** A UTF-8 name; malformed UTF-8 is a compile error, not lossy text. */
    pub fn name(&mut self) -> Result<String, Error> {
        let n = self.u32()? as usize;
        String::from_utf8(self.take(n)?.to_vec())
            .map_err(|_| Error::invalid("invalid UTF-8 WebAssembly name"))
    }
    /** Prefixed opcodes use the bounded encoded index accepted by the V8 binary decoder. */
    pub(super) fn prefixed_opcode(&mut self) -> Result<u32, Error> {
        let index = self.u32()?;
        if index > 0xfff { return Err(Error::invalid(&format!("Invalid prefixed opcode {index}"))); }
        Ok(index)
    }
    /** One vector count, bounded by both the input and existing VM binding capacity. */
    pub fn count(&mut self) -> Result<usize, Error> {
        let n = self.u32()? as usize;
        Module::check_count(n)?;
        if n > self.bytes.len().saturating_sub(self.at) {
            return Err(Error::invalid("vector count exceeds remaining bytes"));
        }
        Ok(n)
    }
    /** Decode a declared scalar type. */
    pub(super) fn ty(&mut self) -> Result<Type, Error> {
        let byte = self.byte()?;
        if matches!(byte, 0x68 | 0x75) && !self.wasmfx_enabled {
            return Err(Error::invalid(
                "continuation references require --wasm-wasmfx",
            ));
        }
        if byte == 0x40 {
            return Err(Error::invalid("invalid value type").at(self.base + self.at - 1));
        }
        if !self.stringref_enabled {
            let name = match byte {
                0x67 => Some("stringref"),
                0x66 => Some("stringview_wtf8ref"),
                0x60 => Some("stringview_wtf16ref"),
                0x61 => Some("stringview_iterref"),
                _ => None,
            };
            if let Some(name) = name {
                return Err(Error::invalid(&format!(
                    "invalid value type '{name}', enable with --wasm-stringref"
                )));
            }
        }
        if matches!(byte, 0x63 | 0x64) {
            super::gc_types::heap(self, byte == 0x63)
        } else {
            Type::byte(byte)
        }
    }
    /** A function parameter or result vector. */
    pub(super) fn type_list(&mut self) -> Result<Vec<Type>, Error> {
        let n = self.count()?;
        (0..n).map(|_| self.ty()).collect()
    }
    /** The grammar retains 64-bit LEB limits before applying the declared address-type page bound. */
    fn limits(&mut self, memory: bool) -> Result<Limits, Error> {
        let flags = self.u32()?;
        let limit_offset = self.base + self.at;
        if !memory && flags & 2 != 0 {
            return Err(Error::invalid("shared tables are not supported yet"));
        }
        if flags & !7 != 0 {
            return Err(Error::invalid("invalid WebAssembly limits flags"));
        }
        let address64 = flags & 4 != 0;
        if flags & 2 != 0 && flags & 1 == 0 {
            return Err(Error::invalid("shared memory requires a maximum"));
        }
        let min = if address64 {
            self.u64()?
        } else {
            u64::from(self.u32()?)
        };
        let maximum_offset = self.base + self.at;
        let max = if flags & 1 != 0 {
            Some(if address64 {
                self.u64()?
            } else {
                u64::from(self.u32()?)
            })
        } else {
            None
        };
        let bound = if memory {
            u64::from(if address64 {
                super::address::MAX_MEMORY64_PAGES
            } else {
                MAX_WASM_PAGES
            })
        } else if address64 {
            u64::MAX
        } else {
            u64::from(u32::MAX)
        };
        if !memory && min > MAX_DECLARED_TABLE_MINIMUM {
            return Err(Error::invalid(&format!("initial table size ({min} elements) is larger than implementation limit ({MAX_DECLARED_TABLE_MINIMUM} elements)")).at(limit_offset));
        }
        if memory && min > bound {
            return Err(Error::invalid(&format!("initial memory size ({min} pages) is larger than implementation limit ({bound} pages)")).at(limit_offset));
        }
        if memory && max.is_some_and(|value| value > bound) {
            return Err(Error::invalid(&format!("maximum memory size ({} pages) is larger than implementation limit ({bound} pages)", max.unwrap())).at(maximum_offset));
        }
        if min > bound || max.is_some_and(|m| m > bound || m < min) {
            return Err(Error::invalid("invalid WebAssembly limits"));
        }
        Ok(Limits {
            min,
            max,
            shared: flags & 2 != 0,
            address64,
        })
    }
    /** An unsigned u64 LEB rejects its final unused bits before native integer shifts occur. */
    pub(super) fn u64(&mut self) -> Result<u64, Error> {
        let mut value = 0;
        for index in 0..10 {
            let byte = self.byte()?;
            if index == 9 && byte > 1 {
                return Err(Error::invalid("unsigned 64-bit LEB overflow"));
            }
            value |= u64::from(byte & 127) << (index * 7);
            if byte & 128 == 0 {
                return Ok(value);
            }
        }
        Err(Error::invalid("unterminated unsigned 64-bit LEB"))
    }
    /** Ordinary memory instructions reject ordering immediates reserved for atomic operations. */
    pub(super) fn memarg(&mut self, module: &Module) -> Result<(u32, u32, u64), Error> {
        self.ordered_memarg(module, None)
    }
    /** Memory ordering is decoded before the offset and cannot be mistaken for an alignment bit. */
    pub(super) fn ordered_memarg(
        &mut self,
        module: &Module,
        rmw: Option<bool>,
    ) -> Result<(u32, u32, u64), Error> {
        let mut flags = self.u32()?;
        if flags & !127 != 0 {
            return Err(Error::invalid("invalid memory alignment flags"));
        }
        let index = if flags & 64 != 0 {
            self.memory_index()?
        } else {
            0
        };
        flags &= !64;
        if flags & 16 != 0 {
            if !self.acquire_release_enabled {
                return Err(Error::invalid(
                    "atomic memory ordering requires --wasm-acquire-release",
                ));
            }
            flags &= !16;
            let Some(rmw) = rmw else {
                return Err(Error::invalid(
                    "memory ordering immediate invalid on non-atomic operation",
                ));
            };
            let order = self.byte()?;
            let read = order & 15;
            let write = order >> 4;
            if read > 1 || write != if rmw { read } else { 0 } {
                return Err(Error::invalid("invalid atomic memory ordering"));
            }
        }
        let limits = super::address::limits(module, index)?;
        let offset = self.u64()?;
        if !limits.address64 && offset > u64::from(u32::MAX) {
            return Err(Error::invalid(
                "memory32 offset exceeds the maximum address",
            ));
        }
        Ok((flags, index, offset))
    }
    /** Retain the immediate's purpose when a memory index exceeds its LEB encoding. */
    fn memory_index(&mut self) -> Result<u32, Error> {
        self.u32().map_err(|mut error| {
            if error.message == "unsigned LEB128 overflow"
                || error.message == "unterminated unsigned LEB128"
            {
                error.message = "length overflow while decoding memory index".into();
            }
            error
        })
    }
    /** Imported and declared tables require a reference element type. */
    fn table_type(&mut self) -> Result<(Type, Limits), Error> {
        let ty = self.ty()?;
        if ty.reference().is_none() {
            return Err(Error::invalid("table element must be a reference type"));
        }
        Ok((ty, self.limits(false)?))
    }
    /** Mutability is a byte enum, not an arbitrary truthy number. */
    fn global_type(&mut self) -> Result<(Type, bool), Error> {
        let ty = self.ty()?;
        let flag = self.byte()?;
        if flag & 2 != 0 {
            return Err(Error::invalid("shared globals are not supported yet"));
        }
        if flag > 1 {
            return Err(Error::invalid("invalid global mutability"));
        }
        Ok((ty, flag == 1))
    }
    /** Extended constants contain only pure arithmetic, references and checked GC allocations. */
    fn constant(&mut self, module: &Module) -> Result<Constant, Error> {
        let mut code = Vec::new();
        loop {
            let position = self.base + self.at;
            let op = self.byte()?;
            if op == 0x0b {
                break;
            }
            let instruction = match op {
                0x41..=0x44 | 0x23 | 0xd0 | 0xd2 | 0xfd => {
                    let value = self.constant_op(op)?;
                    if let Constant::Function(index) = value {
                        if index as usize >= module.function_types().len() {
                            return Err(Error::invalid(&format!(
                                "function index #{index} is out of bounds"
                            ))
                            .at(position));
                        }
                    }
                    Instruction::Constant(value)
                }
                0x6a..=0x6c | 0x7c..=0x7e => Instruction::Simple(op),
                0xfb => {
                    let instruction = super::gc::decode(self)?;
                    if !matches!(&instruction,Instruction::Gc(op,_)if matches!(op,0|1|6|7|8|26..=28|32|33|0x82))
                    {
                        return Err(Error::invalid(&format!(
                            "opcode {} is not allowed in constant expressions",
                            super::diagnostics::opcode(op)
                        ))
                        .at(position));
                    }
                    instruction
                }
                #[cfg(not(feature = "conformance"))]
                0xfe if self.bytes.get(self.at) == Some(&7) => return Err(Error::unsupported("WebAssembly shared wait queues")),
                #[cfg(feature = "conformance")]
                0xfe if self.bytes.get(self.at) == Some(&7) => {
                    if !self.shared_enabled { return Err(Error::invalid("wait queues require --wasm-shared").at(position)); }
                    self.byte()?;
                    Instruction::Atomic(7, 0, 0)
                }
                0xfe if self.bytes.get(self.at) == Some(&4) => {
                    self.byte()?;
                    return Err(Error::invalid("opcode pause is not allowed in constant expressions").at(position));
                }
                _ => {
                    return Err(Error::invalid(&format!(
                        "opcode {} is not allowed in constant expressions",
                        super::diagnostics::opcode(op)
                    ))
                    .at(position));
                }
            };
            code.push(instruction);
            Module::check_count(code.len())?;
        }
        if code.len() == 1
            && let Instruction::Constant(value) = &code[0]
        {
            return Ok(value.clone());
        }
        Ok(Constant::Expression(code))
    }
    /** Numeric constants and reference constants share their instruction representation. */
    fn constant_op(&mut self, op: u8) -> Result<Constant, Error> {
        Ok(match op {
            0x41 => Constant::I32(self.signed(32)? as i32),
            0x42 => Constant::I64(self.signed(64)?),
            0x43 => Constant::F32(f32::from_le_bytes(self.take(4)?.try_into().unwrap())),
            0x44 => Constant::F64(f64::from_le_bytes(self.take(8)?.try_into().unwrap())),
            0xfd if self.u32()? == 12 => Constant::V128(self.take(16)?.try_into().unwrap()),
            0x23 => Constant::Global(self.u32()?),
            0xd0 => Constant::Null(super::gc_types::heap(self, true)?),
            0xd2 => Constant::Function(self.u32()?),
            _ => {
                return Err(Error::unsupported(
                    "WebAssembly extended constant expression",
                ));
            }
        })
    }
    /** Block types can refer to a function type, supporting multi-value block signatures. */
    fn block_type(&mut self, module: &Module) -> Result<Signature, Error> {
        let at = self.at;
        let first = self.byte()?;
        if first == 0x40 {
            return Ok(Signature::default());
        }
        if matches!(first, 0x63 | 0x64) || Type::byte(first).is_ok() {
            self.at = at;
            let ty = self.ty()?;
            return Ok(Signature {
                params: vec![],
                results: vec![ty],
            });
        }
        self.at = at;
        let index = self.signed(33)?;
        if !matches!(
            module.definitions.get(index as usize).map(|d| &d.kind),
            Some(super::gc_types::Kind::Function(_))
        ) {
            return Err(Error::invalid("block type must be a function signature"));
        }
        module
            .types
            .get(index as usize)
            .cloned()
            .ok_or_else(|| Error::invalid("unknown block type"))
    }
    /** A structured instruction sequence ends only at end or the owning if's else. */
    fn instructions(
        &mut self,
        module: &Module,
        depth: usize,
    ) -> Result<(Vec<Instruction>, u8), Error> {
        if depth > MAX_DEPTH {
            return Err(Error::invalid(
                "WebAssembly control nesting exceeds V9 call-depth limit",
            ));
        }
        let mut out = Vec::new();
        loop {
            let position = self.base + self.at;
            if self.at == self.bytes.len() {
                return Err(
                    Error::invalid("function body must end with \"end\" opcode").at(position)
                );
            }
            let op = self.byte()?;
            let instruction = match op {
                0x0b | 0x05 | 0x07 | 0x19 | 0x18 => return Ok((out, op)),
                0x02..=0x04 => {
                    let ty = self.block_type(module)?;
                    let (body, has_else) = self.instructions(module, depth + 1)?;
                    let otherwise = if has_else == 5 {
                        if op != 4 {
                            return Err(Error::invalid("else outside if"));
                        }
                        let (code, again) = self.instructions(module, depth + 1)?;
                        if again != 0x0b {
                            return Err(Error::invalid("duplicate else"));
                        }
                        code
                    } else {
                        if has_else != 0x0b {
                            return Err(Error::invalid("catch outside try"));
                        }
                        vec![]
                    };
                    Instruction::Block(op, ty, body, otherwise)
                }
                0x06 => {
                    let sig = self.block_type(module)?;
                    let (body, mut end) = self.instructions(module, depth + 1)?;
                    let mut handlers = Vec::new();
                    let mut all = false;
                    while matches!(end, 7 | 0x19) {
                        if all {
                            return Err(Error::invalid("catch after catch_all"));
                        }
                        let tag = if end == 7 {
                            Some(self.u32()?)
                        } else {
                            all = true;
                            None
                        };
                        let (code, next) = self.instructions(module, depth + 1)?;
                        handlers.push((tag, code));
                        end = next;
                    }
                    if end == 0x18 && handlers.is_empty() {
                        Instruction::Delegate(sig, body, self.u32()?)
                    } else {
                        if end != 0x0b {
                            return Err(Error::invalid("invalid try termination"));
                        }
                        Instruction::Try(sig, body, handlers)
                    }
                }
                0x08 => Instruction::Throw(self.u32()?),
                0x09 => Instruction::Rethrow(self.u32()?),
                0x0a => Instruction::ThrowRef,
                0x1f => {
                    let sig = self.block_type(module)?;
                    let count = self.count()?;
                    let mut handlers = Vec::new();
                    for _ in 0..count {
                        let kind = self.byte()?;
                        if kind > 3 {
                            return Err(Error::invalid("invalid try_table catch kind"));
                        }
                        let tag = if kind < 2 { Some(self.u32()?) } else { None };
                        handlers.push(Catch {
                            tag,
                            reference: kind & 1 != 0,
                            depth: self.u32()?,
                        });
                    }
                    let (body, end) = self.instructions(module, depth + 1)?;
                    if end != 0x0b {
                        return Err(Error::invalid("invalid try_table termination"));
                    }
                    Instruction::TryTable(sig, handlers, body)
                }
                0x0c => Instruction::Branch(self.u32()?),
                0x0d => Instruction::BranchIf(self.u32()?),
                0x0e => {
                    let n = self.count()?;
                    if n > MAX_BRANCH_TABLE {
                        return Err(Error::invalid("invalid table count"));
                    }
                    let labels = (0..n).map(|_| self.u32()).collect::<Result<Vec<_>, _>>()?;
                    Instruction::BranchTable(labels, self.u32()?)
                }
                0x10 => Instruction::Call(self.u32()?),
                0x11 => Instruction::Indirect(self.u32()?, self.u32()?),
                0x14 => Instruction::CallRef(self.u32()?),
                0x12 => Instruction::TailCall(self.u32()?),
                0x13 => Instruction::TailIndirect(self.u32()?, self.u32()?),
                0x15 => Instruction::TailRef(self.u32()?),
                0xd5 | 0xd6 => Instruction::BranchNull(op, self.u32()?),
                0x1c => {
                    if self.u32()? != 1 {
                        return Err(Error::invalid("typed select requires one result type"));
                    }
                    Instruction::Select(self.ty()?)
                }
                0x20..=0x22 => Instruction::Local(op, self.u32()?),
                0x23..=0x24 => Instruction::Global(op, self.u32()?),
                0x25..=0x26 => Instruction::Table(op, self.u32()?),
                0x28..=0x3e => {
                    let (align, index, offset) = self.memarg(module)?;
                    Instruction::Memory(op, align, index, offset)
                }
                0x3f..=0x40 => Instruction::MemoryIndex(op, self.memory_index()?),
                0x41..=0x44 | 0xd0 | 0xd2 => Instruction::Constant(self.constant_op(op)?),
                0xfd => super::simd::decode(self, module)?,
                0xfe => super::atomic::decode(self, module)?,
                0xfb => super::gc::decode(self)?,
                0xe0 => {
                    if !self.wasmfx_enabled {
                        return Err(Error::invalid(
                            "continuation instructions require --wasm-wasmfx",
                        ));
                    }
                    let index = self.u32()?;
                    Instruction::Continuation(op, index)
                }
                0xe3 | 0xe4 | 0xe5 => {
                    if !self.wasmfx_enabled {
                        return Err(Error::invalid("resume requires --wasm-wasmfx"));
                    }
                    let index = self.u32()?;
                    let tag = if op == 0xe4 { Some(self.u32()?) } else { None };
                    let count = self.count()?;
                    let mut handlers = Vec::with_capacity(count);
                    for _ in 0..count {
                        let kind = self.byte()?;
                        let tag = self.u32()?;
                        if kind > 1 {
                            return Err(Error::invalid("invalid continuation effect handler"));
                        }
                        handlers.push(EffectHandler {
                            tag,
                            depth: if kind == 0 { self.u32()? } else { 0 },
                            switch: kind == 1,
                        });
                    }
                    if op == 0xe3 {
                        Instruction::Resume(index, handlers)
                    } else {
                        Instruction::ResumeThrow(index, tag, handlers)
                    }
                }
                0xe1 => {
                    if !self.wasmfx_enabled {
                        return Err(Error::invalid(
                            "continuation instructions require --wasm-wasmfx",
                        ));
                    }
                    Instruction::ContinuationBind(self.u32()?, self.u32()?)
                }
                0xe2 => {
                    if !self.wasmfx_enabled {
                        return Err(Error::invalid("suspend requires --wasm-wasmfx"));
                    }
                    Instruction::Suspend(self.u32()?)
                }
                0xe6 => {
                    if !self.wasmfx_enabled {
                        return Err(Error::invalid("switch requires --wasm-wasmfx"));
                    }
                    Instruction::Switch(self.u32()?, self.u32()?)
                }
                0xfc => {
                    let sub = self.prefixed_opcode()?;
                    if (19..=22).contains(&sub) && !self.wide_arithmetic_enabled {
                        return Err(Error::invalid(
                            "wide arithmetic requires --wasm-wide-arithmetic",
                        ));
                    }
                    if matches!(sub, 0x30 | 0x31) {
                        if !self.fp16_enabled {
                            return Err(Error::invalid(
                                "f16 memory instructions require --wasm-fp16",
                            ));
                        }
                        let (align, index, offset) = self.memarg(module)?;
                        Instruction::Memory(
                            if sub == 0x30 { 0xc5 } else { 0xc6 },
                            align,
                            index,
                            offset,
                        )
                    } else {
                        let count = match sub {
                            0..=7 | 19..=22 => 0,
                            8 | 10 | 12 | 14 => 2,
                            9 | 11 | 13 | 15..=17 => 1,
                            18 => return Err(Error::unsupported("WebAssembly memory.discard")),
                            _ => {
                                return Err(Error::invalid(&format!(
                                    "invalid numeric opcode: 0xfc{sub:x}"
                                )));
                            }
                        };
                        let args = (0..count)
                            .map(|_| self.u32())
                            .collect::<Result<Vec<_>, _>>()?;
                        Instruction::Bulk(sub, args)
                    }
                }
                0x00 | 0x01 | 0x0f | 0x1a | 0x1b | 0x45..=0xc4 | 0xd1 | 0xd3 | 0xd4 => {
                    Instruction::Simple(op)
                }
                _ => return Err(Error::invalid(&format!("Invalid opcode 0x{op:02x}")).at(position)),
            };
            out.push(Instruction::Located(position, Box::new(instruction)));
            Module::check_count(out.len())?;
        }
    }
}

/** Validate the envelope, decode sections in order, then apply full supported-subset typing. */
pub(super) fn parse(bytes: &[u8]) -> Result<Module, Error> {
    parse_with_features(bytes, true, true, true, true, true, true, true, true)
}
/** Compilation applies the optional proposal state selected by the current embedding. */
pub(super) fn parse_with_features(
    bytes: &[u8],
    stringref_enabled: bool,
    custom_descriptors_enabled: bool,
    acquire_release_enabled: bool,
    fp16_enabled: bool,
    wide_arithmetic_enabled: bool,
    allow_mixed_eh: bool,
    wasmfx_enabled: bool,
    shared_enabled: bool,
) -> Result<Module, Error> {
    if bytes.len() > BUFFER_BYTES {
        return Err(Error::invalid("WebAssembly binary exceeds V9 buffer limit"));
    }
    if bytes.len() < MAGIC.len() {
        return Err(Error::invalid("expected magic word").at(0));
    }
    let mut reader = Reader::new(bytes);
    reader.stringref_enabled = stringref_enabled;
    reader.custom_descriptors_enabled = custom_descriptors_enabled;
    reader.acquire_release_enabled = acquire_release_enabled;
    reader.fp16_enabled = fp16_enabled;
    reader.wide_arithmetic_enabled = wide_arithmetic_enabled;
    reader.wasmfx_enabled = wasmfx_enabled;
    reader.shared_enabled = shared_enabled;
    let header = reader.take(MAGIC.len())?;
    if header[..4] != MAGIC[..4] {
        return Err(Error::invalid("expected magic word").at(0));
    }
    if header[4..] != MAGIC[4..] {
        return Err(Error::invalid("expected version 1").at(4));
    }
    let mut module = Module::default();
    module.custom_descriptors_enabled = custom_descriptors_enabled;
    module.wasmfx_enabled = wasmfx_enabled;
    module.names = super::diagnostics::function_names(bytes);
    let mut seen = std::collections::BTreeSet::new();
    let mut last = 0;
    while reader.at < bytes.len() {
        let section_offset = reader.at;
        let section = reader.byte()?;
        let size = reader.u32().map_err(|mut error| {
            if error.message == "unsigned LEB128 overflow"
                || error.message == "unterminated unsigned LEB128"
            {
                error.message = "length overflow while decoding section length".into();
                error.offset = Some(reader.at.saturating_sub(1));
            }
            error
        })? as usize;
        if section > 14 {
            return Err(
                Error::invalid(&format!("unknown section code #0x{section:02x}"))
                    .at(reader.base + reader.at),
            );
        }
        if section == 14 {
            if !stringref_enabled {
                return Err(Error::invalid(
                    "unexpected section <StringRef> (enable with --wasm-stringref)",
                ));
            }
        }
        let remaining = bytes.len() - reader.at;
        if size > remaining {
            let name = match section {
                1 => "Type",
                2 => "Import",
                3 => "Function",
                4 => "Table",
                5 => "Memory",
                6 => "Global",
                7 => "Export",
                8 => "Start",
                9 => "Element",
                10 => "Code",
                11 => "Data",
                12 => "DataCount",
                13 => "Tag",
                _ => "Unknown",
            };
            return Err(Error::invalid(&format!("section (code {section}, \"{name}\") extends past end of the module (length {size}, remaining bytes {remaining})")).at(section_offset));
        }
        let mut payload = reader.section(size)?;
        if section != 0 {
            if !seen.insert(section) {
                if section == 8 {
                    return Err(
                        Error::invalid("unexpected section <Start>").at(reader.base + reader.at)
                    );
                }
                if section == 10 {
                    return Err(Error::invalid("unexpected section <Code>").at(payload.base));
                }
                return Err(Error::invalid("duplicate WebAssembly section"));
            }
            let order = match section {
                13 => 6,
                14 => 7,
                6..=9 => section + 2,
                12 => 12,
                10 => 13,
                11 => 14,
                n => n,
            };
            if order < last {
                if section == 8 {
                    return Err(
                        Error::invalid("unexpected section <Start>").at(reader.base + reader.at)
                    );
                }
                return Err(Error::invalid("WebAssembly sections out of order"));
            }
            last = order;
        }
        section_data(&mut module, section, &mut payload)?;
        if payload.at != payload.bytes.len() {
            return Err(Error::invalid(&format!(
                "section was shorter than expected size ({} bytes expected, {} decoded)",
                payload.bytes.len(),
                payload.at
            ))
            .at(payload.base + payload.at));
        }
    }
    if let Some(expected) = module.data_count {
        if expected as usize != module.data.len() {
            return Err(Error::invalid(&format!(
                "data segments count {} mismatch ({expected} expected)",
                module.data.len()
            ))
            .at(bytes.len()));
        }
    }
    if !module.functions.is_empty() && !seen.contains(&10) {
        return Err(Error::invalid(&format!(
            "function count is {}, but code section is absent",
            module.functions.len()
        ))
        .at(bytes.len()));
    }
    super::validate::module(&module, allow_mixed_eh)?;
    Ok(module)
}

/** Section parsing owns only inert values and never instantiates or calls imported JavaScript. */
fn section_data(module: &mut Module, id: u8, r: &mut Reader) -> Result<(), Error> {
    if id == 0 {
        let name = r.name()?;
        let bytes = r.take(r.bytes.len() - r.at)?.to_vec();
        module.custom.push((name, bytes));
        return Ok(());
    }
    if id == 8 {
        let offset = r.base + r.at;
        let index = r.u32()?;
        let count = module.function_types().len();
        if index as usize >= count {
            let entries = if count == 1 { "entry" } else { "entries" };
            return Err(Error::invalid(&format!(
                "function index {index} out of bounds ({count} {entries})"
            ))
            .at(offset));
        }
        module.start = Some(index);
        return Ok(());
    }
    if id == 12 {
        module.data_count = Some(r.u32()?);
        return Ok(());
    }
    if id == 14 {
        let deferred = r.u32()?;
        if deferred != 0 {
            return Err(Error::invalid(&format!(
                "Invalid deferred string literal count {deferred} (expected 0)"
            )));
        }
        let count = r.count()?;
        for _ in 0..count {
            let length = r.u32()? as usize;
            let bytes = r.take(length)?;
            let units = super::strings::decode_wtf8(bytes)?;
            let value = crate::v9::value::JsString::from_units(units)
                .map_err(|_| Error::invalid("string literal exceeds V9 string limit"))?;
            module.string_literals.push(value);
        }
        return Ok(());
    }
    let count = r.count().map_err(|mut error| {
        if id == 10 && error.message == "unexpected end of WebAssembly binary" {
            error.message = "reached end while decoding functions count".into();
            error.offset = Some(r.base + r.at);
        }
        error
    })?;
    for _ in 0..count {
        match id {
            1 => super::gc_types::read_group(module, r)?,
            2 => {
                let offset = r.base + r.at;
                let owner = r.name()?;
                let name = r.name()?;
                let code = r.byte()?;
                if code == 32 && !r.custom_descriptors_enabled {
                    return Err(Error::invalid(
                        "exact function imports require --wasm-custom-descriptors",
                    ));
                }
                let kind = match code {
                    0 | 32 => External::Function(r.u32()?),
                    1 => {
                        let (t, l) = r.table_type()?;
                        External::Table(t, l)
                    }
                    2 => External::Memory(r.limits(true)?),
                    3 => {
                        let (t, m) = r.global_type()?;
                        External::Global(t, m)
                    }
                    4 => {
                        if r.byte()? != 0 {
                            return Err(Error::invalid("invalid tag attribute"));
                        }
                        External::Tag(r.u32()?)
                    }
                    _ => return Err(Error::unsupported("WebAssembly import kind")),
                };
                module.imports.push(Import {
                    offset,
                    exact_function: code == 32,
                    module: owner,
                    name,
                    kind,
                })
            }
            3 => module.functions.push(r.u32()?),
            4 => {
                let initialized = if r.bytes.get(r.at) == Some(&0x40) {
                    r.byte()?;
                    if r.byte()? != 0 {
                        return Err(Error::invalid("invalid table initializer marker"));
                    }
                    true
                } else {
                    false
                };
                let (ty, limits) = r.table_type()?;
                let value = if initialized {
                    Some(r.constant(module)?)
                } else {
                    None
                };
                module.tables.push((ty, limits, value));
            }
            5 => module.memories.push(r.limits(true)?),
            13 => {
                if r.byte()? != 0 {
                    return Err(Error::invalid("invalid tag attribute"));
                }
                module.tags.push(r.u32()?);
            }
            6 => {
                let (t, m) = r.global_type()?;
                let value = r.constant(module)?;
                module.globals.push((t, m, value))
            }
            7 => {
                let name = r.name()?;
                let name_offset = r.base + r.at - name.len();
                let kind = r.byte()?;
                let index_offset = r.base + r.at;
                let index = r.u32()?;
                if kind == 4 {
                    let count = super::exceptions::tag_types(module).len();
                    if index as usize >= count {
                        let entries = if count == 1 { "entry" } else { "entries" };
                        return Err(Error::invalid(&format!(
                            "tag index {index} out of bounds ({count} {entries})"
                        ))
                        .at(index_offset));
                    }
                }
                module.exports.push(Export {
                    name,
                    kind,
                    index,
                    name_offset,
                })
            }
            9 => module.elements.push(element(r, module)?),
            10 => {
                let n = r.u32()? as usize;
                let mut body = r.section(n)?;
                let index = (module.imported_functions() + module.bodies.len()) as u32;
                let name = module.names.get(&index).cloned();
                let decoded = (|| {
                    let groups = body.count()?;
                    let mut locals = Vec::new();
                    for _ in 0..groups {
                        let n = body.u32()? as usize;
                        let ty = body.ty()?;
                        Module::check_count(locals.len().saturating_add(n))?;
                        locals.extend(std::iter::repeat_n(ty, n))
                    }
                    let (code, has_else) = body.instructions(module, 0)?;
                    if has_else != 0x0b || body.at != body.bytes.len() {
                        return Err(Error::invalid("invalid function body termination"));
                    }
                    Ok(Body {
                        locals,
                        code,
                        end: body.base + body.at - 1,
                    })
                })()
                .map_err(|error: Error| error.at(body.base + body.at).in_function(index, name))?;
                module.bodies.push(decoded)
            }
            11 => {
                let flags = r.u32()?;
                let memory = match flags {
                    0 => Some(0),
                    1 => None,
                    2 => Some(r.u32()?),
                    _ => return Err(Error::unsupported("WebAssembly data segment")),
                };
                let offset = if memory.is_some() {
                    Some(r.constant(module)?)
                } else {
                    None
                };
                let n = r.u32()? as usize;
                let bytes = r.take(n)?.to_vec();
                module.data.push(Data {
                    memory,
                    offset,
                    bytes,
                })
            }
            _ => return Err(Error::unsupported("WebAssembly section")),
        }
    }
    Ok(())
}

/** MVP element segments and reference-expression segments use the same typed initializers. */
fn element(r: &mut Reader, module: &Module) -> Result<Element, Error> {
    let flags = r.u32()?;
    if flags > 7 {
        return Err(Error::invalid("invalid element segment flags"));
    }
    let active = flags & 1 == 0;
    let table = if active {
        Some(if flags & 2 != 0 { r.u32()? } else { 0 })
    } else {
        None
    };
    let offset = if active {
        Some(r.constant(module)?)
    } else {
        None
    };
    let mut ty = Type::Func;
    if flags & 4 == 0 {
        if flags != 0 && r.byte()? != 0 {
            return Err(Error::invalid("invalid element kind"));
        }
    } else if flags != 4 {
        ty = r.ty()?;
    }
    let n = r.count()?;
    let values = (0..n)
        .map(|_| {
            if flags & 4 != 0 {
                r.constant(module)
            } else {
                r.u32().map(Constant::Function)
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Element {
        passive: matches!(flags, 1 | 5),
        ty,
        table,
        offset,
        values,
    })
}
