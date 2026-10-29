/**
 * The XML reader: one pass over a document's source that checks well-formedness and emits its
 * events. Start tags, end tags, comments, CDATA sections and references are read in the order the
 * grammar gives; the first violation stops the pass with its line. Processing instructions and
 * the document type are read and dropped.
 */
use crate::scan::{decode, is_name_char, is_name_start, is_space, normalize_newlines};
use crate::scopes::Scopes;
use crate::{Attribute, Error, Event, MAX_DEPTH, Name, XML_NAMESPACE, XMLNS_NAMESPACE};
use std::collections::HashSet;

/** A start tag's attributes as written: the name as sliced from the input, the value decoded. */
type Attributes<'a> = Vec<(&'a str, String)>;

/** Parse a whole document into its events, or the first well-formedness error. A byte order
mark is skipped, and line endings are normalized before anything is read. */
pub fn parse(source: &str) -> Result<Vec<Event>, Error> {
    let text = normalize_newlines(source);
    let body = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let mut parser = Parser {
        reader: Reader { src: body, pos: 0 },
        events: Vec::new(),
        open: Vec::new(),
        scopes: Scopes::new(),
    };
    parser.document()?;
    Ok(parser.events)
}

/** A position in the source and the reads that move it. Every move lands on a character boundary. */
struct Reader<'a> {
    /** The normalized source. */
    src: &'a str,
    /** The byte offset read so far. */
    pos: usize,
}

impl<'a> Reader<'a> {
    /** The unread source. */
    fn rest(&self) -> &'a str {
        self.src.get(self.pos..).unwrap_or("")
    }
    /** Whether the unread source starts with `text`. */
    fn starts(&self, text: &str) -> bool {
        self.rest().starts_with(text)
    }
    /** Consume `text` when the source starts with it. */
    fn eat(&mut self, text: &str) -> bool {
        let found = self.starts(text);
        if found {
            self.pos += text.len();
        }
        found
    }
    /** Consume the white space at the position, reporting whether there was any. */
    fn skip_space(&mut self) -> bool {
        let bytes = self.rest().as_bytes();
        let count = bytes.iter().take_while(|byte| is_space(**byte)).count();
        self.pos += count;
        count > 0
    }
    /** The byte at the position, if any. */
    fn peek(&self) -> Option<u8> {
        self.rest().as_bytes().first().copied()
    }
    /** Read a name: it must start one, and it runs while name characters do. */
    fn name(&mut self) -> Result<&'a str, Error> {
        let bytes = self.rest().as_bytes();
        if !bytes.first().is_some_and(|b| is_name_start(*b)) {
            return Err(self.fail("expected a name"));
        }
        let len = bytes.iter().take_while(|b| is_name_char(**b)).count();
        let name = &self.rest()[..len];
        self.pos += len;
        Ok(name)
    }
    /** Read up to and past `end`, returning what came before it. */
    fn until(&mut self, end: &str, what: &str) -> Result<&'a str, Error> {
        let rest = self.rest();
        let at = rest.find(end).ok_or_else(|| self.fail(what))?;
        self.pos += at + end.len();
        Ok(&rest[..at])
    }
    /** Read a quoted literal, returning its body; the quote must close before the source ends. */
    fn quoted(&mut self) -> Result<&'a str, Error> {
        let end = match self.peek() {
            Some(b'"') => "\"",
            Some(b'\'') => "'",
            _ => return Err(self.fail("expected a quoted value")),
        };
        self.pos += 1;
        self.until(end, "unterminated quoted value")
    }
    /** An error at the current line, for the message the caller returns. */
    fn fail(&self, what: &str) -> Error {
        let line = self.src.as_bytes()[..self.pos.min(self.src.len())]
            .iter()
            .filter(|byte| **byte == b'\n')
            .count()
            + 1;
        Error {
            message: format!("line {line}: {what}"),
        }
    }
}

