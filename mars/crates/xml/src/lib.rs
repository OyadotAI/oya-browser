/**
 * A namespace-aware XML 1.0 well-formedness parser for V9's XML documents: an iframe whose
 * source is XML, XHTML or SVG gets a real document built from these events. The parser reads the
 * source once and returns its events in document order (an element's start with its resolved
 * name and attributes, its end, its text and its comments), so the caller builds the tree with
 * a stack and nothing here recurses.
 *
 * Eye's XML parser (crates/eye/src/document/xml.rs) stays where it is: it is welded to Eye's HTML
 * tokenizer, entity table and parse-error block, and it builds Eye's nodes for painting. This
 * crate has no dependencies and no page state, so V9 and any other engine can reach it.
 *
 * Ceilings: a DTD is skipped rather than read, so entities it declares are undefined; names accept
 * any non-ASCII character; processing instructions are dropped; nesting is capped at MAX_DEPTH.
 */
mod parse;
mod scan;
mod scopes;

pub use parse::parse;

/// The namespace the `xml` prefix is always bound to.
pub const XML_NAMESPACE: &str = "http://www.w3.org/XML/1998/namespace";
/// The namespace of `xmlns` declarations, which appear among an element's attributes.
pub const XMLNS_NAMESPACE: &str = "http://www.w3.org/2000/xmlns/";
/// Open elements one document may nest. A DOM tree this deep would overflow the recursive
/// walks that style and print it, so a deeper document is refused as not well-formed.
pub const MAX_DEPTH: usize = 512;

/** An element or attribute name after namespace resolution. */
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Name {
    /// The namespace URI, or None for the null namespace.
    pub namespace: Option<String>,
    /// The prefix as written, when there was one.
    pub prefix: Option<String>,
    /// The local name, after the colon.
    pub local: String,
}

impl Name {
    /** The name as written: the prefix, a colon and the local name, or the local name alone. */
    pub fn qualified(&self) -> String {
        match &self.prefix {
            Some(prefix) => format!("{prefix}:{}", self.local),
            None => self.local.clone(),
        }
    }
}

/** One attribute of a start tag, with its value decoded and white space normalized. */
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attribute {
    /// The attribute's resolved name.
    pub name: Name,
    /// The decoded value.
    pub value: String,
}

/** One step of a document in document order. */
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// An element starts; its attributes are in document order.
    Start {
        /// The element's resolved name.
        name: Name,
        /// Its attributes, namespace declarations included.
        attributes: Vec<Attribute>,
    },
    /// The innermost open element ends.
    End,
    /// Character data, CDATA included, with references decoded. Adjacent runs are joined.
    Text(String),
    /// A comment's text, without its delimiters.
    Comment(String),
}

/** The first well-formedness error, with the line it was found on. Parsing stops there. */
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    /// What is wrong, prefixed with the line number.
    pub message: String,
}
