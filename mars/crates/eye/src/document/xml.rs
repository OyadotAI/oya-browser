use super::Document;
/** XML documents (XHTML and SVG), ported from the essentials of Blink's XMLDocumentParser over
libxml2: a well-formed XML 1.0 tokenizer that feeds the same tree builder the HTML parser uses, so
an `.xht` page builds the same Document. Self-closing tags close, CDATA is text everywhere,
namespaces resolve (XHTML elements are HTML elements, SVG elements are SVG, any other is inert),
and the first well-formedness error stops parsing, as Blink does, with Blink's parse-error block
shown above what was built up to it. */
use super::entities::decode;
use super::markup::{self, Attributes};
use super::recovery::VOID;
use super::tokenizer::{Token, Tokenizer};
use crate::constants::NODES;
use crate::protocol::{EngineError, EngineResult};
use std::collections::{HashMap, HashSet};

/** The XHTML namespace: only its elements get HTML semantics, and only its void elements are void. */
const XHTML_NS: &str = "http://www.w3.org/1999/xhtml";
/** The SVG namespace: only its elements get SVG semantics. */
const SVG_NS: &str = "http://www.w3.org/2000/svg";
/// The SVG elements Eye draws, as SVG spells them (names are case-sensitive in XML). The tree
/// builder knows them lowercased, as the HTML parser hands them over.
const SVG_NAMES: [&str; 19] = [
    "svg",
    "g",
    "path",
    "use",
    "rect",
    "circle",
    "ellipse",
    "line",
    "polyline",
    "polygon",
    "symbol",
    "defs",
    "linearGradient",
    "radialGradient",
    "stop",
    "clipPath",
    "mask",
    "pattern",
    "marker",
];
/** Nodes Blink's parse-error block adds above the page. */
const ERROR_NODES: usize = 4;
/// Start tags one document may hold, raw-text content included, so the error block still fits
/// under the node bound. It also bounds nesting depth, which can never exceed it.
const ELEMENTS: usize = NODES - ERROR_NODES;
/** A parse error message is cut here, so a hostile name cannot grow the error block. */
const MESSAGE_CHARS: usize = 256;
/** Media types whose documents Blink parses as XML rather than HTML. */
const XML_TYPES: [&str; 4] = [
    "application/xhtml+xml",
    "image/svg+xml",
    "application/xml",
    "text/xml",
];

/// Whether a response's media type essence (lowercase, parameters dropped) is parsed as XML,
/// as Blink's DOMImplementation::IsXMLMIMEType decides for the types Mars renders. Navigation
/// asks this one place so the runtime and Eye never disagree about which parser a page gets.
pub fn xml_mime(essence: &str) -> bool {
    XML_TYPES.contains(&essence)
}

/** One open element: its qualified name as written, the tag the tree builder sees, and the
prefixes its start tag bound, unbound again when it closes. */
struct Scope {
    qname: String,
    tag: String,
    bindings: Vec<String>,
    /** An XHTML void element written with an end tag: the builder never opened it. */
    void: bool,
}

/** The XML token source. It emits the HTML tokenizer's tokens, so tree construction is shared. */
pub(super) struct XmlTokenizer<'a> {
    source: &'a str,
    rest: &'a str,
    stack: Vec<Scope>,
    /** Each prefix's in-scope namespaces, innermost last, so a lookup never walks the stack. */
    bound: HashMap<String, Vec<String>>,
    /** Start tags read so far, bounded by ELEMENTS. */
    elements: usize,
    /** The end tag a self-closing start tag implies, delivered next. */
    pending: Option<Token>,
    seen_root: bool,
    /** The first well-formedness error, with its position; parsing stops there. */
    pub error: Option<String>,
}

