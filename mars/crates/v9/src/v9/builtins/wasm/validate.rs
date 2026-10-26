/** Structural and stack typing for every instruction the owned Wasm interpreter accepts. */
use super::{gc_types, types::*};

/** Operand typing within one control frame; unreachable code has a polymorphic empty stack. */
#[derive(Clone)]
struct Stack<'a> {
    module: &'a Module,
    initialized: Vec<bool>,
    caught: Vec<usize>,
    values: Vec<Type>,
    producers: Vec<&'static str>,
    current: &'static str,
    last: &'static str,
    unreachable: bool,
}
impl Stack<'_> {
    /** Pop a typed value, preserving polymorphism only after an unconditional transfer. */
    fn pop(&mut self) -> Result<Option<Type>, Error> {
        self.last = self.producers.pop().unwrap_or("expression");
        match self.values.pop() {
            Some(t) => Ok(Some(t)),
            None if self.unreachable => Ok(None),
            None => Err(Error::invalid("WebAssembly operand stack underflow")),
        }
    }
    /** Consume exactly one expected type. */
    fn expect(&mut self, ty: Type) -> Result<(), Error> {
        if let Some(got) = self.pop()?
            && !gc_types::subtype(self.module, got, ty)
        {
            return Err(Error::invalid(&format!(
                "expected type {}, found {} of type {}",
                ty.display(),
                self.last,
                got.display()
            )));
        }
        Ok(())
    }
    /** Every operand records the instruction that actually produced it. */
    fn push(&mut self, ty: Type) {
        self.values.push(ty);
        self.producers.push(self.current)
    }
    /** Block signatures and call results share the same provenance-preserving append path. */
    fn extend<I>(&mut self, values: I)
    where
        I: IntoIterator,
        I::Item: std::borrow::Borrow<Type>,
    {
        for ty in values {
            let ty: &Type = std::borrow::Borrow::borrow(&ty);
            self.push(*ty)
        }
    }
    /** Branch operands retain their declared order in diagnostics, including unreachable polymorphism. */
    fn branch(&mut self, types: &[Type]) -> Result<(), Error> {
        for (index, ty) in types.iter().enumerate().rev() {
            if let Some(got) = self.pop()?
                && !gc_types::subtype(self.module, got, *ty)
            {
                return Err(Error::invalid(&format!(
                    "type error in branch[{index}] (expected {}, got {})",
                    ty.name(),
                    got.name()
                )));
            }
        }
        Ok(())
    }
    /** Parameters and results occur in source order on the operand stack. */
    fn take(&mut self, types: &[Type]) -> Result<(), Error> {
        for ty in types.iter().rev() {
            self.expect(*ty)?
        }
        Ok(())
    }
    /** Finish one control frame without allowing surplus operands. */
    fn finish(mut self, results: &[Type]) -> Result<(), Error> {
        self.take(results)?;
        if !self.values.is_empty() {
            return Err(Error::invalid(
                "WebAssembly control frame leaves extra values",
            ));
        }
        Ok(())
    }
    /** A branch discards the current frame's values, not its enclosing typing context. */
    fn stop(&mut self) {
        self.values.clear();
        self.producers.clear();
        self.unreachable = true;
    }
}

