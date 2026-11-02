/*!
 * The WHATWG URL Standard's basic URL parser, as Chromium's url library and Blink's KURL apply
 * it: one state machine over code points, with the state overrides the URL setters use, host
 * parsing (IPv4 with its hex and octal parts, IPv6 with compression, opaque hosts and IDNA
 * through UTS #46), the percent-encode sets, serialization and origins.
 *
 * Setters mutate the record in place and ignore failure, as the standard's setter steps do.
 */

/** A parsed URL record. `opaque` marks an opaque path, held whole in `path[0]`. */
#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::v9) struct UrlRecord {
    /// The ASCII-lowercase scheme.
    pub scheme: String,
    /// Percent-encoded userinfo name.
    pub username: String,
    /// Percent-encoded userinfo password.
    pub password: String,
    /// The serialized host; `Some("")` is the empty host.
    pub host: Option<String>,
    /// The port, never the scheme's default.
    pub port: Option<u16>,
    /// Path segments, or the one opaque path string when `opaque`.
    pub path: Vec<String>,
    /// Whether the path is opaque (`mailto:x`, `javascript:...`).
    pub opaque: bool,
    /// The percent-encoded query.
    pub query: Option<String>,
    /// The percent-encoded fragment.
    pub fragment: Option<String>,
}

/** The parser's states, named as the standard names them. */
#[derive(Clone, Copy, PartialEq, Debug)]
pub(in crate::v9) enum State {
    SchemeStart,
    Scheme,
    NoScheme,
    SpecialRelativeOrAuthority,
    PathOrAuthority,
    Relative,
    RelativeSlash,
    SpecialAuthoritySlashes,
    SpecialAuthorityIgnoreSlashes,
    Authority,
    Host,
    Hostname,
    Port,
    File,
    FileSlash,
    FileHost,
    PathStart,
    Path,
    OpaquePath,
    Query,
    Fragment,
}

/** The special schemes and their default ports. */
const SPECIAL: &[(&str, Option<u16>)] = &[
    ("ftp", Some(21)),
    ("file", None),
    ("http", Some(80)),
    ("https", Some(443)),
    ("ws", Some(80)),
    ("wss", Some(443)),
];

/** Whether a scheme is special. */
pub(in crate::v9) fn is_special(scheme: &str) -> bool {
    SPECIAL.iter().any(|(name, _)| *name == scheme)
}

/** A special scheme's default port. */
fn default_port(scheme: &str) -> Option<u16> {
    SPECIAL
        .iter()
        .find(|(name, _)| *name == scheme)
        .and_then(|(_, port)| *port)
}

/** The percent-encode sets, each a superset of the one before. */
#[derive(Clone, Copy, PartialEq, PartialOrd)]
pub(in crate::v9) enum EncodeSet {
    C0,
    Fragment,
    Query,
    SpecialQuery,
    Path,
    Userinfo,
    Component,
    Form,
}

/** Whether `c` is in `set`. */
fn in_set(c: char, set: EncodeSet) -> bool {
    if (c as u32) < 0x20 || (c as u32) > 0x7E {
        return true;
    }
    match set {
        EncodeSet::C0 => false,
        EncodeSet::Fragment => matches!(c, ' ' | '"' | '<' | '>' | '`'),
        EncodeSet::Query => matches!(c, ' ' | '"' | '#' | '<' | '>'),
        EncodeSet::SpecialQuery => matches!(c, ' ' | '"' | '#' | '<' | '>' | '\''),
        EncodeSet::Path => matches!(c, ' ' | '"' | '#' | '<' | '>' | '?' | '^' | '`' | '{' | '}'),
        EncodeSet::Userinfo => {
            in_set(c, EncodeSet::Path)
                || matches!(c, '/' | ':' | ';' | '=' | '@' | '[' | '\\' | ']' | '|')
        }
        EncodeSet::Component => {
            in_set(c, EncodeSet::Userinfo) || matches!(c, '$' | '%' | '&' | '+' | ',')
        }
        EncodeSet::Form => {
            in_set(c, EncodeSet::Component) || matches!(c, '!' | '\'' | '(' | ')' | '~')
        }
    }
}

/** UTF-8 percent-encode one code point onto `out`. */
pub(in crate::v9) fn encode_char(out: &mut String, c: char, set: EncodeSet) {
    if !in_set(c, set) {
        out.push(c);
        return;
    }
    let mut bytes = [0; 4];
    for byte in c.encode_utf8(&mut bytes).bytes() {
        out.push_str(&format!("%{byte:02X}"));
    }
}

/** UTF-8 percent-encode a string. */
pub(in crate::v9) fn encode(text: &str, set: EncodeSet) -> String {
    let mut out = String::with_capacity(text.len());
    text.chars().for_each(|c| encode_char(&mut out, c, set));
    out
}

