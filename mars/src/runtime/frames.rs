/**
 * Embedded documents: what an `<iframe>` (its `srcdoc`, else its `src`), an `<object data>` or
 * an `<embed src>` shows. Navigation loads each one through the same governed subresource path
 * images take (the page's exact-origin grants, deadline and authority; `data:` decodes
 * locally), parses it in the document service and keeps it, with its stylesheets, images and
 * own frames, beside the page. Every layout then styles and lays each one out in its frame's
 * content box, the frame's viewport for `vw`, `vh` and media queries, and Eye splices the result
 * into the page's display under the frame's clip, layer and identity range.
 *
 * An iframe whose `src` is another origin's document is that document's own page
 * (`frame_pages`): its scripts run in their own realm and talk to the embedder by postMessage.
 * Other embedded documents run no script, and a same-origin one is reachable from the page's
 * scripts as its contentDocument. None takes input yet. FRAME_COUNT frames per page, FRAME_DEPTH
 * levels of nesting and FRAME_BYTES_TOTAL source bytes bound the work; a frame past them, or
 * one that fails to load, stays an empty box (an object or embed shows its fallback).
 */
use super::images::{Budget, Fetched, ImageAsset, data_body, svg_address, svg_url, vector};
use super::navigation::Gaps;
use super::page::{Page, StylesCall};
use super::supervisor::Runtime;
use crate::document::{DisplayList, Document, FRAME_VIEWPORT_MIN, Node, RenderContext};
use crate::network::{Destination, WebRequest};
use crate::protocol::{EngineError, EngineResult};
use crate::v9::FrameDocument;
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

/// Frames one page may load, nested ones included.
pub(super) const FRAME_COUNT: usize = 16;
/// How deep frames nest: a frame's document may hold frames, theirs one more level.
pub(super) const FRAME_DEPTH: usize = 3;
/// Source bytes all of a page's frame documents may total: one page's worth.
pub(super) const FRAME_BYTES_TOTAL: usize = crate::constants::HTML_BYTES;
/// Each frame's boxes take identities this many bits above its own nodes', above any page
/// node identity, so neither painters nor agents mistake one for an embedder node.
const FRAME_ID_SHIFT: u32 = 32;
/// The viewport a frame's document is styled in to choose its images, before layout gives it
/// the frame's real content box: HTML's default 300 by 150 frame.
const DEFAULT_VIEWPORT: (f64, f64) = (300.0, 150.0);
/// The only HTTP status whose body is the embedded document.
pub(super) const HTTP_OK: u16 = 200;
/// Frame answers a page keeps in its environment, one per distinct URL its scripts asked for.
const SCRIPT_FRAME_ANSWERS: usize = 256;

/** One embedded document, ready to lay out in its frame. */
#[derive(Clone)]
pub(super) struct Frame {
    /// The embedder's frame element.
    owner: usize,
    /// The parsed document, its images already resolved.
    document: Document,
    /// Its URL, which its relative addresses and nested frames resolve against.
    url: url::Url,
    /// Its governed stylesheet sources.
    sheets: Vec<String>,
    /// What its boxes add to their identities (see FRAME_ID_SHIFT).
    id_base: usize,
    /// Frames inside it.
    frames: Vec<Frame>,
    /// The frame's own page when its document runs scripts (`frame_pages`); `document` and
    /// `sheets` then follow that page's after every step.
    pub(super) page: Option<Box<Page>>,
}

