/** Inert, revalidated WebAssembly module data; none of these records cross V9's state boundary. */
use super::constants::*;

/** The supported scalar and reference value types. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Type {
    I32,
    I64,
    F32,
    F64,
    V128,
    Func,
    Extern,
    Ref(i64, bool),
    Exact(i64, bool),
    Shared(i64, bool),
}
impl Type {
    /** Decode the binary value-type byte without treating unknown proposals as valid values. */
    pub fn byte(byte: u8) -> Result<Self, Error> {
        Ok(match byte {
            0x7f => Self::I32,
            0x7e => Self::I64,
            0x7d => Self::F32,
            0x7c => Self::F64,
            0x7b => Self::V128,
            0x70 => Self::Func,
            0x6f => Self::Extern,
            0x60 | 0x61 | 0x66 => Self::Ref(i64::from(byte) - 128, false),
            0x67..=0x6e | 0x71..=0x75 => Self::Ref(i64::from(byte) - 128, true),
            _ => return Err(Error::unsupported("WebAssembly value type")),
        })
    }
    /** The JS descriptor spelling. */
    pub fn name(self) -> &'static str {
        match self {
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::F32 => "f32",
            Self::F64 => "f64",
            Self::V128 => "v128",
            Self::Func => "funcref",
            Self::Extern => "externref",
            Self::Ref(-32, _) => "stringview_wtf16",
            Self::Ref(-31, _) => "stringview_iter",
            Self::Ref(-26, _) => "stringview_wtf8",
            Self::Ref(-25, _) => "string",
            Self::Shared(..) => "shared reference",
            Self::Ref(_, _) | Self::Exact(..) => "reference",
        }
    }
    /** Views, vectors and engine exception/continuation references have no direct JavaScript value form. */
    pub fn js_compatible(self) -> bool {
        !matches!(
            self,
            Self::V128 | Self::Ref(-32 | -31 | -26 | -24 | -23 | -12 | -11, _)
        )
    }
    /** Changing nullability retains the exact heap-type proof instead of silently widening it. */
    pub fn with_nullability(self, nullable: bool) -> Self {
        match self {
            Self::Exact(heap, _) => Self::Exact(heap, nullable),
            Self::Shared(heap, _) => Self::Shared(heap, nullable),
            Self::Ref(heap, _) => Self::Ref(heap, nullable),
            Self::Func => Self::Ref(-16, nullable),
            Self::Extern => Self::Ref(-17, nullable),
            other => other,
        }
    }
    /** Indexed reference diagnostics include nullability and exactness from the actual stack contract. */
    pub fn display(self) -> String {
        match self {
            Self::Exact(heap, nullable) => {
                format!("(ref {}exact {heap})", if nullable { "null " } else { "" })
            }
            Self::Ref(heap, nullable) if heap >= 0 => {
                format!("(ref {}{heap})", if nullable { "null " } else { "" })
            }
            _ => self.name().into(),
        }
    }
    /** Every reference exposes its abstract heap type or module-local concrete index. */
    pub fn reference(self) -> Option<(i64, bool)> {
        match self {
            Self::Func => Some((-16, true)),
            Self::Extern => Some((-17, true)),
            Self::Ref(heap, nullable)
            | Self::Exact(heap, nullable)
            | Self::Shared(heap, nullable) => Some((heap, nullable)),
            _ => None,
        }
    }
}
/** Invalid modules and unsupported capabilities must remain distinguishable. */
#[derive(Clone, Debug)]
pub(super) struct Error {
    pub message: String,
    pub unsupported: bool,
    pub offset: Option<usize>,
    pub function: Option<(u32, Option<String>)>,
}
impl Error {
    /** The innermost parser or instruction owns the first precise byte offset. */
    pub fn at(mut self, offset: usize) -> Self {
        if self.offset.is_none() {
            self.offset = Some(offset)
        }
        self
    }
    /** Body metadata provides context even when decoding fails before the name custom section. */
    pub fn in_function(mut self, index: u32, name: Option<String>) -> Self {
        if self.function.is_none() {
            self.function = Some((index, name))
        }
        self
    }
    /** Context and offsets describe the real failure; API names are added at their native boundary. */
    pub fn render(&self) -> String {
        let mut message = match &self.function {
            Some((index, name)) => format!(
                "Compiling function #{index}{} failed: {}",
                name.as_ref()
                    .map(|n| {
                        let label = n.chars().take(80).collect::<String>();
                        format!(":{label:?}")
                    })
                    .unwrap_or_default(),
                self.message
            ),
            None => self.message.clone(),
        };
        if let Some(offset) = self.offset {
            message.push_str(&format!(" @+{offset}"))
        }
        message
    }
    /** A binary or type-validation failure. */
    pub fn invalid(message: &str) -> Self {
        Self {
            message: message.into(),
            unsupported: false,
            offset: None,
            function: None,
        }
    }
    /** A recognized extension that this owned interpreter does not implement yet. */
    pub fn unsupported(message: &str) -> Self {
        Self {
            message: message.into(),
            unsupported: true,
            offset: None,
            function: None,
        }
    }
}
/** A function's exact parameter and result types. */
#[derive(Clone, Debug, Default)]
pub(super) struct Signature {
    pub params: Vec<Type>,
    pub results: Vec<Type>,
}
/** Declared minimum and optional maximum, in pages or table elements. */
#[derive(Clone, Copy, Debug)]
pub(super) struct Limits {
    pub min: u32,
    pub max: Option<u32>,
    pub shared: bool,
    pub address64: bool,
}
/** Each import has one concrete external type. */
#[derive(Clone, Debug)]
pub(super) enum External {
    Function(u32),
    Table(Type, Limits),
    Memory(Limits),
    Global(Type, bool),
    Tag(u32),
}
/** The import name and its type. */
#[derive(Clone, Debug)]
pub(super) struct Import {
    pub module: String,
    pub name: String,
    pub kind: External,
}
/** An exported name and the corresponding index space. */
#[derive(Clone, Debug)]
pub(super) struct Export {
    pub name: String,
    pub kind: u8,
    pub index: u32,
}
/** A typed constant expression, possibly reading an imported immutable global. */
#[derive(Clone, Debug)]
pub(super) enum Constant {
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    V128([u8; 16]),
    Global(u32),
    Null(Type),
    Function(u32),
    Expression(Vec<Instruction>),
}
/** Structured instructions keep runtime branches out of the Rust parser stack. */
#[derive(Clone, Debug)]
pub(super) enum Instruction {
    Located(usize, Box<Instruction>),
    Simple(u8),
    Select(Type),
    Simd(u32, Vec<u32>),
    Atomic(u32, u32, u64),
    GcAtomic(u32, u32, u32),
    Gc(u32, Vec<i64>),
    CallRef(u32),
    TailCall(u32),
    TailIndirect(u32, u32),
    TailRef(u32),
    Throw(u32),
    Rethrow(u32),
    ThrowRef,
    Delegate(Signature, Vec<Instruction>, u32),
    Try(
        Signature,
        Vec<Instruction>,
        Vec<(Option<u32>, Vec<Instruction>)>,
    ),
    TryTable(Signature, Vec<Catch>, Vec<Instruction>),
    BranchNull(u8, u32),
    BranchCast(u32, u32, Type, Type),
    BranchDescriptor(u32, u32, Type, Type),
    Block(u8, Signature, Vec<Instruction>, Vec<Instruction>),
    Branch(u32),
    BranchIf(u32),
    BranchTable(Vec<u32>, u32),
    Call(u32),
    Indirect(u32, u32),
    Local(u8, u32),
    Global(u8, u32),
    Table(u8, u32),
    Memory(u8, u32, u32, u64),
    MemoryIndex(u8, u32),
    Constant(Constant),
    Bulk(u32, Vec<u32>),
}
/** A try_table arm targets an outer label and optionally carries the original exception reference. */
#[derive(Clone, Debug)]
pub(super) struct Catch {
    pub tag: Option<u32>,
    pub reference: bool,
    pub depth: u32,
}
/** A validated body's locals and structured instruction sequence. */
#[derive(Clone, Debug)]
pub(super) struct Body {
    pub locals: Vec<Type>,
    pub code: Vec<Instruction>,
    pub end: usize,
}
/** Active data initialized once at instantiation; passive data is retained for bulk operations. */
#[derive(Clone, Debug)]
pub(super) struct Data {
    pub memory: Option<u32>,
    pub offset: Option<Constant>,
    pub bytes: Vec<u8>,
}
/** An active or passive table initializer. */
#[derive(Clone, Debug)]
pub(super) struct Element {
    pub passive: bool,
    pub ty: Type,
    pub table: Option<u32>,
    pub offset: Option<Constant>,
    pub values: Vec<Constant>,
}
/** Reconstructed from immutable private bytes for every native use. */
#[derive(Clone, Debug, Default)]
pub(super) struct Module {
    pub types: Vec<Signature>,
    pub names: std::collections::BTreeMap<u32, String>,
    pub definitions: Vec<super::gc_types::Definition>,
    pub imports: Vec<Import>,
    pub functions: Vec<u32>,
    pub tables: Vec<(Type, Limits, Option<Constant>)>,
    pub memories: Vec<Limits>,
    pub tags: Vec<u32>,
    pub globals: Vec<(Type, bool, Constant)>,
    pub exports: Vec<Export>,
    pub start: Option<u32>,
    pub bodies: Vec<Body>,
    pub data: Vec<Data>,
    pub elements: Vec<Element>,
    pub custom: Vec<(String, Vec<u8>)>,
    pub data_count: Option<u32>,
}
impl Module {
    /** Function index lookup stays separate from the type table's struct and array members. */
    pub fn function_type(&self, index: u32) -> Result<u32, Error> {
        let imported = self.imported_functions();
        if (index as usize) < imported {
            self.imports
                .iter()
                .filter_map(|i| match i.kind {
                    External::Function(t) => Some(t),
                    _ => None,
                })
                .nth(index as usize)
        } else {
            self.functions.get(index as usize - imported).copied()
        }
        .ok_or_else(|| Error::invalid("unknown function index"))
    }
    /** Function index space begins with imported functions. */
    pub fn function_types(&self) -> Vec<u32> {
        self.imports
            .iter()
            .filter_map(|i| match i.kind {
                External::Function(t) => Some(t),
                _ => None,
            })
            .chain(self.functions.iter().copied())
            .collect()
    }
    /** A function index's checked signature. */
    pub fn signature(&self, index: u32) -> Result<&Signature, Error> {
        let imported = self.imported_functions();
        let ty = if (index as usize) < imported {
            self.imports
                .iter()
                .filter_map(|i| match i.kind {
                    External::Function(t) => Some(t),
                    _ => None,
                })
                .nth(index as usize)
        } else {
            self.functions.get(index as usize - imported).copied()
        };
        ty.and_then(|i| self.types.get(i as usize))
            .ok_or_else(|| Error::invalid("unknown function index"))
    }
    /** Imported functions have no local body. */
    pub fn imported_functions(&self) -> usize {
        self.imports
            .iter()
            .filter(|i| matches!(i.kind, External::Function(_)))
            .count()
    }
    /** Reject allocations before vectors can exceed the existing VM's retained-state limits. */
    pub fn check_count(count: usize) -> Result<(), Error> {
        if count > MAX_LOCALS {
            Err(Error::invalid(
                "WebAssembly declarations exceed the V9 binding limit",
            ))
        } else {
            Ok(())
        }
    }
}