/** Percent-decode bytes: `%XX` becomes the byte, anything else stays. */
pub(in crate::v9) fn percent_decode(input: &[u8]) -> Vec<u8> {
    let hex = |b: u8| (b as char).to_digit(16);
    let mut out = Vec::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        if input[i] == b'%'
            && i + 2 < input.len()
            && let (Some(h), Some(l)) = (hex(input[i + 1]), hex(input[i + 2]))
        {
            out.push((h * 16 + l) as u8);
            i += 3;
            continue;
        }
        out.push(input[i]);
        i += 1;
    }
    out
}

/** A Windows drive letter: an ASCII alpha and then `:` or `|` (only `:` when normalized). */
fn is_drive(text: &[char], normalized: bool) -> bool {
    text.len() == 2
        && text[0].is_ascii_alphabetic()
        && (text[1] == ':' || (!normalized && text[1] == '|'))
}

/** Whether a segment is a normalized Windows drive letter. */
fn is_normalized_drive(segment: &str) -> bool {
    is_drive(&segment.chars().collect::<Vec<_>>(), true)
}

/** Whether `rest` starts with a Windows drive letter: the letter, then the end or a delimiter. */
fn starts_with_drive(rest: &[char]) -> bool {
    rest.len() >= 2
        && is_drive(&rest[..2], false)
        && (rest.len() == 2 || matches!(rest[2], '/' | '\\' | '?' | '#'))
}

/** `.` or its percent-encoded form. */
fn single_dot(buffer: &str) -> bool {
    buffer == "." || buffer.eq_ignore_ascii_case("%2e")
}

/** `..` and its percent-encoded forms. */
fn double_dot(buffer: &str) -> bool {
    matches!(
        buffer.to_ascii_lowercase().as_str(),
        ".." | ".%2e" | "%2e." | "%2e%2e"
    )
}

impl UrlRecord {
    /** Whether the URL is special. */
    pub(in crate::v9) fn special(&self) -> bool {
        is_special(&self.scheme)
    }

    /** Whether the URL has a username or password. */
    fn has_credentials(&self) -> bool {
        !self.username.is_empty() || !self.password.is_empty()
    }

    /** No userinfo or port can be set: no host, the empty host, or a file URL. */
    pub(in crate::v9) fn cannot_have_credentials(&self) -> bool {
        self.host.as_deref().is_none_or(str::is_empty) || self.scheme == "file"
    }

    /** Drop the last path segment, keeping a file URL's lone drive letter. */
    fn shorten(&mut self) {
        if self.scheme == "file" && self.path.len() == 1 && is_normalized_drive(&self.path[0]) {
            return;
        }
        self.path.pop();
    }

    /** The path as `pathname` reports it. */
    pub(in crate::v9) fn pathname(&self) -> String {
        if self.opaque {
            return self.path.first().cloned().unwrap_or_default();
        }
        self.path.iter().map(|s| format!("/{s}")).collect()
    }

    /** `host:port` or just the host. */
    pub(in crate::v9) fn host_port(&self) -> String {
        match (&self.host, self.port) {
            (None, _) => String::new(),
            (Some(host), None) => host.clone(),
            (Some(host), Some(port)) => format!("{host}:{port}"),
        }
    }

    /** The URL serializer. */
    pub(in crate::v9) fn href(&self) -> String {
        let mut out = format!("{}:", self.scheme);
        if let Some(host) = &self.host {
            out.push_str("//");
            if self.has_credentials() {
                out.push_str(&self.username);
                if !self.password.is_empty() {
                    out.push(':');
                    out.push_str(&self.password);
                }
                out.push('@');
            }
            out.push_str(host);
            if let Some(port) = self.port {
                out.push_str(&format!(":{port}"));
            }
        } else if !self.opaque && self.path.len() > 1 && self.path[0].is_empty() {
            out.push_str("/.");
        }
        out.push_str(&self.pathname());
        if let Some(query) = &self.query {
            out.push('?');
            out.push_str(query);
        }
        if let Some(fragment) = &self.fragment {
            out.push('#');
            out.push_str(fragment);
        }
        out
    }

    /** The serialized origin: a tuple origin for the network schemes and http(s) blobs, and
    `null` for everything else. */
    pub(in crate::v9) fn origin(&self) -> String {
        match self.scheme.as_str() {
            "blob" => match parse(&self.pathname(), None) {
                Some(inner) if matches!(inner.scheme.as_str(), "http" | "https") => inner.origin(),
                _ => "null".into(),
            },
            "ftp" | "http" | "https" | "ws" | "wss" => {
                format!("{}://{}", self.scheme, self.host_port())
            }
            _ => "null".into(),
        }
    }
}

