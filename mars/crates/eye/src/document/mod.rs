/** Eye owns the bounded document, cascade and rendering model. */
mod content;
mod context;
mod entities;
mod forms;
mod markup;
mod mutation;
mod parser;
mod quirks;
mod recovery;
mod render;
mod script;
mod shadow;
mod style;
mod table;
mod tokenizer;
mod xml;
use crate::constants::LAYOUT_UNIT;
use crate::protocol::{EngineError, EngineResult};
pub use content::validate_content;
pub use context::{Appearance, RenderContext};
pub use forms::{Submission, form_owner, label, own_text, select, set_checked, submission};
use mars_core::dom::descendant_ids;
pub use mars_core::dom::{
    DomChange, FragmentParser, InsertPosition, MetadataElement, ScriptChange, ScriptDocument,
    ScriptMode, ScriptNode, TextChange, validate_text,
};
pub use mutation::{apply_changes, apply_script_changes};
pub use parser::{Fragment, parse, parse_fragment, parse_with_charset, parse_xml};
pub use render::{
    ClipRect, DisplayBackground, DisplayDecoration, DisplayItem, DisplayList, DisplayShadow,
    FillRule, GradientStop, ImageSize, LineCap, LineJoin, OpacityGroup, PaintLine, PathCommand,
    Pin, ScrollArea, ScrollOffsets, VectorPath, agent_hidden, agent_visible, painted_bounds,
    render, render_scrolled, render_with_context, role, wheel_target,
};
pub use script::{DomFragments, Fragments, script_document};
use serde::{Deserialize, Serialize};
pub use shadow::{SHADOW_ROOT, flatten, has_shadow};
use std::collections::BTreeMap;
pub use style::{
    AlignItems, BACKGROUND_IMAGES, BackgroundLayer, BoxShadow, BoxSizing, ColorScheme, ColorStop,
    ComputedStyle, DocumentFormatting, FlexBasis, FlexDirection, FlexWrap, FontFamily,
    FormattingElement, JustifyContent, LineHeight, ObjectFit, Overflow, Position, StyleSource,
    TextTransform, WhiteSpace, background_urls, compute_styles, compute_styles_with_context,
    rebase_urls,
};
pub use style::{BorderStyle, painter_rgba};
pub use style::{
    FONT_FACES, FONT_FILE_BYTES, FONT_FILES, FONT_NAME_BYTES, FONT_TOTAL_BYTES, FontDisplay,
    FontFace, FontRequest, FontSource, FontStyle, LoadedFont, content_key, font_requests,
    install_fonts,
};
pub use xml::xml_mime;

/** Private computed values are validated before layout even when a Rust caller bypasses CSS parsing. */
pub fn validate_styles(document: &Document) -> EngineResult<()> {
    validate_content(document)?;
    document.formatting.html.style.validate()?;
    document.formatting.body.style.validate()?;
    for node in &document.nodes {
        node.style.validate()?;
    }
    Ok(())
}

/// A document candidate is published only after all script changes validate.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    /// Source order determines deterministic layout order.
    pub nodes: Vec<Node>,
    /// Ordered classic programs are interpreted by V9, never by the host runtime.
    #[serde(default)]
    pub scripts: Vec<ScriptResource>,
    /// Metadata belongs to the document, not an extra page view.
    #[serde(default)]
    pub title: String,
    /// Required CSS sources retain source order across inline and linked sheets.
    #[serde(default)]
    pub styles: Vec<StyleSource>,
    /// Value-free compatibility diagnostics remain bounded.
    #[serde(default)]
    pub diagnostics: Vec<String>,
    /// Non-target wrapper styles preserve root/body semantics without allocating native node IDs.
    #[serde(default)]
    pub formatting: DocumentFormatting,
    /// `<meta http-equiv=Content-Security-Policy>` texts in source order, at most CSP_POLICIES of
    /// TEXT_BYTES each. Eye only records them; the coordinator enforces them when it fetches.
    #[serde(default)]
    pub csp: Vec<String>,
    /// Metadata and data-block elements in source order, at most METADATA_ELEMENTS. They are
    /// never nodes, so layout, paint and agents never see them; scripts find them through V9.
    #[serde(default)]
    pub metadata: Vec<MetadataElement>,
}

/// What a script element holds; modules and import maps go to V9's module loader.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptKind {
    /// A classic script.
    #[default]
    Classic,
    /// `type=module`: deferred unless `async`, fetched in CORS mode with its imports.
    Module,
    /// `type=importmap`: inline JSON mapping module specifiers to URLs.
    ImportMap,
}

