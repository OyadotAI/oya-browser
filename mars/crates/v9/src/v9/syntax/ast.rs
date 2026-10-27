/** The complete classic-script and module syntax tree V9 compiles; it carries positions, never native handles. */
use crate::protocol::SourceLocation;
use crate::v9::value::JsString;
use std::rc::Rc;

/** Identifier names are shared, since the same name appears many times in one bundle. They are
atomic so compiled code, which holds them, can be cached across V9's per-request threads. */
pub(in crate::v9) type Name = std::sync::Arc<str>;

/** A compact source position; byte offsets are page-relative so code identities never collide. */
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(in crate::v9) struct Pos {
    /// Page-relative byte offset of the first token.
    pub offset: u32,
    /// One-based line inside its own script.
    pub line: u32,
    /// One-based UTF-16 column inside its line.
    pub column: u32,
}

impl Pos {
    /** Diagnostics keep the shared protocol location shape. */
    pub fn location(self) -> SourceLocation {
        SourceLocation {
            byte_offset: self.offset as usize,
            line: self.line as usize,
            column: self.column as usize,
            script_index: None,
        }
    }
    /** Token locations convert without losing the page-relative offset. */
    pub fn from_location(location: SourceLocation) -> Self {
        Self {
            offset: location.byte_offset as u32,
            line: location.line as u32,
            column: location.column as u32,
        }
    }
}

/** One parsed classic script or module. */
pub(in crate::v9) struct Program {
    /// Statements in source order; a module's export declarations stand here as plain ones.
    pub body: Vec<Stmt>,
    /// Whether a directive prologue made the script strict (a module always is).
    pub strict: bool,
    /// Position of the first byte, which identifies the script's top-level code.
    pub pos: Pos,
    /// A module's imports and exports; None for a classic script.
    pub module: Option<Box<ModuleSyntax>>,
}

/** What a module asks of others and offers them, in source order. */
#[derive(Default)]
pub(in crate::v9) struct ModuleSyntax {
    /// Distinct module specifiers, in first-appearance order; entries name them by index.
    pub requests: Vec<String>,
    /// Bindings imported from other modules.
    pub imports: Vec<ImportEntry>,
    /// Names this module exports.
    pub exports: Vec<ExportEntry>,
}

/** One imported binding: `local` reads `imported` of request `request`, or its namespace. */
pub(in crate::v9) struct ImportEntry {
    pub local: Name,
    pub request: usize,
    /// None for `* as local`.
    pub imported: Option<Name>,
}

/** One exported name. */
pub(in crate::v9) enum ExportEntry {
    /** `export` of a binding this module declares. */
    Local { export: Name, local: Name, pos: Pos },
    /** `export { imported as export } from` or, with None, `export * as export from`. */
    Indirect {
        export: Name,
        request: usize,
        imported: Option<Name>,
    },
    /** `export * from`: every name of the request but `default`. */
    Star(usize),
}

/** The hidden binding a module's `export default expression` initializes. */
pub(in crate::v9) const DEFAULT_BINDING: &str = "*default*";

/** A statement with its position. */
pub(in crate::v9) struct Stmt {
    /// What the statement does.
    pub kind: StmtKind,
    /// Where it starts.
    pub pos: Pos,
}

/** Declaration kinds share one shape for var, let and const. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::v9) enum VarKind {
    Var,
    Let,
    Const,
    /** A synchronous immutable binding registered for lexical disposal. */
    Using,
    /** An immutable binding registered for awaited lexical disposal. */
    AwaitUsing,
}
impl VarKind {
    /** Resource bindings have the same assignment and dead-zone rules as const bindings. */
    pub(in crate::v9) fn immutable(self) -> bool {
        matches!(self, Self::Const | Self::Using | Self::AwaitUsing)
    }
    /** Only resource declarations capture and later invoke disposal methods. */
    pub(in crate::v9) fn resource(self) -> bool {
        matches!(self, Self::Using | Self::AwaitUsing)
    }
}

