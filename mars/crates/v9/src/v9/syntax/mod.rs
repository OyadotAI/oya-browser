/** V9's full classic-script parser: tokens become a bounded syntax tree that the compiler lowers. */
mod ast;
mod expr;
mod func;
mod module;
mod stmt;
pub(in crate::v9) use ast::*;

use super::{
    errors,
    lexer::{Kind, Token},
    operator::{AssignmentOperator, Operator, UpdateOperator},
};
use crate::{
    constants::{
        EXPRESSION_DEPTH, FUNCTION_NESTING, STATEMENT_DEPTH, STATEMENTS, SYNTAX_DEPTH, TOKENS,
    },
    protocol::{EngineError, EngineResult},
};

/** Parse one script. `strict` is true when the caller's code is already strict (dynamic code). */
pub(in crate::v9) fn parse(tokens: Vec<Token>) -> EngineResult<Program> {
    Parser::new(tokens, false).program()
}

/** Parse one module: strict, with import and export declarations and top-level `await`. */
pub(in crate::v9) fn parse_module(tokens: Vec<Token>) -> EngineResult<Program> {
    let mut parser = Parser::new(tokens, true);
    parser.module = Some(Box::default());
    parser.context.asynchronous = true;
    parser.program()
}

/** What the enclosing function allows, saved and restored around nested functions. */
#[derive(Clone, Copy, Default)]
struct Context {
    /// `return` is legal.
    function: bool,
    /// `yield` is an operator.
    generator: bool,
    /// `await` is an operator.
    asynchronous: bool,
    /// `break` without a label is legal.
    breakable: bool,
    /// `continue` without a label is legal.
    continuable: bool,
    /// `super.x` is legal.
    method: bool,
    /// `super()` is legal.
    derived: bool,
    /// A class field initializer or static block: `arguments` is an early error here.
    no_arguments: bool,
    /// Directly inside a class static block, where `await` is reserved.
    static_block: bool,
}

/** A recursive-descent parser over the scanner's tokens. */
pub(in crate::v9) struct Parser {
    tokens: Vec<Token>,
    position: usize,
    /// Index of the matching `)` for each `(`, so arrow heads are found without backtracking.
    closers: Vec<u32>,
    strict: bool,
    context: Context,
    /// Live recursion, bounded by SYNTAX_DEPTH so no input can exhaust the stack.
    depth: usize,
    /// Expression nodes built, bounded by TOKENS.
    nodes: usize,
    /// Statements built, bounded by STATEMENTS.
    statements: usize,
    /// Nested function literals, bounded by FUNCTION_NESTING.
    nesting: usize,
    /// Expression and statement nesting inside the current function body.
    levels: Levels,
    /// The enclosing functions' levels, restored when a nested function literal ends.
    outer_levels: Vec<Levels>,
    /// Labels in scope, innermost last; the flag marks loop labels.
    labels: Vec<(Name, bool)>,
    /// A module's imports and exports as they are parsed; None for a classic script.
    module: Option<Box<ModuleSyntax>>,
    /// The private-name environment: one scope per enclosing class body, innermost last.
    privates: Vec<PrivateScope>,
}

/** The `#names` one class body declares and the ones its code references. */
#[derive(Default)]
struct PrivateScope {
    declared: Vec<(Name, PrivateKind)>,
    used: Vec<(Name, Pos)>,
}

/** What a `#name` declaration is: a getter and a setter of the same placement may share one. */
#[derive(Clone, Copy, PartialEq)]
enum PrivateKind {
    /// A field or method.
    Other,
    /// A getter; true when static.
    Getter(bool),
    /// A setter; true when static.
    Setter(bool),
}

/** Nesting inside one function body, bounded by EXPRESSION_DEPTH and STATEMENT_DEPTH. Each
function literal starts again from zero: FUNCTION_NESTING and SYNTAX_DEPTH bound the whole. */
#[derive(Clone, Copy, Default)]
struct Levels {
    /// Nested assignment expressions (arguments, operands, conditional arms).
    expressions: usize,
    /// Nested statements (blocks and single-statement bodies).
    statements: usize,
}

/** No parenthesis closes here. */
const NO_CLOSER: u32 = u32::MAX;