/** An open element: its name as written and the prefixes its start tag bound. */
struct Open<'a> {
    /** The qualified name as the start tag wrote it; the end tag must match. */
    qualified: &'a str,
    /** The prefixes this element bound, unbound again when it ends. */
    bound: Vec<&'a str>,
}

/** The reader with its output and the element and namespace stacks. */
struct Parser<'a> {
    reader: Reader<'a>,
    events: Vec<Event>,
    open: Vec<Open<'a>>,
    scopes: Scopes,
}

impl<'a> Parser<'a> {
    /** The whole document: the prolog, the root element, then only misc after it. */
    fn document(&mut self) -> Result<(), Error> {
        self.misc(true)?;
        if !self.reader.starts("<") {
            return Err(self.reader.fail("the document has no root element"));
        }
        self.start_tag()?;
        while !self.open.is_empty() {
            self.content()?;
        }
        self.misc(false)?;
        if !self.reader.rest().is_empty() {
            return Err(self.reader.fail("content follows the root element"));
        }
        Ok(())
    }
    /** Comments, processing instructions and white space; the document type once, if allowed. */
    fn misc(&mut self, mut doctype: bool) -> Result<(), Error> {
        loop {
            self.reader.skip_space();
            if self.reader.starts("<!--") {
                self.comment()?;
            } else if self.reader.starts("<?") {
                self.processing_instruction()?;
            } else if doctype && self.reader.starts("<!DOCTYPE") {
                self.doctype()?;
                doctype = false;
            } else {
                return Ok(());
            }
        }
    }
    /** One item of element content: a tag, a comment, CDATA, a processing instruction or text. */
    fn content(&mut self) -> Result<(), Error> {
        if self.reader.rest().is_empty() {
            return Err(self.reader.fail("an element is not closed"));
        }
        if self.reader.starts("</") {
            self.end_tag()
        } else if self.reader.starts("<!--") {
            self.comment()
        } else if self.reader.starts("<![CDATA[") {
            self.cdata()
        } else if self.reader.starts("<?") {
            self.processing_instruction()
        } else if self.reader.starts("<") {
            self.start_tag()
        } else {
            self.text()
        }
    }
    /** Character data up to the next `<`, with references decoded. `]]>` may not appear in it. */
    fn text(&mut self) -> Result<(), Error> {
        let rest = self.reader.rest();
        let len = rest.find('<').unwrap_or(rest.len());
        let raw = &rest[..len];
        if raw.contains("]]>") {
            return Err(self.reader.fail("']]>' is not allowed in character data"));
        }
        let text = decode(raw).map_err(|message| self.reader.fail(&message))?;
        self.reader.pos += len;
        self.push_text(text);
        Ok(())
    }
    /** A CDATA section: its body is text as written, with no references. */
    fn cdata(&mut self) -> Result<(), Error> {
        self.reader.eat("<![CDATA[");
        let body = self.reader.until("]]>", "unterminated CDATA section")?;
        self.push_text(body.to_string());
        Ok(())
    }
    /** Append text, joining it to a text event that directly precedes it. */
    fn push_text(&mut self, text: String) {
        if let Some(Event::Text(last)) = self.events.last_mut() {
            last.push_str(&text);
        } else {
            self.events.push(Event::Text(text));
        }
    }
    /** A comment; its body may not contain `--`. */
    fn comment(&mut self) -> Result<(), Error> {
        self.reader.eat("<!--");
        let body = self.reader.until("-->", "unterminated comment")?;
        if body.contains("--") {
            return Err(self.reader.fail("'--' is not allowed in a comment"));
        }
        self.events.push(Event::Comment(body.to_string()));
        Ok(())
    }
    /** A processing instruction, read past and dropped. */
    fn processing_instruction(&mut self) -> Result<(), Error> {
        self.reader.eat("<?");
        self.reader.name()?;
        self.reader
            .until("?>", "unterminated processing instruction")?;
        Ok(())
    }
    /** The document type declaration, read past and dropped, including an internal subset. */
    fn doctype(&mut self) -> Result<(), Error> {
        self.reader.eat("<!DOCTYPE");
        let rest = self.reader.rest();
        let mut from = 0;
        if let Some(open) = rest.find('[')
            && rest.find('>').is_none_or(|gt| open < gt)
        {
            let close = rest[open..]
                .find(']')
                .ok_or_else(|| self.reader.fail("unterminated DOCTYPE"))?;
            from = open + close + 1;
        }
        let gt = rest[from..]
            .find('>')
            .ok_or_else(|| self.reader.fail("unterminated DOCTYPE"))?;
        self.reader.pos += from + gt + 1;
        Ok(())
    }
    /** A start tag, or an empty-element tag, which also ends at once. */
    fn start_tag(&mut self) -> Result<(), Error> {
        self.reader.eat("<");
        let qualified = self.reader.name()?;
        if self.open.len() >= MAX_DEPTH {
            return Err(self.reader.fail("elements are nested too deeply"));
        }
        let (raw, empty) = self.attributes()?;
        let bound = self.declare(&raw)?;
        let name = self.element_name(qualified)?;
        let attributes = self.attribute_names(&raw)?;
        self.events.push(Event::Start { name, attributes });
        if empty {
            self.events.push(Event::End);
            self.unbind(&bound);
        } else {
            self.open.push(Open { qualified, bound });
        }
        Ok(())
    }
    /** The attributes of a start tag as written, and whether the tag closes itself. */
    fn attributes(&mut self) -> Result<(Attributes<'a>, bool), Error> {
        let mut raw = Vec::new();
        loop {
            let spaced = self.reader.skip_space();
            if self.reader.eat("/>") {
                return Ok((raw, true));
            }
            if self.reader.eat(">") {
                return Ok((raw, false));
            }
            if !spaced {
                return Err(self.reader.fail("expected white space before an attribute"));
            }
            let name = self.reader.name()?;
            self.reader.skip_space();
            if !self.reader.eat("=") {
                return Err(self.reader.fail("expected '=' after an attribute name"));
            }
            self.reader.skip_space();
            raw.push((name, self.attribute_value()?));
        }
    }
    /** An attribute value: no `<`, white space characters become spaces, references decode. */
    fn attribute_value(&mut self) -> Result<String, Error> {
        let raw = self.reader.quoted()?;
        if raw.contains('<') {
            return Err(self.reader.fail("'<' is not allowed in an attribute value"));
        }
        let spaced = raw.replace(['\t', '\r', '\n'], " ");
        decode(&spaced).map_err(|message| self.reader.fail(&message))
    }
    /** Bind the namespace declarations among `attributes`, returning the prefixes bound. */
    fn declare(&mut self, attributes: &[(&'a str, String)]) -> Result<Vec<&'a str>, Error> {
        let mut bound = Vec::new();
        for (name, value) in attributes {
            let name: &'a str = name;
            let Some(prefix) = declared_prefix(name) else {
                continue;
            };
            if prefix.is_empty() && name != "xmlns" {
                return Err(self.reader.fail("an empty namespace prefix"));
            }
            if prefix == "xml" {
                continue;
            }
            if !prefix.is_empty() && value.is_empty() {
                return Err(self.reader.fail("a namespace prefix cannot be undeclared"));
            }
            let uri = (!value.is_empty()).then(|| value.clone());
            self.scopes.bind(prefix, uri);
            bound.push(prefix);
        }
        Ok(bound)
    }
    /** Pop the bindings an element pushed. */
    fn unbind(&mut self, prefixes: &[&str]) {
        for prefix in prefixes {
            self.scopes.unbind(prefix);
        }
    }
    /** The resolved name of an element: its prefix's namespace, or the default one. */
    fn element_name(&self, qualified: &str) -> Result<Name, Error> {
        let (prefix, local) = split_qualified(qualified).map_err(|m| self.reader.fail(m))?;
        let namespace = match prefix {
            None => self.scopes.default_namespace(),
            Some("xmlns") => return Err(self.reader.fail("the xmlns prefix is reserved")),
            Some(prefix) => Some(
                self.scopes
                    .prefixed(prefix)
                    .ok_or_else(|| self.reader.fail("an element prefix is not bound"))?,
            ),
        };
        Ok(Name {
            namespace,
            prefix: prefix.map(str::to_string),
            local: local.to_string(),
        })
    }
    /** The resolved attributes of a start tag. Two attributes with one expanded name are an error. */
    fn attribute_names(&self, raw: &[(&str, String)]) -> Result<Vec<Attribute>, Error> {
        let mut seen = HashSet::new();
        let mut out = Vec::with_capacity(raw.len());
        for (qualified, value) in raw {
            let name = self.attribute_name(qualified)?;
            if !seen.insert((name.namespace.clone(), name.local.clone())) {
                return Err(self.reader.fail("an attribute appears twice"));
            }
            out.push(Attribute {
                name,
                value: value.clone(),
            });
        }
        Ok(out)
    }
    /** The resolved name of one attribute. Declarations live in the xmlns namespace; unprefixed
    attributes have no namespace; the `xml` prefix is always bound. */
    fn attribute_name(&self, qualified: &str) -> Result<Name, Error> {
        if qualified == "xmlns" {
            return Ok(declaration(None, "xmlns"));
        }
        if let Some(prefix) = qualified.strip_prefix("xmlns:") {
            return Ok(declaration(Some("xmlns"), prefix));
        }
        let (prefix, local) = split_qualified(qualified).map_err(|m| self.reader.fail(m))?;
        let namespace = match prefix {
            None => None,
            Some("xml") => Some(XML_NAMESPACE.to_string()),
            Some(prefix) => Some(
                self.scopes
                    .prefixed(prefix)
                    .ok_or_else(|| self.reader.fail("an attribute prefix is not bound"))?,
            ),
        };
        Ok(Name {
            namespace,
            prefix: prefix.map(str::to_string),
            local: local.to_string(),
        })
    }
    /** An end tag: it must close the innermost open element. */
    fn end_tag(&mut self) -> Result<(), Error> {
        self.reader.eat("</");
        let name = self.reader.name()?;
        self.reader.skip_space();
        if !self.reader.eat(">") {
            return Err(self.reader.fail("expected '>' to end an end tag"));
        }
        let open = self
            .open
            .pop()
            .ok_or_else(|| self.reader.fail("an end tag has no start"))?;
        if open.qualified != name {
            return Err(self.reader.fail("an end tag does not match its start tag"));
        }
        self.unbind(&open.bound);
        self.events.push(Event::End);
        Ok(())
    }
}

/** The resolved name of a namespace declaration attribute. */
fn declaration(prefix: Option<&str>, local: &str) -> Name {
    Name {
        namespace: Some(XMLNS_NAMESPACE.to_string()),
        prefix: prefix.map(str::to_string),
        local: local.to_string(),
    }
}

/** A name split at its colon into prefix and local name. A name with an empty part or a second
colon is not a qualified name. */
fn split_qualified(name: &str) -> Result<(Option<&str>, &str), &'static str> {
    match name.split_once(':') {
        None => Ok((None, name)),
        Some((prefix, local))
            if !prefix.is_empty() && !local.is_empty() && !local.contains(':') =>
        {
            Ok((Some(prefix), local))
        }
        Some(_) => Err("a qualified name is malformed"),
    }
}

/** The prefix a namespace declaration binds: empty for `xmlns`, the text after `xmlns:` for a
prefixed one, and None for any other attribute. */
fn declared_prefix(name: &str) -> Option<&str> {
    if name == "xmlns" {
        return Some("");
    }
    name.strip_prefix("xmlns:")
}