/** All declarations and bodies are checked before any start function or import can execute. */
pub(super) fn module(m: &Module) -> Result<(), Error> {
    gc_types::validate(m)?;
    if m.data_count.is_some_and(|n| n as usize != m.data.len()) {
        return Err(Error::invalid("data count and data section lengths differ"));
    }
    if m.functions.len() != m.bodies.len() {
        return Err(Error::invalid("function and code section counts differ"));
    }
    let function_types = m.function_types();
    for ty in &function_types {
        if !matches!(
            m.definitions.get(*ty as usize).map(|d| &d.kind),
            Some(gc_types::Kind::Function(_))
        ) {
            return Err(Error::invalid("unknown function type"));
        }
    }
    for index in super::exceptions::tag_types(m) {
        if !matches!(m.definitions.get(index as usize).map(|d|&d.kind),Some(gc_types::Kind::Function(sig))if sig.results.is_empty())
        {
            return Err(Error::invalid(
                "tag type must be a function without results",
            ));
        }
    }
    let globals = global_types(m);
    let tables = table_types(m);
    let memories = memory_count(m);
    for (ty, _, value) in &m.tables {
        match value {
            Some(value) if !gc_types::subtype(m, constant_type(m, value, true)?, *ty) => {
                return Err(Error::invalid("table initializer type mismatch"));
            }
            None if !gc_types::defaultable(*ty) => {
                return Err(Error::invalid("nonnullable table requires an initializer"));
            }
            _ => (),
        }
    }
    for ty in &tables {
        gc_types::check_type(m, *ty)?;
    }
    for (ty, _) in &globals {
        gc_types::check_type(m, *ty)?;
    }
    for element in &m.elements {
        gc_types::check_type(m, element.ty)?;
        if element.ty.reference().is_none() {
            return Err(Error::invalid("element segment requires a reference type"));
        }
        for value in &element.values {
            if !gc_types::subtype(m, constant_type(m, value, true)?, element.ty) {
                return Err(Error::invalid("element initializer type mismatch"));
            }
        }
    }
    let mut names = std::collections::BTreeSet::new();
    for export in &m.exports {
        if !names.insert(&export.name) {
            return Err(Error::invalid("duplicate export name"));
        }
        let bound = match export.kind {
            0 => function_types.len(),
            1 => tables.len(),
            2 => memories,
            3 => globals.len(),
            4 => super::exceptions::tag_types(m).len(),
            _ => return Err(Error::invalid("unknown export kind")),
        };
        if export.index as usize >= bound {
            return Err(Error::invalid("unknown export index"));
        }
    }
    for (i, (ty, _, value)) in m.globals.iter().enumerate() {
        let t = constant_type(m, value, true)?;
        if !gc_types::subtype(m, t, *ty) {
            return Err(Error::invalid("global initializer type mismatch"));
        }
        let _ = i;
    }
    if let Some(start) = m.start {
        let sig = m.signature(start)?;
        if !sig.params.is_empty() || !sig.results.is_empty() {
            return Err(Error::invalid(
                "start function must have no parameters or results",
            ));
        }
    }
    for data in &m.data {
        if let Some(memory) = data.memory {
            if memory as usize >= memories {
                return Err(Error::invalid("unknown data segment memory"));
            }
            if data
                .offset
                .as_ref()
                .map(|v| constant_type(m, v, true))
                .transpose()?
                != Some(super::address::ty(m, memory)?)
            {
                return Err(Error::invalid("data offset must match memory address type"));
            }
        }
    }
    for element in &m.elements {
        if let Some(table) = element.table {
            let ty = *tables
                .get(table as usize)
                .ok_or_else(|| Error::invalid("unknown element table"))?;
            if !gc_types::subtype(m, element.ty, ty) {
                return Err(Error::invalid("element segment and table types differ"));
            }
            if element
                .offset
                .as_ref()
                .map(|v| constant_type(m, v, true))
                .transpose()?
                != Some(Type::I32)
            {
                return Err(Error::invalid("element offset must be i32"));
            }
            for value in &element.values {
                if !gc_types::subtype(m, constant_type(m, value, true)?, ty) {
                    return Err(Error::invalid("element initializer type mismatch"));
                }
            }
        }
    }
    for (index, body) in m.bodies.iter().enumerate() {
        let sig = m.signature((m.imported_functions() + index) as u32)?;
        let mut locals = sig.params.clone();
        locals.extend(body.locals.iter());
        for ty in &body.locals {
            gc_types::check_type(m, *ty)?;
        }
        let initialized = std::iter::repeat_n(true, sig.params.len())
            .chain(body.locals.iter().map(|t| gc_types::defaultable(*t)))
            .collect();
        let mut stack = Stack {
            module: m,
            initialized,
            caught: vec![],
            values: Vec::new(),
            producers: vec![],
            current: "expression",
            last: "expression",
            unreachable: false,
        };
        let mut labels = vec![sig.results.clone()];
        let result = sequence(
            m,
            &body.code,
            &locals,
            &mut labels,
            &sig.results,
            &mut stack,
        )
        .and_then(|()| stack.finish(&sig.results));
        let function = (m.imported_functions() + index) as u32;
        result.map_err(|error| {
            error
                .at(body.end)
                .in_function(function, m.names.get(&function).cloned())
        })?;
    }
    Ok(())
}