impl Parser {
    /** Matching parentheses are found once, in one bounded pass. */
    fn new(tokens: Vec<Token>, strict: bool) -> Self {
        let mut closers = vec![NO_CLOSER; tokens.len()];
        let mut open = Vec::new();
        for (index, token) in tokens.iter().enumerate() {
            match token.kind {
                Kind::Symbol('(') => open.push(index),
                Kind::Symbol(')') => {
                    if let Some(start) = open.pop() {
                        closers[start] = index as u32;
                    }
                }
                _ => {}
            }
        }
        Self {
            tokens,
            position: 0,
            closers,
            strict,
            context: Context::default(),
            depth: 0,
            nodes: 0,
            statements: 0,
            nesting: 0,
            levels: Levels::default(),
            outer_levels: Vec::new(),
            labels: Vec::new(),
            module: None,
            privates: Vec::new(),
        }
    }
    /** A whole script: a directive prologue and a statement list. */
    fn program(mut self) -> EngineResult<Program> {
        let pos = self.pos();
        self.strict |= self.directives();
        let body = self.statement_list(true)?;
        if !matches!(self.peek().kind, Kind::End) {
            return Err(self.unexpected());
        }
        let module = self.module.take();
        if let Some(module) = &module {
            module::check_exports(module, pos)?;
        }
        Ok(Program {
            body,
            strict: self.strict,
            pos,
            module,
        })
    }
    /** True when the prologue at the cursor contains an exact `"use strict"` directive. */
    fn directives(&self) -> bool {
        let mut index = self.position;
        while let Some(token) = self.tokens.get(index) {
            if !matches!(token.kind, Kind::String(_)) {
                return false;
            }
            let next = self.tokens.get(index + 1).map(|token| &token.kind);
            let ends = matches!(next, Some(Kind::Symbol(';' | '}') | Kind::End))
                || self
                    .tokens
                    .get(index + 1)
                    .is_some_and(|token| token.line_terminator_before)
                    && !matches!(
                        next,
                        Some(
                            Kind::Symbol('.' | '[' | '(' | '?' | ',' | '=')
                                | Kind::Operator(_)
                                | Kind::Punct(_)
                                | Kind::Template { .. }
                        )
                    );
            if !ends {
                return false;
            }
            if token.exact_strict {
                return true;
            }
            index += 1;
            if matches!(
                self.tokens.get(index).map(|t| &t.kind),
                Some(Kind::Symbol(';'))
            ) {
                index += 1;
            }
        }
        false
    }

