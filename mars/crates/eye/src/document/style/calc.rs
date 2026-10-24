/** Bounded `calc()`, `min()`, `max()` and `clamp()` over lengths, percentages and numbers. */
use super::{
    constants::{CALC_DEPTH, CALC_TOKENS},
    unsupported,
    values::{Dimension, signed_number, unit},
};
use crate::{document::RenderContext, protocol::EngineResult};
/** One lexical piece of a math expression. */
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Token {
    /// A number with its unit text (possibly empty or `%`).
    Number(String),
    /// One of `+ - * /`.
    Operator(u8),
    /// A function name (`calc`, `min`, ...) or empty for a bare parenthesis.
    Open(String),
    /// Closing parenthesis.
    Close,
    /// Argument separator.
    Comma,
}
/** Intermediate values keep plain numbers apart from lengths, as CSS type checking does. */
#[derive(Clone, Copy, Debug)]
enum Value {
    /// Unitless.
    Number(f64),
    /// px plus percent.
    Length(Dimension),
}
/** Lexes an expression; signs glued to a following digit start a number. */
pub(super) fn tokens(source: &str) -> EngineResult<Vec<Token>> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        let signed = matches!(byte, b'+' | b'-')
            && bytes
                .get(index + 1)
                .is_some_and(|next| next.is_ascii_digit() || *next == b'.')
            && !matches!(tokens.last(), Some(Token::Number(_) | Token::Close));
        index = if byte.is_ascii_whitespace() {
            index + 1
        } else if byte.is_ascii_digit() || byte == b'.' || signed {
            number(source, index, &mut tokens)
        } else if byte.is_ascii_alphabetic() {
            function(source, index, &mut tokens)?
        } else {
            tokens.push(match byte {
                b'(' => Token::Open(String::new()),
                b')' => Token::Close,
                b',' => Token::Comma,
                b'+' | b'-' | b'*' | b'/' => Token::Operator(byte),
                _ => return Err(unsupported()),
            });
            index + 1
        };
        if tokens.len() > CALC_TOKENS {
            return Err(unsupported());
        }
    }
    Ok(tokens)
}
/** A number runs through digits, a dot, an exponent and its trailing unit letters or `%`. */
fn number(source: &str, start: usize, tokens: &mut Vec<Token>) -> usize {
    let bytes = source.as_bytes();
    let mut end = start + 1;
    while end < bytes.len()
        && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'.' || bytes[end] == b'%')
    {
        end += 1;
    }
    tokens.push(Token::Number(source[start..end].into()));
    end
}
/** Identifiers are only function names here. */
fn function(source: &str, start: usize, tokens: &mut Vec<Token>) -> EngineResult<usize> {
    let bytes = source.as_bytes();
    let mut end = start;
    while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'-') {
        end += 1;
    }
    if bytes.get(end) != Some(&b'(') {
        return Err(unsupported());
    }
    tokens.push(Token::Open(source[start..end].into()));
    Ok(end + 1)
}
/** Evaluates a whole token list to one plain number, as `span calc(10 - 3 + 1)` or `opacity: calc(1 / 2)` need. */
pub(super) fn evaluate_number(tokens: &[Token]) -> EngineResult<f64> {
    let mut parser = Parser {
        tokens,
        index: 0,
        font: 0.0,
        context: RenderContext::default(),
    };
    let value = parser.factor(0)?;
    match value {
        Value::Number(number) if parser.index == tokens.len() && number.is_finite() => Ok(number),
        _ => Err(unsupported()),
    }
}
/** Evaluates a whole token list to one length. */
pub(super) fn evaluate(
    tokens: &[Token],
    font: f64,
    context: RenderContext,
) -> EngineResult<Dimension> {
    let mut parser = Parser {
        tokens,
        index: 0,
        font,
        context,
    };
    let value = parser.factor(0)?;
    if parser.index != tokens.len() {
        return Err(unsupported());
    }
    let dimension = match value {
        Value::Length(dimension) => dimension,
        Value::Number(0.0) => Dimension {
            px: 0.0,
            percent: 0.0,
        },
        Value::Number(_) => return Err(unsupported()),
    };
    if dimension.px.is_finite() && dimension.percent.is_finite() {
        Ok(dimension)
    } else {
        Err(unsupported())
    }
}
/** Recursive descent over a bounded token slice. */
struct Parser<'a> {
    /// The lexed expression.
    tokens: &'a [Token],
    /// Next token.
    index: usize,
    /// Font size for em units.
    font: f64,
    /// Viewport for vw/vh units.
    context: RenderContext,
}
impl Parser<'_> {
    /** Takes the next token. */
    fn next(&mut self) -> Option<&Token> {
        let token = self.tokens.get(self.index);
        self.index += 1;
        token
    }
    /** Sums and differences of products. */
    fn sum(&mut self, depth: usize) -> EngineResult<Value> {
        let mut value = self.product(depth)?;
        while let Some(Token::Operator(op @ (b'+' | b'-'))) = self.tokens.get(self.index) {
            let sign = if *op == b'+' { 1.0 } else { -1.0 };
            self.index += 1;
            value = add(value, self.product(depth)?, sign)?;
        }
        Ok(value)
    }
    /** Products and quotients where one side is a plain number. */
    fn product(&mut self, depth: usize) -> EngineResult<Value> {
        let mut value = self.factor(depth)?;
        while let Some(Token::Operator(op @ (b'*' | b'/'))) = self.tokens.get(self.index) {
            let divide = *op == b'/';
            self.index += 1;
            value = multiply(value, self.factor(depth)?, divide)?;
        }
        Ok(value)
    }
    /** A number, a parenthesis or a math function. */
    fn factor(&mut self, depth: usize) -> EngineResult<Value> {
        if depth > CALC_DEPTH {
            return Err(unsupported());
        }
        match self.next().cloned() {
            Some(Token::Number(text)) => leaf(&text, self.font, self.context),
            Some(Token::Open(name)) => self.call(&name, depth + 1),
            _ => Err(unsupported()),
        }
    }
    /** `calc` and parentheses hold one sum; `min`, `max` and `clamp` take comma lists. */
    fn call(&mut self, name: &str, depth: usize) -> EngineResult<Value> {
        let mut arguments = vec![self.sum(depth)?];
        while self.tokens.get(self.index) == Some(&Token::Comma) {
            self.index += 1;
            arguments.push(self.sum(depth)?);
        }
        if self.next() != Some(&Token::Close) {
            return Err(unsupported());
        }
        match (name, arguments.as_slice()) {
            ("" | "calc", [value]) => Ok(*value),
            ("min", values) => extreme(values, f64::min),
            ("max", values) => extreme(values, f64::max),
            ("clamp", [low, value, high]) => {
                let value = extreme(&[*value, *high], f64::min)?;
                extreme(&[*low, value], f64::max)
            }
            _ => Err(unsupported()),
        }
    }
}
/** A unitless token is a number; anything else is a length. */
fn leaf(text: &str, font: f64, context: RenderContext) -> EngineResult<Value> {
    if let Ok(number) = signed_number(text) {
        return Ok(Value::Number(number));
    }
    unit(text, font, context).map(Value::Length)
}
/** Same-kind addition; a literal zero acts as a zero length. */
fn add(left: Value, right: Value, sign: f64) -> EngineResult<Value> {
    Ok(match (left, right) {
        (Value::Number(a), Value::Number(b)) => Value::Number(a + sign * b),
        (Value::Length(a), Value::Length(b)) => Value::Length(Dimension {
            px: a.px + sign * b.px,
            percent: a.percent + sign * b.percent,
        }),
        _ => return Err(unsupported()),
    })
}
/** Scaling a length by a number, or dividing by a nonzero number. */
fn multiply(left: Value, right: Value, divide: bool) -> EngineResult<Value> {
    let scale = |dimension: Dimension, factor: f64| {
        Value::Length(Dimension {
            px: dimension.px * factor,
            percent: dimension.percent * factor,
        })
    };
    Ok(match (left, right, divide) {
        (_, Value::Number(0.0), true) => return Err(unsupported()),
        (Value::Number(a), Value::Number(b), true) => Value::Number(a / b),
        (Value::Number(a), Value::Number(b), false) => Value::Number(a * b),
        (Value::Length(a), Value::Number(b), true) => scale(a, 1.0 / b),
        (Value::Length(a), Value::Number(b), false)
        | (Value::Number(b), Value::Length(a), false) => scale(a, b),
        _ => return Err(unsupported()),
    })
}
/** min/max compare pure numbers or pure pixel lengths; mixed percentages need layout. */
fn extreme(values: &[Value], pick: fn(f64, f64) -> f64) -> EngineResult<Value> {
    let mut result: Option<Value> = None;
    for value in values {
        result = Some(match (result, value) {
            (None, value) => *value,
            (Some(Value::Number(a)), Value::Number(b)) => Value::Number(pick(a, *b)),
            (Some(Value::Length(a)), Value::Length(b)) if a.percent == 0.0 && b.percent == 0.0 => {
                Value::Length(Dimension {
                    px: pick(a.px, b.px),
                    percent: 0.0,
                })
            }
            _ => return Err(unsupported()),
        });
    }
    result.ok_or_else(unsupported)
}