/** Global index space is imported globals followed by declared globals. */
pub(super) fn global_types(m: &Module) -> Vec<(Type, bool)> {
    m.imports
        .iter()
        .filter_map(|i| match i.kind {
            External::Global(t, b) => Some((t, b)),
            _ => None,
        })
        .chain(m.globals.iter().map(|(t, b, _)| (*t, *b)))
        .collect()
}
/** Table element types are retained for indirect-call and table instruction checks. */
pub(super) fn table_types(m: &Module) -> Vec<Type> {
    m.imports
        .iter()
        .filter_map(|i| match i.kind {
            External::Table(t, _) => Some(t),
            _ => None,
        })
        .chain(m.tables.iter().map(|(t, _, _)| *t))
        .collect()
}
/** All memory instruction indices must resolve before interpretation. */
pub(super) fn memory_count(m: &Module) -> usize {
    m.imports
        .iter()
        .filter(|i| matches!(i.kind, External::Memory(_)))
        .count()
        + m.memories.len()
}
/** Constant global reads may only reference imported immutable globals. */
fn constant_type(m: &Module, value: &Constant, initializer: bool) -> Result<Type, Error> {
    Ok(match value {
        Constant::Expression(code) => constant_expression_type(m, code, initializer)?,
        Constant::I32(_) => Type::I32,
        Constant::I64(_) => Type::I64,
        Constant::F32(_) => Type::F32,
        Constant::F64(_) => Type::F64,
        Constant::V128(_) => Type::V128,
        Constant::Null(t) if t.reference().is_some() => {
            gc_types::check_type(m, *t)?;
            *t
        }
        Constant::Function(i) => {
            m.signature(*i)?;
            Type::Ref(i64::from(m.function_type(*i)?), false)
        }
        Constant::Global(i) => {
            let globals = global_types(m);
            let (t, mutable) = *globals
                .get(*i as usize)
                .ok_or_else(|| Error::invalid("unknown initializer global"))?;
            let imported = m
                .imports
                .iter()
                .filter(|i| matches!(i.kind, External::Global(..)))
                .count();
            if initializer && (*i as usize >= imported || mutable) {
                return Err(Error::invalid(
                    "initializer global must be imported and immutable",
                ));
            }
            t
        }
        _ => return Err(Error::invalid("invalid constant reference type")),
    })
}
/** A branch label is relative to the innermost control frame. */
fn label(labels: &[Vec<Type>], depth: u32) -> Result<Vec<Type>, Error> {
    labels
        .iter()
        .rev()
        .nth(depth as usize)
        .cloned()
        .ok_or_else(|| Error::invalid("unknown WebAssembly branch label"))
}