impl<'a> XmlTokenizer<'a> {
    /** Public parsing has already bounded source bytes. */
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            rest: source,
            stack: Vec::new(),
            bound: HashMap::new(),
            elements: 0,
            pending: None,
            seen_root: false,
            error: None,
        }
    }
    /** The next token, or None at the end of input or after the first error. */
    pub fn next(&mut self) -> Option<Token> {
        if let Some(token) = self.pending.take() {
            return Some(token);
        }
        while self.error.is_none() {
            if self.rest.is_empty() {
                self.end_of_input();
                return None;
            }
            match self.step() {
                Ok(Some(token)) => return Some(token),
                Ok(None) => {}
                Err(message) => self.fail(&message),
            }
        }
        None
    }
    /// An element's own text (style, script, title, textarea): its direct text and CDATA
    /// children, comments dropped, through its end tag, as Blink's TextFromChildren reads it.
    pub fn raw(&mut self) -> String {
        let mut text = String::new();
        let mut depth = 0usize;
        while let Some(token) = self.next() {
            match token {
                Token::Text(data) if depth == 0 => text.push_str(&data),
                Token::Start(_, _, false) => depth += 1,
                Token::End(_) if depth == 0 => break,
                Token::End(_) => depth -= 1,
                _ => {}
            }
        }
        text
    }
    /** Records the first error as libxml2 words it, with its line and column. */
    fn fail(&mut self, message: &str) {
        let consumed = &self.source[..self.source.len() - self.rest.len()];
        let line = consumed.matches('\n').count() + 1;
        let column = consumed
            .rsplit('\n')
            .next()
            .map_or(0, |l| l.chars().count())
            + 1;
        let message: String = message.chars().take(MESSAGE_CHARS).collect();
        self.error = Some(format!(
            "error on line {line} at column {column}: {message}"
        ));
    }
    /** A document must have one root element, and every element must be closed. */
    fn end_of_input(&mut self) {
        if let Some(open) = self.stack.last() {
            let message = format!("Premature end of data in tag {}", open.qname);
            self.fail(&message);
        } else if !self.seen_root {
            self.fail("Document is empty");
        }
    }
    /** One construct: text, comment, CDATA, doctype, processing instruction or tag. */
    fn step(&mut self) -> Result<Option<Token>, String> {
        let rest = self.rest;
        if !rest.starts_with('<') {
            let end = rest.find('<').unwrap_or(rest.len());
            let token = self.text(&rest[..end])?;
            self.rest = &rest[end..];
            return Ok(token);
        }
        if let Some(after) = rest.strip_prefix("<!--") {
            let end = after.find("--").ok_or("Comment not terminated")?;
            if !after[end..].starts_with("-->") {
                return Err("Double hyphen within comment".into());
            }
            self.rest = &after[end + 3..];
            return Ok(Some(Token::Comment(after[..end].to_owned())));
        }
        if let Some(after) = rest.strip_prefix("<![CDATA[") {
            if self.stack.is_empty() {
                return Err("StartTag: invalid element name".into());
            }
            let end = after.find("]]>").ok_or("CData section not finished")?;
            self.rest = &after[end + 3..];
            return Ok((end > 0).then(|| Token::Text(after[..end].to_owned())));
        }
        if let Some(after) = rest.strip_prefix("<!DOCTYPE") {
            self.rest = doctype_end(after).ok_or("DOCTYPE improperly terminated")?;
            return Ok(None);
        }
        if let Some(after) = rest.strip_prefix("<?") {
            let end = after.find("?>").ok_or("ParsePI: PI xml never end ...")?;
            self.rest = &after[end + 2..];
            return Ok(None);
        }
        if let Some(after) = rest.strip_prefix("</") {
            return self.end_tag(after);
        }
        self.start_tag(&rest[1..]).map(Some)
    }
    /** Character data: only whitespace may sit outside the root element. */
    fn text(&self, raw: &str) -> Result<Option<Token>, String> {
        if self.stack.is_empty() {
            if !raw.trim_matches(is_space).is_empty() {
                return Err(if self.seen_root {
                    "Extra content at the end of the document".into()
                } else {
                    "Start tag expected, '<' not found".into()
                });
            }
            return Ok(None);
        }
        references(raw)?;
        Ok(Some(Token::Text(decode(raw, false))))
    }
    /** An end tag must name the innermost open element exactly. */
    fn end_tag(&mut self, after: &'a str) -> Result<Option<Token>, String> {
        let (qname, rest) = name(after).ok_or("expected '>'")?;
        let rest = rest
            .trim_start_matches(is_space)
            .strip_prefix('>')
            .ok_or("expected '>'")?;
        let Some(open) = self.stack.last() else {
            return Err("Extra content at the end of the document".into());
        };
        if open.qname != qname {
            return Err(format!(
                "Opening and ending tag mismatch: {} and {qname}",
                open.qname
            ));
        }
        self.rest = rest;
        let open = self.stack.pop().ok_or("expected '>'")?;
        for prefix in &open.bindings {
            self.bound.get_mut(prefix).and_then(Vec::pop);
        }
        Ok((!open.void).then_some(Token::End(open.tag)))
    }
    /// A start tag with quoted, unique attributes; `/>` implies its end tag. Its namespace
    /// decides its tag: see `element_tag`.
    fn start_tag(&mut self, after: &'a str) -> Result<Token, String> {
        let (qname, rest) = name(after).ok_or("StartTag: invalid element name")?;
        if self.elements >= ELEMENTS {
            return Err(format!(
                "Excessive depth or size in document: {ELEMENTS} elements"
            ));
        }
        self.elements += 1;
        let (written, closed, rest) = attributes(qname, rest)?;
        if self.stack.is_empty() && self.seen_root {
            return Err("Extra content at the end of the document".into());
        }
        let bindings: Vec<(String, String)> = written
            .iter()
            .filter_map(|(key, value)| {
                let prefix = if key == "xmlns" {
                    ""
                } else {
                    key.strip_prefix("xmlns:")?
                };
                Some((prefix.to_owned(), value.clone()))
            })
            .collect();
        let (prefix, local) = qname.split_once(':').unwrap_or(("", qname));
        let namespace = self.namespace(prefix, &bindings);
        if !prefix.is_empty() && namespace.is_none() {
            return Err(format!(
                "Namespace prefix {prefix} on {local} is not defined"
            ));
        }
        self.rest = rest;
        self.seen_root = true;
        let (tag, inert) = element_tag(namespace.as_deref(), qname, local);
        let mut attrs = Attributes::new();
        for (key, value) in written {
            if key != "xmlns" && !key.starts_with("xmlns:") {
                let key = if inert { key } else { key.to_ascii_lowercase() };
                attrs.entry(key).or_insert(value);
            }
        }
        let void = namespace.as_deref() == Some(XHTML_NS) && VOID.contains(&tag.as_str());
        if void || !closed {
            if !closed {
                self.open(qname, &tag, bindings, void);
            }
            return Ok(Token::Start(tag, attrs, void));
        }
        self.pending = Some(Token::End(tag.clone()));
        Ok(Token::Start(tag, attrs, false))
    }
    /** Opens an element and brings its namespace bindings into scope. */
    fn open(&mut self, qname: &str, tag: &str, bindings: Vec<(String, String)>, void: bool) {
        let mut prefixes = Vec::with_capacity(bindings.len());
        for (prefix, uri) in bindings {
            self.bound.entry(prefix.clone()).or_default().push(uri);
            prefixes.push(prefix);
        }
        self.stack.push(Scope {
            qname: qname.to_owned(),
            tag: tag.to_owned(),
            bindings: prefixes,
            void,
        });
    }
    /** The namespace a prefix (empty for the default) is bound to here, nearest binding first. */
    fn namespace(&self, prefix: &str, own: &[(String, String)]) -> Option<String> {
        if prefix == "xml" {
            return Some("http://www.w3.org/XML/1998/namespace".into());
        }
        own.iter()
            .find(|(bound, _)| bound == prefix)
            .map(|(_, uri)| uri)
            .or_else(|| self.bound.get(prefix)?.last())
            .filter(|uri| !uri.is_empty())
            .cloned()
    }
}

