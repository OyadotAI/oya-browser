/** Owned bounded tree construction with the HTML Living Standard's error recovery: implied end
tags, misnested and stray tags, table repair and embedded-content fallbacks keep the page instead
of refusing it. Only hard bounds, script policy and charset conflicts still refuse. */
use super::{
    Document, ScriptKind, ScriptMode, ScriptResource, StyleSource, markup,
    recovery::{
        CLOSES_P, DEFAULT_SCOPE, EMBEDDED, HEADINGS, MEDIA_RESOURCES, SCOPED_END, TABLE_CONTENT,
        TABLE_CONTEXT, TABLE_SCOPE, VOID, in_scope, list_item, ordinary_end,
    },
    tokenizer::{Token, Tokenizer},
    xml::{Tokens, XmlTokenizer},
};
use crate::constants::{
    DIAGNOSTICS, DOCUMENT_DEPTH, FRAGMENT_BYTES, HTML_BYTES, METADATA_BYTES, METADATA_ELEMENTS,
    NODES, TEXT_BYTES,
};
use crate::protocol::{EngineError, EngineResult};
use mars_core::dom::MetadataElement;
use mars_core::http::{UTF_8, WINDOWS_1252, encoding_for_label};

/** Table structure start tags HTML ignores outside a table. */
const STRAY_TABLE_PARTS: [&str; 9] = [
    "caption", "col", "colgroup", "tbody", "td", "tfoot", "th", "thead", "tr",
];

/// Complete candidates retain stable preorder identities; unsupported markup falls back.
/// The transport charset is undeclared here, so a meta charset must agree with UTF-8 decoding.
pub fn parse(source: &str) -> EngineResult<Document> {
    parse_with_charset(source, None)
}

/// Parses a page decoded with `charset`, the canonical encoding the transport declared
/// (`mars_core::http` names), or None when it declared none (UTF-8 decoding). A meta charset
/// that reads these bytes differently from the decoding used is an explicit refusal instead
/// of silently misread text.
pub fn parse_with_charset(source: &str, charset: Option<&str>) -> EngineResult<Document> {
    if source.len() > HTML_BYTES {
        return Err(EngineError::limit("HTML bytes", HTML_BYTES));
    }
    let mut tree = Tree::new(source, 0, false);
    tree.decoded = charset.map_or(Some(UTF_8), encoding_for_label);
    tree.run()?;
    tree.document.formatting.quirks = super::quirks::quirks(source);
    finish_page(tree)
}

/// Parses an XML document (XHTML or SVG, chosen by `xml_mime`) into the same Document the HTML
/// parser builds: well-formed XML, no HTML error recovery, never quirks. The first
/// well-formedness error ends parsing with Blink's parse-error block above what was built.
pub fn parse_xml(source: &str, charset: Option<&str>) -> EngineResult<Document> {
    if source.len() > HTML_BYTES {
        return Err(EngineError::limit("HTML bytes", HTML_BYTES));
    }
    let mut tree = Tree::new(source, 0, false);
    tree.tokens = Tokens::Xml(XmlTokenizer::new(source));
    tree.decoded = charset.map_or(Some(UTF_8), encoding_for_label);
    tree.run()?;
    if let Some(message) = tree.tokens.error().map(str::to_owned) {
        super::xml::error_block(&mut tree.document, &message)?;
    }
    finish_page(tree)
}

/** The page passes both parsers share: content bounds, forms and tables. */
fn finish_page(mut tree: Tree) -> EngineResult<Document> {
    super::validate_content(&tree.document)?;
    super::forms::normalize(&mut tree.document)?;
    super::table::annotate(&mut tree.document)?;
    super::validate_content(&tree.document)?;
    Ok(tree.document)
}

/// Script-inserted markup: preorder nodes numbered from `first_id` plus any inline style sheets.
#[derive(Clone, Debug)]
pub struct Fragment {
    /// Top-level nodes have parent None; the caller places them with the shared core helper.
    pub nodes: Vec<super::Node>,
    /// Inline `<style>` sources, in fragment order, appended to the document's sheets.
    pub styles: Vec<StyleSource>,
}

/// Parses script-inserted markup (innerHTML, insertAdjacentHTML, an appended element) with the
/// page parser and the page's fallbacks: frames and media become inert placeholders, unknown
/// elements stay containers, inline handlers are stripped and inserted scripts never run. Only
/// raw-text contexts and the hard bounds refuse. Forms and tables are normalized as a page's are.
pub fn parse_fragment(context: &str, html: &str, first_id: usize) -> EngineResult<Fragment> {
    if html.len() > FRAGMENT_BYTES {
        return Err(EngineError::limit("fragment bytes", FRAGMENT_BYTES));
    }
    if CONTEXT_REFUSED.contains(&context) {
        return Err(EngineError::unsupported(
            "Inserting markup into this element is not supported.",
        ));
    }
    if first_id >= NODES {
        return Err(EngineError::limit("document nodes", NODES));
    }
    let mut tree = Tree::new(html, first_id, true);
    tree.foreign_context = context == "svg" || SVG_CHILDREN.contains(&context);
    tree.run()?;
    super::validate_content(&tree.document)?;
    let mut document = tree.document;
    rebase(&mut document.nodes, first_id, false);
    super::forms::normalize(&mut document)?;
    super::table::annotate(&mut document)?;
    rebase(&mut document.nodes, first_id, true);
    Ok(Fragment {
        nodes: document.nodes,
        styles: document.styles,
    })
}

/// Shifts fragment identities between `first_id`-based and zero-based numbering, so the page's
/// form and table passes, which index nodes by identity, run on a fragment unchanged.
fn rebase(nodes: &mut [super::Node], first_id: usize, up: bool) {
    let shift = |id: &mut usize| *id = if up { *id + first_id } else { *id - first_id };
    for node in nodes {
        shift(&mut node.local_id);
        if let Some(parent) = node.parent.as_mut() {
            shift(parent);
        }
        if let Some(cell) = node.table_cell.as_mut() {
            shift(&mut cell.table);
            shift(&mut cell.row);
            cell.headers.iter_mut().for_each(shift);
        }
    }
}

/** Raw-text contexts hold text, never markup, so inserting markup into them is refused. */
const CONTEXT_REFUSED: [&str; 9] = [
    "script",
    "style",
    "textarea",
    "title",
    "xmp",
    "iframe",
    "noembed",
    "noframes",
    "plaintext",
];
/** Only this drawing subset may appear, and only inside an `svg` element. */
const SVG_CHILDREN: [&str; 18] = [
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
    "lineargradient",
    "radialgradient",
    "stop",
    "clippath",
    "mask",
    "pattern",
    "marker",
];
/** Start tags that break out of foreign content (the tree builder's "any other start tag" list
in the rules for parsing tokens in foreign content): they pop SVG and MathML elements until an
HTML element or integration point is current, then go through the HTML rules. */
const FOREIGN_BREAKOUT: [&str; 45] = [
    "b",
    "big",
    "blockquote",
    "body",
    "br",
    "center",
    "code",
    "dd",
    "div",
    "dl",
    "dt",
    "em",
    "embed",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "head",
    "hr",
    "i",
    "img",
    "li",
    "listing",
    "menu",
    "meta",
    "nobr",
    "ol",
    "p",
    "pre",
    "ruby",
    "s",
    "small",
    "span",
    "strong",
    "strike",
    "sub",
    "sup",
    "table",
    "tt",
    "u",
    "ul",
    "var",
    "font",
];
/** MathML text integration points: their children are HTML, except mglyph and malignmark. */
const MATH_TEXT_POINTS: [&str; 5] = ["mi", "mo", "mn", "ms", "mtext"];
/** SVG HTML integration points: their children are HTML. */
const SVG_HTML_POINTS: [&str; 3] = ["foreignobject", "desc", "title"];

