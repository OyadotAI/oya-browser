/** Recursive Wasm composite types, reference subtyping and cross-module structural equivalence. */
use super::{
    binary::Reader,
    constants::{MAX_DEPTH, MAX_LOCALS},
    types::*,
};

/** Field storage distinguishes packed i8/i16 from the i32 value placed on the operand stack. */
#[derive(Clone, Debug)]
pub(super) struct Field {
    pub ty: Type,
    pub packed: Option<u8>,
    pub mutable: bool,
}
/** A recursive-group member has exactly one composite shape. */
#[derive(Clone, Debug)]
pub(super) enum Kind {
    Function(Signature),
    Struct(Vec<Field>),
    Array(Field),
}
/** Declared inheritance is distinct from references between fields in a recursive group. */
#[derive(Clone, Debug)]
pub(super) struct Definition {
    pub kind: Kind,
    pub shared: bool,
    pub parent: Option<u32>,
    pub final_type: bool,
    pub descriptor: Option<u32>,
    pub describes: Option<u32>,
}

/** Heap type encodings are signed 33-bit indices or one of the specified abstract heap types. */
pub(super) fn heap(r: &mut Reader, nullable: bool) -> Result<Type, Error> {
    if r.bytes.get(r.at) == Some(&0x65) {
        r.byte()?;
        let heap = r.signed(33)?;
        if heap == -25 {
            return Err(Error::invalid(
                "shared string type 'shared string' is not supported",
            ));
        }
        if !matches!(heap, -22..=-13) {
            return Err(Error::invalid("invalid shared heap type"));
        }
        return Ok(Type::Shared(heap, nullable));
    }
    let exact = r.bytes.get(r.at) == Some(&0x62);
    if exact {
        r.byte()?;
    }
    let heap = r.signed(33)?;
    if exact {
        if heap < 0 {
            return Err(Error::invalid("exact heap type must have a type index"));
        }
        return Ok(Type::Exact(heap, nullable));
    }
    if matches!(heap, -36 | -37) {
        return Err(Error::unsupported("WebAssembly waitqueue reference type"));
    }
    if matches!(heap, -32 | -31 | -26) {
        if nullable {
            return Err(Error::invalid("nullable string views don't exist"));
        }
        return Ok(reference(heap, false));
    }
    if heap >= MAX_LOCALS as i64 {
        return Err(Error::invalid(&format!(
            "Type index {heap} is greater than the maximum number of types"
        )));
    }
    if heap < 0 && !matches!(heap, -25..=-11) {
        return Err(Error::invalid(&format!("Unknown heap type {heap}")));
    }
    Ok(reference(heap, nullable))
}
/** Retain legacy nullable function/extern spellings as their canonical internal variants. */
pub(super) fn reference(heap: i64, nullable: bool) -> Type {
    match (heap, nullable) {
        (-16, true) => Type::Func,
        (-17, true) => Type::Extern,
        _ => Type::Ref(heap, nullable),
    }
}
/** A type-section entry is either one subtype or a complete recursive group. */
pub(super) fn read_group(m: &mut Module, r: &mut Reader) -> Result<(), Error> {
    let first = r.byte()?;
    if first == 0x4e {
        let n = r.count()?;
        for _ in 0..n {
            let prefix = r.byte()?;
            read_definition(m, r, prefix)?
        }
    } else {
        read_definition(m, r, first)?
    }
    Ok(())
}
/** Composite fields can refer forward inside their recursive group; validation resolves them later. */
fn read_definition(m: &mut Module, r: &mut Reader, first: u8) -> Result<(), Error> {
    let (mut final_type, mut parent, mut opcode) = (true, None, first);
    if matches!(first, 0x4f | 0x50) {
        final_type = first == 0x4f;
        let count = r.u32()?;
        if count > 1 {
            return Err(Error::invalid("multiple Wasm supertypes"));
        }
        if count == 1 {
            parent = Some(r.u32()?)
        }
        opcode = r.byte()?;
    }
    let shared = opcode == 0x65;
    if shared {
        opcode = r.byte()?;
    }
    let mut descriptor = None;
    let mut describes = None;
    if opcode == 0x4c {
        describes = Some(r.u32()?);
        opcode = r.byte()?;
    }
    if opcode == 0x4d {
        descriptor = Some(r.u32()?);
        opcode = r.byte()?;
    }
    if (descriptor.is_some() || describes.is_some()) && opcode != 0x5f {
        return Err(Error::invalid(
            "only struct types can have custom descriptors",
        ));
    }
    if shared && (descriptor.is_some() || describes.is_some()) {
        return Err(Error::unsupported("WebAssembly shared custom descriptors"));
    }
    let kind = match opcode {
        0x60 => Kind::Function(Signature {
            params: r.type_list()?,
            results: r.type_list()?,
        }),
        0x5f => {
            let count = r.count()?;
            let fields = (0..count)
                .map(|_| field(r))
                .collect::<Result<Vec<_>, _>>()?;
            Kind::Struct(fields)
        }
        0x5e => Kind::Array(field(r)?),
        0x5d => {
            return Err(Error::unsupported(
                "WebAssembly descriptor or continuation type",
            ));
        }
        _ => return Err(Error::invalid("unknown WebAssembly composite type")),
    };
    if shared && !matches!(kind, Kind::Struct(_) | Kind::Array(_)) {
        return Err(Error::unsupported("WebAssembly shared function type"));
    }
    m.types.push(match &kind {
        Kind::Function(sig) => sig.clone(),
        _ => Signature::default(),
    });
    m.definitions.push(Definition {
        kind,
        shared,
        parent,
        final_type,
        descriptor,
        describes,
    });
    Module::check_count(m.types.len())
}
/** Packed storage has an integer operand type and the encoded field mutability byte. */
fn field(r: &mut Reader) -> Result<Field, Error> {
    let byte = r.byte()?;
    let (ty, packed) = match byte {
        0x78 => (Type::I32, Some(8)),
        0x77 => (Type::I32, Some(16)),
        _ => {
            r.at -= 1;
            (r.ty()?, None)
        }
    };
    let mutable = r.byte()?;
    if mutable > 1 {
        return Err(Error::invalid("invalid Wasm field mutability"));
    }
    Ok(Field {
        ty,
        packed,
        mutable: mutable == 1,
    })
}
/** A referenced type must exist; abstract heap types are a closed set. */
pub(super) fn check_type(m: &Module, ty: Type) -> Result<(), Error> {
    if let Some((heap, _)) = ty.reference() {
        if heap >= 0 && heap as usize >= m.definitions.len() {
            return Err(Error::invalid("unknown referenced heap type"));
        }
        if heap < 0
            && !(matches!(heap, -32 | -31 | -26) && !ty.reference().unwrap().1)
            && !matches!(heap, -25..=-11)
        {
            return Err(Error::invalid("unknown abstract heap type"));
        }
    }
    Ok(())
}
/** All declared inheritance is acyclic, non-final and structurally valid before code is checked. */
pub(super) fn validate(m: &Module) -> Result<(), Error> {
    for (i, definition) in m.definitions.iter().enumerate() {
        if let Some(parent) = definition.parent
            && parent as usize >= m.definitions.len()
        {
            return Err(Error::invalid(&format!(
                "type {i}: invalid supertype {parent}"
            )));
        }
    }
    for (i, definition) in m.definitions.iter().enumerate() {
        if definition.shared {
            let fields = match &definition.kind {
                Kind::Struct(fields) => fields.as_slice(),
                Kind::Array(field) => std::slice::from_ref(field),
                _ => &[],
            };
            if fields
                .iter()
                .any(|f| f.ty.reference().is_some() && !is_shared(m, f.ty))
            {
                return Err(Error::invalid(
                    "shared composite contains an unshared reference field",
                ));
            }
            if definition
                .parent
                .is_some_and(|p| !m.definitions[p as usize].shared)
            {
                return Err(Error::invalid("shared subtype requires a shared supertype"));
            }
        }
        if let Some(desc) = definition.descriptor {
            if !matches!(m.definitions.get(desc as usize),Some(other)if matches!(other.kind,Kind::Struct(_))&&other.describes==Some(i as u32))
            {
                return Err(Error::invalid(
                    "custom descriptor must describe its paired struct type",
                ));
            }
        }
        if let Some(described) = definition.describes {
            if !matches!(m.definitions.get(described as usize),Some(other)if matches!(other.kind,Kind::Struct(_))&&other.descriptor==Some(i as u32))
            {
                return Err(Error::invalid(
                    "descriptor type must pair with its described struct",
                ));
            }
        }
        let mut current = Some(i as u32);
        let mut seen = std::collections::BTreeSet::new();
        while let Some(index) = current {
            if !seen.insert(index) {
                return Err(Error::invalid("cyclic Wasm subtype inheritance"));
            }
            let d = m
                .definitions
                .get(index as usize)
                .ok_or_else(|| Error::invalid(&format!("type {i}: invalid supertype {index}")))?;
            current = d.parent;
        }
        for ty in field_types(definition) {
            check_type(m, ty)?
        }
        if let Some(parent) = definition.parent {
            let base = &m.definitions[parent as usize];
            if base.final_type || !valid_subtype(m, &definition.kind, &base.kind) {
                return Err(Error::invalid(&format!(
                    "type {i}: invalid supertype {parent}"
                )));
            }
        }
    }
    Ok(())
}
/** One list of child references supports both field and function validation. */
fn field_types(definition: &Definition) -> Vec<Type> {
    match &definition.kind {
        Kind::Function(sig) => sig.params.iter().chain(&sig.results).copied().collect(),
        Kind::Struct(fields) => fields.iter().map(|f| f.ty).collect(),
        Kind::Array(field) => vec![field.ty],
    }
}
/** Mutable fields are invariant; immutable fields may narrow their reference type. */
fn field_subtype(m: &Module, a: &Field, b: &Field) -> bool {
    a.packed == b.packed
        && a.mutable == b.mutable
        && subtype(m, a.ty, b.ty)
        && (!a.mutable || subtype(m, b.ty, a.ty))
}
/** Structs keep the parent's prefix, while function parameters are contravariant. */
fn valid_subtype(m: &Module, a: &Kind, b: &Kind) -> bool {
    match (a, b) {
        (Kind::Struct(a), Kind::Struct(b)) => {
            a.len() >= b.len() && a.iter().zip(b).all(|(a, b)| field_subtype(m, a, b))
        }
        (Kind::Array(a), Kind::Array(b)) => field_subtype(m, a, b),
        (Kind::Function(a), Kind::Function(b)) => {
            a.params.len() == b.params.len()
                && a.results.len() == b.results.len()
                && a.params
                    .iter()
                    .zip(&b.params)
                    .all(|(a, b)| subtype(m, *b, *a))
                && a.results
                    .iter()
                    .zip(&b.results)
                    .all(|(a, b)| subtype(m, *a, *b))
        }
        _ => false,
    }
}
/** Non-null references can flow to nullable references, but never in the reverse direction. */
pub(super) fn subtype(m: &Module, actual: Type, expected: Type) -> bool {
    if actual == expected {
        return true;
    }
    if actual.reference().is_some()
        && expected.reference().is_some()
        && is_shared(m, actual) != is_shared(m, expected)
    {
        return false;
    }
    if let Type::Exact(expected, nullable) = expected {
        let Some((heap, actual_nullable)) = actual.reference() else {
            return false;
        };
        if actual_nullable && !nullable {
            return false;
        }
        if heap == -15 {
            return true;
        }
        return heap >= 0
            && (matches!(actual, Type::Exact(..))
                || m.definitions
                    .get(heap as usize)
                    .is_some_and(|d| d.final_type))
            && equivalent(m, heap as u32, m, expected as u32);
    }
    let (Some((a, an)), Some((b, bn))) = (actual.reference(), expected.reference()) else {
        return false;
    };
    if an && !bn {
        return false;
    }
    heap_subtype(m, a, b)
}
/** Abstract reference hierarchies and declared concrete supertypes share one lookup. */
fn heap_subtype(m: &Module, a: i64, b: i64) -> bool {
    if a == b {
        return true;
    }
    if a >= 0 && b >= 0 {
        let mut current = Some(a as u32);
        for _ in 0..=m.definitions.len() {
            let Some(index) = current else { return false };
            if equivalent(m, index, m, b as u32) {
                return true;
            }
            current = m.definitions.get(index as usize).and_then(|d| d.parent)
        }
        return false;
    }
    if a == -15 {
        return b >= 0
            && m.definitions
                .get(b as usize)
                .is_some_and(|d| !matches!(d.kind, Kind::Function(_)))
            || matches!(b, -25 | -22..=-18);
    }
    if a == -13 {
        return b == -16
            || b >= 0
                && m.definitions
                    .get(b as usize)
                    .is_some_and(|d| matches!(d.kind, Kind::Function(_)));
    }
    if a == -12 {
        return b == -23;
    }
    if a == -14 {
        return b == -17;
    }
    let abstract_a = if a >= 0 {
        match m.definitions.get(a as usize).map(|d| &d.kind) {
            Some(Kind::Function(_)) => -16,
            Some(Kind::Struct(_)) => -21,
            Some(Kind::Array(_)) => -22,
            None => return false,
        }
    } else {
        a
    };
    abstract_a == b || matches!((abstract_a, b), (-22..=-20, -19 | -18) | (-19 | -25, -18))
}
/** Cross-module comparisons follow the actual recursive graphs, not coincidentally equal indices. */
pub(super) fn equivalent(a: &Module, ai: u32, b: &Module, bi: u32) -> bool {
    equal_definition(a, ai, b, bi, &mut Vec::new(), 0)
}
/** Coinductive comparison terminates on a previously visited pair and bounds native recursion. */
fn equal_definition(
    a: &Module,
    ai: u32,
    b: &Module,
    bi: u32,
    seen: &mut Vec<(u32, u32)>,
    depth: usize,
) -> bool {
    if seen.contains(&(ai, bi)) {
        return true;
    }
    if depth > MAX_DEPTH {
        return false;
    }
    let (Some(ad), Some(bd)) = (
        a.definitions.get(ai as usize),
        b.definitions.get(bi as usize),
    ) else {
        return false;
    };
    seen.push((ai, bi));
    if ad.final_type != bd.final_type || ad.shared != bd.shared {
        return false;
    }
    let parents = match (ad.parent, bd.parent) {
        (None, None) => true,
        (Some(ap), Some(bp)) => equal_definition(a, ap, b, bp, seen, depth + 1),
        _ => false,
    };
    let descriptors = equal_optional(a, ad.descriptor, b, bd.descriptor, seen, depth + 1)
        && equal_optional(a, ad.describes, b, bd.describes, seen, depth + 1);
    parents
        && descriptors
        && match (&ad.kind, &bd.kind) {
            (Kind::Function(x), Kind::Function(y)) => {
                equal_list(a, &x.params, b, &y.params, seen, depth + 1)
                    && equal_list(a, &x.results, b, &y.results, seen, depth + 1)
            }
            (Kind::Struct(x), Kind::Struct(y)) => {
                x.len() == y.len()
                    && x.iter()
                        .zip(y)
                        .all(|(x, y)| equal_field(a, x, b, y, seen, depth + 1))
            }
            (Kind::Array(x), Kind::Array(y)) => equal_field(a, x, b, y, seen, depth + 1),
            _ => false,
        }
}
/** Composite shape includes packed storage and mutability, not just its operand types. */
fn equal_field(
    a: &Module,
    x: &Field,
    b: &Module,
    y: &Field,
    seen: &mut Vec<(u32, u32)>,
    depth: usize,
) -> bool {
    x.packed == y.packed && x.mutable == y.mutable && equal_type(a, x.ty, b, y.ty, seen, depth)
}
/** Function arity and order remain part of canonical type identity. */
fn equal_list(
    a: &Module,
    x: &[Type],
    b: &Module,
    y: &[Type],
    seen: &mut Vec<(u32, u32)>,
    depth: usize,
) -> bool {
    x.len() == y.len()
        && x.iter()
            .zip(y)
            .all(|(x, y)| equal_type(a, *x, b, *y, seen, depth))
}
/** Reference indices are interpreted only in their own module's validated type context. */
fn equal_type(
    a: &Module,
    x: Type,
    b: &Module,
    y: Type,
    seen: &mut Vec<(u32, u32)>,
    depth: usize,
) -> bool {
    if is_shared(a, x) != is_shared(b, y)
        || matches!(x, Type::Exact(..)) != matches!(y, Type::Exact(..))
    {
        return false;
    }
    match (x.reference(), y.reference()) {
        (Some((xi, xn)), Some((yi, yn))) if xi >= 0 && yi >= 0 => {
            xn == yn && equal_definition(a, xi as u32, b, yi as u32, seen, depth + 1)
        }
        _ => x == y,
    }
}
/** Initializers need a default only for numeric/vector and nullable reference fields. */
pub(super) fn defaultable(ty: Type) -> bool {
    ty.reference().is_none_or(|(_, nullable)| nullable)
}
/** A checked struct declaration is reused by static and runtime field access. */
pub(super) fn fields(m: &Module, index: u32) -> Result<&[Field], Error> {
    match m.definitions.get(index as usize).map(|d| &d.kind) {
        Some(Kind::Struct(fields)) => Ok(fields),
        _ => Err(Error::invalid("expected a Wasm struct type")),
    }
}
/** A checked array declaration supplies exactly one element-storage rule. */
pub(super) fn element(m: &Module, index: u32) -> Result<&Field, Error> {
    match m.definitions.get(index as usize).map(|d| &d.kind) {
        Some(Kind::Array(field)) => Ok(field),
        _ => Err(Error::invalid("expected a Wasm array type")),
    }
}

/** Descriptor graph edges take part in canonical identity, including mutually recursive pair links. */
fn equal_optional(
    a: &Module,
    ai: Option<u32>,
    b: &Module,
    bi: Option<u32>,
    seen: &mut Vec<(u32, u32)>,
    depth: usize,
) -> bool {
    match (ai, bi) {
        (None, None) => true,
        (Some(x), Some(y)) => equal_definition(a, x, b, y, seen, depth),
        _ => false,
    }
}

/** Wrapper contracts use the same exactness-aware cross-module canonical comparison as function fields. */
pub(super) fn equivalent_type(a: &Module, x: Type, b: &Module, y: Type) -> bool {
    equal_type(a, x, b, y, &mut Vec::new(), 0)
}

/** Abstract shared qualifiers and indexed composite sharing obey the same invariant. */
pub(super) fn is_shared(m: &Module, ty: Type) -> bool {
    matches!(ty, Type::Shared(..))
        || ty.reference().is_some_and(|(heap, _)| {
            heap >= 0 && m.definitions.get(heap as usize).is_some_and(|d| d.shared)
        })
}