/// The tag the tree builder sees for an element, and whether it is inert. HTML semantics
/// (script, style, link, base, meta, forms, raw text) belong only to the XHTML namespace and
/// SVG drawing only to SVG's, both matched case-sensitively, so a `<script>` in any other
/// namespace, or none, never runs. Every other element is inert and generic, as in Blink: it
/// keeps its name as written, and an unprefixed one is marked with a leading `:` so it can never
/// equal an HTML or SVG tag. ponytail: the mark shows in tagName; a namespace field on Node
/// would let foreign elements keep their bare local names.
fn element_tag(namespace: Option<&str>, qname: &str, local: &str) -> (String, bool) {
    match namespace {
        Some(XHTML_NS) => (local.to_owned(), false),
        Some(SVG_NS) if SVG_NAMES.contains(&local) => (local.to_ascii_lowercase(), false),
        _ if qname.contains(':') => (qname.to_owned(), true),
        _ => (format!(":{qname}"), true),
    }
}

/** The attributes of a start tag after its name, whether it self-closes, and what follows it. */
type Parsed<'a> = (Vec<(String, String)>, bool, &'a str);

/** Reads attributes up to `>` or `/>`; each needs whitespace before it and a quoted value. */
fn attributes<'a>(qname: &str, mut rest: &'a str) -> Result<Parsed<'a>, String> {
    let mut written: Vec<(String, String)> = Vec::new();
    let mut seen = HashSet::new();
    loop {
        let trimmed = rest.trim_start_matches(is_space);
        let spaced = trimmed.len() < rest.len();
        rest = trimmed;
        if let Some(after) = rest.strip_prefix("/>") {
            return Ok((written, true, after));
        }
        if let Some(after) = rest.strip_prefix('>') {
            return Ok((written, false, after));
        }
        if rest.is_empty() {
            return Err(format!("Couldn't find end of Start Tag {qname}"));
        }
        let (key, after) = name(rest)
            .filter(|_| spaced)
            .ok_or("attributes construct error")?;
        let (value, after) = attribute_value(key, after)?;
        if !seen.insert(key) {
            return Err(format!("Attribute {key} redefined"));
        }
        written.push((key.to_owned(), value));
        rest = after;
    }
}

