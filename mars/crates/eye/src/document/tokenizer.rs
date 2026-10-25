/** Owned HTML tokenization with the Living Standard's error recovery: malformed markup becomes
text, comments or dropped tokens, never a refusal, and every scan moves forward so input cannot hang. */
use super::entities::decode;
use super::markup::Attributes;

/** Raw text is requested explicitly by tree construction. */
pub(super) struct Tokenizer<'a> {
    rest: &'a str,
    /** Set by the tree builder inside SVG or MathML, where CDATA sections are text. */
    pub foreign: bool,
}
/** Native identity is allocated only by the tree builder. */
pub(super) enum Token {
    Start(String, Attributes, bool),
    End(String),
    Text(String),
    /// A comment's data, undecoded as HTML leaves it; the tree builder decides where it is kept.
    Comment(String),
}
impl<'a> Tokenizer<'a> {
    /** Public parsing has already bounded source bytes. */
    pub fn new(source: &'a str) -> Self {
        Self {
            rest: source,
            foreign: false,
        }
    }
    /** Comments become tokens; doctypes, processing instructions and bogus markup are skipped. */
    pub fn next(&mut self) -> Option<Token> {
        loop {
            if self.rest.is_empty() {
                return None;
            }
            let text = self.text_end();
            if text > 0 {
                // The data state passes U+0000 through and tree construction drops it in body
                // content, after character references are resolved, as HTML does.
                let token =
                    Token::Text(decode(&newlines(&self.rest[..text]), false).replace('\0', ""));
                self.rest = &self.rest[text..];
                return Some(token);
            }
            let rest = self.rest;
            if let Some(after) = rest.strip_prefix("<!--") {
                return Some(Token::Comment(self.comment(after)));
            } else if self.foreign
                && let Some(after) = rest.strip_prefix("<![CDATA[")
            {
                let end = after.find("]]>").unwrap_or(after.len());
                self.rest = after.get(end + 3..).unwrap_or("");
                if end > 0 {
                    return Some(Token::Text(newlines(&after[..end])));
                }
            } else if rest.starts_with("<!")
                && rest
                    .as_bytes()
                    .get(2..9)
                    .is_some_and(|word| word.eq_ignore_ascii_case(b"doctype"))
            {
                self.skip_past('>');
            } else if let Some(after) = rest.strip_prefix("<!") {
                self.rest = after;
                return Some(self.bogus_comment());
            } else if rest.starts_with("<?") {
                self.rest = &rest[1..];
                return Some(self.bogus_comment());
            } else if let Some(after) = rest.strip_prefix("</") {
                if let Some(token) = self.end_tag(after) {
                    return Some(token);
                }
            } else if let Some(token) = self.start_tag() {
                return Some(token);
            }
        }
    }
    /** The bogus comment state: everything to the next `>` (or the end of input) is the
    comment's data, as `<!x>`, `<?x>` and `</#x>` become comments in HTML. */
    fn bogus_comment(&mut self) -> Token {
        let end = self.rest.find('>').unwrap_or(self.rest.len());
        let data = comment_data(&self.rest[..end]);
        self.rest = self.rest.get(end + 1..).unwrap_or("");
        Token::Comment(data)
    }
    /** Bytes of literal text before the next markup; a `<` that cannot open markup is text. */
    fn text_end(&self) -> usize {
        let bytes = self.rest.as_bytes();
        let mut at = 0;
        while let Some(found) = self.rest[at..].find('<') {
            let index = at + found;
            match bytes.get(index + 1) {
                Some(c) if c.is_ascii_alphabetic() || b"/!?".contains(c) => return index,
                Some(_) => at = index + 1,
                None => return self.rest.len(),
            }
        }
        self.rest.len()
    }
    /** The comment start, comment and comment end states after `<!--`: `-->`, `--!>` and the
    abrupt `<!-->` and `<!--->` forms end it, the end of input emits what was read, and stray
    dashes stay data (`<!----->` holds `-`). Returns the comment's data. */
    fn comment(&mut self, after: &'a str) -> String {
        let (data, end) = comment_end(after);
        self.rest = &after[end..];
        comment_data(&data)
    }
    /** Bogus comments and ignored tags run to the next `>` or the end of input. */
    fn skip_past(&mut self, close: char) {
        self.rest = self
            .rest
            .find(close)
            .map_or("", |end| &self.rest[end + close.len_utf8()..]);
    }
    /** End tags ignore their attributes; `</>` is dropped and other `</x` is a bogus comment. */
    fn end_tag(&mut self, after: &'a str) -> Option<Token> {
        self.rest = after;
        if after.is_empty() {
            return Some(Token::Text("</".into()));
        }
        if let Some(rest) = after.strip_prefix('>') {
            self.rest = rest;
            return None;
        }
        if !after.starts_with(|c: char| c.is_ascii_alphabetic()) {
            return Some(self.bogus_comment());
        }
        let name = self.name();
        let closed = self.rest.contains('>');
        self.skip_past('>');
        closed.then_some(Token::End(name))
    }
    /** A tag the input ends inside is dropped, as HTML's tokenizer does at end of file. */
    fn start_tag(&mut self) -> Option<Token> {
        self.rest = &self.rest[1..];
        let name = self.name();
        let mut attrs = Attributes::new();
        loop {
            self.rest = self
                .rest
                .trim_start_matches(|c: char| c.is_ascii_whitespace());
            if let Some(rest) = self.rest.strip_prefix("/>") {
                self.rest = rest;
                return Some(Token::Start(name, attrs, true));
            }
            if let Some(rest) = self.rest.strip_prefix('>') {
                self.rest = rest;
                return Some(Token::Start(name, attrs, false));
            }
            if let Some(rest) = self.rest.strip_prefix('/') {
                self.rest = rest;
                continue;
            }
            if self.rest.is_empty() {
                return None;
            }
            let (key, value) = self.attribute();
            attrs.entry(key).or_insert(value);
        }
    }
    /** Tag names run to whitespace, `/` or `>` and compare ASCII case-insensitively. */
    fn name(&mut self) -> String {
        let end = self
            .rest
            .find(|c: char| c.is_ascii_whitespace() || c == '/' || c == '>')
            .unwrap_or(self.rest.len());
        let name = self.rest[..end].to_ascii_lowercase();
        self.rest = &self.rest[end..];
        name
    }
    /** Quoted, unquoted and boolean attributes; the first name byte may be `=` as in HTML. */
    fn attribute(&mut self) -> (String, String) {
        let first = self.rest.chars().next().map_or(0, char::len_utf8);
        let end = first
            + self.rest[first..]
                .find(|c: char| c.is_ascii_whitespace() || "=/>".contains(c))
                .unwrap_or(self.rest.len() - first);
        let name = self.rest[..end].to_ascii_lowercase();
        self.rest = self.rest[end..].trim_start_matches(|c: char| c.is_ascii_whitespace());
        let Some(rest) = self.rest.strip_prefix('=') else {
            return (name, String::new());
        };
        self.rest = rest.trim_start_matches(|c: char| c.is_ascii_whitespace());
        let raw = match self.rest.chars().next() {
            Some(quote @ ('"' | '\'')) => {
                let body = &self.rest[1..];
                let end = body.find(quote).unwrap_or(body.len());
                self.rest = body.get(end + 1..).unwrap_or("");
                &body[..end]
            }
            _ => {
                let end = self
                    .rest
                    .find(|c: char| c.is_ascii_whitespace() || c == '>')
                    .unwrap_or(self.rest.len());
                let value = &self.rest[..end];
                self.rest = &self.rest[end..];
                value
            }
        };
        (name, decode(&newlines(raw).replace('\0', "\u{fffd}"), true))
    }
    /** Raw text runs to a genuine case-insensitive `</tag`, or to the end of input when unclosed. */
    pub fn raw(&mut self, tag: &str) -> String {
        let bytes = self.rest.as_bytes();
        let mut at = 0;
        let end = loop {
            let Some(found) = self.rest[at..].find("</") else {
                break self.rest.len();
            };
            let index = at + found;
            let name = index + 2..index + 2 + tag.len();
            if bytes
                .get(name.clone())
                .is_some_and(|name| name.eq_ignore_ascii_case(tag.as_bytes()))
                && bytes
                    .get(name.end)
                    .is_none_or(|c| c.is_ascii_whitespace() || b"/>".contains(c))
            {
                break index;
            }
            at = index + 2;
        };
        let text = newlines(&self.rest[..end]).replace('\0', "\u{fffd}");
        self.rest = &self.rest[end..];
        if !self.rest.is_empty() {
            self.skip_past('>');
        }
        text
    }
}