/** Type each structured instruction while retaining each enclosing branch contract. */
fn sequence(
    m: &Module,
    code: &[Instruction],
    locals: &[Type],
    labels: &mut Vec<Vec<Type>>,
    returns: &[Type],
    s: &mut Stack,
) -> Result<(), Error> {
    for instruction in code {
        if !matches!(instruction, Instruction::Located(..)) {
            s.current = super::diagnostics::producer(instruction)
        }
        match instruction {
            Instruction::Located(position, inner) => sequence(
                m,
                std::slice::from_ref(inner.as_ref()),
                locals,
                labels,
                returns,
                s,
            )
            .map_err(|error| error.at(*position))?,
            Instruction::Constant(c) => s.push(constant_type(m, c, false)?),
            Instruction::Local(op, index) => {
                let t = *locals
                    .get(*index as usize)
                    .ok_or_else(|| Error::invalid(&format!("invalid local index: {index}")))?;
                if *op == 0x20 && !s.initialized[*index as usize] {
                    return Err(Error::invalid("non-defaultable local is not initialized"));
                }
                if *op != 0x20 {
                    s.expect(t)?
                }
                if *op != 0x20 {
                    s.initialized[*index as usize] = true;
                }
                if *op != 0x21 {
                    s.push(t)
                }
            }
            Instruction::Global(op, index) => {
                let (t, mutable) = *global_types(m)
                    .get(*index as usize)
                    .ok_or_else(|| Error::invalid("unknown global index"))?;
                if *op == 0x23 {
                    s.push(t)
                } else {
                    if !mutable {
                        return Err(Error::invalid("global.set requires a mutable global"));
                    }
                    s.expect(t)?
                }
            }
            Instruction::Call(index) => {
                let sig = m.signature(*index)?;
                s.take(&sig.params)?;
                s.extend(&sig.results)
            }
            Instruction::TailCall(index) => {
                let sig = m.signature(*index)?;
                tail_results(m, sig, returns)?;
                s.take(&sig.params)?;
                s.stop()
            }
            Instruction::TailIndirect(index, table) => {
                let sig = m
                    .types
                    .get(*index as usize)
                    .ok_or_else(|| Error::invalid("unknown tail indirect signature"))?;
                if !table_types(m)
                    .get(*table as usize)
                    .is_some_and(|t| gc_types::subtype(m, *t, Type::Func))
                {
                    return Err(Error::invalid(
                        "return_call_indirect requires a function table",
                    ));
                }
                tail_results(m, sig, returns)?;
                s.expect(Type::I32)?;
                s.take(&sig.params)?;
                s.stop()
            }
            Instruction::TailRef(index) => {
                let sig = m
                    .types
                    .get(*index as usize)
                    .ok_or_else(|| Error::invalid("unknown tail call_ref signature"))?;
                if !matches!(
                    m.definitions.get(*index as usize).map(|d| &d.kind),
                    Some(gc_types::Kind::Function(_))
                ) {
                    return Err(Error::invalid("return_call_ref requires a function type"));
                }
                tail_results(m, sig, returns)?;
                s.expect(Type::Ref(i64::from(*index), true))?;
                s.take(&sig.params)?;
                s.stop()
            }
            Instruction::CallRef(index) => {
                let signature = m
                    .types
                    .get(*index as usize)
                    .ok_or_else(|| Error::invalid("unknown call_ref type"))?;
                if !matches!(
                    m.definitions.get(*index as usize).map(|d| &d.kind),
                    Some(gc_types::Kind::Function(_))
                ) {
                    return Err(Error::invalid("call_ref requires a function type"));
                }
                s.expect(Type::Ref(i64::from(*index), true))?;
                s.take(&signature.params)?;
                s.extend(&signature.results);
            }
            Instruction::Indirect(index, table) => {
                let sig = m
                    .types
                    .get(*index as usize)
                    .ok_or_else(|| Error::invalid("unknown indirect call type"))?;
                if !table_types(m)
                    .get(*table as usize)
                    .is_some_and(|t| gc_types::subtype(m, *t, Type::Func))
                {
                    return Err(Error::invalid("call_indirect requires a function table"));
                }
                s.expect(Type::I32)?;
                s.take(&sig.params)?;
                s.extend(&sig.results)
            }
            Instruction::Branch(depth) => {
                s.take(&label(labels, *depth)?)?;
                s.stop()
            }
            Instruction::BranchNull(op, depth) => {
                let value = s.pop()?.unwrap_or(Type::Ref(-15, true));
                let Some((_heap, _)) = value.reference() else {
                    return Err(Error::invalid("null branch requires a reference"));
                };
                let non_null = value.with_nullability(false);
                let types = label(labels, *depth)?;
                if *op == 0xd5 {
                    s.take(&types)?;
                    s.extend(types);
                    s.push(non_null)
                } else if types.is_empty() {
                    // A zero-arity branch discards the non-null reference at its destination.
                } else {
                    let Some((last, prefix)) = types.split_last() else {
                        return Err(Error::invalid(
                            "br_on_non_null label requires a reference result",
                        ));
                    };
                    if !gc_types::subtype(m, non_null, *last) {
                        return Err(Error::invalid("br_on_non_null branch type mismatch"));
                    }
                    s.take(prefix)?;
                    s.extend(prefix);
                }
            }
            Instruction::BranchCast(op, depth, source, target)
            | Instruction::BranchDescriptor(op, depth, source, target) => {
                if *op >= 37 {
                    s.expect(super::gc::descriptor_type(m, *target)?)?;
                }
                gc_types::check_type(m, *source)?;
                gc_types::check_type(m, *target)?;
                let custom_descriptors = m
                    .definitions
                    .iter()
                    .any(|d| d.descriptor.is_some() || d.describes.is_some());
                if if custom_descriptors {
                    !super::gc::same_hierarchy(m, *source, *target)?
                } else {
                    !gc_types::subtype(m, *target, *source)
                } {
                    return Err(Error::invalid("br_on_cast source and target type mismatch"));
                }
                s.expect(*source)?;
                let (_sh, sn) = source
                    .reference()
                    .ok_or_else(|| Error::invalid("br_on_cast source must be a reference"))?;
                let (_, tn) = target
                    .reference()
                    .ok_or_else(|| Error::invalid("br_on_cast target must be a reference"))?;
                let failure = source.with_nullability(sn && !tn);
                let (branch, fall) = if matches!(*op, 24 | 37) {
                    (*target, failure)
                } else {
                    (failure, *target)
                };
                let types = label(labels, *depth)?;
                let Some((last, prefix)) = types.split_last() else {
                    return Err(Error::invalid(
                        "br_on_cast label requires a reference result",
                    ));
                };
                if !gc_types::subtype(m, branch, *last) {
                    return Err(Error::invalid("br_on_cast branch type mismatch"));
                }
                s.take(prefix)?;
                s.extend(prefix);
                s.push(fall);
            }
            Instruction::BranchIf(depth) => {
                s.expect(Type::I32)?;
                let types = label(labels, *depth)?;
                s.take(&types)?;
                s.extend(types)
            }
            Instruction::BranchTable(depths, default) => {
                s.expect(Type::I32)?;
                let types = label(labels, *default)?;
                s.branch(&types)?;
                for d in depths {
                    if label(labels, *d)? != types {
                        return Err(Error::invalid("br_table label types differ"));
                    }
                }
                s.stop()
            }
            Instruction::Throw(index) => {
                let sig = super::exceptions::signature(m, *index)?;
                s.take(&sig.params)?;
                s.stop()
            }
            Instruction::ThrowRef => {
                s.expect(Type::Ref(-23, true))?;
                s.stop()
            }
            Instruction::Rethrow(depth) => {
                let target = labels
                    .len()
                    .checked_sub(*depth as usize + 1)
                    .ok_or_else(|| Error::invalid("invalid rethrow depth"))?;
                if !s.caught.contains(&target) {
                    return Err(Error::invalid("rethrow target must be a catch"));
                }
                s.stop()
            }
            Instruction::Delegate(sig, body, depth) => {
                if *depth as usize >= labels.len() {
                    return Err(Error::invalid("invalid delegate depth"));
                }
                sequence(
                    m,
                    &[Instruction::Block(2, sig.clone(), body.clone(), vec![])],
                    locals,
                    labels,
                    returns,
                    s,
                )?;
            }
            Instruction::Try(sig, body, handlers) => {
                s.take(&sig.params)?;
                labels.push(sig.results.clone());
                let mut inner = Stack {
                    module: m,
                    initialized: s.initialized.clone(),
                    caught: s.caught.clone(),
                    values: sig.params.clone(),
                    producers: vec!["block parameter"; sig.params.len()],
                    current: "expression",
                    last: "expression",
                    unreachable: false,
                };
                sequence(m, body, locals, labels, returns, &mut inner)?;
                inner.finish(&sig.results)?;
                for (tag, code) in handlers {
                    let values = match tag {
                        Some(index) => super::exceptions::signature(m, *index)?.params.clone(),
                        None => vec![],
                    };
                    let mut inner = Stack {
                        module: m,
                        initialized: s.initialized.clone(),
                        caught: s.caught.clone(),
                        producers: vec!["catch parameter"; values.len()],
                        current: "expression",
                        last: "expression",
                        values,
                        unreachable: false,
                    };
                    inner.caught.push(labels.len() - 1);
                    sequence(m, code, locals, labels, returns, &mut inner)?;
                    inner.finish(&sig.results)?;
                }
                labels.pop();
                s.extend(&sig.results);
            }
            Instruction::TryTable(sig, handlers, body) => {
                for arm in handlers {
                    let mut values = match arm.tag {
                        Some(index) => super::exceptions::signature(m, index)?.params.clone(),
                        None => vec![],
                    };
                    if arm.reference {
                        values.push(Type::Ref(-23, false))
                    }
                    let expected = label(labels, arm.depth)?;
                    if values.len() != expected.len()
                        || !values
                            .iter()
                            .zip(expected)
                            .all(|(v, t)| gc_types::subtype(m, *v, t))
                    {
                        return Err(Error::invalid("try_table catch branch type mismatch"));
                    }
                }
                sequence(
                    m,
                    &[Instruction::Block(2, sig.clone(), body.clone(), vec![])],
                    locals,
                    labels,
                    returns,
                    s,
                )?;
            }
            Instruction::Block(op, sig, body, other) => {
                if *op == 4 {
                    s.expect(Type::I32)?
                }
                s.take(&sig.params)?;
                labels.push(if *op == 3 {
                    sig.params.clone()
                } else {
                    sig.results.clone()
                });
                let mut inner = Stack {
                    module: m,
                    initialized: s.initialized.clone(),
                    caught: s.caught.clone(),
                    values: sig.params.clone(),
                    producers: vec!["block parameter"; sig.params.len()],
                    current: "expression",
                    last: "expression",
                    unreachable: false,
                };
                sequence(m, body, locals, labels, returns, &mut inner)?;
                inner.finish(&sig.results)?;
                if *op == 4 {
                    let mut inner = Stack {
                        module: m,
                        initialized: s.initialized.clone(),
                        caught: s.caught.clone(),
                        values: sig.params.clone(),
                        producers: vec!["block parameter"; sig.params.len()],
                        current: "expression",
                        last: "expression",
                        unreachable: false,
                    };
                    sequence(m, other, locals, labels, returns, &mut inner)?;
                    inner.finish(&sig.results)?
                }
                labels.pop();
                s.extend(&sig.results)
            }
            Instruction::Memory(op, align, index, _) => {
                let address = super::address::ty(m, *index)?;
                let (ty, size, store) =
                    memory_type(*op).ok_or_else(|| Error::invalid("unknown memory instruction"))?;
                if *align > size.trailing_zeros() {
                    return Err(Error::invalid("memory alignment exceeds natural alignment"));
                }
                if store {
                    s.expect(ty)?
                }
                s.expect(address)?;
                if !store {
                    s.push(ty)
                }
            }
            Instruction::MemoryIndex(op, index) => {
                if *index as usize >= memory_count(m) {
                    return Err(Error::invalid("unknown memory index"));
                }
                let address = super::address::ty(m, *index)?;
                if *op == 0x40 {
                    s.expect(address)?
                }
                s.push(address)
            }
            Instruction::Table(op, index) => {
                let ty = *table_types(m)
                    .get(*index as usize)
                    .ok_or_else(|| Error::invalid("unknown table index"))?;
                if *op == 0x26 {
                    s.expect(ty)?
                }
                s.expect(Type::I32)?;
                if *op == 0x25 {
                    s.push(ty)
                }
            }
            Instruction::Select(ty) => {
                s.expect(Type::I32)?;
                s.expect(*ty)?;
                s.expect(*ty)?;
                s.push(*ty);
            }
            Instruction::GcAtomic(op, index, slot) => {
                let (params, results) = super::gc::atomic::signature(m, *op, *index, *slot)?;
                s.take(&params)?;
                s.extend(results);
            }
            Instruction::Atomic(op, index, _) => {
                let (params, results) = super::atomic::signature(m, *op, *index)?;
                s.take(&params)?;
                s.extend(results);
            }
            Instruction::Simd(op, args) => {
                let (params, results) = super::simd::signature(m, *op, args)?;
                s.take(&params)?;
                s.extend(results);
            }
            Instruction::Gc(op, args) => {
                let input = s.values.last().copied();
                let (params, mut results) = super::gc::signature(m, *op, args)?;
                for (index, ty) in params.iter().enumerate().rev() {
                    let found = s.values.last().copied();
                    s.expect(*ty).map_err(|mut error| {
                        error.message = if matches!(op, 20..=23) {
                            format!("{} operand of type {} has to be in the same reference type hierarchy: {}",super::diagnostics::gc_opcode(*op),found.map(|ty:Type|ty.name()).unwrap_or("bottom"),error.message)
                        } else {
                            format!(
                                "{}[{index}] {}",
                                super::diagnostics::gc_opcode(*op),
                                error.message
                            )
                        };
                        error
                    })?;
                }
                if *op == 34 {
                    if let Some(Type::Exact(heap, _)) = input
                        && heap == args[0]
                    {
                        results[0] = results[0].with_nullability(false);
                        if let Some((heap, _)) = results[0].reference() {
                            results[0] = Type::Exact(heap, false)
                        }
                    }
                }
                if matches!(op, 20..=23)
                    && args
                        .first()
                        .is_some_and(|heap| matches!(heap, -32 | -31 | -26))
                {
                    return Err(Error::invalid(
                        "string view types cannot be used as cast targets",
                    ));
                }
                s.extend(results);
            }
            Instruction::Simple(op) => simple(*op, returns, s)?,
            Instruction::Bulk(sub, args) => bulk(m, *sub, args, s)?,
        }
    }
    Ok(())
}