/** Parse `input` against an optional base. */
pub(in crate::v9) fn parse(input: &str, base: Option<&UrlRecord>) -> Option<UrlRecord> {
    let trimmed = input.trim_matches(|c: char| c <= ' ');
    let mut url = UrlRecord::default();
    run(trimmed, base, &mut url, None).then_some(url)
}

/** Run the state machine over `input` on `url` (a setter's record when `over` is given).
Returns false for failure. */
pub(in crate::v9) fn run(
    input: &str,
    base: Option<&UrlRecord>,
    url: &mut UrlRecord,
    over: Option<State>,
) -> bool {
    let chars: Vec<char> = input
        .chars()
        .filter(|c| !matches!(c, '\t' | '\n' | '\r'))
        .collect();
    let mut parser = Parser {
        input: chars,
        p: 0,
        buffer: String::new(),
        at_seen: false,
        brackets: false,
        password_seen: false,
        base,
        over,
        state: over.unwrap_or(State::SchemeStart),
    };
    parser.go(url) != Step::Fail
}

/** What one step tells the loop. */
#[derive(PartialEq)]
enum Step {
    Next,
    Done,
    Fail,
}

/** The state machine's variables. `p` is a signed pointer so "decrease by one" can go below 0. */
struct Parser<'a> {
    input: Vec<char>,
    p: isize,
    buffer: String,
    at_seen: bool,
    brackets: bool,
    password_seen: bool,
    base: Option<&'a UrlRecord>,
    over: Option<State>,
    state: State,
}

