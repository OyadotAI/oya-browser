/** A checked WebAssembly binary reader; lengths, indices and LEB encodings never index unchecked. */
use super::{constants::*, types::*};

/** One bounded slice; child section readers cannot consume a neighboring section. */
pub(super) struct Reader<'a> {
    pub bytes: &'a [u8],
    pub at: usize,
    pub base: usize,
}
impl<'a> Reader<'a> {
    /** A fresh section or body reader. */
    pub fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            at: 0,
            base: 0,
        }
    }
    /** Nested readers retain absolute binary offsets without reading outside their own section. */
    fn section(&mut self, count: usize) -> Result<Reader<'a>, Error> {
        let base = self.base + self.at;
        Ok(Reader {
            bytes: self.take(count)?,
            at: 0,
            base,
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
        if flags & !7 != 0 || !memory && flags & 2 != 0 {
            return Err(Error::invalid("invalid WebAssembly limits flags"));
        }
        let address64 = flags & 4 != 0;
        if address64 && !memory {
            return Err(Error::unsupported("WebAssembly table64"));
        }
        if flags & 2 != 0 && flags & 1 == 0 {
            return Err(Error::invalid("shared memory requires a maximum"));
        }
        let min = if address64 {
            self.u64()?
        } else {
            u64::from(self.u32()?)
        };
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
        } else {
            u64::from(u32::MAX)
        };
        if !memory && min > MAX_DECLARED_TABLE_MINIMUM {
            return Err(Error::invalid(
                "table initial size exceeds the maximum declared table size",
            ));
        }
        if min > bound || max.is_some_and(|m| m > bound || m < min) {
            return Err(Error::invalid("invalid WebAssembly limits"));
        }
        Ok(Limits {
            min: min as u32,
            max: max.map(|m| m as u32),
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
        let index = if flags & 64 != 0 { self.u32()? } else { 0 };
        flags &= !64;
        if flags & 16 != 0 {
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
        if flag > 1 {
            return Err(Error::invalid("invalid global mutability"));
        }
        Ok((ty, flag == 1))
    }
    /** Extended constants contain only pure arithmetic, references and checked GC allocations. */
    fn constant(&mut self) -> Result<Constant, Error> {
        let mut code = Vec::new();
        loop {
            let position = self.base + self.at;
            let op = self.byte()?;
            if op == 0x0b {
                break;
            }
            let instruction = match op {
                0x41..=0x44 | 0x23 | 0xd0 | 0xd2 | 0xfd => {
                    Instruction::Constant(self.constant_op(op)?)
                }
                0x6a..=0x6c | 0x7c..=0x7e => Instruction::Simple(op),
                0xfb => {
                    let instruction = super::gc::decode(self)?;
                    if !matches!(&instruction,Instruction::Gc(op,_)if matches!(op,0|1|6|7|8|26..=28|32|33))
                    {
                        return Err(Error::invalid(&format!(
                            "opcode {} is not allowed in constant expressions",
                            super::diagnostics::opcode(op)
                        ))
                        .at(position));
                    }
                    instruction
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
                0x3f..=0x40 => Instruction::MemoryIndex(op, self.u32()?),
                0x41..=0x44 | 0xd0 | 0xd2 => Instruction::Constant(self.constant_op(op)?),
                0xfd => super::simd::decode(self, module)?,
                0xfe => super::atomic::decode(self, module)?,
                0xfb => super::gc::decode(self)?,
                0xfc => {
                    let sub = self.u32()?;
                    let count = match sub {
                        0..=7 | 19..=22 => 0,
                        8 | 10 | 12 | 14 => 2,
                        9 | 11 | 13 | 15..=17 => 1,
                        _ => return Err(Error::unsupported("WebAssembly bulk instruction")),
                    };
                    let args = (0..count)
                        .map(|_| self.u32())
                        .collect::<Result<Vec<_>, _>>()?;
                    Instruction::Bulk(sub, args)
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
    if bytes.len() > BUFFER_BYTES {
        return Err(Error::invalid("WebAssembly binary exceeds V9 buffer limit"));
    }
    if bytes.len() < MAGIC.len() {
        return Err(Error::invalid("expected magic word").at(0));
    }
    let mut reader = Reader::new(bytes);
    if reader.take(MAGIC.len())? != MAGIC {
        return Err(Error::invalid("expected magic word and version 1"));
    }
    let mut module = Module::default();
    module.names = super::diagnostics::function_names(bytes);
    let mut seen = std::collections::BTreeSet::new();
    let mut last = 0;
    while reader.at < bytes.len() {
        let section = reader.byte()?;
        let size = reader.u32()? as usize;
        if section > 14 {
            return Err(
                Error::invalid(&format!("unknown section code #0x{section:02x}"))
                    .at(reader.base + reader.at),
            );
        }
        if section == 14 {
            return Err(Error::unsupported("WebAssembly string literal section"));
        }
        let mut payload = reader.section(size)?;
        if section != 0 {
            if !seen.insert(section) {
                return Err(Error::invalid("duplicate WebAssembly section"));
            }
            let order = match section {
                13 => 6,
                6..=9 => section + 1,
                12 => 11,
                10 => 12,
                11 => 13,
                n => n,
            };
            if order < last {
                return Err(Error::invalid("WebAssembly sections out of order"));
            }
            last = order;
        }
        section_data(&mut module, section, &mut payload)?;
        if payload.at != payload.bytes.len() {
            return Err(Error::invalid("unconsumed WebAssembly section bytes"));
        }
    }
    super::validate::module(&module)?;
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
        module.start = Some(r.u32()?);
        return Ok(());
    }
    if id == 12 {
        module.data_count = Some(r.u32()?);
        return Ok(());
    }
    let count = r.count()?;
    for _ in 0..count {
        match id {
            1 => super::gc_types::read_group(module, r)?,
            2 => {
                let owner = r.name()?;
                let name = r.name()?;
                let kind = match r.byte()? {
                    0 => External::Function(r.u32()?),
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
                    Some(r.constant()?)
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
                let value = r.constant()?;
                module.globals.push((t, m, value))
            }
            7 => {
                let name = r.name()?;
                let kind = r.byte()?;
                let index = r.u32()?;
                module.exports.push(Export { name, kind, index })
            }
            9 => module.elements.push(element(r)?),
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
                    Some(r.constant()?)
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
fn element(r: &mut Reader) -> Result<Element, Error> {
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
    let offset = if active { Some(r.constant()?) } else { None };
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
                r.constant()
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