/** `= "value"` with references checked and literal whitespace normalized to spaces. */
fn attribute_value<'a>(key: &str, rest: &'a str) -> Result<(String, &'a str), String> {
    let rest = rest
        .trim_start_matches(is_space)
        .strip_prefix('=')
        .ok_or_else(|| format!("Specification mandates value for attribute {key}"))?
        .trim_start_matches(is_space);
    let quote = rest
        .chars()
        .next()
        .filter(|c| *c == '"' || *c == '\'')
        .ok_or("AttValue: \" or ' expected")?;
    let body = &rest[1..];
    let end = body.find(quote).ok_or("AttValue: ' expected")?;
    let value = &body[..end];
    if value.contains('<') {
        return Err("Unescaped '<' not allowed in attributes values".into());
    }
    references(value)?;
    let value = value.replace(['\t', '\n', '\r'], " ");
    Ok((decode(&value, true), &body[end + 1..]))
}

/// Every `&` must begin a complete reference: `&#digits;`, `&#xhex;` or `&name;`. Names are
/// resolved by the HTML table, as libxml2 does for XHTML doctypes Blink recognizes.
fn references(text: &str) -> Result<(), String> {
    for (at, _) in text.match_indices('&') {
        let rest = &text[at + 1..];
        let body = rest.find(';').map(|end| &rest[..end]);
        let valid = body.is_some_and(|body| {
            if let Some(hex) = body.strip_prefix("#x") {
                !hex.is_empty() && hex.bytes().all(|b| b.is_ascii_hexdigit())
            } else if let Some(digits) = body.strip_prefix('#') {
                !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
            } else {
                name(body).is_some_and(|(_, after)| after.is_empty())
            }
        });
        if !valid {
            return Err("xmlParseEntityRef: no name".into());
        }
    }
    Ok(())
}

/** An XML name at the start of `text`, and what follows it. */
fn name(text: &str) -> Option<(&str, &str)> {
    let first = text.chars().next()?;
    if !(first.is_alphabetic() || first == '_' || first == ':' || !first.is_ascii()) {
        return None;
    }
    let end = text
        .find(|c: char| !(c.is_alphanumeric() || "-._:".contains(c) || !c.is_ascii()))
        .unwrap_or(text.len());
    Some(text.split_at(end))
}