    /** The current token; End repeats once the input is exhausted. */
    fn peek(&self) -> &Token {
        &self.tokens[self.position.min(self.tokens.len() - 1)]
    }
    /** The token `offset` places ahead. */
    fn peek_at(&self, offset: usize) -> &Token {
        &self.tokens[(self.position + offset).min(self.tokens.len() - 1)]
    }
    /** Consume one token. */
    fn next(&mut self) -> Token {
        let token = self.peek().clone();
        if !matches!(token.kind, Kind::End) {
            self.position += 1;
        }
        token
    }
    /** Position of the current token. */
    fn pos(&self) -> Pos {
        Pos::from_location(self.peek().location)
    }
    /** Punctuator spelling of the current token. */
    fn punct(&self) -> Option<&'static str> {
        spelling(&self.peek().kind)
    }
    /** True if the current token is this punctuator. */
    fn at(&self, spelling: &str) -> bool {
        self.punct() == Some(spelling)
    }
    /** True if the current token is this identifier or keyword. */
    fn at_name(&self, name: &str) -> bool {
        matches!(&self.peek().kind, Kind::Name(found) if found == name)
    }
    /** Consume the punctuator if present. */
    fn eat(&mut self, spelling: &str) -> bool {
        if self.at(spelling) {
            self.next();
            true
        } else {
            false
        }
    }
    /** Consume the keyword if present. */
    fn eat_name(&mut self, name: &str) -> bool {
        if self.at_name(name) {
            self.next();
            true
        } else {
            false
        }
    }
    /** Require a punctuator. */
    fn expect(&mut self, spelling: &str) -> EngineResult<()> {
        if self.eat(spelling) {
            Ok(())
        } else {
            Err(self.unexpected())
        }
    }
    /** Require a keyword. */
    fn expect_name(&mut self, name: &str) -> EngineResult<()> {
        if self.eat_name(name) {
            Ok(())
        } else {
            Err(self.unexpected())
        }
    }
    /** A semicolon, or a place automatic semicolon insertion allows one. */
    fn semicolon(&mut self) -> EngineResult<()> {
        if self.eat(";") {
            return Ok(());
        }
        if self.peek().line_terminator_before
            || matches!(self.peek().kind, Kind::End)
            || self.at("}")
        {
            return Ok(());
        }
        Err(self.unexpected())
    }
    /** The syntax error for the current token. */
    fn unexpected(&self) -> EngineError {
        let token = self.peek();
        let what = match &token.kind {
            Kind::End => "unexpected end of input",
            Kind::Opaque(_) => "unexpected literal",
            _ => "unexpected token",
        };
        errors::syntax(token.location, what)
    }
    /** A syntax error at a position. */
    fn error_at(&self, pos: Pos, reason: &str) -> EngineError {
        errors::syntax(pos.location(), reason)
    }
    /** Run a production one level deeper, bounded by SYNTAX_DEPTH. */
    fn enter<T>(&mut self, parse: impl FnOnce(&mut Self) -> EngineResult<T>) -> EngineResult<T> {
        if self.depth >= SYNTAX_DEPTH {
            return Err(errors::located(
                EngineError::limit("V9 syntax depth", SYNTAX_DEPTH),
                self.peek().location,
                "parse",
            ));
        }
        self.depth += 1;
        let result = parse(self);
        self.depth -= 1;
        result
    }
    /** Build an expression node, counting it against TOKENS. */
    fn node(&mut self, kind: ExprKind, pos: Pos) -> EngineResult<Expr> {
        self.nodes += 1;
        if self.nodes > TOKENS {
            return Err(errors::located(
                EngineError::limit("V9 AST nodes", TOKENS),
                pos.location(),
                "parse",
            ));
        }
        Ok(Expr { kind, pos })
    }
    /** Count a statement against STATEMENTS. */
    fn count_statement(&mut self, pos: Pos) -> EngineResult<()> {
        self.statements += 1;
        if self.statements > STATEMENTS {
            return Err(errors::located(
                EngineError::limit("V9 statements", STATEMENTS),
                pos.location(),
                "parse",
            ));
        }
        Ok(())
    }
    /** Enter a nested function literal, bounded by FUNCTION_NESTING. */
    fn nest(&mut self, pos: Pos) -> EngineResult<()> {
        if self.nesting >= FUNCTION_NESTING {
            return Err(errors::located(
                EngineError::limit("V9 function nesting", FUNCTION_NESTING),
                pos.location(),
                "parse",
            ));
        }
        self.nesting += 1;
        self.outer_levels.push(std::mem::take(&mut self.levels));
        Ok(())
    }
    /** Leave a nested function literal, restoring the enclosing body's levels. */
    fn unnest(&mut self) {
        self.nesting -= 1;
        self.levels = self.outer_levels.pop().unwrap_or_default();
    }
    /** Run a production one expression level deeper, bounded by EXPRESSION_DEPTH. */
    fn enter_expression<T>(
        &mut self,
        parse: impl FnOnce(&mut Self) -> EngineResult<T>,
    ) -> EngineResult<T> {
        if self.levels.expressions >= EXPRESSION_DEPTH {
            return Err(self.level_limit("V9 expression nesting", EXPRESSION_DEPTH));
        }
        self.levels.expressions += 1;
        let result = self.enter(parse);
        self.levels.expressions -= 1;
        result
    }
    /** Run a production one statement level deeper, bounded by STATEMENT_DEPTH. */
    fn enter_statement<T>(
        &mut self,
        parse: impl FnOnce(&mut Self) -> EngineResult<T>,
    ) -> EngineResult<T> {
        if self.levels.statements >= STATEMENT_DEPTH {
            return Err(self.level_limit("V9 statement nesting", STATEMENT_DEPTH));
        }
        self.levels.statements += 1;
        let result = self.enter(parse);
        self.levels.statements -= 1;
        result
    }
    /** The limit error for a nesting bound, located at the current token. */
    fn level_limit(&self, what: &str, limit: usize) -> EngineError {
        errors::located(
            EngineError::limit(what, limit),
            self.peek().location,
            "parse",
        )
    }
    /** An identifier usable as a binding or reference in the current context. */
    fn identifier(&mut self) -> EngineResult<Name> {
        let token = self.peek().clone();
        let Kind::Name(name) = &token.kind else {
            return Err(self.unexpected());
        };
        if reserved(name, self.strict)
            || name == "yield" && (self.context.generator || self.strict)
            || name == "await" && (self.context.asynchronous || self.context.static_block)
        {
            return Err(errors::syntax(token.location, "unexpected reserved word"));
        }
        self.next();
        Ok(Name::from(name.as_str()))
    }
    /** `arguments` cannot be referenced from a class field initializer or static block. */
    fn check_arguments(&self, name: &str, pos: Pos) -> EngineResult<()> {
        if self.context.no_arguments && name == "arguments" {
            return Err(self.error_at(pos, "'arguments' is not allowed in class field initializer or static initialization block"));
        }
        Ok(())
    }
    /** Record a `#name` reference; outside every class body it is an early error. */
    fn use_private(&mut self, name: &Name, pos: Pos) -> EngineResult<()> {
        match self.privates.last_mut() {
            Some(scope) => {
                scope.used.push((name.clone(), pos));
                Ok(())
            }
            None => Err(self.error_at(
                pos,
                &format!("Private field '#{name}' must be declared in an enclosing class"),
            )),
        }
    }
    /** Declare a `#name` in the innermost class body; only a getter and setter pair may share one. */
    fn declare_private(&mut self, name: &Name, kind: PrivateKind, pos: Pos) -> EngineResult<()> {
        let Some(scope) = self.privates.last_mut() else {
            return Err(self.error_at(pos, "unexpected private name"));
        };
        let mut earlier = scope.declared.iter().filter(|(seen, _)| seen == name);
        let clash = match (earlier.next(), earlier.next()) {
            (None, _) => false,
            (Some((_, first)), None) => !matches!(
                (*first, kind),
                (PrivateKind::Getter(a), PrivateKind::Setter(b))
                    | (PrivateKind::Setter(a), PrivateKind::Getter(b)) if a == b
            ),
            _ => true,
        };
        if clash {
            return Err(self.error_at(pos, "Identifier has already been declared"));
        }
        scope.declared.push((name.clone(), kind));
        Ok(())
    }
    /** Close a class body: references it does not declare belong to the enclosing class. */
    fn close_privates(&mut self) -> EngineResult<()> {
        let scope = self.privates.pop().unwrap_or_default();
        for (name, pos) in scope.used {
            if !scope.declared.iter().any(|(seen, _)| *seen == name) {
                self.use_private(&name, pos)?;
            }
        }
        Ok(())
    }
    /** Any identifier name, including keywords, as after a dot. */
    fn property_name(&mut self) -> EngineResult<Name> {
        match self.next().kind {
            Kind::Name(name) => Ok(Name::from(name.as_str())),
            _ => {
                self.position -= 1;
                Err(self.unexpected())
            }
        }
    }
    /** True if the token is an identifier that can start an expression or binding. */
    fn at_identifier(&self) -> bool {
        matches!(&self.peek().kind, Kind::Name(name) if !reserved(name, self.strict))
    }
}