/** What a frame's own page takes from its embedder: the embedder's origin (its `parent`), the
frame's `name` (its `window.name`) and a script identity of its own, so V9 keeps its state apart
from the embedder's. */
pub(super) struct Embedding {
    pub origin: String,
    pub name: String,
    pub identity: String,
}
impl Frame {
    /** The frame of iframe `owner` showing its own `page`, the page's images added to `images`
    under the frame's identity range. */
    pub(super) fn of_page(
        owner: usize,
        page: Page,
        id_base: usize,
        images: &mut BTreeMap<usize, ImageAsset>,
    ) -> Self {
        images.extend(
            page.images
                .iter()
                .map(|(id, asset)| (id + id_base, asset.clone())),
        );
        Self {
            owner,
            document: page.document.clone(),
            url: url::Url::parse(&page.url).ok().unwrap_or_else(blank),
            sheets: page.resolved_stylesheets.clone(),
            id_base,
            frames: Vec::new(),
            page: Some(Box::new(page)),
        }
    }
    /** Takes the document, sheets and images its page has now, replacing the frame's images. */
    pub(super) fn follow_page(&mut self, images: &mut BTreeMap<usize, ImageAsset>) {
        let Some(page) = &self.page else {
            return;
        };
        self.document.clone_from(&page.document);
        self.sheets.clone_from(&page.resolved_stylesheets);
        let range = self.id_base..self.id_base + (1 << FRAME_ID_SHIFT);
        images.retain(|id, _| !range.contains(id));
        images.extend(
            page.images
                .iter()
                .map(|(id, asset)| (id + self.id_base, asset.clone())),
        );
    }
    /** The iframe element this frame shows in. */
    pub(super) fn owner(&self) -> usize {
        self.owner
    }
    /** The first identity past this frame's range, where a frame added later may start. */
    pub(super) fn id_end(&self) -> usize {
        self.id_base + (1 << FRAME_ID_SHIFT)
    }
    /** The display identity assigned to this document's local nodes. */
    pub(super) fn id_base(&self) -> usize {
        self.id_base
    }
    /** Resolve a painted identity only inside this frame's reserved range. */
    pub(super) fn node(&self, painted: usize) -> Option<&Node> {
        (self.id_base..self.id_end()).contains(&painted).then(|| {
            self.document
                .nodes
                .iter()
                .find(|node| node.local_id == painted - self.id_base)
        })?
    }
    /** The styled document the window uses to hit test the frame's painted nodes. */
    pub(super) fn document(&self) -> &Document {
        &self.document
    }
    /** Resolve a frame link against its own address, never the embedder's. */
    pub(super) fn resolve(&self, href: &str) -> Option<String> {
        self.url.join(href).ok().map(|url| url.into())
    }
}

/** about:blank, for a frame page whose address no longer parses. */
fn blank() -> url::Url {
    url::Url::parse("about:blank").expect("about:blank parses")
}

/** A page's frames, shared by every copy of the page. */
pub(super) type Frames = Arc<Vec<Frame>>;

/** Where a frame element's content comes from. */
enum Source {
    /// An iframe's `srcdoc` markup, in the embedder's base URL.
    Markup(String),
    /// An address, and the `type` the element declared.
    Address(String, Option<String>),
}
/** What one frame source produced. */
enum Content {
    /// An HTML or XHTML document and its final URL.
    Document(Box<Document>, url::Url),
    /// An SVG drawing, painted by Eye as an image is.
    Drawing(String, (u32, u32)),
}
/** The page-wide frame allowance left. */
#[derive(Default)]
struct Allowance {
    /// Frames claimed so far.
    count: usize,
    /// Frame source bytes admitted so far.
    bytes: usize,
    /// The documents of the page's own frames that scripts may reach, for the environment.
    scripts: Vec<FrameDocument>,
}

/** The content source of a frame element, if it names one. */
fn source(node: &Node) -> Option<Source> {
    let attribute = |name: &str| {
        node.attributes
            .get(name)
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
    };
    let declared = || attribute("type").map(str::to_ascii_lowercase);
    match node.tag.as_str() {
        "iframe" if node.attributes.contains_key("srcdoc") => {
            Some(Source::Markup(node.attributes["srcdoc"].clone()))
        }
        "iframe" => attribute("src")
            .filter(|src| !src.eq_ignore_ascii_case("about:blank"))
            .map(|src| Source::Address(src.into(), None)),
        "object" => attribute("data").map(|src| Source::Address(src.into(), declared())),
        "embed" => attribute("src").map(|src| Source::Address(src.into(), declared())),
        _ => None,
    }
}

