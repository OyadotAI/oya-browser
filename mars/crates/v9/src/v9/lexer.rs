/** V8 scanner-guided bounded lexical rules; upstream mapping and notices live beside this crate. */
mod number;
use super::{
    errors, literals,
    operator::{AssignmentOperator, Operator, UpdateOperator},
    source::{Source, line_terminator, whitespace},
    value::JsString,
};
use crate::{
    constants::{SCRIPT_BYTES, TOKENS},
    protocol::{EngineError, EngineResult, SourceLocation},
};
use number::number;

/** Tokens retain locations independently of the value or identifier they carry. */
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Token {
    pub kind: Kind,
    pub location: SourceLocation,
    /// Decoded equality cannot make an escaped string a strict-mode directive.
    pub exact_strict: bool,
    /** Trivia before this token, rather than lines inside its predecessor, controls restricted grammar. */
    pub line_terminator_before: bool,
}
/** Numeric/string literals retain JavaScript types through AST construction. */
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Kind {
    String(JsString),
    Number(f64),
    Name(String),
    Operator(Operator),
    Assignment(AssignmentOperator),
    Update(UpdateOperator),
    Symbol(char),
    /** Arrow heads are assignment-level grammar rather than binary operators. */
    Arrow,
    /** Ellipsis is one bounded token, admitted only by array-element grammar. */
    Spread,
    /** A literal the scanner recognized but V9 cannot hold as a value (BigInt, an overlong
    string); evaluating it raises an unsupported error naming this feature. */
    Opaque(&'static str),
    /** A BigInt literal within V9's 128-bit bound. */
    BigInt(i128),
    /** Operators that V9 parses for syntax but does not evaluate, such as `**` or `>>>`. */
    Punct(&'static str),
    /** `#name` class member names. */
    Private(String),
    /** One template chunk: its cooked text (None when overlong or invalid), whether it starts
    at a backtick, and whether it ends at one rather than at `${`. */
    Template {
        cooked: Option<JsString>,
        /** The chunk's source text, for String.raw and tag functions. */
        raw: JsString,
        head: bool,
        tail: bool,
    },
    /** A regular expression literal: its pattern source and flags, compiled when evaluated. */
    Regex {
        pattern: String,
        flags: String,
    },
    End,
}
/** Scanning consumes the complete bounded script before any evaluation can begin. */
pub(super) fn tokenize(source: &str) -> EngineResult<Vec<Token>> {
    tokenize_with_limit(source, SCRIPT_BYTES)
}
/** One scanner applies identical lexical rules under an operation-specific source bound. */
fn tokenize_with_limit(source: &str, limit: usize) -> EngineResult<Vec<Token>> {
    if source.len() > limit {
        return Err(EngineError::limit("script bytes", limit));
    }
    let mut source = Source::new(source);
    if source.rest().starts_with("#!") {
        while source.peek().is_some_and(|c| !line_terminator(c)) {
            source.next();
        }
    }
    let mut tokens: Vec<Token> = Vec::new();
    // True entries are open template substitutions; false entries are ordinary braces.
    let mut braces: Vec<bool> = Vec::new();
    loop {
        let previous_line = source.location().line;
        let has_token = skip(&mut source)?;
        let line_terminator_before = source.location().line != previous_line;
        if !has_token {
            tokens.push(Token {
                kind: Kind::End,
                location: source.location(),
                exact_strict: false,
                line_terminator_before,
            });
            break;
        }
        if tokens.len() >= TOKENS {
            return Err(errors::located(
                EngineError::limit("V9 tokens", TOKENS),
                source.location(),
                "scan",
            ));
        }
        let location = source.location();
        let exact_strict = source.rest().starts_with("\"use strict\"")
            || source.rest().starts_with("'use strict'");
        let regex = regex_allowed(tokens.last().map(|token| &token.kind));
        let kind = next_kind(&mut source, &mut braces, regex).map_err(|mut error| {
            error.phase = Some("scan".into());
            errors::located(error, location, "scan")
        })?;
        tokens.push(Token {
            kind,
            location,
            exact_strict,
            line_terminator_before,
        });
    }
    Ok(tokens)
}
/** Braces decide whether `}` closes a block or resumes the template it interrupted. */
fn next_kind(source: &mut Source<'_>, braces: &mut Vec<bool>, regex: bool) -> EngineResult<Kind> {
    let start = source.location();
    let resume = match source.peek() {
        Some('`') => Some(true),
        Some('}') if braces.last() == Some(&true) => Some(false),
        _ => None,
    };
    if let Some(head) = resume {
        source.next();
        if !head {
            braces.pop();
        }
        let before = source.rest();
        let (cooked, tail) = literals::template(source, start)?;
        let consumed = &before[..before.len() - source.rest().len()];
        let text = consumed
            .strip_suffix(if tail { "`" } else { "${" })
            .unwrap_or(consumed);
        let raw = JsString::from_utf8(&text.replace("\r\n", "\n").replace('\r', "\n"))
            .unwrap_or_else(|_| JsString::empty());
        if !tail {
            push_brace(braces, true, start)?;
        }
        return Ok(Kind::Template {
            cooked,
            raw,
            head,
            tail,
        });
    }
    let kind = token(source, regex)?;
    match kind {
        Kind::Symbol('{') => push_brace(braces, false, start)?,
        Kind::Symbol('}') => {
            braces.pop();
        }
        _ => {}
    }
    Ok(kind)
}
/** Brace nesting shares the statement depth bound so hostile input cannot grow this stack. */
fn push_brace(braces: &mut Vec<bool>, template: bool, start: SourceLocation) -> EngineResult<()> {
    if braces.len() >= crate::constants::SYNTAX_DEPTH {
        return Err(errors::located(
            EngineError::limit("V9 syntax depth", crate::constants::SYNTAX_DEPTH),
            start,
            "scan",
        ));
    }
    braces.push(template);
    Ok(())
}
/** A slash starts a regular expression wherever an expression may begin, as browsers decide it. */
fn regex_allowed(previous: Option<&Kind>) -> bool {
    match previous {
        None => true,
        Some(Kind::Name(name)) => [
            "return",
            "typeof",
            "instanceof",
            "in",
            "of",
            "new",
            "delete",
            "void",
            "throw",
            "case",
            "do",
            "else",
            "yield",
            "await",
            "extends",
        ]
        .contains(&name.as_str()),
        Some(
            Kind::Number(_)
            | Kind::BigInt(_)
            | Kind::String(_)
            | Kind::Opaque(_)
            | Kind::Private(_)
            | Kind::Regex { .. }
            | Kind::Update(_)
            | Kind::Symbol(')' | ']'),
        ) => false,
        Some(Kind::Template { tail, .. }) => !tail,
        Some(_) => true,
    }
}
/** Comments terminate on JavaScript line terminators, not Rust's broader Unicode whitespace. */
fn skip(source: &mut Source<'_>) -> EngineResult<bool> {
    loop {
        if source.peek().is_some_and(whitespace) {
            source.next();
            continue;
        }
        if source.rest().starts_with("//") {
            while source.peek().is_some_and(|c| !line_terminator(c)) {
                source.next();
            }
            continue;
        }
        if source.rest().starts_with("/*") {
            let start = source.location();
            source.next();
            source.next();
            while !source.rest().starts_with("*/") {
                if source.next().is_none() {
                    return Err(errors::located(
                        errors::syntax(start, "unterminated block comment"),
                        start,
                        "scan",
                    ));
                }
            }
            source.next();
            source.next();
            continue;
        }
        return Ok(source.peek().is_some());
    }
}
/** Longest-token checks keep every JavaScript punctuator whole, including ones V9 only parses. */
fn token(source: &mut Source<'_>, regex: bool) -> EngineResult<Kind> {
    let start = source.location();
    let character = source.peek().expect("scanner cursor is nonempty");
    if character == '\'' || character == '"' {
        source.next();
        return Ok(literals::string(source, character, start)?.map_or(
            Kind::Opaque("string literals over the string length limit"),
            Kind::String,
        ));
    }
    if source.rest().starts_with("...") {
        consume(source, "...");
        return Ok(Kind::Spread);
    }
    if character.is_ascii_digit()
        || character == '.'
            && source
                .rest()
                .chars()
                .nth(1)
                .is_some_and(|c| c.is_ascii_digit())
    {
        return number(source);
    }
    if identifier_start(character) || character == '\\' {
        return identifier(source, start).map(Kind::Name);
    }
    if character == '#' {
        source.next();
        return identifier(source, start).map(Kind::Private);
    }
    if character == '/' && regex {
        return regex_literal(source, start);
    }
    if source.rest().starts_with("=>") {
        consume(source, "=>");
        return Ok(Kind::Arrow);
    }
    if let Some(spelling) = PUNCTUATORS
        .iter()
        .find(|spelling| source.rest().starts_with(**spelling))
    {
        consume(source, spelling);
        return Ok(Kind::Punct(spelling));
    }
    if source.rest().starts_with("?.")
        && !source
            .rest()
            .chars()
            .nth(2)
            .is_some_and(|c| c.is_ascii_digit())
    {
        consume(source, "?.");
        return Ok(Kind::Punct("?."));
    }
    if let Some((spelling, kind)) = mutation_token(source.rest()) {
        consume(source, spelling);
        return Ok(kind);
    }
    if let Some((spelling, operator)) = Operator::scan(source.rest()) {
        consume(source, spelling);
        return Ok(Kind::Operator(operator));
    }
    if ".()=;{}[],:?".contains(character) {
        source.next();
        return Ok(Kind::Symbol(character));
    }
    if let Some(spelling) = ["&", "|", "^", "~"]
        .into_iter()
        .find(|spelling| source.rest().starts_with(spelling))
    {
        source.next();
        return Ok(Kind::Punct(spelling));
    }
    Err(errors::syntax(start, "unexpected character"))
}
/** Operators V9 recognizes for syntax only, longest spelling first so none splits another. */
const PUNCTUATORS: [&str; 15] = [
    ">>>=", ">>>", "<<=", ">>=", "**=", "&&=", "||=", "??=", "**", "??", "<<", ">>", "&=", "|=",
    "^=",
];
/** Fixed spellings advance by characters so locations keep their UTF-16 columns. */
fn consume(source: &mut Source<'_>, spelling: &str) {
    for _ in spelling.chars() {
        source.next();
    }
}
/** Identifier names decode `\\u` escapes; an empty name (a bare `#` or `\\`) is a syntax error. */
fn identifier(source: &mut Source<'_>, start: SourceLocation) -> EngineResult<String> {
    let mut name = String::new();
    loop {
        match source.peek() {
            Some('\\') => {
                source.next();
                if source.next() != Some('u') {
                    return Err(errors::syntax(start, "invalid identifier escape"));
                }
                let value = literals::unicode(source, start)?;
                name.push(
                    char::from_u32(value)
                        .ok_or_else(|| errors::syntax(start, "invalid identifier escape"))?,
                );
            }
            Some(c) if identifier_start(c) || c.is_ascii_digit() => {
                source.next();
                name.push(c);
            }
            _ => break,
        }
    }
    if name.is_empty() {
        return Err(errors::syntax(start, "unexpected character"));
    }
    Ok(name)
}
/** The body is read with class and escape awareness; V9's own engine compiles it on use. */
fn regex_literal(source: &mut Source<'_>, start: SourceLocation) -> EngineResult<Kind> {
    source.next();
    let body = source.rest();
    let mut class = false;
    loop {
        let before = source.rest();
        match source.next() {
            None => return Err(errors::syntax(start, "unterminated regular expression")),
            Some(c) if line_terminator(c) => {
                return Err(errors::syntax(start, "unterminated regular expression"));
            }
            Some('\\') => {
                if source.next().is_none_or(line_terminator) {
                    return Err(errors::syntax(start, "unterminated regular expression"));
                }
            }
            Some('[') => class = true,
            Some(']') => class = false,
            Some('/') if !class => {
                let pattern = body[..body.len() - before.len()].to_string();
                let flags_start = source.rest();
                while source
                    .peek()
                    .is_some_and(|c| identifier_start(c) || c.is_ascii_digit())
                {
                    source.next();
                }
                let flags = flags_start[..flags_start.len() - source.rest().len()].to_string();
                return Ok(Kind::Regex { pattern, flags });
            }
            Some(_) => {}
        }
    }
}
/** Count and compound tokens are recognized only after rejecting every longer unsupported prefix. */
fn mutation_token(source: &str) -> Option<(&'static str, Kind)> {
    [
        ("++", Kind::Update(UpdateOperator::Increment)),
        ("--", Kind::Update(UpdateOperator::Decrement)),
        ("+=", Kind::Assignment(AssignmentOperator::Add)),
        ("-=", Kind::Assignment(AssignmentOperator::Subtract)),
        ("*=", Kind::Assignment(AssignmentOperator::Multiply)),
        ("/=", Kind::Assignment(AssignmentOperator::Divide)),
        ("%=", Kind::Assignment(AssignmentOperator::Remainder)),
    ]
    .into_iter()
    .find(|(spelling, _)| source.starts_with(spelling))
}
/** Non-ASCII letters and marks count as identifier characters; non-ASCII space never reaches here. */
fn identifier_start(character: char) -> bool {
    character.is_ascii_alphabetic()
        || character == '_'
        || character == '$'
        || !character.is_ascii() && !character.is_control() && !whitespace(character)
}