/** Every punctuator's spelling, whichever token kind the scanner gave it. */
pub(in crate::v9) fn spelling(kind: &Kind) -> Option<&'static str> {
    Some(match kind {
        Kind::Operator(operator) => match operator {
            Operator::Add => "+",
            Operator::Subtract => "-",
            Operator::Multiply => "*",
            Operator::Divide => "/",
            Operator::Remainder => "%",
            Operator::Not => "!",
            Operator::Equal => "===",
            Operator::NotEqual => "!==",
            Operator::AbstractEqual => "==",
            Operator::AbstractNotEqual => "!=",
            Operator::Less => "<",
            Operator::LessEqual => "<=",
            Operator::Greater => ">",
            Operator::GreaterEqual => ">=",
            Operator::And => "&&",
            Operator::Or => "||",
        },
        Kind::Assignment(operator) => match operator {
            AssignmentOperator::Add => "+=",
            AssignmentOperator::Subtract => "-=",
            AssignmentOperator::Multiply => "*=",
            AssignmentOperator::Divide => "/=",
            AssignmentOperator::Remainder => "%=",
        },
        Kind::Update(UpdateOperator::Increment) => "++",
        Kind::Update(UpdateOperator::Decrement) => "--",
        Kind::Symbol(symbol) => match symbol {
            '.' => ".",
            '(' => "(",
            ')' => ")",
            '=' => "=",
            ';' => ";",
            '{' => "{",
            '}' => "}",
            '[' => "[",
            ']' => "]",
            ',' => ",",
            ':' => ":",
            '?' => "?",
            _ => return None,
        },
        Kind::Arrow => "=>",
        Kind::Spread => "...",
        Kind::Punct(spelling) => spelling,
        _ => return None,
    })
}

/** Words that can never be identifier references; strict mode reserves a few more. */
fn reserved(name: &str, strict: bool) -> bool {
    const ALWAYS: [&str; 36] = [
        "break",
        "case",
        "catch",
        "class",
        "const",
        "continue",
        "debugger",
        "default",
        "delete",
        "do",
        "else",
        "enum",
        "export",
        "extends",
        "false",
        "finally",
        "for",
        "function",
        "if",
        "import",
        "in",
        "instanceof",
        "new",
        "null",
        "return",
        "super",
        "switch",
        "this",
        "throw",
        "true",
        "try",
        "typeof",
        "var",
        "void",
        "while",
        "with",
    ];
    const STRICT: [&str; 9] = [
        "implements",
        "interface",
        "let",
        "package",
        "private",
        "protected",
        "public",
        "static",
        "yield",
    ];
    ALWAYS.contains(&name) || strict && STRICT.contains(&name)
}