impl Runtime {
    /**
     * Loads the documents the frames of the styled `document` show, depth first in document
     * order, adding their images to `images` under their identity ranges. An object or embed
     * that loaded is marked `embedded`, so Eye lays it out as a replaced box; one showing a
     * drawing carries it as an img carries an SVG. Also returns the documents of the page's own
     * same-origin frames, which V9 hands to scripts.
     */
    pub(super) fn load_frames(
        &mut self,
        document: &mut Document,
        base: &str,
        deadline: Instant,
        images: &mut BTreeMap<usize, ImageAsset>,
    ) -> EngineResult<(Vec<Frame>, Vec<FrameDocument>)> {
        let base =
            url::Url::parse(base).map_err(|_| EngineError::invalid("Invalid frame base URL."))?;
        let mut allowance = Allowance::default();
        let frames = self.frames_in(document, &base, deadline, images, &mut allowance, 0)?;
        Ok((frames, allowance.scripts))
    }
    /** The frames of one document at nesting `depth`. */
    fn frames_in(
        &mut self,
        document: &mut Document,
        base: &url::Url,
        deadline: Instant,
        images: &mut BTreeMap<usize, ImageAsset>,
        allowance: &mut Allowance,
        depth: usize,
    ) -> EngineResult<Vec<Frame>> {
        let mut frames = Vec::new();
        if depth >= FRAME_DEPTH {
            return Ok(frames);
        }
        let owners: Vec<(usize, Source)> = (document.nodes.iter().enumerate())
            .filter(|(_, node)| !node.hidden())
            .filter_map(|(index, node)| Some((index, source(node)?)))
            .collect();
        for (index, source) in owners {
            if allowance.count >= FRAME_COUNT {
                break;
            }
            allowance.count += 1;
            let id_base = allowance.count << FRAME_ID_SHIFT;
            if depth == 0
                && let Source::Address(address, None) = &source
                && document.nodes[index].tag == "iframe"
            {
                let owner = document.nodes[index].local_id;
                let attributes = document.nodes[index].attributes.clone();
                let opened =
                    self.frame_page(owner, address, &attributes, base, id_base, deadline, images)?;
                if let Some(frame) = opened {
                    document.nodes[index].embedded = true;
                    frames.push(frame);
                    continue;
                }
            }
            let node = &mut document.nodes[index];
            let Some((content, script)) = self.frame_content(source, base, deadline, allowance)?
            else {
                continue;
            };
            if depth == 0 {
                allowance.scripts.extend(script);
            }
            match content {
                Content::Drawing(markup, size) => {
                    node.image_src = Some(svg_url(&markup));
                    node.image_size = Some(size);
                    node.embedded = true;
                }
                Content::Document(child, url) => {
                    node.embedded = true;
                    let owner = node.local_id;
                    let frame = self.frame(*child, owner, &url, id_base, deadline, images)?;
                    frames.push(self.nest(frame, deadline, images, allowance, depth)?);
                }
            }
        }
        Ok(frames)
    }
    /** `frame` with the frames of its own document loaded, one level deeper. */
    fn nest(
        &mut self,
        mut frame: Frame,
        deadline: Instant,
        images: &mut BTreeMap<usize, ImageAsset>,
        allowance: &mut Allowance,
        depth: usize,
    ) -> EngineResult<Frame> {
        let base = frame.url.clone();
        frame.frames = self.frames_in(
            &mut frame.document,
            &base,
            deadline,
            images,
            allowance,
            depth + 1,
        )?;
        Ok(frame)
    }
    /** A frame for the parsed `document` at `url`: its stylesheets, then its images styled at
    the default frame size, under the page's image budget. */
    fn frame(
        &mut self,
        mut document: Document,
        owner: usize,
        url: &url::Url,
        id_base: usize,
        deadline: Instant,
        images: &mut BTreeMap<usize, ImageAsset>,
    ) -> EngineResult<Frame> {
        document.scripts.clear();
        let sheets = self.stylesheets(&document, url.as_str(), deadline, &mut Gaps::default())?;
        let (width, height) = DEFAULT_VIEWPORT;
        let context = RenderContext {
            width,
            height,
            ..self.render_context
        };
        let mut styled = self.frame_styled(&document, &sheets, context)?;
        let mut budget = Budget::spent(images);
        let loaded = self.load_images(
            &mut styled,
            url,
            deadline,
            &mut budget,
            &|_| true,
            &mut Vec::new(),
        )?;
        images.extend(loaded.into_iter().map(|(id, asset)| (id + id_base, asset)));
        Ok(Frame {
            owner,
            document: styled,
            url: url.clone(),
            sheets,
            id_base,
            frames: Vec::new(),
            page: None,
        })
    }
    /** What one frame source shows, with the document scripts may reach when the frame is
    same-origin; None when it names nothing Mars can embed or fails to load (only authority,
    service and deadline failures stop the navigation). */
    fn frame_content(
        &mut self,
        source: Source,
        base: &url::Url,
        deadline: Instant,
        allowance: &mut Allowance,
    ) -> EngineResult<Option<(Content, Option<FrameDocument>)>> {
        let fetched = match source {
            Source::Markup(markup) => Ok((markup, "text/html".into(), base.clone(), None, None)),
            Source::Address(address, declared) => {
                let requested = base.join(&address).ok();
                self.frame_source(&address, base, deadline)
                    .map(|(text, mime, url, charset)| {
                        let mime = declared.filter(|_| mime.is_empty()).unwrap_or(mime);
                        (text, mime, url, charset, requested)
                    })
            }
        };
        let (text, mime, url, charset, requested) = match fetched {
            Ok(fetched) => fetched,
            Err(error) if super::images::stops_loading(&error) => return Err(error),
            Err(_) => return Ok(None),
        };
        if Instant::now() >= deadline {
            return Err(EngineError::navigation_timeout());
        }
        if allowance.bytes + text.len() > FRAME_BYTES_TOTAL {
            return Ok(None);
        }
        allowance.bytes += text.len();
        let Some(content) = self.parse_content(&text, &mime, url.clone(), charset)? else {
            return Ok(None);
        };
        let script = script_document(base, requested.as_ref(), &url, text, &mime);
        Ok(Some((content, script)))
    }
    /** A frame document's text, media type, final URL and declared charset: a `data:` URL
    decodes here, anything else is one governed document request from the embedding page. */
    fn frame_source(
        &mut self,
        address: &str,
        base: &url::Url,
        deadline: Instant,
    ) -> EngineResult<(String, String, url::Url, Option<String>)> {
        let url = base
            .join(address)
            .map_err(|_| EngineError::invalid("Invalid frame URL."))?;
        if url.scheme() == "data" {
            let (mime, bytes) = data_body(url.as_str(), FRAME_BYTES_TOTAL)?;
            let text = String::from_utf8(bytes)
                .map_err(|_| EngineError::unsupported("The frame is not UTF-8 text."))?;
            return Ok((text, mime, base.clone(), None));
        }
        let request = WebRequest {
            destination: Destination::Document,
            ..WebRequest::get(url.as_str())
        };
        let acquired = self.acquire(request, deadline, Some(base.clone()), false, true)?;
        let response = acquired.response;
        if response.status != HTTP_OK || response.image_data {
            return Err(EngineError::unsupported("The frame could not be loaded."));
        }
        let mime = if svg_address(&url) && response.mime.is_empty() {
            "image/svg+xml".into()
        } else {
            response.mime
        };
        let final_url = url::Url::parse(&response.url).unwrap_or(url);
        Ok((response.source, mime, final_url, response.charset))
    }
    /** HTML and XML documents parse in the document service; SVG becomes a drawing. */
    fn parse_content(
        &mut self,
        text: &str,
        mime: &str,
        url: url::Url,
        charset: Option<String>,
    ) -> EngineResult<Option<Content>> {
        if mime == "image/svg+xml" {
            return Ok(match vector(mime, text) {
                Ok(Fetched::Vector(markup, size)) => Some(Content::Drawing(markup, size)),
                _ => None,
            });
        }
        let xml = crate::document::xml_mime(mime);
        if mime != "text/html" && !xml {
            return Ok(None);
        }
        let parse = json!({"source": text, "charset": charset, "xml": xml});
        let document = match self.call_typed("document", "document.parse", &parse, None) {
            Ok(document) => document,
            Err(error) if super::images::stops_loading(&error) => return Err(error),
            Err(_) => return Ok(None),
        };
        Ok(Some(Content::Document(Box::new(document), url)))
    }
    /** One cascade of a frame's document under its frame viewport `context`. */
    fn frame_styled(
        &mut self,
        document: &Document,
        sheets: &[String],
        context: RenderContext,
    ) -> EngineResult<Document> {
        let call = StylesCall {
            document,
            sheets,
            context,
        };
        self.call_typed("document", "document.frame_styles", &call, None)
    }
    /**
     * Lays out each of `frames` in the content box its frame element got in `display` and
     * splices it in. A frame whose element painted no box, or whose content box is under
     * FRAME_VIEWPORT_MIN on a side, shows nothing.
     */
    pub(super) fn embed_frames(
        &mut self,
        display: &mut DisplayList,
        frames: &[Frame],
    ) -> EngineResult<()> {
        let mut placed: Vec<(usize, &Frame)> = frames
            .iter()
            .filter_map(|frame| {
                let at = display
                    .items
                    .iter()
                    .position(|item| item.local_id == frame.owner)?;
                Some((at, frame))
            })
            .collect();
        // Later boxes first, so each splice leaves the earlier positions in place.
        placed.sort_by(|a, b| b.0.cmp(&a.0));
        for (at, frame) in placed {
            let [_, _, width, height] = crate::document::frame_viewport(&display.items[at]);
            if width < FRAME_VIEWPORT_MIN || height < FRAME_VIEWPORT_MIN {
                continue;
            }
            let (child, document) = self.lay_out_frame(frame, width, height)?;
            crate::document::embed_frame(display, at, child, &document, frame.id_base);
        }
        Ok(())
    }
    /** One frame's document styled and laid out at its viewport size, its own frames spliced. */
    fn lay_out_frame(
        &mut self,
        frame: &Frame,
        width: f64,
        height: f64,
    ) -> EngineResult<(DisplayList, Document)> {
        let context = RenderContext {
            width: width.min(MAX_FRAME_VIEWPORT),
            height: height.min(MAX_FRAME_VIEWPORT),
            ..self.render_context
        };
        let styled = self.frame_styled(&frame.document, &frame.sheets, context)?;
        let call = RenderCall {
            document: &styled,
            context,
        };
        let mut child: DisplayList =
            self.call_typed("document", "document.frame_render", &call, None)?;
        self.embed_frames(&mut child, &frame.frames)?;
        Ok((child, styled))
    }
}
/// The largest frame viewport side: Eye's window bound, past which a frame lays its document
/// out at this size and clips the rest.
const MAX_FRAME_VIEWPORT: f64 = 4096.0;