/** Scalar opcodes have fixed input/output types; no JS coercions enter Wasm execution. */
pub(super) fn numeric_type(op: u8) -> Option<(Type, usize, Type)> {
    use Type::*;
    Some(match op {
        0x45 => (I32, 1, I32),
        0x46..=0x4f => (I32, 2, I32),
        0x50 => (I64, 1, I32),
        0x51..=0x5a => (I64, 2, I32),
        0x5b..=0x60 => (F32, 2, I32),
        0x61..=0x66 => (F64, 2, I32),
        0x67..=0x69 => (I32, 1, I32),
        0x6a..=0x78 => (I32, 2, I32),
        0x79..=0x7b => (I64, 1, I64),
        0x7c..=0x8a => (I64, 2, I64),
        0x8b..=0x91 => (F32, 1, F32),
        0x92..=0x98 => (F32, 2, F32),
        0x99..=0x9f => (F64, 1, F64),
        0xa0..=0xa6 => (F64, 2, F64),
        0xa7 => (I64, 1, I32),
        0xa8..=0xa9 => (F32, 1, I32),
        0xaa..=0xab => (F64, 1, I32),
        0xac..=0xad => (I32, 1, I64),
        0xae..=0xaf => (F32, 1, I64),
        0xb0..=0xb1 => (F64, 1, I64),
        0xb2..=0xb3 => (I32, 1, F32),
        0xb4..=0xb5 => (I64, 1, F32),
        0xb6 => (F64, 1, F32),
        0xb7..=0xb8 => (I32, 1, F64),
        0xb9..=0xba => (I64, 1, F64),
        0xbb => (F32, 1, F64),
        0xbc => (F32, 1, I32),
        0xbd => (F64, 1, I64),
        0xbe => (I32, 1, F32),
        0xbf => (I64, 1, F64),
        0xc0..=0xc1 => (I32, 1, I32),
        0xc2..=0xc4 => (I64, 1, I64),
        _ => return None,
    })
}
/** Memory opcode widths also constrain encoded alignment and load/store value types. */
pub(super) fn memory_type(op: u8) -> Option<(Type, u32, bool)> {
    use Type::*;
    let t = match op {
        0x28 | 0x2c..=0x2f | 0x36 | 0x3a..=0x3b => I32,
        0x29 | 0x30..=0x35 | 0x37 | 0x3c..=0x3e => I64,
        0x2a | 0x38 => F32,
        0x2b | 0x39 => F64,
        _ => return None,
    };
    let size = match op {
        0x2c | 0x2d | 0x30 | 0x31 | 0x3a | 0x3c => 1,
        0x2e | 0x2f | 0x32 | 0x33 | 0x3b | 0x3d => 2,
        0x28 | 0x2a | 0x34 | 0x35 | 0x36 | 0x38 | 0x3e => 4,
        _ => 8,
    };
    Some((t, size, op >= 0x36))
}
/** Non-immediate stack instructions retain unreachable polymorphism. */
fn simple(op: u8, returns: &[Type], s: &mut Stack) -> Result<(), Error> {
    match op {
        0 => s.stop(),
        1 => {}
        0x0f => {
            s.take(returns)?;
            s.stop()
        }
        0x1a => {
            s.pop()?;
        }
        0x1b => {
            s.expect(Type::I32)?;
            let b = s.pop()?;
            let a = s.pop()?;
            if a.is_some() && b.is_some() && a != b {
                return Err(Error::invalid("select operand types differ"));
            }
            let t = a.or(b).unwrap_or(Type::I32);
            if t.reference().is_some() {
                return Err(Error::invalid("reference select requires an explicit type"));
            }
            s.push(t)
        }
        0xd1 => {
            if s.pop()?.is_some_and(|t| t.reference().is_none()) {
                return Err(Error::invalid("ref.is_null requires a reference"));
            }
            s.push(Type::I32)
        }
        0xd3 => {
            let mut shared = None;
            for _ in 0..2 {
                if let Some(ty) = s.pop()? {
                    let flag = gc_types::is_shared(s.module, ty);
                    let expected = if flag {
                        Type::Shared(-19, true)
                    } else {
                        Type::Ref(-19, true)
                    };
                    if !gc_types::subtype(s.module, ty, expected) {
                        return Err(Error::invalid("ref.eq requires eqref operands"));
                    }
                    if shared.is_some_and(|prior| prior != flag) {
                        return Err(Error::invalid(
                            "ref.eq: sharedness of both operands must match",
                        ));
                    }
                    shared = Some(flag);
                }
            }
            s.push(Type::I32)
        }
        0xd4 => {
            let Some(ty) = s.pop()? else { return Ok(()) };
            let Some((_heap, _)) = ty.reference() else {
                return Err(Error::invalid("ref.as_non_null requires a reference"));
            };
            s.push(ty.with_nullability(false));
        }
        _ => {
            let (input, count, output) =
                numeric_type(op).ok_or_else(|| Error::invalid("unknown numeric opcode"))?;
            for _ in 0..count {
                s.expect(input)?
            }
            s.push(output)
        }
    }
    Ok(())
}
/** Bulk instructions check every referenced segment and index, not only their operand count. */
fn bulk(m: &Module, sub: u32, args: &[u32], s: &mut Stack) -> Result<(), Error> {
    if (19..=22).contains(&sub) {
        for _ in 0..if sub <= 20 { 4 } else { 2 } {
            s.expect(Type::I64)?
        }
        s.extend([Type::I64, Type::I64]);
        return Ok(());
    }
    if sub >= 12 {
        let (params, results) = super::segments::table_signature(m, sub, args)?;
        s.take(&params)?;
        s.extend(results);
        return Ok(());
    }
    if matches!(sub, 8 | 9) && m.data_count.is_none() {
        return Err(Error::invalid(
            "memory initialization requires a data count section",
        ));
    }
    if sub <= 7 {
        let input = if sub % 4 < 2 { Type::F32 } else { Type::F64 };
        s.expect(input)?;
        s.push(if sub < 4 { Type::I32 } else { Type::I64 });
        return Ok(());
    }
    let memory = |i: u32| {
        if i as usize >= memory_count(m) {
            Err(Error::invalid("unknown bulk memory"))
        } else {
            Ok(())
        }
    };
    match sub {
        8 => {
            if args[0] as usize >= m.data.len() {
                return Err(Error::invalid("unknown data segment"));
            }
            memory(args[1])?;
        }
        9 => {
            if args[0] as usize >= m.data.len() {
                return Err(Error::invalid("unknown data segment"));
            }
            return Ok(());
        }
        10 => {
            memory(args[0])?;
            memory(args[1])?;
        }
        11 => memory(args[0])?,
        _ => return Err(Error::unsupported("WebAssembly bulk table operations")),
    }
    let target = super::address::ty(m, args[if sub == 8 { 1 } else { 0 }])?;
    let source = if sub == 10 {
        super::address::ty(m, args[1])?
    } else {
        Type::I32
    };
    let count = if sub == 8 || sub == 10 && (target == Type::I32 || source == Type::I32) {
        Type::I32
    } else {
        target
    };
    s.expect(count)?;
    s.expect(source)?;
    s.expect(target)?;
    Ok(())
}

