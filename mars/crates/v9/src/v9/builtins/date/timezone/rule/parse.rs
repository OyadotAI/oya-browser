/** A bounded ASCII parser validates POSIX names, offsets, recurrence days and transition time suffixes. */
use super::*;

/** A cursor never exposes unparsed or unchecked host strings to date arithmetic. */
struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}

/** Daylight names require explicit recurrence rules; fixed zones terminate after their offset. */
pub(super) fn parse(text: &str) -> Option<Rule> {
    if text.is_empty() || !text.is_ascii() {
        return None;
    }
    let mut cursor = Cursor {
        bytes: text.as_bytes(),
        at: 0,
    };
    cursor.name()?;
    let standard = -cursor.time(24)?;
    if cursor.at == cursor.bytes.len() {
        return Some(Rule {
            standard,
            daylight: standard,
            start: None,
            end: None,
        });
    }
    cursor.name()?;
    let daylight = if cursor.peek() == Some(b',') {
        standard + 3600
    } else {
        -cursor.time(24)?
    };
    if daylight.unsigned_abs() > 86400 {
        return None;
    }
    cursor.expect(b',')?;
    let start = cursor.transition()?;
    cursor.expect(b',')?;
    let end = cursor.transition()?;
    (cursor.at == cursor.bytes.len()).then_some(Rule {
        standard,
        daylight,
        start: Some(start),
        end: Some(end),
    })
}

impl Cursor<'_> {
    /** Names are either alphabetic abbreviations or quoted names containing signs and digits. */
    fn name(&mut self) -> Option<()> {
        let quoted = self.eat(b'<');
        let start = self.at;
        while self.peek().is_some_and(|byte| {
            byte.is_ascii_alphabetic()
                || quoted && (byte.is_ascii_digit() || matches!(byte, b'+' | b'-'))
        }) {
            self.at += 1;
        }
        if self.at - start < 3 {
            return None;
        }
        if quoted {
            self.expect(b'>')?;
        }
        Some(())
    }
    /** Offsets and transition times share signed hour/minute/second syntax with different hour bounds. */
    fn time(&mut self, max_hour: u32) -> Option<i32> {
        let negative = self.eat(b'-');
        if !negative {
            self.eat(b'+');
        }
        let hours = self.number(max_hour)?;
        let minutes = if self.eat(b':') { self.number(59)? } else { 0 };
        let seconds = if self.eat(b':') { self.number(59)? } else { 0 };
        let value = (hours * 3600 + minutes * 60 + seconds) as i32;
        if max_hour == 24 && value > 86400 {
            return None;
        }
        Some(if negative { -value } else { value })
    }
    /** Rules support POSIX M, J and zero-based ordinal days with optional extended transition times. */
    fn transition(&mut self) -> Option<Transition> {
        let day = if self.eat(b'M') {
            let month = self.number(12)?;
            if month == 0 {
                return None;
            }
            self.expect(b'.')?;
            let week = self.number(5)?;
            if week == 0 {
                return None;
            }
            self.expect(b'.')?;
            Day::Month(month, week, self.number(6)?)
        } else if self.eat(b'J') {
            let day = self.number(365)?;
            if day == 0 {
                return None;
            }
            Day::Julian(day)
        } else {
            Day::Ordinal(self.number(365)?)
        };
        let mut transition = Transition {
            day,
            seconds: 7200,
            basis: Basis::Wall,
        };
        if self.eat(b'/') {
            transition.seconds = self.time(167)?;
            transition.basis = match self.peek() {
                Some(b'u' | b'g' | b'z') => {
                    self.at += 1;
                    Basis::Utc
                }
                Some(b's') => {
                    self.at += 1;
                    Basis::Standard
                }
                Some(b'w') => {
                    self.at += 1;
                    Basis::Wall
                }
                _ => Basis::Wall,
            };
        }
        Some(transition)
    }
    /** Checked decimal accumulation rejects missing digits and oversized fields without integer overflow. */
    fn number(&mut self, maximum: u32) -> Option<u32> {
        let start = self.at;
        let mut value = 0u32;
        while let Some(byte) = self.peek().filter(u8::is_ascii_digit) {
            self.at += 1;
            value = value.checked_mul(10)?.checked_add(u32::from(byte - b'0'))?;
            if value > maximum {
                return None;
            }
        }
        (self.at > start).then_some(value)
    }
    /** Peek does not consume or accept non-ASCII input. */
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }
    /** Optional delimiters advance only when present. */
    fn eat(&mut self, byte: u8) -> bool {
        if self.peek() == Some(byte) {
            self.at += 1;
            true
        } else {
            false
        }
    }
    /** Required delimiters reject a partial host rule. */
    fn expect(&mut self, byte: u8) -> Option<()> {
        self.eat(byte).then_some(())
    }
}
