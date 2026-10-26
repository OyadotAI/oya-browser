/** Date tokenization preserves delimiter adjacency, nested comments and recognized keyword prefixes. */
use super::*;

/** Only the keywords recognized by the legacy date grammar receive semantic meaning. */
#[derive(Clone, Copy)]
pub(super) enum Keyword {
    Month(u32),
    Period(u32),
    Zone(i32, bool),
    Unknown,
}
/** Number spelling length remains available for timezone and millisecond interpretation. */
#[derive(Clone, Copy)]
pub(super) enum Token {
    Number(u32, usize),
    Keyword(Keyword),
    Symbol(char),
    Space,
    Unknown,
    End,
}
/** A bounded source string is scanned once without expanding comments or guessing numeric layouts. */
pub(super) struct Scanner {
    chars: Vec<char>,
    position: usize,
    next: Token,
}
impl Scanner {
    /** Initialize one token of lookahead. */
    pub fn new(text: &str) -> Self {
        let mut scanner = Self {
            chars: text.chars().collect(),
            position: 0,
            next: Token::End,
        };
        scanner.next = scanner.scan();
        scanner
    }
    /** Consuming lookahead always advances, including over otherwise ignored punctuation. */
    pub fn next(&mut self) -> Token {
        let next = self.next;
        self.next = self.scan();
        next
    }
    /** Token lookahead never skips adjacency-significant whitespace or comments. */
    pub fn peek(&self) -> Token {
        self.next
    }
    /** A delimiter is consumed only when it is the next exact symbol. */
    pub fn eat(&mut self, symbol: char) -> bool {
        if matches!(self.next,Token::Symbol(next)if next==symbol) {
            self.next();
            true
        } else {
            false
        }
    }
    /** The scanner treats NUL as end of input, matching the reference date reader. */
    fn current(&self) -> Option<char> {
        self.chars
            .get(self.position)
            .copied()
            .filter(|ch| *ch != '\0')
    }
    /** Numerals, symbols, words, whitespace and parenthesized comments are distinct token classes. */
    fn scan(&mut self) -> Token {
        let Some(ch) = self.current() else {
            return Token::End;
        };
        if ch.is_ascii_digit() {
            return self.number();
        }
        if matches!(ch, ':' | '-' | '+' | '.' | ')') {
            self.position += 1;
            return Token::Symbol(ch);
        }
        if ch >= 'A' && !crate::v9::source::whitespace(ch) {
            return self.word();
        }
        if crate::v9::source::whitespace(ch) {
            self.position += 1;
            return Token::Space;
        }
        if ch == '(' {
            let mut depth = 0;
            while let Some(ch) = self.current() {
                self.position += 1;
                if ch == '(' {
                    depth += 1;
                } else if ch == ')' {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
            }
            return Token::Unknown;
        }
        self.position += 1;
        Token::Unknown
    }
    /** Leading zeroes do not consume significant precision, while total spelling length remains exact. */
    fn number(&mut self) -> Token {
        let start = self.position;
        let mut value = 0;
        let mut significant = 0;
        while let Some(ch) = self.current().filter(char::is_ascii_digit) {
            self.position += 1;
            if ch == '0' && significant == 0 {
                continue;
            }
            if significant < 9 {
                value = value * 10 + (ch as u32 - '0' as u32);
            }
            significant += 1;
        }
        Token::Number(value, self.position - start)
    }
    /** Month words use three-letter prefixes; other keywords require their exact full spelling. */
    fn word(&mut self) -> Token {
        let start = self.position;
        while self
            .current()
            .is_some_and(|ch| ch >= 'A' && !crate::v9::source::whitespace(ch))
        {
            self.position += 1;
        }
        let word = self.chars[start..self.position]
            .iter()
            .collect::<String>()
            .to_ascii_lowercase();
        if let Some(month) = MONTHS
            .iter()
            .position(|month| word.starts_with(&month.to_ascii_lowercase()))
        {
            return Token::Keyword(Keyword::Month(month as u32 + 1));
        }
        Token::Keyword(match word.as_str() {
            "am" => Keyword::Period(0),
            "pm" => Keyword::Period(12),
            "ut" | "utc" | "gmt" => Keyword::Zone(0, false),
            "z" => Keyword::Zone(0, true),
            "est" => Keyword::Zone(-5, false),
            "edt" => Keyword::Zone(-4, false),
            "cst" => Keyword::Zone(-6, false),
            "cdt" => Keyword::Zone(-5, false),
            "mst" | "pdt" => Keyword::Zone(-7, false),
            "mdt" => Keyword::Zone(-6, false),
            "pst" => Keyword::Zone(-8, false),
            _ => Keyword::Unknown,
        })
    }
}