/** Input stream preprocessing: CRLF pairs and lone CRs become LF, applied to each token's text
since the tokenizer borrows the source. A CR then plain text then LF is two newlines. */
fn newlines(text: &str) -> String {
    if text.contains('\r') {
        text.replace("\r\n", "\n").replace('\r', "\n")
    } else {
        text.to_owned()
    }
}

/** Comment data with U+0000 replaced, as every comment state does, and newlines normalized. */
fn comment_data(data: &str) -> String {
    newlines(data).replace('\0', "\u{fffd}")
}

/** Runs HTML's comment states over the text after `<!--`; returns the data and the byte
offset just past the comment. The less-than sign states only report nesting errors, so they
add nothing beyond the characters themselves. */
fn comment_end(after: &str) -> (String, usize) {
    #[derive(PartialEq)]
    enum State {
        Start,
        StartDash,
        Comment,
        EndDash,
        End,
        EndBang,
    }
    let mut state = State::Start;
    let mut data = String::new();
    let chars = after.char_indices();
    for (at, c) in chars {
        let next = at + c.len_utf8();
        state = match (state, c) {
            (State::Start | State::StartDash | State::End | State::EndBang, '>') => {
                return (data, next);
            }
            (State::Start, '-') => State::StartDash,
            (State::StartDash | State::EndDash, '-') => State::End,
            (State::Comment, '-') => State::EndDash,
            (State::End, '!') => State::EndBang,
            (State::End, '-') => {
                data.push('-');
                State::End
            }
            (State::EndBang, '-') => {
                data.push_str("--!");
                State::EndDash
            }
            (state, c) => {
                data.push_str(match state {
                    State::StartDash | State::EndDash => "-",
                    State::End => "--",
                    State::EndBang => "--!",
                    _ => "",
                });
                data.push(c);
                State::Comment
            }
        };
    }
    (data, after.len())
}