/** XML whitespace. */
fn is_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r')
}

/** What follows a doctype, skipping quoted literals and an internal subset in brackets. */
fn doctype_end(text: &str) -> Option<&str> {
    let mut quote = None;
    let mut depth = 0usize;
    for (at, c) in text.char_indices() {
        match (quote, c) {
            (Some(q), _) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '[') => depth += 1,
            (None, ']') => depth = depth.saturating_sub(1),
            (None, '>') if depth == 0 => return Some(&text[at + 1..]),
            _ => {}
        }
    }
    None
}

/** Which tokenizer feeds tree construction. */
pub(super) enum Tokens<'a> {
    Html(Tokenizer<'a>),
    Xml(XmlTokenizer<'a>),
}

impl Tokens<'_> {
    /** The next token from whichever tokenizer reads this document. */
    pub fn next(&mut self) -> Option<Token> {
        match self {
            Self::Html(tokens) => tokens.next(),
            Self::Xml(tokens) => tokens.next(),
        }
    }
    /** Raw text in HTML; an element's own text content in XML. */
    pub fn raw(&mut self, tag: &str) -> String {
        match self {
            Self::Html(tokens) => tokens.raw(tag),
            Self::Xml(tokens) => tokens.raw(),
        }
    }
    /** HTML reads CDATA as text only in foreign content; XML always does. */
    pub fn set_foreign(&mut self, foreign: bool) {
        if let Self::Html(tokens) = self {
            tokens.foreign = foreign;
        }
    }
    /** Whether this is an XML document, whose tree has no HTML error recovery. */
    pub fn xml(&self) -> bool {
        matches!(self, Self::Xml(_))
    }
    /** The XML parse error that stopped parsing, if any. */
    pub fn error(&self) -> Option<&str> {
        match self {
            Self::Xml(tokens) => tokens.error.as_deref(),
            Self::Html(_) => None,
        }
    }
}

/** Blink's parsererror box style (XMLErrors::InsertErrorMessageBlock). */
const ERROR_STYLE: &str = "display:block;white-space:pre;border:2px solid #c77;padding:0 1em 0 1em;margin:1em;background-color:#fdd;color:black";
/** The error message line's style in that box. */
const MESSAGE_STYLE: &str = "display:block;font-family:monospace;font-size:12px";

/// Puts Blink's parse-error block first in the page: a heading, the libxml2 message and the
/// note that what follows is the page up to the first error. Existing nodes are renumbered
/// after it, keeping preorder identities. Refuses rather than exceed the node bound.
pub(super) fn error_block(document: &mut Document, message: &str) -> EngineResult<()> {
    const BLOCK: usize = ERROR_NODES;
    if document.nodes.len() + BLOCK > NODES {
        return Err(EngineError::limit("document nodes", NODES));
    }
    for node in &mut document.nodes {
        if node.html_id == format!("@{}", node.local_id) {
            node.html_id = format!("@{}", node.local_id + BLOCK);
        }
        node.local_id += BLOCK;
        node.parent = node.parent.map(|parent| parent + BLOCK);
    }
    let parts = [
        ("div", None, ERROR_STYLE, ""),
        (
            "h3",
            Some(0),
            "",
            "This page contains the following errors:",
        ),
        ("div", Some(0), MESSAGE_STYLE, message),
        (
            "h3",
            Some(0),
            "",
            "Below is a rendering of the page up to the first error.",
        ),
    ];
    let mut block = Vec::with_capacity(BLOCK);
    for (id, (tag, parent, style, text)) in parts.into_iter().enumerate() {
        let attrs = Attributes::from([("style".to_owned(), style.to_owned())]);
        let attrs = if style.is_empty() {
            Attributes::new()
        } else {
            attrs
        };
        let mut node = markup::node(tag, id, parent, attrs)?;
        node.text = text.to_owned();
        block.push(node);
    }
    block.append(&mut document.nodes);
    document.nodes = block;
    Ok(())
}