/** Tail calls may return covariant references, but must satisfy every result of the replaced frame. */
fn tail_results(m: &Module, sig: &Signature, returns: &[Type]) -> Result<(), Error> {
    if sig.results.len() != returns.len()
        || !sig
            .results
            .iter()
            .zip(returns)
            .all(|(a, b)| gc_types::subtype(m, *a, *b))
    {
        Err(Error::invalid("tail call result types mismatch"))
    } else {
        Ok(())
    }
}

/** Const typing uses the same checked operand contracts while forbidding mutable global dependencies. */
fn constant_expression_type(
    m: &Module,
    code: &[Instruction],
    initializer: bool,
) -> Result<Type, Error> {
    let mut stack = Stack {
        module: m,
        initialized: vec![],
        caught: vec![],
        values: vec![],
        producers: vec![],
        current: "expression",
        last: "expression",
        unreachable: false,
    };
    for instruction in code {
        stack.current = super::diagnostics::producer(instruction);
        match instruction {
            Instruction::Constant(value) => stack.push(constant_type(m, value, initializer)?),
            Instruction::Simple(op @ (0x6a..=0x6c | 0x7c..=0x7e)) => {
                let ty = if *op < 0x7c { Type::I32 } else { Type::I64 };
                stack.expect(ty)?;
                stack.expect(ty)?;
                stack.push(ty)
            }
            Instruction::Gc(op, args) => {
                let (params, results) = super::gc::signature(m, *op, args)?;
                stack.take(&params)?;
                stack.extend(results)
            }
            _ => return Err(Error::invalid("invalid constant instruction")),
        }
    }
    if stack.values.len() != 1 {
        return Err(Error::invalid(
            "constant expression must produce exactly one value",
        ));
    }
    Ok(stack.values[0])
}

