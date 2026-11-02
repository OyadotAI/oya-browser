/** Typed operators keep longest-token scanning and precedence in one owned language table. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Operator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    Not,
    Equal,
    NotEqual,
    AbstractEqual,
    AbstractNotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    And,
    Or,
}
impl Operator {
    /** Longer spellings precede their prefixes; unsupported operators are checked first. */
    pub fn scan(source: &str) -> Option<(&'static str, Self)> {
        [
            ("===", Self::Equal),
            ("!==", Self::NotEqual),
            ("==", Self::AbstractEqual),
            ("!=", Self::AbstractNotEqual),
            ("<=", Self::LessEqual),
            (">=", Self::GreaterEqual),
            ("&&", Self::And),
            ("||", Self::Or),
            ("+", Self::Add),
            ("-", Self::Subtract),
            ("*", Self::Multiply),
            ("/", Self::Divide),
            ("%", Self::Remainder),
            ("!", Self::Not),
            ("<", Self::Less),
            (">", Self::Greater),
        ]
        .into_iter()
        .find(|(spelling, _)| source.starts_with(spelling))
    }
}
/** Assignment tokens select retained-reference stores without duplicating target evaluation. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AssignmentOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
}
/** Count expressions preserve the numeric old value independently from their store. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum UpdateOperator {
    Increment,
    Decrement,
}
