/**
 * Embedded documents: what an `<iframe>` (its `srcdoc`, else its `src`), an `<object data>` or
 * an `<embed src>` shows. Navigation loads each one through the same governed subresource path
 * images take (the page's exact-origin grants, deadline and authority; `data:` decodes
 * locally), parses it in the document service and keeps it, with its stylesheets, images and
 * own frames, beside the page. Every layout then styles and lays each one out in its frame's
 * content box, the frame's viewport for `vw`, `vh` and media queries, and Eye splices the result
 * into the page's display under the frame's clip, layer and identity range.
 *
 * Embedded documents run no script and take no input: Mars has no nested browsing contexts, so
 * a page's scripts never reach a frame's document. FRAME_COUNT frames per page, FRAME_DEPTH
 * levels of nesting and FRAME_BYTES_TOTAL source bytes bound the work; a frame past them, or
 * one that fails to load, stays an empty box (an object or embed shows its fallback).
 */
use super::images::{Budget, Fetched, ImageAsset, data_body, svg_address, svg_url, vector};
use super::navigation::Gaps;
use super::page::StylesCall;
use super::supervisor::Runtime;
use crate::document::{DisplayList, Document, FRAME_VIEWPORT_MIN, Node, RenderContext};
use crate::network::{Destination, WebRequest};
use crate::protocol::{EngineError, EngineResult};
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
const HTTP_OK: u16 = 200;

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
     * drawing carries it as an img carries an SVG.
     */
    pub(super) fn load_frames(
        &mut self,
        document: &mut Document,
        base: &str,
        deadline: Instant,
        images: &mut BTreeMap<usize, ImageAsset>,
    ) -> EngineResult<Vec<Frame>> {
        let base =
            url::Url::parse(base).map_err(|_| EngineError::invalid("Invalid frame base URL."))?;
        let mut allowance = Allowance::default();
        self.frames_in(document, &base, deadline, images, &mut allowance, 0)
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
            let node = &mut document.nodes[index];
            match self.frame_content(source, base, deadline, allowance)? {
                Some(Content::Drawing(markup, size)) => {
                    node.image_src = Some(svg_url(&markup));
                    node.image_size = Some(size);
                    node.embedded = true;
                }
                Some(Content::Document(child, url)) => {
                    node.embedded = true;
                    let owner = node.local_id;
                    let frame = self.frame(*child, owner, &url, id_base, deadline, images)?;
                    frames.push(self.nest(frame, deadline, images, allowance, depth)?);
                }
                None => {}
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
        })
    }
    /** What one frame source shows; None when it names nothing Mars can embed or fails to load
    (only authority, service and deadline failures stop the navigation). */
    fn frame_content(
        &mut self,
        source: Source,
        base: &url::Url,
        deadline: Instant,
        allowance: &mut Allowance,
    ) -> EngineResult<Option<Content>> {
        let fetched = match source {
            Source::Markup(markup) => Ok((markup, "text/html".into(), base.clone(), None)),
            Source::Address(address, declared) => {
                self.frame_source(&address, base, deadline)
                    .map(|(text, mime, url, charset)| {
                        (
                            text,
                            declared.filter(|_| mime.is_empty()).unwrap_or(mime),
                            url,
                            charset,
                        )
                    })
            }
        };
        let (text, mime, url, charset) = match fetched {
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
        self.parse_content(&text, &mime, url, charset)
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