/** A suspended path is retyped from original structured code, including every active block's lower operands. */
pub(super) fn suspended_stack_path(
    m: &Module,
    index: u32,
    path: &[(usize, u8)],
) -> Result<(Vec<Type>, Vec<bool>, Vec<usize>), Error> {
    let sig = m.signature(index)?;
    let body = m
        .bodies
        .get(index as usize - m.imported_functions())
        .ok_or_else(|| Error::invalid("invalid continuation body"))?;
    let mut locals = sig.params.clone();
    locals.extend(&body.locals);
    let mut stack = Stack {
        module: m,
        initialized: std::iter::repeat_n(true, sig.params.len())
            .chain(body.locals.iter().map(|ty| gc_types::defaultable(*ty)))
            .collect(),
        caught: vec![],
        values: vec![],
        producers: vec![],
        current: "expression",
        last: "expression",
        unreachable: false,
    };
    let mut labels = vec![sig.results.clone()];
    let mut code = body.code.as_slice();
    let mut prefixes: Vec<Vec<Type>> = vec![];
    let mut heights = vec![];
    if path.is_empty() || path.len() > super::constants::MAX_DEPTH {
        return Err(Error::invalid("invalid continuation source path"));
    }
    for (level, (at, branch)) in path.iter().enumerate() {
        let mut instruction = code
            .get(*at)
            .ok_or_else(|| Error::invalid("invalid continuation source index"))?;
        while let Instruction::Located(_, inner) = instruction {
            instruction = inner;
        }
        if level + 1 == path.len() {
            if *branch != 2 {
                return Err(Error::invalid("invalid continuation leaf path"));
            }
            let results = match instruction {
                Instruction::Call(target) => m.signature(*target)?.results.clone(),
                Instruction::CallRef(ty) | Instruction::Indirect(ty, _) => m
                    .types
                    .get(*ty as usize)
                    .ok_or_else(|| Error::invalid("invalid continuation call type"))?
                    .results
                    .clone(),
                _ => return Err(Error::invalid("continuation position is not a call")),
            };
            sequence(
                m,
                &code[..=*at],
                &locals,
                &mut labels,
                &sig.results,
                &mut stack,
            )?;
            if stack.unreachable {
                return Err(Error::invalid("unreachable continuation position"));
            }
            stack.take(&results)?;
        } else {
            sequence(
                m,
                &code[..*at],
                &locals,
                &mut labels,
                &sig.results,
                &mut stack,
            )?;
            let Instruction::Block(kind, signature, body, otherwise) = instruction else {
                return Err(Error::invalid(
                    "continuation ancestor is not a control scope",
                ));
            };
            if *branch > 1 || *branch == 1 && *kind != 4 {
                return Err(Error::invalid("invalid continuation branch path"));
            }
            if *kind == 4 {
                stack.expect(Type::I32)?;
            }
            stack.take(&signature.params)?;
            heights.push(prefixes.iter().map(Vec::len).sum::<usize>() + stack.values.len());
            prefixes.push(std::mem::take(&mut stack.values));
            stack.values = signature.params.clone();
            stack.producers = vec!["block parameter"; stack.values.len()];
            stack.unreachable = false;
            labels.push(if *kind == 3 {
                signature.params.clone()
            } else {
                signature.results.clone()
            });
            code = if *branch == 0 { body } else { otherwise };
        }
    }
    let mut values = prefixes.into_iter().flatten().collect::<Vec<_>>();
    values.extend(stack.values);
    Ok((values, stack.initialized, heights))
}