impl Parser<'_> {
    /** The code point at the pointer, or None at the end. */
    fn c(&self) -> Option<char> {
        usize::try_from(self.p)
            .ok()
            .and_then(|p| self.input.get(p).copied())
    }

    /** The code points after the pointer. */
    fn remaining(&self) -> &[char] {
        let at = usize::try_from(self.p + 1).unwrap_or(0);
        self.input.get(at..).unwrap_or(&[])
    }

    /** The code points from the pointer on. */
    fn rest_here(&self) -> &[char] {
        let at = usize::try_from(self.p).unwrap_or(0);
        self.input.get(at..).unwrap_or(&[])
    }

    /** Run every state until the end of input or an early return. */
    fn go(&mut self, url: &mut UrlRecord) -> Step {
        loop {
            let step = self.step(url);
            if step != Step::Next {
                return step;
            }
            if self.c().is_none() && self.p >= 0 {
                return Step::Done;
            }
            self.p += 1;
        }
    }

    /** One state's handling of the current code point. */
    fn step(&mut self, url: &mut UrlRecord) -> Step {
        let c = self.c();
        match self.state {
            State::SchemeStart => self.scheme_start(c),
            State::Scheme => self.scheme(url, c),
            State::NoScheme => self.no_scheme(url, c),
            State::SpecialRelativeOrAuthority => {
                if c == Some('/') && self.remaining().first() == Some(&'/') {
                    self.state = State::SpecialAuthorityIgnoreSlashes;
                    self.p += 1;
                } else {
                    self.state = State::Relative;
                    self.p -= 1;
                }
                Step::Next
            }
            State::PathOrAuthority => {
                if c == Some('/') {
                    self.state = State::Authority;
                } else {
                    self.state = State::Path;
                    self.p -= 1;
                }
                Step::Next
            }
            State::Relative => self.relative(url, c),
            State::RelativeSlash => self.relative_slash(url, c),
            State::SpecialAuthoritySlashes => {
                self.state = State::SpecialAuthorityIgnoreSlashes;
                if c == Some('/') && self.remaining().first() == Some(&'/') {
                    self.p += 1;
                } else {
                    self.p -= 1;
                }
                Step::Next
            }
            State::SpecialAuthorityIgnoreSlashes => {
                if !matches!(c, Some('/' | '\\')) {
                    self.state = State::Authority;
                    self.p -= 1;
                }
                Step::Next
            }
            State::Authority => self.authority(url, c),
            State::Host | State::Hostname => self.host(url, c),
            State::Port => self.port(url, c),
            State::File => self.file(url, c),
            State::FileSlash => self.file_slash(url, c),
            State::FileHost => self.file_host(url, c),
            State::PathStart => self.path_start(url, c),
            State::Path => self.path(url, c),
            State::OpaquePath => self.opaque_path(url, c),
            State::Query => self.query(url, c),
            State::Fragment => {
                if let Some(c) = c {
                    let fragment = url.fragment.get_or_insert_with(String::new);
                    encode_char(fragment, c, EncodeSet::Fragment);
                }
                Step::Next
            }
        }
    }

    /** scheme start state. */
    fn scheme_start(&mut self, c: Option<char>) -> Step {
        match c {
            Some(c) if c.is_ascii_alphabetic() => {
                self.buffer.push(c.to_ascii_lowercase());
                self.state = State::Scheme;
            }
            _ if self.over.is_none() => {
                self.state = State::NoScheme;
                self.p -= 1;
            }
            _ => return Step::Fail,
        }
        Step::Next
    }

    /** scheme state. */
    fn scheme(&mut self, url: &mut UrlRecord, c: Option<char>) -> Step {
        match c {
            Some(c) if c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.') => {
                self.buffer.push(c.to_ascii_lowercase());
                Step::Next
            }
            Some(':') => self.scheme_end(url),
            _ if self.over.is_none() => {
                self.buffer.clear();
                self.state = State::NoScheme;
                self.p = -1;
                Step::Next
            }
            _ => Step::Fail,
        }
    }

    /** The scheme state's `:`: commit the scheme and pick the next state. */
    fn scheme_end(&mut self, url: &mut UrlRecord) -> Step {
        if self.over.is_some()
            && (url.special() != is_special(&self.buffer)
                || ((url.has_credentials() || url.port.is_some()) && self.buffer == "file")
                || (url.scheme == "file" && url.host.as_deref() == Some("")))
        {
            return Step::Done;
        }
        url.scheme = std::mem::take(&mut self.buffer);
        if self.over.is_some() {
            if url.port.is_some() && url.port == default_port(&url.scheme) {
                url.port = None;
            }
            return Step::Done;
        }
        if url.scheme == "file" {
            self.state = State::File;
        } else if url.special() && self.base.is_some_and(|b| b.scheme == url.scheme) {
            self.state = State::SpecialRelativeOrAuthority;
        } else if url.special() {
            self.state = State::SpecialAuthoritySlashes;
        } else if self.remaining().first() == Some(&'/') {
            self.state = State::PathOrAuthority;
            self.p += 1;
        } else {
            url.opaque = true;
            url.path = vec![String::new()];
            self.state = State::OpaquePath;
        }
        Step::Next
    }

    /** no scheme state. */
    fn no_scheme(&mut self, url: &mut UrlRecord, c: Option<char>) -> Step {
        let Some(base) = self.base else {
            return Step::Fail;
        };
        if base.opaque {
            if c != Some('#') {
                return Step::Fail;
            }
            url.scheme = base.scheme.clone();
            url.path = base.path.clone();
            url.opaque = true;
            url.query = base.query.clone();
            url.fragment = Some(String::new());
            self.state = State::Fragment;
        } else {
            self.state = if base.scheme == "file" {
                State::File
            } else {
                State::Relative
            };
            self.p -= 1;
        }
        Step::Next
    }

    /** relative state. */
    fn relative(&mut self, url: &mut UrlRecord, c: Option<char>) -> Step {
        let base = self.base.expect("relative state has a base");
        url.scheme = base.scheme.clone();
        if c == Some('/') || (url.special() && c == Some('\\')) {
            self.state = State::RelativeSlash;
            return Step::Next;
        }
        url.username = base.username.clone();
        url.password = base.password.clone();
        url.host = base.host.clone();
        url.port = base.port;
        url.path = base.path.clone();
        url.query = base.query.clone();
        match c {
            Some('?') => {
                url.query = Some(String::new());
                self.state = State::Query;
            }
            Some('#') => {
                url.fragment = Some(String::new());
                self.state = State::Fragment;
            }
            Some(_) => {
                url.query = None;
                url.shorten();
                self.state = State::Path;
                self.p -= 1;
            }
            None => {}
        }
        Step::Next
    }

    /** relative slash state. */
    fn relative_slash(&mut self, url: &mut UrlRecord, c: Option<char>) -> Step {
        if url.special() && matches!(c, Some('/' | '\\')) {
            self.state = State::SpecialAuthorityIgnoreSlashes;
        } else if c == Some('/') {
            self.state = State::Authority;
        } else {
            let base = self.base.expect("relative slash state has a base");
            url.username = base.username.clone();
            url.password = base.password.clone();
            url.host = base.host.clone();
            url.port = base.port;
            self.state = State::Path;
            self.p -= 1;
        }
        Step::Next
    }

    /** authority state. */
    fn authority(&mut self, url: &mut UrlRecord, c: Option<char>) -> Step {
        match c {
            Some('@') => {
                if self.at_seen {
                    self.buffer.insert_str(0, "%40");
                }
                self.at_seen = true;
                for ch in std::mem::take(&mut self.buffer).chars() {
                    if ch == ':' && !self.password_seen {
                        self.password_seen = true;
                        continue;
                    }
                    let target = if self.password_seen {
                        &mut url.password
                    } else {
                        &mut url.username
                    };
                    encode_char(target, ch, EncodeSet::Userinfo);
                }
            }
            None | Some('/' | '?' | '#') => return self.authority_end(),
            Some('\\') if url.special() => return self.authority_end(),
            Some(c) => self.buffer.push(c),
        }
        Step::Next
    }

    /** The end of the authority: rewind to the start of the host. */
    fn authority_end(&mut self) -> Step {
        if self.at_seen && self.buffer.is_empty() {
            return Step::Fail;
        }
        self.p -= self.buffer.chars().count() as isize + 1;
        self.buffer.clear();
        self.state = State::Host;
        Step::Next
    }

    /** host and hostname states. */
    fn host(&mut self, url: &mut UrlRecord, c: Option<char>) -> Step {
        if self.over.is_some() && url.scheme == "file" {
            self.p -= 1;
            self.state = State::FileHost;
            return Step::Next;
        }
        let end = matches!(c, None | Some('/' | '?' | '#')) || (url.special() && c == Some('\\'));
        if c == Some(':') && !self.brackets {
            if self.buffer.is_empty() || self.over == Some(State::Hostname) {
                return Step::Fail;
            }
            let Some(host) = parse_host(&self.buffer, !url.special()) else {
                return Step::Fail;
            };
            url.host = Some(host);
            self.buffer.clear();
            self.state = State::Port;
        } else if end {
            self.p -= 1;
            if url.special() && self.buffer.is_empty() {
                return Step::Fail;
            }
            if self.over.is_some()
                && self.buffer.is_empty()
                && (url.has_credentials() || url.port.is_some())
            {
                return Step::Done;
            }
            let Some(host) = parse_host(&self.buffer, !url.special()) else {
                return Step::Fail;
            };
            url.host = Some(host);
            self.buffer.clear();
            self.state = State::PathStart;
            if self.over.is_some() {
                return Step::Done;
            }
        } else if let Some(c) = c {
            match c {
                '[' => self.brackets = true,
                ']' => self.brackets = false,
                _ => {}
            }
            self.buffer.push(c);
        }
        Step::Next
    }

    /** port state. */
    fn port(&mut self, url: &mut UrlRecord, c: Option<char>) -> Step {
        let end = matches!(c, None | Some('/' | '?' | '#')) || (url.special() && c == Some('\\'));
        match c {
            Some(d) if d.is_ascii_digit() => self.buffer.push(d),
            _ if end || self.over.is_some() => {
                if !self.buffer.is_empty() {
                    let digits = self.buffer.trim_start_matches('0');
                    let Some(port) = (if digits.len() > 5 {
                        None
                    } else {
                        digits.parse::<u32>().ok().or(Some(0))
                    })
                    .filter(|port| *port <= u32::from(u16::MAX)) else {
                        return Step::Fail;
                    };
                    let port = port as u16;
                    url.port = (default_port(&url.scheme) != Some(port)).then_some(port);
                    self.buffer.clear();
                    if self.over.is_some() {
                        return Step::Done;
                    }
                }
                if self.over.is_some() {
                    return Step::Fail;
                }
                self.state = State::PathStart;
                self.p -= 1;
            }
            _ => return Step::Fail,
        }
        Step::Next
    }

    /** file state. */
    fn file(&mut self, url: &mut UrlRecord, c: Option<char>) -> Step {
        url.scheme = "file".into();
        url.host = Some(String::new());
        if matches!(c, Some('/' | '\\')) {
            self.state = State::FileSlash;
            return Step::Next;
        }
        match self.base.filter(|b| b.scheme == "file") {
            Some(base) => {
                url.host = base.host.clone();
                url.path = base.path.clone();
                url.query = base.query.clone();
                match c {
                    Some('?') => {
                        url.query = Some(String::new());
                        self.state = State::Query;
                    }
                    Some('#') => {
                        url.fragment = Some(String::new());
                        self.state = State::Fragment;
                    }
                    Some(_) => {
                        url.query = None;
                        if starts_with_drive(self.rest_here()) {
                            url.path.clear();
                        } else {
                            url.shorten();
                        }
                        self.state = State::Path;
                        self.p -= 1;
                    }
                    None => {}
                }
            }
            None => {
                self.state = State::Path;
                self.p -= 1;
            }
        }
        Step::Next
    }

    /** file slash state. */
    fn file_slash(&mut self, url: &mut UrlRecord, c: Option<char>) -> Step {
        if matches!(c, Some('/' | '\\')) {
            self.state = State::FileHost;
            return Step::Next;
        }
        if let Some(base) = self.base.filter(|b| b.scheme == "file") {
            url.host = base.host.clone();
            if !starts_with_drive(self.rest_here())
                && base.path.first().is_some_and(|s| is_normalized_drive(s))
            {
                url.path.push(base.path[0].clone());
            }
        }
        self.state = State::Path;
        self.p -= 1;
        Step::Next
    }

    /** file host state. */
    fn file_host(&mut self, url: &mut UrlRecord, c: Option<char>) -> Step {
        let Some(ch) = c.filter(|c| !matches!(c, '/' | '\\' | '?' | '#')) else {
            self.p -= 1;
            let buffer: Vec<char> = self.buffer.chars().collect();
            if self.over.is_none() && is_drive(&buffer, false) {
                self.state = State::Path;
            } else if self.buffer.is_empty() {
                url.host = Some(String::new());
                if self.over.is_some() {
                    return Step::Done;
                }
                self.state = State::PathStart;
            } else {
                let Some(mut host) = parse_host(&self.buffer, false) else {
                    return Step::Fail;
                };
                if host == "localhost" {
                    host.clear();
                }
                url.host = Some(host);
                if self.over.is_some() {
                    return Step::Done;
                }
                self.buffer.clear();
                self.state = State::PathStart;
            }
            return Step::Next;
        };
        self.buffer.push(ch);
        Step::Next
    }

    /** path start state. */
    fn path_start(&mut self, url: &mut UrlRecord, c: Option<char>) -> Step {
        if url.special() {
            self.state = State::Path;
            if !matches!(c, Some('/' | '\\')) {
                self.p -= 1;
            }
        } else if self.over.is_none() && c == Some('?') {
            url.query = Some(String::new());
            self.state = State::Query;
        } else if self.over.is_none() && c == Some('#') {
            url.fragment = Some(String::new());
            self.state = State::Fragment;
        } else if c.is_some() {
            self.state = State::Path;
            if c != Some('/') {
                self.p -= 1;
            }
        } else if self.over.is_some() && url.host.is_none() {
            url.path.push(String::new());
        }
        Step::Next
    }

    /** path state. */
    fn path(&mut self, url: &mut UrlRecord, c: Option<char>) -> Step {
        let slash = c == Some('/') || (url.special() && c == Some('\\'));
        let end = c.is_none() || slash || (self.over.is_none() && matches!(c, Some('?' | '#')));
        if !end {
            encode_char(&mut self.buffer, c.expect("not the end"), EncodeSet::Path);
            return Step::Next;
        }
        if double_dot(&self.buffer) {
            url.shorten();
            if !slash {
                url.path.push(String::new());
            }
        } else if single_dot(&self.buffer) && !slash {
            url.path.push(String::new());
        } else if !single_dot(&self.buffer) {
            let chars: Vec<char> = self.buffer.chars().collect();
            if url.scheme == "file" && url.path.is_empty() && is_drive(&chars, false) {
                self.buffer = format!("{}:", chars[0]);
            }
            url.path.push(self.buffer.clone());
        }
        self.buffer.clear();
        match c {
            Some('?') => {
                url.query = Some(String::new());
                self.state = State::Query;
            }
            Some('#') => {
                url.fragment = Some(String::new());
                self.state = State::Fragment;
            }
            _ => {}
        }
        Step::Next
    }

    /** opaque path state: a space before `?` or `#` is encoded so it survives. */
    fn opaque_path(&mut self, url: &mut UrlRecord, c: Option<char>) -> Step {
        match c {
            Some('?') => {
                url.query = Some(String::new());
                self.state = State::Query;
            }
            Some('#') => {
                url.fragment = Some(String::new());
                self.state = State::Fragment;
            }
            Some(' ') => {
                let next = self.remaining().first();
                let text = if matches!(next, Some('?' | '#')) {
                    "%20"
                } else {
                    " "
                };
                url.path[0].push_str(text);
            }
            Some(c) => encode_char(&mut url.path[0], c, EncodeSet::C0),
            None => {}
        }
        Step::Next
    }

    /** query state. */
    fn query(&mut self, url: &mut UrlRecord, c: Option<char>) -> Step {
        if c.is_none() || (self.over.is_none() && c == Some('#')) {
            let set = if url.special() {
                EncodeSet::SpecialQuery
            } else {
                EncodeSet::Query
            };
            let encoded = encode(&std::mem::take(&mut self.buffer), set);
            url.query.get_or_insert_with(String::new).push_str(&encoded);
            if c == Some('#') {
                url.fragment = Some(String::new());
                self.state = State::Fragment;
            }
        } else if let Some(c) = c {
            self.buffer.push(c);
        }
        Step::Next
    }
}