impl ScriptKind {
    /// Classic scripts carry no kind on the wire, as before modules existed.
    pub fn is_classic(&self) -> bool {
        *self == Self::Classic
    }
}

/// Meta-delivered policies one document keeps; later ones are dropped with a diagnostic.
pub const CSP_POLICIES: usize = 8;

/// Page scripts retain order while the coordinator resolves external source bytes.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScriptResource {
    /// Inline code or the resolved body of an accepted external classic script.
    pub source: String,
    /// External location remains private coordinator data until fetched.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub src: Option<String>,
    /// Execution phase: blocking first, then defer, then async, each in document order.
    #[serde(default)]
    pub mode: ScriptMode,
    /// A classic script, a module script or an import map.
    #[serde(default, skip_serializing_if = "ScriptKind::is_classic")]
    pub kind: ScriptKind,
    /// Subresource Integrity metadata; the coordinator verifies the fetched body against it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integrity: Option<String>,
    /// The element's `nonce`, matched against Content-Security-Policy `'nonce-…'` sources.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nonce: Option<String>,
    /// The element's own attributes (`id`, `data-*`, `type`), within the page's METADATA_BYTES,
    /// so the head record V9 gives the script answers `getElementById` and dataset reads the
    /// way the real element does (Google's sign-in locates its bundle by `id="base-js"`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attributes: BTreeMap<String, String>,
}

/// Native node identity is separate from its source HTML identifier.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Node {
    /// Document-local identity; the coordinator adds the page generation.
    pub local_id: usize,
    /// Used only by the supported script DOM operation.
    pub html_id: String,
    /// One of the supported text element tags.
    pub tag: String,
    /// Decoded UTF-8 text, with a bound shared by every mutation path.
    pub text: String,
    /// Preorder tree relationships retain native identities during text replacement.
    #[serde(default)]
    pub parent: Option<usize>,
    /// Input values remain literal data, independent of textContent.
    #[serde(default)]
    pub value: String,
    /// Relative addresses are resolved by the runtime against its committed URL.
    #[serde(default)]
    pub href: Option<String>,
    /// Supported V9 event source has no access to native agent authority.
    #[serde(default)]
    pub handler: Option<String>,
    /// Both native fields and agent inspection use this placeholder.
    #[serde(default)]
    pub placeholder: String,
    /// Disabled controls reject every input path.
    #[serde(default)]
    pub disabled: bool,
    /// Inert author attributes never select native classes, callbacks or authority.
    #[serde(default)]
    pub attributes: std::collections::BTreeMap<String, String>,
    /// Current control state is independent of the initial source attributes.
    #[serde(default)]
    pub checked: bool,
    /// Options retain current selection independently of source markup.
    #[serde(default)]
    pub selected: bool,
    /// Owned CSS supplies both paint and action visibility.
    #[serde(default, skip_serializing_if = "default_style")]
    pub style: ComputedStyle,
    /// Only implemented constraints produce a value-free associated error.
    #[serde(default)]
    pub validation: Option<String>,
    /// Image address remains private document data until governed navigation fetches it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_src: Option<String>,
    /// Alternative text is visible and inspectable when image bytes are unavailable.
    #[serde(default)]
    pub image_alt: String,
    /// Intrinsic dimensions are set by the coordinator after bounded decode.
    #[serde(default)]
    pub image_size: Option<(u32, u32)>,
    /// A custom element V9 upgraded, for `:defined` (uncustomized elements count as defined
    /// too; only an undefined custom element is not).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub defined: bool,
    /// Parsed cell relationships let inspection use the same stable native identities.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub table_cell: Option<TableCell>,
    /// Source whitespace directly followed this element. The parser drops whitespace-only text,
    /// so this is how inline layout tells `<b>a</b> <i>b</i>` (spaced) from `<b>a</b><i>b</i>`
    /// (adjacent, as syntax-highlighted code is).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub space_after: bool,
    /// Source whitespace began this element's content before any text or child, as in an
    /// otherwise empty `<span> </span>` spacer between two links.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub space_leading: bool,
    /// The dropped white space `space_leading` stands for, which preserved white space
    /// (`pre`, `pre-wrap`) lays out as written.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub leading_space: String,
    /// The page's focused element, as a script's `focus()` left it, for `:focus` and
    /// `:focus-within`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub focused: bool,
}