/** One `name = value` item of a declaration. */
pub(in crate::v9) struct Declarator {
    /// A name or a destructuring pattern.
    pub target: Pattern,
    /// The optional initializer.
    pub init: Option<Expr>,
}

/** The left side of a for-in or for-of head. */
pub(in crate::v9) enum ForHead {
    /// `var`, `let` or `const` declaring one target.
    Declare(VarKind, Pattern),
    /// An existing reference or destructuring assignment target.
    Target(Pattern),
}

/** The first clause of a C-style for loop. */
pub(in crate::v9) enum ForInit {
    Declare(VarKind, Vec<Declarator>),
    Expr(Expr),
}

/** One switch clause; `test` is None for `default`. */
pub(in crate::v9) struct Case {
    pub test: Option<Expr>,
    pub body: Vec<Stmt>,
}

/** Every statement form of a classic script. */
pub(in crate::v9) enum StmtKind {
    Expr(Expr),
    Var(VarKind, Vec<Declarator>),
    Function(Rc<Func>),
    Class(Rc<Class>),
    Return(Option<Expr>),
    If(Expr, Box<Stmt>, Option<Box<Stmt>>),
    Block(Vec<Stmt>),
    For {
        init: Option<ForInit>,
        test: Option<Expr>,
        update: Option<Expr>,
        body: Box<Stmt>,
    },
    ForIn(ForHead, Expr, Box<Stmt>),
    ForOf(ForHead, Expr, Box<Stmt>, bool),
    While(Expr, Box<Stmt>),
    DoWhile(Box<Stmt>, Expr),
    Break(Option<Name>),
    Continue(Option<Name>),
    Throw(Expr),
    Try {
        block: Vec<Stmt>,
        param: Option<Pattern>,
        handler: Option<Vec<Stmt>>,
        finalizer: Option<Vec<Stmt>>,
    },
    Switch(Expr, Vec<Case>),
    Labeled(Name, Box<Stmt>),
    Empty,
    /** `with (object) body`: names in the body look in the object first (an object
    environment record, V8's with context). */
    With(Expr, Box<Stmt>),
    /** A class field definition; appears only inside a class's synthesized initializer. */
    Field(PropKey, Option<Expr>),
}

/** An expression with its position. */
pub(in crate::v9) struct Expr {
    pub kind: ExprKind,
    pub pos: Pos,
}

/** Prefix operators. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::v9) enum UnaryOp {
    Minus,
    Plus,
    Not,
    BitNot,
    Typeof,
    Void,
    Delete,
}

/** Binary operators other than the short-circuiting ones. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::v9) enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Exp,
    Shl,
    Shr,
    UShr,
    BitAnd,
    BitOr,
    BitXor,
    Eq,
    Ne,
    StrictEq,
    StrictNe,
    Lt,
    Le,
    Gt,
    Ge,
    In,
    InstanceOf,
}

/** Short-circuiting operators. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::v9) enum LogicalOp {
    And,
    Or,
    Nullish,
}

/** Assignment forms: plain, arithmetic compound, or logical. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::v9) enum AssignOp {
    Assign,
    Binary(BinaryOp),
    Logical(LogicalOp),
}

/** The property part of a member expression. */
pub(in crate::v9) enum MemberProp {
    Name(Name),
    Computed(Box<Expr>),
    Private(Name),
}

/** One element of an array literal or argument list. */
pub(in crate::v9) enum Element {
    Value(Expr),
    Spread(Expr),
    Hole,
}

/** Object-literal property keys. */
pub(in crate::v9) enum PropKey {
    Name(Name),
    Computed(Box<Expr>),
    Private(Name),
}