/** Code points no host may contain. */
fn forbidden_host(c: char) -> bool {
    matches!(
        c,
        '\0' | '\t'
            | '\n'
            | '\r'
            | ' '
            | '#'
            | '/'
            | ':'
            | '<'
            | '>'
            | '?'
            | '@'
            | '['
            | '\\'
            | ']'
            | '^'
            | '|'
    )
}

/** Code points no domain may contain. */
fn forbidden_domain(c: char) -> bool {
    forbidden_host(c) || (c as u32) <= 0x1F || c == '%' || c == '\x7F'
}

/** The host parser: the serialized host, or None for failure. */
pub(in crate::v9) fn parse_host(input: &str, opaque: bool) -> Option<String> {
    if let Some(inner) = input.strip_prefix('[') {
        let inner = inner.strip_suffix(']')?;
        return parse_ipv6(inner).map(|pieces| format!("[{}]", serialize_ipv6(&pieces)));
    }
    if opaque {
        if input.chars().any(forbidden_host) {
            return None;
        }
        return Some(encode(input, EncodeSet::C0));
    }
    let domain = String::from_utf8_lossy(&percent_decode(input.as_bytes())).into_owned();
    let ascii = domain_to_ascii(&domain)?;
    if ascii.chars().any(forbidden_domain) {
        return None;
    }
    if ends_in_number(&ascii) {
        return parse_ipv4(&ascii).map(|ip| {
            let b = ip.to_be_bytes();
            format!("{}.{}.{}.{}", b[0], b[1], b[2], b[3])
        });
    }
    Some(ascii)
}