/** document.frame_render parameters, written straight to the frame. */
#[derive(serde::Serialize)]
struct RenderCall<'a> {
    /// The frame's styled document.
    document: &'a Document,
    /// Its viewport: the frame's content box size.
    context: RenderContext,
}

/** Whether two addresses share an origin. An opaque origin, as a `data:` address has, shares none. */
pub(super) fn same_origin(page: &url::Url, other: &url::Url) -> bool {
    page.origin() == other.origin()
}

/** The document a same-origin frame exposes to scripts: its source, keyed by the address it
requested. Cross-origin frames still load and paint, but no script reaches them. */
fn script_document(
    page: &url::Url,
    requested: Option<&url::Url>,
    final_url: &url::Url,
    text: String,
    media_type: &str,
) -> Option<FrameDocument> {
    let requested = requested?;
    (same_origin(page, requested) && same_origin(page, final_url)).then(|| FrameDocument {
        address: requested.as_str().to_string(),
        media_type: media_type.to_string(),
        source: Some(text),
    })
}

impl Runtime {
    /**
     * Answers the frame documents a V9 step asked for, recording each answer in the page's
     * environment so the next step's `load` event can fire. `give_up` (the module rounds ran out)
     * or the deadline refuses everything that is still pending. A refused frame of another
     * origin then loads as a page of its own (`attach_frame_pages`); returns whether one did.
     */
    pub(super) fn fetch_frames(
        &mut self,
        page: &mut Page,
        deadline: Instant,
        give_up: bool,
    ) -> EngineResult<bool> {
        let requests = std::mem::take(&mut page.scripting.modules.frame_requests);
        let mut others = Vec::new();
        for request in requests {
            let answer = self.answer_frame(page, &request.url, deadline, give_up)?;
            if answer.source.is_none() && !give_up {
                others.push(request.url.clone());
            }
            page.scripting
                .environment
                .frame_documents
                .insert(request.url, answer);
        }
        self.attach_frame_pages(page, &others, deadline)
    }
    /** The answer to one frame request: a same-origin document of a type Mars parses, within the
    page's frame bytes, or a refusal. The frame is never fetched cross-origin. */
    fn answer_frame(
        &mut self,
        page: &mut Page,
        address: &str,
        deadline: Instant,
        give_up: bool,
    ) -> EngineResult<FrameDocument> {
        let refused = FrameDocument {
            address: address.to_string(),
            media_type: String::new(),
            source: None,
        };
        let (Ok(requested), Ok(page_url)) = (url::Url::parse(address), url::Url::parse(&page.url))
        else {
            return Ok(refused);
        };
        let answered = page.scripting.environment.frame_documents.len();
        if give_up || Instant::now() >= deadline || answered >= SCRIPT_FRAME_ANSWERS {
            return Ok(refused);
        }
        if !same_origin(&page_url, &requested) {
            return Ok(refused);
        }
        let (text, media_type, final_url, _) = match self.frame_source(address, &page_url, deadline)
        {
            Ok(fetched) => fetched,
            Err(error) if super::images::stops_loading(&error) => return Err(error),
            Err(_) => return Ok(refused),
        };
        let parsed = media_type == "text/html" || crate::document::xml_mime(&media_type);
        let fits = page.scripting.modules.frame_bytes + text.len() <= FRAME_BYTES_TOTAL;
        if !parsed || !fits || !same_origin(&page_url, &final_url) {
            return Ok(refused);
        }
        page.scripting.modules.frame_bytes += text.len();
        Ok(FrameDocument {
            address: address.to_string(),
            media_type,
            source: Some(text),
        })
    }
}

/// The frame-document rule, exposed to the frame tests in this module's sibling file.
#[cfg(test)]
pub(super) fn script_document_for_tests(
    page: &url::Url,
    requested: Option<&url::Url>,
    final_url: &url::Url,
    text: &str,
    media_type: &str,
) -> Option<FrameDocument> {
    script_document(page, requested, final_url, text.to_string(), media_type)
}

#[cfg(test)]
#[path = "frame_tests.rs"]
mod frame_tests;