/** The namespace an open element was created in, as the tree builder tracks it. */
#[derive(Clone, Copy, PartialEq)]
enum Namespace {
    Html,
    Svg,
    MathMl,
}

/** Elements that may stay in the head; any other start tag implies the body. */
const HEAD_CONTENT: [&str; 9] = [
    "title", "style", "link", "meta", "script", "base", "noscript", "template", "head",
];
/** Raw-text elements whose content is never rendered: scripting is on, so noscript is inert. */
const INERT_RAW: [&str; 4] = ["noscript", "noembed", "noframes", "iframe"];
/** Placeholder width and height for frames, video, canvas and objects without dimensions, in CSS px. */
const EMBED_SIZE: (u32, u32) = (300, 150);
/** Height of an audio element showing its controls, in CSS px. */
const AUDIO_CONTROLS_HEIGHT: u32 = 54;
/** One line of placeholder label at the default font size, subtracted from the padded height. */
const PLACEHOLDER_LINE: u32 = 24;
/** The largest author dimension a placeholder honors, so a hostile size cannot exceed layout bounds. */
const PLACEHOLDER_MAX: u32 = 4096;

/** Optional closing recovery stays within the existing recursion budget. */
struct Tree<'a> {
    tokens: Tokens<'a>,
    document: Document,
    /** Vec indexes of open elements; identities are `base` plus the index. */
    stack: Vec<usize>,
    head: bool,
    /** Stored static bytes are admitted before appending to any node. */
    text_bytes: usize,
    /** The first allocated identity: zero for pages, the change's `first_id` for fragments. */
    base: usize,
    /** Fragment mode refuses everything outside the inert insertion allowlist. */
    fragment: bool,
    /** Canonical encoding the source was decoded with; None when the transport named an unknown one. */
    decoded: Option<&'static str>,
    /** Pure-ASCII sources read identically as UTF-8 and windows-1252. */
    ascii: bool,
    /** Nested form start tags ignored as HTML ignores them, so their end tags are ignored too. */
    ignored_forms: usize,
    /** A fragment inserted into an SVG drawing parses as if an `svg` were open. */
    foreign_context: bool,
    /** An absolute `<base href>` that relative link, image, form and resource addresses resolve against. */
    base_url: Option<url::Url>,
    /** Attribute and text bytes the page's metadata elements hold, bounded by METADATA_BYTES. */
    metadata_bytes: usize,
    /** Set just after a `<pre>` or `<listing>` start tag: a newline right after it is dropped,
    as HTML's in-body mode ignores the next LF token there. */
    after_pre: bool,
}
impl<'a> Tree<'a> {
    /** Pages and fragments share one builder; only the base identity and mode differ. */
    fn new(source: &'a str, base: usize, fragment: bool) -> Self {
        Self {
            tokens: Tokens::Html(Tokenizer::new(source)),
            document: Document {
                nodes: Vec::new(),
                scripts: Vec::new(),
                title: String::new(),
                styles: Vec::new(),
                diagnostics: Vec::new(),
                formatting: Default::default(),
                csp: Vec::new(),
                metadata: Vec::new(),
            },
            stack: Vec::new(),
            head: false,
            text_bytes: 0,
            base,
            fragment,
            decoded: Some(UTF_8),
            ascii: source.is_ascii(),
            ignored_forms: 0,
            foreign_context: false,
            base_url: None,
            metadata_bytes: 0,
            after_pre: false,
        }
    }
    /** Every token is consumed, then every still-open element closes at end of input. */
    fn run(&mut self) -> EngineResult<()> {
        loop {
            let foreign = self.in_svg()
                || self
                    .current_namespace()
                    .is_some_and(|(_, namespace)| namespace != Namespace::Html);
            self.tokens.set_foreign(foreign);
            let Some(token) = self.tokens.next() else {
                break;
            };
            self.consume(token)?;
        }
        self.close(0);
        Ok(())
    }
    /** Identity of the open element at a stack position. */
    fn local(&self, index: usize) -> usize {
        self.document.nodes[index].local_id
    }
    /** Open element tags paired with their stack positions, for the scope searches. */
    fn open(&self) -> impl DoubleEndedIterator<Item = (usize, &str)> {
        self.stack
            .iter()
            .enumerate()
            .map(|(position, index)| (position, self.document.nodes[*index].tag.as_str()))
    }
    /** The current node's tag, or the empty string at the top level. */
    fn current(&self) -> &str {
        self.stack
            .last()
            .map_or("", |index| self.document.nodes[*index].tag.as_str())
    }
    /** SVG drawing elements exist only beneath an open `svg`. */
    fn in_svg(&self) -> bool {
        self.foreign_context
            || self
                .stack
                .iter()
                .any(|index| self.document.nodes[*index].tag == "svg")
    }
    /** The namespace a `tag` start token creates its element in under the open element at
    stack position `parent` (None: the top level, or a fragment's context): HTMLTreeBuilder's
    integration points hand their children back to HTML, and `svg` and `math` open their own. */
    fn child_namespace(&self, parent: Option<(usize, Namespace)>, tag: &str) -> Namespace {
        let inherited = match parent {
            None if self.foreign_context => Namespace::Svg,
            None => Namespace::Html,
            Some((position, namespace)) => {
                let node = &self.document.nodes[self.stack[position]];
                let point = match namespace {
                    Namespace::Html => true,
                    Namespace::Svg => SVG_HTML_POINTS.contains(&node.tag.as_str()),
                    Namespace::MathMl => {
                        (MATH_TEXT_POINTS.contains(&node.tag.as_str())
                            && !["mglyph", "malignmark"].contains(&tag))
                            || (node.tag == "annotation-xml"
                                && (tag == "svg" || html_annotation(node)))
                    }
                };
                if point { Namespace::Html } else { namespace }
            }
        };
        match (inherited, tag) {
            (Namespace::Html, "svg") => Namespace::Svg,
            (Namespace::Html, "math") => Namespace::MathMl,
            (namespace, _) => namespace,
        }
    }
    /** Each open element's namespace, from the root of the stack down. */
    fn namespaces(&self) -> Vec<Namespace> {
        let mut out: Vec<Namespace> = Vec::with_capacity(self.stack.len());
        for (position, index) in self.stack.iter().enumerate() {
            let parent = position.checked_sub(1).map(|p| (p, out[p]));
            out.push(self.child_namespace(parent, &self.document.nodes[*index].tag));
        }
        out
    }
    /** The current node and its namespace, when any SVG or MathML element is open; None means
    the current node is HTML (or nothing is open outside a foreign fragment context). */
    fn current_namespace(&self) -> Option<(usize, Namespace)> {
        let foreign = self.stack.iter().any(|index| {
            let tag = self.document.nodes[*index].tag.as_str();
            tag == "svg" || tag == "math"
        });
        if !foreign {
            return None;
        }
        let namespaces = self.namespaces();
        namespaces.last().map(|ns| (namespaces.len() - 1, *ns))
    }
    /// The foreign-content breakout: an HTML start tag (or a `font` with color, face or size)
    /// whose current node is SVG or MathML and no integration point pops open elements until
    /// an HTML element or integration point is current, so `<svg><p>` and `<math><mrow><p>`
    /// put the paragraph beside the drawing instead of losing it inside one.
    fn foreign_breakout(&mut self, tag: &str, attrs: &markup::Attributes) {
        if self.tokens.xml()
            || !FOREIGN_BREAKOUT.contains(&tag)
            || (tag == "font"
                && !["color", "face", "size"]
                    .iter()
                    .any(|a| attrs.contains_key(*a)))
        {
            return;
        }
        let Some(current) = self.current_namespace() else {
            return;
        };
        if self.child_namespace(Some(current), "div") == Namespace::Html {
            return;
        }
        self.diagnose("An HTML start tag inside SVG or MathML ended the drawing or formula.");
        let namespaces = self.namespaces();
        let keep = (0..namespaces.len())
            .rev()
            .find(|p| self.child_namespace(Some((*p, namespaces[*p])), "div") == Namespace::Html)
            .map_or(0, |p| p + 1);
        self.close(keep);
    }
    /** One value-free, deduplicated note, bounded like the cascade's diagnostics. */
    fn diagnose(&mut self, message: impl Into<String>) {
        let message = message.into();
        let diagnostics = &mut self.document.diagnostics;
        if diagnostics.len() < DIAGNOSTICS && !diagnostics.contains(&message) {
            diagnostics.push(message);
        }
    }
    /** Wrapper markup has no extra display node. */
    fn consume(&mut self, mut token: Token) -> EngineResult<()> {
        // Only HTML drops it: XML keeps every character (white-space-pre-001's leading line).
        if std::mem::take(&mut self.after_pre)
            && !self.tokens.xml()
            && let Token::Text(text) = &mut token
            && text.starts_with('\n')
        {
            text.remove(0);
            if text.is_empty() {
                return Ok(());
            }
        }
        match token {
            Token::Start(tag, attrs, closed) => self.start(tag, attrs, closed),
            Token::End(tag) => self.end(&tag),
            Token::Text(text) => self.text(text),
            Token::Comment(data) => self.comment(data),
        }
    }
    /// A comment inside an open element becomes a `#comment` node whose value is its data (the
    /// DOM's nodeValue), so scripts read it through childNodes: LinkedIn embeds configuration
    /// JSON as `<code id=..><!--json--></code>`. It has no text, so nothing paints, labels or
    /// finds it. Script-inserted fragments keep their comments, top level included, because
    /// component libraries insert comment markers and position later insertions against them;
    /// V9's own fragment reader keeps them the same way. The head, SVG drawings, oversized
    /// comments and a page already at the node bound drop them.
    /// ponytail: comments directly under body, or outside html, are dropped because Eye has no
    /// body node to tell them apart; keep body-level ones if a page reads them.
    fn comment(&mut self, data: String) -> EngineResult<()> {
        let local = self.base + self.document.nodes.len();
        let parent = self.stack.last().map(|index| self.local(*index));
        if parent.is_none() && !self.fragment {
            return Ok(());
        }
        if self.head || self.in_svg() || data.len() > TEXT_BYTES || local >= NODES {
            return Ok(());
        }
        let mut node = markup::node("#comment", local, parent, Default::default())?;
        node.value = data;
        self.document.nodes.push(node);
        Ok(())
    }
    /** End tags close their element in the scope HTML gives it; unmatched ones are ignored. */
    fn end(&mut self, tag: &str) -> EngineResult<()> {
        match tag {
            "head" => self.head = false,
            // A declarative shadow root's `</template>` closes it; templates themselves never stay open.
            "template" => {
                let root = self
                    .open()
                    .rev()
                    .find(|(_, open)| *open == super::shadow::SHADOW_ROOT)
                    .map(|(position, _)| position);
                if let Some(position) = root {
                    self.close(position);
                }
            }
            "body" | "html" => {}
            "br" if !self.tokens.xml() => {
                return self.start("br".into(), Default::default(), true);
            }
            "form" if self.ignored_forms > 0 => self.ignored_forms -= 1,
            // XML end tags always name the current element; one the builder never opened
            // (flattened past the depth bound, or skipped) closes nothing.
            _ if self.tokens.xml() => {
                if self.current() == tag {
                    self.close(self.stack.len() - 1);
                }
            }
            _ => {
                if let Some(position) = self.end_position(tag) {
                    self.close(position);
                }
            }
        }
        Ok(())
    }
    /** Which open element an end tag closes, if any. */
    fn end_position(&self, tag: &str) -> Option<usize> {
        let (targets, boundaries): (&[&str], Vec<&str>) = match tag {
            "p" => (&["p"], [&DEFAULT_SCOPE[..], &["button"]].concat()),
            "li" => (&["li"], [&DEFAULT_SCOPE[..], &["ol", "ul"]].concat()),
            "td" | "th" | "tr" | "tbody" | "thead" | "tfoot" | "table" => {
                (&[tag][..], TABLE_SCOPE.to_vec())
            }
            _ if HEADINGS.contains(&tag) => (&HEADINGS[..], DEFAULT_SCOPE.to_vec()),
            _ if SCOPED_END.contains(&tag) => (&[tag][..], DEFAULT_SCOPE.to_vec()),
            _ => return ordinary_end(self.open(), tag),
        };
        in_scope(self.open(), targets, &boundaries)
    }
    /** Pops open elements down to `position`, finishing each so empty media gain a placeholder. */
    fn close(&mut self, position: usize) {
        while self.stack.len() > position {
            let index = self.stack.pop().unwrap_or_default();
            self.finish(index);
        }
    }
    /** Metadata/resource handling cannot create native authority. */
    fn start(&mut self, tag: String, attrs: markup::Attributes, closed: bool) -> EngineResult<()> {
        // HTML reads a stray `image` as `img`; inside a drawing it is SVG's own `image`.
        let tag = if tag == "image" && !self.in_svg() {
            "img".into()
        } else {
            tag
        };
        if self.fragment && self.fragment_start(&tag, &attrs)? {
            return Ok(());
        }
        self.foreign_breakout(&tag, &attrs);
        if self.in_svg() && !SVG_CHILDREN.contains(&tag.as_str()) {
            self.diagnose("SVG content outside the supported drawing subset is not drawn.");
            if !closed {
                self.skip(&tag);
            }
            return Ok(());
        }
        if self.head && !HEAD_CONTENT.contains(&tag.as_str()) {
            self.head = false;
        }
        match tag.as_str() {
            "head" => self.head = true,
            "html" | "body" => {
                self.head = false;
                let mut attrs = attrs;
                for message in markup::recover(&tag, &mut attrs) {
                    self.diagnose(message);
                }
                let formatting = if tag == "html" {
                    &mut self.document.formatting.html
                } else {
                    &mut self.document.formatting.body
                };
                for (key, value) in attrs {
                    formatting.attributes.entry(key).or_insert(value);
                }
            }
            "title" => {
                let title = self.title();
                self.metadata(&tag, &attrs, title);
            }
            "style" => {
                let css = self.tokens.raw("style");
                self.metadata(&tag, &attrs, css.clone());
                let css = match conditional_media(&attrs) {
                    Some(media) => format!("@media {media} {{\n{css}\n}}"),
                    None => css,
                };
                if !self.shadow_style(&attrs, &css)? {
                    self.document.styles.push(StyleSource::Inline(css));
                }
            }
            "link" => {
                self.link(&attrs);
                self.metadata(&tag, &attrs, String::new());
            }
            "meta" => {
                self.meta(&attrs)?;
                self.metadata(&tag, &attrs, String::new());
            }
            "script" => self.script(attrs)?,
            "base" => {
                self.base(&attrs);
                self.metadata(&tag, &attrs, String::new());
            }
            _ => return self.body_start(tag, attrs, closed),
        }
        Ok(())
    }
    /// Fragment-only handling: document-level wrappers and metadata change nothing when a script
    /// inserts them. An inserted script or style element stays in the tree as an empty inert
    /// element, as it is in the DOM, so an element a script built parses back to the structure
    /// the script engine holds: the script never runs (it is never recorded) and the style's
    /// rules join the page's sheets. Returns whether the tag was handled here.
    fn fragment_start(&mut self, tag: &str, attrs: &markup::Attributes) -> EngineResult<bool> {
        match tag {
            "html" | "head" | "body" | "meta" | "base" => Ok(true),
            "title" => {
                self.tokens.raw(tag);
                Ok(true)
            }
            "script" | "style" => {
                let text = self.tokens.raw(tag);
                if tag == "style" {
                    let css = match conditional_media(attrs) {
                        Some(media) => format!("@media {media} {{\n{text}\n}}"),
                        None => text,
                    };
                    if self.shadow_style(attrs, &css)? {
                        return Ok(true);
                    }
                    self.document.styles.push(StyleSource::Inline(css));
                } else {
                    self.diagnose("Inserted scripts are inert and never run.");
                }
                if self.element(tag.into(), attrs.clone(), false)? {
                    self.close_current(tag);
                }
                Ok(true)
            }
            _ => Ok(false),
        }
    }
    /** Body content: inert raw text, templates, and ordinary elements after implied closes. */
    fn body_start(
        &mut self,
        tag: String,
        attrs: markup::Attributes,
        closed: bool,
    ) -> EngineResult<()> {
        if tag == "form" && self.open().any(|(_, open)| open == "form") {
            self.ignored_forms += 1;
            return Ok(());
        }
        if tag == "noscript" && self.head {
            self.tokens.raw("noscript");
            return Ok(());
        }
        if ["noembed", "noframes"].contains(&tag.as_str()) {
            self.tokens.raw(&tag);
            return Ok(());
        }
        if self.stray_table_part(&tag) {
            self.diagnose("A table part outside any table was ignored; its text stays as text.");
            return Ok(());
        }
        if tag == "template" && declarative_shadow(&attrs) {
            let mode = attrs.get("shadowrootmode").cloned().unwrap_or_default();
            let attrs = markup::Attributes::from([("mode".to_string(), mode.to_ascii_lowercase())]);
            self.element(super::shadow::SHADOW_ROOT.into(), attrs, false)?;
            return Ok(());
        }
        self.implied_closes(&tag);
        let created = self.element(tag.clone(), attrs, closed)?;
        match tag.as_str() {
            _ if !created => {}
            "template" => self.template(),
            _ if INERT_RAW.contains(&tag.as_str()) => {
                self.tokens.raw(&tag);
                self.close_current(&tag);
            }
            "xmp" | "plaintext" => {
                let text = self.tokens.raw(&tag);
                self.text(text)?;
                self.close_current(&tag);
            }
            "embed" => self.finish(self.document.nodes.len() - 1),
            "pre" | "listing" => self.after_pre = true,
            _ => {}
        }
        Ok(())
    }
    /** Closes the element just opened for raw content, running its placeholder step even when
    the depth bound kept it from opening. */
    fn close_current(&mut self, tag: &str) {
        if self.current() == tag {
            self.close(self.stack.len() - 1);
        } else if self
            .document
            .nodes
            .last()
            .is_some_and(|node| node.tag == tag)
        {
            self.finish(self.document.nodes.len() - 1);
        }
    }
    /// HTML's "in body" mode ignores caption, col, colgroup, row-group, row and cell start tags
    /// when no table (or template) is open; script fragments keep their context's own rules.
    fn stray_table_part(&self, tag: &str) -> bool {
        !self.tokens.xml()
            && STRAY_TABLE_PARTS.contains(&tag)
            && !self.fragment
            && !self
                .open()
                .any(|(_, open)| open == "table" || open == "template")
    }
    /** Start tags that end open paragraphs, list items, options, headings, links and table parts. */
    fn implied_closes(&mut self, tag: &str) {
        if self.tokens.xml() {
            return;
        }
        if CLOSES_P.contains(&tag)
            && let Some(position) = in_scope(
                self.open(),
                &["p"],
                &[&DEFAULT_SCOPE[..], &["button"]].concat(),
            )
        {
            self.close(position);
        }
        let position = match tag {
            _ if HEADINGS.contains(&tag) => HEADINGS
                .contains(&self.current())
                .then(|| self.stack.len() - 1),
            "li" => list_item(self.open(), &["li"]),
            "dd" | "dt" => list_item(self.open(), &["dd", "dt"]),
            "option" => (self.current() == "option").then(|| self.stack.len() - 1),
            "optgroup" => self
                .open()
                .rev()
                .take_while(|(_, open)| ["option", "optgroup"].contains(open))
                .last()
                .map(|(position, _)| position),
            "a" | "button" | "nobr" => in_scope(self.open(), &[tag], &DEFAULT_SCOPE),
            _ => None,
        };
        if let Some(position) = position {
            self.close(position);
        }
        self.table_closes(tag);
    }
    /** Table parts clear back to their table, row group or row context, as HTML's table modes do. */
    fn table_closes(&mut self, tag: &str) {
        let contexts: &[&str] = match tag {
            "td" | "th" => &["tr", "tbody", "thead", "tfoot", "table"],
            "tr" => &["tbody", "thead", "tfoot", "table"],
            "tbody" | "thead" | "tfoot" | "caption" | "colgroup" => &["table"],
            // A column inside an open column group stays in it ("in column group" mode).
            "col" => &["colgroup", "table"],
            "table" if TABLE_CONTEXT.contains(&self.current()) => &["table"],
            _ => return,
        };
        let Some(position) = self.table_context(contexts) else {
            return;
        };
        if tag == "table" {
            self.diagnose("A table started directly inside another table closes the first one.");
            self.close(position);
        } else {
            self.close(position + 1);
        }
    }
    /** The innermost open table context in table scope; a form inserted into table structure counts. */
    fn table_context(&self, contexts: &[&str]) -> Option<usize> {
        for (position, tag) in self.open().rev() {
            if contexts.contains(&tag) || (tag == "form" && self.table_form(position)) {
                return Some(position);
            }
            if TABLE_SCOPE.contains(&tag) {
                return None;
            }
        }
        None
    }
    /** A form whose parent is table structure keeps the rows inside it, so its controls stay owned. */
    fn table_form(&self, position: usize) -> bool {
        self.document.nodes[self.stack[position]]
            .parent
            .and_then(|parent| self.document.nodes.get(parent - self.base))
            .is_some_and(|parent| TABLE_CONTEXT.contains(&parent.tag.as_str()))
    }
    /** Cells start an implied row; other content inside table structure moves beside the table. */
    fn insertion_parent(&mut self, tag: &str) -> EngineResult<Option<usize>> {
        let current = self.current().to_owned();
        if self.tokens.xml() {
            return Ok(self.stack.last().map(|index| self.local(*index)));
        }
        if ["td", "th"].contains(&tag)
            && current != "tr"
            && TABLE_CONTEXT.contains(&current.as_str())
        {
            self.diagnose("Table cells outside a row were placed in an implied row.");
            self.element("tr".into(), Default::default(), false)?;
        }
        if !TABLE_CONTENT.contains(&tag)
            && let Some(parent) = self.foster_parent()
        {
            return Ok(parent);
        }
        Ok(self.stack.last().map(|index| self.local(*index)))
    }
    /** HTML foster parenting: content misplaced in open table structure belongs to the table's
    parent. None when the current node is not table structure inside a table. */
    fn foster_parent(&mut self) -> Option<Option<usize>> {
        if self.tokens.xml() || !TABLE_CONTEXT.contains(&self.current()) {
            return None;
        }
        let table = self
            .stack
            .iter()
            .rev()
            .find(|index| self.document.nodes[**index].tag == "table")?;
        let parent = self.document.nodes[*table].parent;
        self.diagnose("Content misplaced inside table structure was moved beside the table.");
        Some(parent)
    }
    /** A `<style>` inside a shadow tree stays an element holding its text, scoped to that tree;
    returns false when the style belongs to the page (or is too large to keep as text). */
    fn shadow_style(&mut self, attrs: &markup::Attributes, css: &str) -> EngineResult<bool> {
        let in_shadow = self
            .open()
            .any(|(_, open)| open == super::shadow::SHADOW_ROOT);
        if !in_shadow || css.len() > crate::constants::TEXT_BYTES {
            return Ok(false);
        }
        self.text_bytes = super::content::add_static_bytes(self.text_bytes, css.len())?;
        if self.element("style".into(), attrs.clone(), false)? {
            if let Some(node) = self.document.nodes.last_mut() {
                node.text = css.to_string();
            }
            self.close_current("style");
        }
        Ok(true)
    }
    /** Template content is inert: it is consumed, balanced by nesting, and never becomes nodes. */
    fn template(&mut self) {
        self.close_current("template");
        self.diagnose("Template content is inert and was not rendered.");
        self.skip("template");
    }
    /** Consumes tokens through the end tag balancing `tag`, or an open ancestor's end tag, keeping raw text opaque. */
    fn skip(&mut self, tag: &str) {
        // Elements opened inside the skipped content, so their own end tags (a `div` inside an
        // SVG `foreignObject`) close them rather than an open ancestor of the same name.
        let mut inner = vec![tag.to_string()];
        while !inner.is_empty() {
            match self.tokens.next() {
                None => return,
                Some(Token::Start(name, _, closed)) => {
                    if ["script", "style", "textarea", "title"].contains(&name.as_str())
                        && !self.in_svg()
                    {
                        self.tokens.raw(&name);
                    } else if !closed
                        && !VOID.contains(&name.as_str())
                        && inner.len() < DOCUMENT_DEPTH
                    {
                        inner.push(name);
                    }
                }
                Some(Token::End(name)) if inner.contains(&name) => {
                    let open = inner
                        .iter()
                        .rposition(|open| *open == name)
                        .unwrap_or_default();
                    inner.truncate(open);
                }
                // An end tag of an open ancestor (the enclosing `</svg>`, the parent element)
                // ends the skipped content too, so an unclosed element never swallows the page.
                Some(Token::End(name)) if self.open().any(|(_, open)| open == name) => {
                    let _ = self.end(&name);
                    return;
                }
                Some(_) => {}
            }
        }
    }
    /** The first title sets the document title, cut to the text bound at a character boundary.
    Returns this title element's text. */
    fn title(&mut self) -> String {
        let title = markup::entities(&self.tokens.raw("title"));
        if self.document.title.is_empty() {
            self.document.title = self.truncated(title.clone(), TEXT_BYTES);
        }
        title
    }
    /// Keeps one page metadata element for scripts (MetadataElement): attributes recovered as a
    /// node's are, and attribute values plus text within METADATA_BYTES for the page, so text
    /// is dropped first and the element only when its attributes alone do not fit. Nothing past
    /// METADATA_ELEMENTS is kept. Script-inserted markup adds none: V9 holds what it built.
    fn metadata(&mut self, tag: &str, attrs: &markup::Attributes, text: String) {
        if self.fragment {
            return;
        }
        let mut attributes = attrs.clone();
        for message in markup::recover(tag, &mut attributes) {
            self.diagnose(message);
        }
        let bytes = attributes.values().map(String::len).sum::<usize>();
        let fits = |used: usize, more: usize| used + more <= METADATA_BYTES;
        if self.document.metadata.len() >= METADATA_ELEMENTS || !fits(self.metadata_bytes, bytes) {
            self.diagnose("Metadata past the page bound is not visible to scripts.");
            return;
        }
        let text = if fits(self.metadata_bytes + bytes, text.len()) {
            text
        } else {
            self.diagnose("Metadata past the page bound is not visible to scripts.");
            String::new()
        };
        self.metadata_bytes += bytes + text.len();
        // Before any body content (no open element, no node yet) it belongs to the head, as
        // HTML's "after head" mode puts it there; otherwise it stays where the source has it.
        let in_head = self.head || self.stack.is_empty() && self.document.nodes.is_empty();
        let in_body = !in_head;
        self.document.metadata.push(MetadataElement {
            tag: tag.into(),
            attributes,
            text,
            in_body,
            parent: self.stack.last().map(|index| self.local(*index)),
            next: self.base + self.document.nodes.len(),
        });
    }
    /** Linked sheets load in source order; one with a media condition carries it to the cascade. */
    fn link(&mut self, attrs: &markup::Attributes) {
        if !attrs.get("rel").is_some_and(|rel| {
            rel.split_ascii_whitespace()
                .any(|r| r.eq_ignore_ascii_case("stylesheet"))
        }) {
            return;
        }
        let Some(href) = attrs.get("href").filter(|href| !href.trim().is_empty()) else {
            return;
        };
        let href = self.resolve(href);
        let source = match conditional_media(attrs) {
            Some(media) if media.len() <= TEXT_BYTES => StyleSource::LinkedMedia {
                href,
                media: media.to_owned(),
            },
            Some(_) => return,
            None => StyleSource::Linked(href),
        };
        self.document.styles.push(source);
    }
    /** Only an absolute http(s) base is honored; anything else would need the document URL. */
    fn base(&mut self, attrs: &markup::Attributes) {
        let Some(href) = attrs.get("href") else {
            return;
        };
        if self.base_url.is_some() {
            return;
        }
        match url::Url::parse(href.trim()) {
            Ok(url) if ["http", "https"].contains(&url.scheme()) => self.base_url = Some(url),
            _ => self.diagnose("A relative or non-web base address was ignored."),
        }
    }
    /** Relative addresses resolve against an honored base; script URLs and bad joins stay as written. */
    fn resolve(&self, address: &str) -> String {
        match &self.base_url {
            Some(base) if !markup::script_url(address) => base
                .join(address.trim())
                .map_or_else(|_| address.to_owned(), String::from),
            _ => address.to_owned(),
        }
    }
    /// Charset and content-type metadata must agree with decoding. Other http-equiv pragmas never
    /// refuse the page: refresh is noted and not followed, set-cookie is ignored as Chromium
    /// ignores it, a Content-Security-Policy is recorded for the coordinator, the rest are noted.
    fn meta(&mut self, attrs: &markup::Attributes) -> EngineResult<()> {
        if let Some(label) = attrs.get("charset") {
            self.charset(label)?;
        }
        if attrs
            .get("name")
            .is_some_and(|name| name.trim().eq_ignore_ascii_case("color-scheme"))
            && let Some(content) = attrs.get("content").filter(|c| c.len() <= TEXT_BYTES)
        {
            // The style engine reads the first one as the root's color-scheme hint.
            self.document
                .formatting
                .html
                .attributes
                .entry("meta color-scheme".into())
                .or_insert(content.into());
        }
        let Some(equiv) = attrs.get("http-equiv") else {
            return Ok(());
        };
        let content = attrs.get("content").map(String::as_str).unwrap_or("");
        match equiv.trim().to_ascii_lowercase().as_str() {
            "content-type" => match content_charset(content) {
                Some(label) => self.charset(label),
                None => Ok(()),
            },
            "x-ua-compatible" | "set-cookie" => Ok(()),
            "refresh" => {
                self.diagnose("A meta refresh was not followed.");
                Ok(())
            }
            "content-security-policy" => {
                self.policy(content);
                Ok(())
            }
            _ => {
                self.diagnose("An unsupported http-equiv pragma was ignored.");
                Ok(())
            }
        }
    }
    /** Records one meta policy within its count and size bounds. */
    fn policy(&mut self, content: &str) {
        if self.fragment {
            return;
        }
        if self.document.csp.len() >= super::CSP_POLICIES || content.len() > TEXT_BYTES {
            self.diagnose("A Content-Security-Policy past the policy bounds was dropped.");
            return;
        }
        self.document.csp.push(content.to_owned());
        self.diagnose("A meta Content-Security-Policy was recorded for enforcement.");
    }
    /** Only labels that read these bytes as the decoding did pass; UTF-8 and windows-1252 agree on ASCII. */
    fn charset(&self, label: &str) -> EngineResult<()> {
        let meta = encoding_for_label(label);
        let ascii_safe = |e: Option<&str>| matches!(e, Some(UTF_8 | WINDOWS_1252));
        let agrees = meta.is_some() && meta == self.decoded
            || self.ascii && ascii_safe(meta) && ascii_safe(self.decoded);
        if agrees {
            return Ok(());
        }
        Err(EngineError::unsupported(
            "The page declares a character encoding that differs from the one used to read it.",
        ))
    }
    /** V9 accepts ordered classic programs; external async and defer scripts record their phase. */
    fn script(&mut self, attrs: markup::Attributes) -> EngineResult<()> {
        let source = self.tokens.raw("script");
        if let Some(kind) = module_kind(&attrs) {
            return self.module_script(attrs, source, kind);
        }
        // Modules run, so their nomodule fallbacks must not.
        if !executable(&attrs) || attrs.contains_key("nomodule") {
            // A data block (JSON, a template) is never run; scripts read it as text.
            self.metadata("script", &attrs, source);
            return Ok(());
        }
        // Size and count are enforced per script by the coordinator, which skips rather than refuses.
        let src = attrs.get("src").map(|src| self.resolve(src));
        let mode = match (
            src.is_some(),
            attrs.contains_key("async"),
            attrs.contains_key("defer"),
        ) {
            (true, true, _) => ScriptMode::Async,
            (true, false, true) => ScriptMode::Defer,
            _ => ScriptMode::Blocking,
        };
        let attributes = self.script_attributes(&attrs);
        self.document.scripts.push(ScriptResource {
            source,
            src,
            mode,
            kind: ScriptKind::Classic,
            integrity: attrs.get("integrity").cloned(),
            nonce: attrs.get("nonce").cloned(),
            attributes,
        });
        Ok(())
    }
    /// A script element's attributes for its V9 head record, charged to the same METADATA_BYTES
    /// budget as metadata elements; past it the record keeps only its `src`.
    fn script_attributes(&mut self, attrs: &markup::Attributes) -> markup::Attributes {
        let bytes = attrs.values().map(String::len).sum::<usize>();
        if self.metadata_bytes + bytes > METADATA_BYTES {
            self.diagnose("Metadata past the page bound is not visible to scripts.");
            return markup::Attributes::new();
        }
        self.metadata_bytes += bytes;
        attrs.clone()
    }
    /** A module script runs deferred, or as soon as its graph loads with `async` (inline ones
    too); an import map is inline JSON, applied before any module loads. */
    fn module_script(
        &mut self,
        attrs: markup::Attributes,
        source: String,
        kind: ScriptKind,
    ) -> EngineResult<()> {
        let src = attrs.get("src").map(|src| self.resolve(src));
        if kind == ScriptKind::ImportMap && src.is_some() {
            self.diagnose("An external import map was ignored; import maps must be inline.");
            return Ok(());
        }
        let mode = match kind {
            ScriptKind::ImportMap => ScriptMode::Blocking,
            _ if attrs.contains_key("async") => ScriptMode::Async,
            _ => ScriptMode::Defer,
        };
        let attributes = self.script_attributes(&attrs);
        self.document.scripts.push(ScriptResource {
            source,
            src,
            mode,
            kind,
            integrity: attrs.get("integrity").cloned(),
            nonce: attrs.get("nonce").cloned(),
            attributes,
        });
        Ok(())
    }
    /// Inserts one element and opens it unless it is void. Past the depth bound the element is
    /// still kept but not opened, so deeper content flattens into siblings as Chromium does.
    /// Returns whether the element is open for content.
    fn element(
        &mut self,
        tag: String,
        mut attrs: markup::Attributes,
        closed: bool,
    ) -> EngineResult<bool> {
        if self.base + self.document.nodes.len() >= NODES {
            return Err(EngineError::limit("document nodes", NODES));
        }
        for message in markup::recover(&tag, &mut attrs) {
            self.diagnose(message);
        }
        if self.fragment {
            // Script-inserted markup never gains an inline handler, not even a button's onclick.
            attrs.retain(|key, _| !key.starts_with("on"));
        }
        self.resolve_addresses(&tag, &mut attrs);
        // A self-closing start tag is acknowledged only for a foreign element, which then
        // closes at once: `<math><mrow/><mi/></math>` holds two siblings.
        let foreign_math = !self.tokens.xml()
            && self.child_namespace(self.current_namespace(), &tag) == Namespace::MathMl;
        let parent = self.insertion_parent(&tag)?;
        let index = self.document.nodes.len();
        let mut node = markup::node(&tag, self.base + index, parent, attrs)?;
        if let Some(href) = node.href.take() {
            node.href = Some(self.resolve(&href));
        }
        if tag == "textarea" {
            let value = markup::entities(&self.tokens.raw("textarea"));
            let value = value.strip_prefix('\n').unwrap_or(&value).to_owned();
            node.value = self.truncated(value, TEXT_BYTES);
            self.document.nodes.push(node);
            return Ok(false);
        }
        self.document.nodes.push(node);
        let foreign = self.in_svg() || tag == "svg" || foreign_math;
        if VOID.contains(&tag.as_str()) || (closed && foreign) {
            return Ok(true);
        }
        if self.stack.len() + 1 >= DOCUMENT_DEPTH {
            self.diagnose("Markup nested past the depth bound was flattened.");
            return Ok(true);
        }
        self.stack.push(index);
        Ok(true)
    }
    /** Image sources and form actions resolve against an honored base like link targets do. */
    fn resolve_addresses(&self, tag: &str, attrs: &mut markup::Attributes) {
        let key = match tag {
            "img" => "src",
            "form" => "action",
            _ => return,
        };
        if let Some(value) = attrs.get(key).map(|value| self.resolve(value)) {
            attrs.insert(key.into(), value);
        }
    }
    /// Closing an embedded element: media or objects without fallback content get one inert
    /// placeholder box. An iframe needs none: it is a replaced box of its own, which the
    /// runtime fills with its document. Nothing is loaded or executed here.
    fn finish(&mut self, index: usize) {
        let node = &self.document.nodes[index];
        if !EMBEDDED.contains(&node.tag.as_str()) || node.tag == "iframe" {
            return;
        }
        let fallback = !node.text.is_empty()
            || self.document.nodes[index + 1..].iter().any(|child| {
                child.tag != "#comment" && !MEDIA_RESOURCES.contains(&child.tag.as_str())
            });
        // A video with a poster shows the poster frame, painted as its own background (see the
        // presentational hints), so it needs no stand-in box or label.
        let posted = node.tag == "video"
            && node
                .attributes
                .get("poster")
                .is_some_and(|url| !url.trim().is_empty());
        if fallback || posted {
            return;
        }
        if let Some((width, height)) = placeholder_size(node) {
            let label = placeholder_label(&node.tag);
            self.diagnose(label);
            if self.fragment {
                self.label(index, label);
            } else {
                self.placeholder(index, label, width, height);
            }
        }
    }
    /// Script-inserted embedded content carries its placeholder label as its own text instead
    /// of a placeholder child, so an element a script built parses back to the same structure
    /// it has in the script engine, which refuses an insertion whose structure would change.
    fn label(&mut self, owner: usize, label: &str) {
        if let Ok(total) = super::content::add_static_bytes(self.text_bytes, label.len()) {
            self.text_bytes = total;
            self.document.nodes[owner].text = label.to_owned();
        }
    }
    /** A sized, labelled, non-actionable group box standing in for unsupported embedded content. */
    fn placeholder(&mut self, owner: usize, label: &str, width: String, height: u32) {
        let depth = std::iter::successors(self.document.nodes[owner].parent, |id| {
            self.document.nodes[*id - self.base].parent
        })
        .count();
        if self.base + self.document.nodes.len() >= NODES
            || depth + 1 >= DOCUMENT_DEPTH
            || super::content::add_static_bytes(self.text_bytes, label.len()).is_err()
        {
            return;
        }
        let padding = height.saturating_sub(PLACEHOLDER_LINE);
        let attributes = markup::Attributes::from([
            ("aria-label".to_owned(), label.to_owned()),
            (
                "style".to_owned(),
                format!(
                    "display:block;width:{width};padding-bottom:{padding}px;border:1px solid #a0a0a0;color:#555555"
                ),
            ),
        ]);
        let local = self.base + self.document.nodes.len();
        if let Ok(mut node) = markup::node("div", local, Some(self.local(owner)), attributes) {
            node.text = label.to_owned();
            self.text_bytes += label.len();
            self.document.nodes.push(node);
        }
    }
    /** Literal text stays attached to its source element where unambiguous. */
    fn text(&mut self, text: String) -> EngineResult<()> {
        // Only HTML's ASCII white space is droppable: a no-break space (`<td>&nbsp;</td>`) or
        // an ideographic space is text, which layout keeps (and hangs at a line's end).
        if text.chars().all(|c| c.is_ascii_whitespace()) && !self.in_preformatted() {
            self.space_after_sibling(&text);
            return Ok(());
        }
        self.head = false;
        if self.in_svg() {
            self.diagnose("Text inside SVG drawings is not drawn.");
            return Ok(());
        }
        let parent = self.foster_parent();
        // Nodes are appended in preorder, so any node after an open element is its descendant.
        if parent.is_none()
            && let Some(index) = self.stack.last().copied()
            && self.document.nodes.len() == index + 1
        {
            let node = &self.document.nodes[index];
            let limit = super::content::text_limit(node);
            let text = self.truncated(text, limit.saturating_sub(node.text.len()));
            self.text_bytes = super::content::add_static_bytes(self.text_bytes, text.len())?;
            self.document.nodes[index].text.push_str(&text);
            return Ok(());
        }
        if self.base + self.document.nodes.len() >= NODES {
            return Err(EngineError::limit("document nodes", NODES));
        }
        let parent = parent.unwrap_or_else(|| self.stack.last().map(|index| self.local(*index)));
        // Inserting a character where the previous sibling is a Text node extends it, so text a
        // dropped tag or comment split stays one node, as in HTML.
        if let Some(last) = self.document.nodes.last()
            && last.tag == "#text"
            && last.parent == parent
        {
            let limit = super::content::text_limit(last).saturating_sub(last.text.len());
            let text = self.truncated(text, limit);
            self.text_bytes = super::content::add_static_bytes(self.text_bytes, text.len())?;
            if let Some(last) = self.document.nodes.last_mut() {
                last.text.push_str(&text);
            }
            return Ok(());
        }
        let mut node = markup::node(
            "#text",
            self.base + self.document.nodes.len(),
            parent,
            Default::default(),
        )?;
        let text = self.truncated(text, super::content::text_limit(&node));
        self.text_bytes = super::content::add_static_bytes(self.text_bytes, text.len())?;
        node.text = text;
        self.document.nodes.push(node);
        Ok(())
    }
    /** Whether an open `<pre>` or `<listing>` keeps whitespace-only text as text: its newlines
     * and spaces between highlighted spans are content (nodejs.org's code ran together). */
    fn in_preformatted(&self) -> bool {
        self.stack
            .iter()
            .any(|index| ["pre", "listing"].contains(&self.document.nodes[*index].tag.as_str()))
    }
    /** Dropped whitespace-only text marks the element it follows, the last node's ancestor that
     * is a child of the open element, or the open element itself when it has no content yet, so
     * layout keeps the space between two inline elements. */
    fn space_after_sibling(&mut self, text: &str) {
        let parent = self.stack.last().map(|index| self.local(*index));
        let mut current = self.document.nodes.len().checked_sub(1);
        if let Some(open) = self.stack.last().copied()
            && current == Some(open)
            && self.document.nodes[open].text.is_empty()
        {
            let node = &mut self.document.nodes[open];
            node.space_leading = true;
            // Only ASCII white space is kept, so any byte count is a character boundary.
            if text
                .bytes()
                .all(|b| matches!(b, b' ' | b'\t' | b'\n' | b'\r'))
            {
                let room = LEADING_SPACE_BYTES.saturating_sub(node.leading_space.len());
                node.leading_space.push_str(&text[..text.len().min(room)]);
            }
            return;
        }
        while let Some(index) = current {
            let node = &self.document.nodes[index];
            if node.parent == parent {
                if node.tag != "#text" && !self.stack.contains(&index) {
                    self.document.nodes[index].space_after = true;
                }
                return;
            }
            current = node
                .parent
                .and_then(|local| local.checked_sub(self.base))
                .filter(|position| *position < index);
        }
    }
    /** Text past a per-node bound is cut at a character boundary and noted, never refused. */
    fn truncated(&mut self, mut text: String, limit: usize) -> String {
        if text.len() > limit {
            text.truncate(text.floor_char_boundary(limit));
            self.diagnose("Text longer than the per-node bound was truncated.");
        }
        text
    }
}