/// Table relationships are derived from bounded source order during parsing.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TableCell {
    /// Stable page-local table owner identity.
    pub table: usize,
    /// Stable page-local row owner identity.
    pub row: usize,
    /// One-based row position within the table.
    pub row_index: usize,
    /// One-based column position within the row.
    pub column_index: usize,
    /// Header node identities associated with this cell.
    pub headers: Vec<usize>,
}
/** Default style metadata need not consume private wire budget for every unstyled node. */
fn default_style(style: &ComputedStyle) -> bool {
    *style == ComputedStyle::default()
}
impl Node {
    /// Control semantics are explicit rather than inferred by AppKit.
    pub fn input_type(&self) -> &str {
        self.attributes
            .get("type")
            .map(String::as_str)
            .unwrap_or(if self.tag == "button" {
                "submit"
            } else {
                "text"
            })
    }
    /// Protected and hidden values stay inside the private document boundary.
    pub fn secret(&self) -> bool {
        self.tag == "input" && ["password", "hidden"].contains(&self.input_type())
    }
    /// Inherited visibility is resolved before native targeting.
    pub fn hidden(&self) -> bool {
        self.style.hidden
            || self.attributes.contains_key("hidden")
            || (self.tag == "input" && self.input_type() == "hidden")
    }
    /// Read-only fields submit but refuse editing.
    pub fn readonly(&self) -> bool {
        self.attributes.contains_key("readonly")
    }
}

/// Native layout uses named units so callers cannot mistake it for pixels.
#[derive(Debug, Serialize)]
pub struct Layout {
    /// Unicode scalar count, without font metrics or terminal width inference.
    pub unit: &'static str,
    /// One row per source element.
    pub rows: Vec<Row>,
}

/// One bounded text element's geometry.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    /// Retains identity for agent targeting.
    pub local_id: usize,
    /// All rows start in the first column in this subset.
    pub x: usize,
    /// Source-order row number.
    pub y: usize,
    /// Number of Unicode scalar values in the text.
    pub width: usize,
    /// Every text element occupies one abstract row.
    pub height: usize,
}

/// An invalid change leaves the entire input document unchanged.
pub fn apply(document: &Document, changes: &[TextChange]) -> EngineResult<Document> {
    validate_content(document)?;
    for change in changes {
        validate_change(document, change)?;
    }
    let mut candidate = document.clone();
    for change in changes {
        mutation::apply_text(&mut candidate, change)?;
    }
    validate_content(&candidate)?;
    Ok(candidate)
}

/** Every mutation shares the parser's text and exact-document target limits. */
fn validate_change(document: &Document, change: &TextChange) -> EngineResult<()> {
    validate_text(&change.text)?;
    let node = document
        .nodes
        .iter()
        .find(|node| node.html_id == change.html_id)
        .ok_or_else(invalid_target)?;
    if !text_writable(node) {
        return Err(EngineError::unsupported(
            "textContent cannot replace this control.",
        ));
    }
    Ok(())
}

/// Value editing never changes native identity or executes input text.
pub fn set_value(document: &Document, local: usize, value: &str) -> EngineResult<Document> {
    validate_content(document)?;
    validate_text(value)?;
    let mut candidate = document.clone();
    let node = candidate
        .nodes
        .iter_mut()
        .find(|node| node.local_id == local)
        .ok_or_else(invalid_target)?;
    if !["input", "textarea"].contains(&node.tag.as_str())
        || node.disabled
        || node.hidden()
        || node.readonly()
        || (node.tag == "input"
            && !["text", "password", "email", "search", "url", "tel"].contains(&node.input_type()))
    {
        return Err(EngineError::new(
            "invalid_target",
            "The target is not an enabled input.",
            "Inspect and choose an editable input.",
        ));
    }
    node.value = value.into();
    node.validation = None;
    validate_content(&candidate)?;
    Ok(candidate)
}

/** Both projected and actual changes enforce the same supported target semantics. */
fn text_writable(node: &Node) -> bool {
    !["input", "br"].contains(&node.tag.as_str())
}

/** Descendant replacement is bounded and never reuses deleted local identities. */
fn descendants(document: &Document, parent: usize) -> Vec<usize> {
    descendant_ids(
        document
            .nodes
            .iter()
            .map(|node| (node.local_id, node.parent)),
        parent,
    )
}

/** All mutation paths resolve exactly one current-document target. */
fn invalid_target() -> EngineError {
    EngineError::new(
        "invalid_target",
        "The node is not part of the current document.",
        "Inspect the current page and use a returned node ID.",
    )
}

/// Produces repeatable geometry without pretending to implement CSS.
pub fn layout(document: &Document) -> Layout {
    let rows = document
        .nodes
        .iter()
        .enumerate()
        .map(|(y, node)| Row {
            local_id: node.local_id,
            x: 0,
            y,
            width: node.text.chars().count(),
            height: 1,
        })
        .collect();
    Layout {
        unit: LAYOUT_UNIT,
        rows,
    }
}
