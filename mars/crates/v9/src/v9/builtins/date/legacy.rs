/** Legacy Date parsing composes scanned calendar, clock and timezone tokens in their original order. */
use super::*;
mod compose;
mod scan;
use compose::State;
use scan::{Keyword, Scanner, Token};

/** Recognized month prefixes and zone words are handled separately from ignorable leading garbage. */
pub(super) fn parse(text: &str) -> Option<(f64, bool)> {
    let mut scanner = Scanner::new(text);
    let mut state = State::default();
    loop {
        match scanner.next() {
            Token::End => return state.finish(),
            Token::Number(number, digits) => {
                state.read_number = true;
                number_token(&mut scanner, &mut state, number, digits)?;
            }
            Token::Keyword(keyword) => keyword_token(&mut scanner, &mut state, keyword)?,
            Token::Symbol(sign @ ('+' | '-')) if state.utc() || !state.time.is_empty() => {
                zone_token(&mut scanner, &mut state, sign)?
            }
            Token::Symbol('+' | '-' | ')') if state.read_number => return None,
            _ => {}
        }
    }
}

/** Clock delimiters take precedence over timezone minutes and ordinary calendar components. */
fn number_token(
    scanner: &mut Scanner,
    state: &mut State,
    number: u32,
    _digits: usize,
) -> Option<()> {
    if scanner.eat(':') {
        if scanner.eat(':') {
            if !state.time.is_empty() {
                return None;
            }
            state.add_time(number)?;
            state.add_time(0)?;
        } else {
            state.add_time(number)?;
            scanner.eat('.');
        }
        return Some(());
    }
    let decimal = scanner.eat('.');
    if decimal && state.expects_time(number) {
        state.add_time(number)?;
        let Token::Number(value, digits) = scanner.next() else {
            return None;
        };
        let millis = if digits < 3 {
            value * 10u32.pow((3 - digits) as u32)
        } else {
            value / 10u32.pow(digits.min(9).saturating_sub(3) as u32)
        };
        state.final_time(millis)?;
    } else if state.zone_hours.is_some() && state.zone_minutes.is_none() && number < 60 {
        state.zone_minutes = Some(number);
    } else if state.expects_time(number) {
        state.final_time(number)?;
        if !matches!(
            scanner.peek(),
            Token::End
                | Token::Space
                | Token::Keyword(Keyword::Zone(0, true))
                | Token::Symbol('+' | '-')
        ) {
            return None;
        }
    } else {
        if state.days.len() == 3 {
            return None;
        }
        state.days.push(number);
        scanner.eat('-');
    }
    Some(())
}

/** Only month names may have arbitrary trailing letters; unknown words after numbers are invalid. */
fn keyword_token(scanner: &mut Scanner, state: &mut State, keyword: Keyword) -> Option<()> {
    match keyword {
        Keyword::Period(offset) if !state.time.is_empty() => state.period = Some(offset),
        Keyword::Month(month) => {
            state.month = Some(month);
            scanner.eat('-');
        }
        Keyword::Zone(offset, _) if state.read_number => {
            state.zone_sign = offset.signum();
            if offset == 0 {
                state.zone_sign = 1;
            }
            state.zone_hours = Some(offset.unsigned_abs());
            state.zone_minutes = Some(0);
        }
        _ => {
            if state.read_number || matches!(scanner.peek(), Token::Number(..)) {
                return None;
            }
        }
    }
    Some(())
}

/** Offsets permit GMT-8, compact hhmm and colon forms, preserving the scanner's remaining minute token. */
fn zone_token(scanner: &mut Scanner, state: &mut State, sign: char) -> Option<()> {
    state.zone_sign = if sign == '-' { -1 } else { 1 };
    let (number, digits) = if let Token::Number(number, digits) = scanner.peek() {
        scanner.next();
        (number, digits)
    } else {
        (0, 0)
    };
    state.read_number = true;
    if matches!(scanner.peek(), Token::Symbol(':')) {
        state.zone_hours = Some(number);
        state.zone_minutes = None;
    } else if matches!(digits, 1 | 2) {
        state.zone_hours = Some(number);
        state.zone_minutes = Some(0);
    } else if matches!(digits, 3 | 4) {
        state.zone_hours = Some(number / 100);
        state.zone_minutes = Some(number % 100);
    } else {
        return None;
    }
    Some(())
}