/** domain to ASCII: an all-ASCII domain is lowercased as is (its `xn--` labels are kept even
when their Punycode does not decode, as the URL Standard's tests and Chromium keep them); any
other goes through UTS #46 (nontransitional, CheckHyphens off, the URL deny list). */
pub(in crate::v9) fn domain_to_ascii(domain: &str) -> Option<String> {
    let result = if domain.is_ascii() {
        domain.to_ascii_lowercase()
    } else {
        idna::domain_to_ascii_cow(domain.as_bytes(), idna::AsciiDenyList::URL)
            .ok()?
            .into_owned()
    };
    (!result.is_empty()).then_some(result)
}

/** Whether the last label (ignoring one trailing dot) is a number, so the host is IPv4. */
fn ends_in_number(host: &str) -> bool {
    let mut parts: Vec<&str> = host.split('.').collect();
    if parts.last() == Some(&"") {
        if parts.len() == 1 {
            return false;
        }
        parts.pop();
    }
    let last = parts.last().copied().unwrap_or("");
    if !last.is_empty() && last.bytes().all(|b| b.is_ascii_digit()) {
        return true;
    }
    ipv4_number(last).is_some()
}

/** One IPv4 part: decimal, `0x` hex or leading-zero octal; None for failure. Values that
overflow saturate, so they fail the range checks after. */
fn ipv4_number(part: &str) -> Option<u64> {
    if part.is_empty() {
        return None;
    }
    let (digits, radix) = if let Some(hex) = part.strip_prefix("0x").or(part.strip_prefix("0X")) {
        (hex, 16)
    } else if part.len() >= 2 && part.starts_with('0') {
        (&part[1..], 8)
    } else {
        (part, 10)
    };
    if digits.is_empty() {
        return Some(0);
    }
    digits.chars().try_fold(0u64, |acc, c| {
        let d = c.to_digit(radix)?;
        Some(
            acc.saturating_mul(u64::from(radix))
                .saturating_add(u64::from(d)),
        )
    })
}