/** One object-literal member. */
pub(in crate::v9) enum PropDef {
    Value(PropKey, Expr),
    Shorthand(Name, Pos),
    /** `{a = 1}`, valid only once the literal becomes a destructuring pattern. */
    ShorthandDefault(Name, Pos, Box<Expr>),
    Method(PropKey, Rc<Func>),
    Getter(PropKey, Rc<Func>),
    Setter(PropKey, Rc<Func>),
    Spread(Expr),
}

/** Every expression form. */
pub(in crate::v9) enum ExprKind {
    Number(f64),
    String(JsString),
    /** A literal the scanner could not hold (BigInt, an overlong string); evaluating it fails. */
    Opaque(&'static str),
    /** Legacy call assignment targets evaluate the call then throw before accessing a property. */
    InvalidReference(&'static str),
    /** A BigInt literal. */
    BigInt(i128),
    /** Wide literal digits are compiled into exact bounded runtime integers. */
    BigIntWide(Name),
    Template(Vec<Option<JsString>>, Vec<Expr>),
    /** Tag, cooked chunks, raw chunks, substitutions. */
    Tagged(Box<Expr>, Vec<Option<JsString>>, Vec<JsString>, Vec<Expr>),
    Regex(Name, Name),
    Ident(Name),
    This,
    Super,
    NewTarget,
    Null,
    Bool(bool),
    Array(Vec<Element>),
    Object(Vec<PropDef>),
    Function(Rc<Func>),
    Class(Rc<Class>),
    Unary(UnaryOp, Box<Expr>),
    Update(bool, bool, Box<Expr>),
    Binary(BinaryOp, Box<Expr>, Box<Expr>),
    Logical(LogicalOp, Box<Expr>, Box<Expr>),
    Assign(AssignOp, Box<Pattern>, Box<Expr>),
    Conditional(Box<Expr>, Box<Expr>, Box<Expr>),
    Call(Box<Expr>, Vec<Element>, bool),
    New(Box<Expr>, Vec<Element>),
    Member(Box<Expr>, MemberProp, bool),
    /** The extent an optional link short-circuits. */
    Chain(Box<Expr>),
    Sequence(Vec<Expr>),
    Yield(Option<Box<Expr>>, bool),
    Await(Box<Expr>),
    /** `#x in obj`. */
    PrivateIn(Name, Box<Expr>),
    /** `import.meta`, in a module. */
    ImportMeta,
    /** `import(specifier)`; a second options argument is evaluated and ignored. */
    Import(Box<Expr>, Option<Box<Expr>>),
}

/** Destructuring targets and binding patterns. */
pub(in crate::v9) enum Pattern {
    Ident(Name, Pos),
    /** A member expression target, allowed only in assignments. */
    Expr(Box<Expr>),
    Object(Vec<PatProp>, Option<Box<Pattern>>, Pos),
    Array(Vec<Option<Pattern>>, Option<Box<Pattern>>, Pos),
    Default(Box<Pattern>, Box<Expr>),
}

/** One `key: target` member of an object pattern. */
pub(in crate::v9) struct PatProp {
    pub key: PropKey,
    pub value: Pattern,
}

/** Function body forms. */
pub(in crate::v9) enum Body {
    Block(Vec<Stmt>),
    /** A concise arrow body. */
    Expr(Box<Expr>),
}

/** What kind of callable a function literal makes. */
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::v9) struct FuncFlags {
    pub arrow: bool,
    pub asynchronous: bool,
    pub generator: bool,
    /// Object and class methods have a home object and no prototype.
    pub method: bool,
    pub getter: bool,
    pub setter: bool,
    /// A class constructor; it must be called with new.
    pub constructor: bool,
    /// A class constructor of a class with `extends`.
    pub derived: bool,
    /// A class field initializer pseudo-function.
    pub fields: bool,
    /// Created by `new Function` or indirect eval rather than page source.
    pub dynamic: bool,
}

/** Site tags keep synthesized code apart from source functions that start at the same byte. */
pub(in crate::v9) const SITE_SOURCE: u32 = 0;
/** A constructor synthesized for a class without one. */
pub(in crate::v9) const SITE_CONSTRUCTOR: u32 = 1;
/** A class's instance field initializer. */
pub(in crate::v9) const SITE_FIELDS: u32 = 2;
/** A class's static field and block initializer. */
pub(in crate::v9) const SITE_STATICS: u32 = 3;
/** A script's top-level code. */
pub(in crate::v9) const SITE_SCRIPT: u32 = 4;
/** The frame name V8 prints for a class's synthesized instance-field initializer. */
pub(in crate::v9) const INSTANCE_INITIALIZER_NAME: &str = "<instance_members_initializer>";
/** The frame name V8 prints for a class's synthesized static initializer. */
pub(in crate::v9) const STATIC_INITIALIZER_NAME: &str = "<static_initializer>";
/** How many tags each byte offset carries. */
pub(in crate::v9) const SITE_TAGS: u32 = 5;

/** The code identity of a script's top level. */
pub(in crate::v9) fn site_of(pos: Pos) -> u32 {
    pos.offset * SITE_TAGS + SITE_SCRIPT
}

/** A function literal; its start offset is its code identity. */
pub(in crate::v9) struct Func {
    /// Identity of the code across requests: page-relative offset plus a tag.
    pub site: u32,
    /// Binding name of a declaration or named expression.
    pub name: Option<Name>,
    /// Observable `name`, including inferred names.
    pub display: std::cell::RefCell<Name>,
    /// Parameters, possibly patterns with defaults.
    pub params: Vec<Pattern>,
    /// `...rest` parameter.
    pub rest: Option<Pattern>,
    /// The body.
    pub body: Body,
    /// The kind flags.
    pub flags: FuncFlags,
    /// Effective strictness, including inherited directives.
    pub strict: bool,
    /// Where the literal starts.
    pub pos: Pos,
    /// Exact source boundaries, including class bodies for their constructor function.
    pub source_span: std::cell::RefCell<Option<(u32, u32)>>,
    /// Count of parameters before the first default or rest, as `length` reports.
    pub length: u32,
}

/** A class method kind; fields and static blocks move into synthesized initializers. */
pub(in crate::v9) enum MemberKind {
    Method(Rc<Func>),
    Getter(Rc<Func>),
    Setter(Rc<Func>),
    /** A real accessor pair uses this hidden private field for its per-instance storage. */
    AutoAccessor(Name),
}

/** One class element. */
pub(in crate::v9) struct ClassMember {
    pub key: PropKey,
    pub is_static: bool,
    pub kind: MemberKind,
    pub pos: Pos,
}

/** A class literal. */
pub(in crate::v9) struct Class {
    pub name: Option<Name>,
    pub superclass: Option<Expr>,
    /// The explicit constructor, or a synthesized one.
    pub constructor: Rc<Func>,
    pub members: Vec<ClassMember>,
    /// Instance field initializers compiled as one function run by the constructor.
    pub fields: Option<Rc<Func>>,
    /// Static fields and static blocks, in order, run once with the class as `this`.
    pub statics: Option<Rc<Func>>,
    /// The `#names` this class body declares; a reference to any other belongs to an outer class.
    pub privates: Vec<Name>,
    pub pos: Pos,
}

/** Eval inherits SuperProperty grammar from methods and lexical arrows. */
pub(in crate::v9) const EVAL_SUPER_PROPERTY: u8 = 1;
/** Eval inherits SuperCall grammar only from derived constructors and their lexical arrows. */
pub(in crate::v9) const EVAL_SUPER_CALL: u8 = 2;
/** Eval may reference new.target without admitting a ReturnStatement. */
pub(in crate::v9) const EVAL_NEW_TARGET: u8 = 4;
/** Class initialization code forbids arguments, including within direct eval. */
pub(in crate::v9) const EVAL_NO_ARGUMENTS: u8 = 8;