/** How much dropped leading white space an element keeps for preserved white space to lay out:
 * enough for any indentation or spacer, a bound on what every element of a page may carry. */
const LEADING_SPACE_BYTES: usize = 64;
/** A `media` attribute that is not simply all or screen; inline sheets wrap it as `@media` so the
cascade evaluates it against the viewport like any other media rule. */
fn conditional_media(attrs: &markup::Attributes) -> Option<&str> {
    attrs
        .get("media")
        .map(|media| media.trim())
        .filter(|media| !media.is_empty() && !super::style::linked_media_applies(media))
}

/** Author width and height attributes, or the element's default size; None for a zero-size box. */
fn placeholder_size(node: &super::Node) -> Option<(String, u32)> {
    let default = match node.tag.as_str() {
        "audio" if node.attributes.contains_key("controls") => {
            (EMBED_SIZE.0, AUDIO_CONTROLS_HEIGHT)
        }
        "audio" => return None,
        _ => EMBED_SIZE,
    };
    let height = dimension(node.attributes.get("height")).map_or(default.1, |(value, _)| value);
    let width = match dimension(node.attributes.get("width")) {
        Some((value, true)) => format!("{}%", value.min(100)),
        Some((value, false)) => format!("{value}px"),
        None => format!("{}px", default.0),
    };
    (height > 0 && !["0px", "0%"].contains(&width.as_str()) && !node.hidden())
        .then_some((width, height))
}