/** The IPv4 parser. */
fn parse_ipv4(host: &str) -> Option<u32> {
    let mut parts: Vec<&str> = host.split('.').collect();
    if parts.last() == Some(&"") && parts.len() > 1 {
        parts.pop();
    }
    if parts.len() > 4 {
        return None;
    }
    let numbers: Vec<u64> = parts
        .iter()
        .map(|p| ipv4_number(p))
        .collect::<Option<_>>()?;
    let (last, rest) = numbers.split_last()?;
    if rest.iter().any(|n| *n > 255) || *last >= 256u64.pow(5 - numbers.len() as u32) {
        return None;
    }
    let mut ip = *last;
    for (i, n) in rest.iter().enumerate() {
        ip += n * 256u64.pow(3 - i as u32);
    }
    u32::try_from(ip).ok()
}

/** The IPv6 parser, over the text between the brackets. */
fn parse_ipv6(input: &str) -> Option<[u16; 8]> {
    let c: Vec<char> = input.chars().collect();
    let mut address = [0u16; 8];
    let (mut piece, mut compress, mut p) = (0usize, None, 0usize);
    if c.first() == Some(&':') {
        if c.get(1) != Some(&':') {
            return None;
        }
        p = 2;
        piece = 1;
        compress = Some(1);
    }
    while p < c.len() {
        if piece == 8 {
            return None;
        }
        if c[p] == ':' {
            if compress.is_some() {
                return None;
            }
            p += 1;
            piece += 1;
            compress = Some(piece);
            continue;
        }
        let (mut value, mut length) = (0u32, 0);
        while length < 4 && p < c.len() && c[p].is_ascii_hexdigit() {
            value = value * 16 + c[p].to_digit(16)?;
            p += 1;
            length += 1;
        }
        if p < c.len() && c[p] == '.' {
            if length == 0 || piece > 6 {
                return None;
            }
            p -= length;
            return parse_ipv6_v4(&c[p..], address, piece, compress);
        }
        if p < c.len() && c[p] == ':' {
            p += 1;
            if p >= c.len() {
                return None;
            }
        } else if p < c.len() {
            return None;
        }
        address[piece] = value as u16;
        piece += 1;
    }
    finish_ipv6(address, piece, compress)
}

