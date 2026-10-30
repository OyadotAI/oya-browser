/** Shared bounded SVG path grammar normalizes commands for Eye and retained Canvas paths. */
use crate::constants::SVG_PATH_SEGMENTS;
use crate::protocol::{EngineError, EngineResult};
mod arcs;
mod scanner;
mod state;
use scanner::{Scanner, Token};
use state::State;

/** Source scanning has an independent bound even when whitespace or coincident arcs emit no geometry. */
pub const SOURCE_BYTES: usize = 256 * 1024;
/** Zero-output commands must not evade the geometry limit's work bound. */
const COMMANDS: usize = SVG_PATH_SEGMENTS * 4;
/** Syntax errors alone permit prefix recovery; numeric and resource refusals must propagate. */
const SYNTAX: &str = "svg_path_syntax";

/** Absolute user-space segments preserve Eye's existing cubic-based renderer interface. */
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Segment {
    /** Start an independent contour. */
    Move(f64, f64),
    /** Connect to an absolute endpoint. */
    Line(f64, f64),
    /** Two control points and the endpoint of an absolute cubic. */
    Cubic([f64; 6]),
    /** Connect to this contour's start and restore its logical current point. */
    Close,
}

/** Eye preserves its all-or-nothing shape contract when the input is malformed. */
pub fn parse(data: &str) -> EngineResult<Vec<Segment>> {
    parse_mode(data, false)
}

/** Canvas strings retain complete preceding commands when a later command is malformed. */
pub fn parse_prefix(data: &str) -> EngineResult<Vec<Segment>> {
    parse_mode(data, true)
}

/** Recovery never hides point, work, source-size or numeric-range failures. */
fn parse_mode(data: &str, recover: bool) -> EngineResult<Vec<Segment>> {
    if data.len() > SOURCE_BYTES {
        return Err(quota());
    }
    let mut scanner = Scanner::new(data);
    let mut state = State::default();
    let result = scan(&mut scanner, &mut state);
    match result {
        Ok(()) => Ok(state.segments),
        Err(error) if recover && error.code == SYNTAX => Ok(state.segments),
        Err(error) => Err(error),
    }
}

/** Each iteration consumes input or terminates; a numeric token can never repeat closePath. */
fn scan(scanner: &mut Scanner<'_>, state: &mut State) -> EngineResult<()> {
    let mut command = None;
    let mut groups = 0;
    while let Some(next) = scanner.command_or_number()? {
        let letter = match next {
            Token::Command(letter) => letter,
            Token::Number => command
                .filter(|letter| !matches!(letter, 'Z' | 'z'))
                .ok_or_else(malformed)?,
        };
        if command.is_none() && !matches!(letter, 'M' | 'm') {
            return Err(malformed());
        }
        if groups == COMMANDS {
            return Err(quota());
        }
        groups += 1;
        command = Some(state.step(letter, scanner)?);
    }
    Ok(())
}

/** Syntax failures may end a Canvas prefix but never modify already completed geometry. */
fn malformed() -> EngineError {
    EngineError::new(
        SYNTAX,
        "This page uses malformed SVG path data.",
        "Use valid SVG path commands.",
    )
}

/** Finite token values can still overflow during relative-coordinate or arc normalization. */
fn numeric() -> EngineError {
    EngineError::new(
        "canvas_geometry",
        "SVG path exceeds this build's numeric range.",
        "Use smaller finite path coordinates.",
    )
}

/** The Canvas binding translates this resource failure into a catchable DOM exception. */
fn quota() -> EngineError {
    EngineError::invalid("QuotaExceededError: SVG path parsing budget exceeded.")
}