/** A dimension attribute's leading digits, capped, and whether it was a percentage. */
fn dimension(value: Option<&String>) -> Option<(u32, bool)> {
    let value = value?.trim();
    let digits = value
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(value.len());
    let number = value[..digits].parse::<u32>().ok()?.min(PLACEHOLDER_MAX);
    Some((number, value[digits..].starts_with('%')))
}

/** Whether a node's text is the placeholder label a fragment gave its embedded element. */
pub(super) fn labelled(node: &super::Node) -> bool {
    EMBEDDED.contains(&node.tag.as_str()) && node.text == placeholder_label(&node.tag)
}

/** The visible and accessible name of each placeholder kind. */
fn placeholder_label(tag: &str) -> &'static str {
    match tag {
        "iframe" => "Embedded frame (not supported yet)",
        "video" => "Video (not supported yet)",
        "audio" => "Audio (not supported yet)",
        "canvas" => "Canvas drawing (not supported yet)",
        _ => "Embedded content (not supported yet)",
    }
}

/** HTML's "extract a character encoding from a meta element": the label after `charset=`. */
fn content_charset(content: &str) -> Option<&str> {
    let lower = content.to_ascii_lowercase();
    let mut from = 0;
    loop {
        let at = from + lower[from..].find("charset")? + "charset".len();
        let rest = content[at..].trim_start_matches(|c: char| c.is_ascii_whitespace());
        let Some(value) = rest.strip_prefix('=') else {
            from = at;
            continue;
        };
        let value = value.trim_start_matches(|c: char| c.is_ascii_whitespace());
        return match value.chars().next()? {
            quote @ ('"' | '\'') => value[1..].find(quote).map(|end| &value[1..][..end]),
            _ => value
                .split(|c: char| c == ';' || c.is_ascii_whitespace())
                .next()
                .filter(|label| !label.is_empty()),
        };
    }
}