/** The IPv6 parser's embedded dotted IPv4 tail. */
fn parse_ipv6_v4(
    c: &[char],
    mut address: [u16; 8],
    mut piece: usize,
    compress: Option<usize>,
) -> Option<[u16; 8]> {
    let mut seen = 0;
    let mut p = 0;
    while p < c.len() {
        let mut value: Option<u32> = None;
        if seen > 0 {
            if c[p] == '.' && seen < 4 {
                p += 1;
            } else {
                return None;
            }
        }
        if p >= c.len() || !c[p].is_ascii_digit() {
            return None;
        }
        while p < c.len() && c[p].is_ascii_digit() {
            let n = c[p].to_digit(10)?;
            value = match value {
                None => Some(n),
                Some(0) => return None,
                Some(v) => Some(v * 10 + n),
            };
            if value? > 255 {
                return None;
            }
            p += 1;
        }
        address[piece] = address[piece] * 0x100 + value? as u16;
        seen += 1;
        if seen == 2 || seen == 4 {
            piece += 1;
        }
    }
    if seen != 4 {
        return None;
    }
    finish_ipv6(address, piece, compress)
}

/** Move the pieces after `::` to the end, or fail when there are not eight. */
fn finish_ipv6(mut address: [u16; 8], piece: usize, compress: Option<usize>) -> Option<[u16; 8]> {
    match compress {
        Some(start) => {
            let mut swaps = piece - start;
            let mut index = 7;
            while index != 0 && swaps > 0 {
                address.swap(index, start + swaps - 1);
                index -= 1;
                swaps -= 1;
            }
            Some(address)
        }
        None if piece == 8 => Some(address),
        None => None,
    }
}

/** IPv6 serialization: lowercase hex, the first longest run of two or more zeros as `::`. */
fn serialize_ipv6(address: &[u16; 8]) -> String {
    let (mut best, mut best_len) = (None, 1);
    let mut i = 0;
    while i < 8 {
        let start = i;
        while i < 8 && address[i] == 0 {
            i += 1;
        }
        if i - start > best_len {
            best = Some(start);
            best_len = i - start;
        }
        i += 1;
    }
    let mut out = String::new();
    let mut ignore0 = false;
    for (index, piece) in address.iter().enumerate() {
        if ignore0 && *piece == 0 {
            continue;
        }
        ignore0 = false;
        if best == Some(index) {
            out.push_str(if index == 0 { "::" } else { ":" });
            ignore0 = true;
            continue;
        }
        out.push_str(&format!("{piece:x}"));
        if index != 7 {
            out.push(':');
        }
    }
    out
}

/** application/x-www-form-urlencoded parsing. */
pub(in crate::v9) fn form_parse(input: &str) -> Vec<(String, String)> {
    input
        .split('&')
        .filter(|s| !s.is_empty())
        .map(|sequence| {
            let (name, value) = sequence.split_once('=').unwrap_or((sequence, ""));
            let decode = |s: &str| {
                let bytes = percent_decode(s.replace('+', " ").as_bytes());
                String::from_utf8_lossy(&bytes).into_owned()
            };
            (decode(name), decode(value))
        })
        .collect()
}

/** application/x-www-form-urlencoded serialization. */
pub(in crate::v9) fn form_serialize(pairs: &[(String, String)]) -> String {
    let encode_part = |s: &str| {
        let mut out = String::new();
        for c in s.chars() {
            if c == ' ' {
                out.push('+');
            } else {
                encode_char(&mut out, c, EncodeSet::Form);
            }
        }
        out
    };
    pairs
        .iter()
        .map(|(name, value)| format!("{}={}", encode_part(name), encode_part(value)))
        .collect::<Vec<_>>()
        .join("&")
}
