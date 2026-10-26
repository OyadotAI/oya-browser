/** Dynamic function arguments must terminate at their own grammar boundaries before any generated code executes. */
use super::*;

/** The generated wrapper has one anonymous function; author text cannot consume either supplied closing delimiter. */
pub(in crate::v9) fn validate_dynamic_function(
    source: &str,
    parameters_end: usize,
    body_end: usize,
) -> EngineResult<()> {
    let mut parser = Parser::new(super::super::lexer::tokenize(source)?, false);
    parser.expect("(")?;
    let pos = parser.pos();
    let asynchronous = parser.eat_name("async");
    parser.expect_name("function")?;
    parser.dynamic_parameters_end(parameters_end)?;
    parser.function_rest(pos, asynchronous, false)?;
    if parser.consumed_end() as usize != body_end + 1 {
        return Err(parser.unexpected());
    }
    parser.expect(")")?;
    if !matches!(parser.peek().kind, Kind::End) {
        return Err(parser.unexpected());
    }
    Ok(())
}

impl Parser {
    /** V8's expected_parameters_end_pos check prevents parameter text from closing early or consuming the body. */
    fn dynamic_parameters_end(&self, expected: usize) -> EngineResult<()> {
        let opening = self.position + usize::from(self.at("*"));
        let closing = self
            .closers
            .get(opening)
            .copied()
            .filter(|value| *value != NO_CLOSER);
        if closing
            .and_then(|index| self.tokens.get(index as usize))
            .is_some_and(|token| token.location.byte_offset == expected)
        {
            Ok(())
        } else {
            Err(self.unexpected())
        }
    }
}