/** A module script or an import map, by its `type`. */
fn module_kind(attrs: &markup::Attributes) -> Option<ScriptKind> {
    let kind = attrs.get("type")?.trim().to_ascii_lowercase();
    match kind.as_str() {
        "module" => Some(ScriptKind::Module),
        "importmap" => Some(ScriptKind::ImportMap),
        _ => None,
    }
}

/** Classic JavaScript runs; JSON, templates and other data blocks stay inert. */
fn executable(attrs: &markup::Attributes) -> bool {
    let Some(kind) = attrs.get("type") else {
        return true;
    };
    let kind = kind.trim().to_ascii_lowercase();
    kind.is_empty()
        || [
            "text/javascript",
            "application/javascript",
            "text/ecmascript",
            "application/ecmascript",
        ]
        .contains(&kind.as_str())
}

/** `<template shadowrootmode=open|closed>` declares a shadow root for its parent element. */
fn declarative_shadow(attrs: &markup::Attributes) -> bool {
    attrs.get("shadowrootmode").is_some_and(|mode| {
        mode.eq_ignore_ascii_case("open") || mode.eq_ignore_ascii_case("closed")
    })
}

/** An `annotation-xml` whose encoding names HTML is an HTML integration point. */
fn html_annotation(node: &super::Node) -> bool {
    node.attributes.get("encoding").is_some_and(|encoding| {
        encoding.eq_ignore_ascii_case("text/html")
            || encoding.eq_ignore_ascii_case("application/xhtml+xml")
    })
}
