/**
 * Page domain: navigation and the events a committed document produces, told the way Blink's
 * InspectorPageAgent and InspectorNetworkAgent tell them (frameNavigated, execution contexts,
 * the lifecycle through networkIdle, the document request's Network events), screenshots with
 * clips, scripts evaluated on every new document, history and the frame tree. Mars commits whole
 * documents, so every event of a navigation fires at once, before the command's answer.
 */
use super::{Fault, Handler, Session, network};
use serde_json::{Value, json};
use std::io::Cursor;

/// Chrome's error when a navigation's URL does not parse.
const INVALID_URL: &str = "Cannot navigate to invalid URL";
/// Lifecycle names Blink reports for one committed document, in order.
const LIFECYCLE: &[&str] = &[
    "init",
    "DOMContentLoaded",
    "load",
    "networkAlmostIdle",
    "networkIdle",
];
/// Screenshot encodings Mars produces.
const PNG: &str = "png";
/// Chrome's referrer policy default, reported on every request.
const REFERRER_POLICY: &str = "strict-origin-when-cross-origin";

/** The Page commands. */
pub(super) const HANDLERS: &[(&str, Handler)] = &[
    ("Page.getFrameTree", Session::frame_tree),
    ("Page.navigate", Session::navigate),
    ("Page.reload", Session::reload),
    ("Page.getNavigationHistory", Session::navigation_history),
    ("Page.navigateToHistoryEntry", Session::navigate_history),
    ("Page.captureScreenshot", Session::screenshot),
    ("Page.createIsolatedWorld", Session::isolated_world),
    (
        "Page.addScriptToEvaluateOnNewDocument",
        Session::new_document_script,
    ),
    (
        "Page.addScriptToEvaluateOnLoad",
        Session::new_document_script,
    ),
    (
        "Page.removeScriptToEvaluateOnNewDocument",
        Session::remove_new_document_script,
    ),
    ("Page.setDocumentContent", Session::set_document_content),
    ("Page.close", Session::page_close),
    ("Page.handleJavaScriptDialog", Session::dialog),
    ("Page.getLayoutMetrics", Session::layout_metrics),
];

impl Session {
    /** The frame of the tab's current document. */
    fn frame(&self, tab: &str, url: &str) -> Value {
        let loader = self.targets.get(tab).map_or(0, |target| target.loader);
        let parsed = url::Url::parse(url).ok();
        let origin = parsed
            .as_ref()
            .map(|parsed| parsed.origin().ascii_serialization())
            .unwrap_or_default();
        let host = parsed
            .as_ref()
            .and_then(|parsed| parsed.host_str())
            .unwrap_or_default();
        let secure = parsed.as_ref().is_some_and(|parsed| {
            parsed.scheme() == "https" || ["localhost", "127.0.0.1", "[::1]"].contains(&host)
        });
        json!({"id":tab,"loaderId":loader_id(loader),"url":url.split('#').next().unwrap_or_default(),"domainAndRegistry":host,"securityOrigin":origin,"mimeType":"text/html","secureContextType":if secure {"Secure"} else {"InsecureScheme"},"crossOriginIsolatedContextType":"NotIsolated","gatedAPIFeatures":[]})
    }

    /** An execution context description. */
    pub(super) fn context_description(
        id: u64,
        tab: &str,
        url: &str,
        name: &str,
        default: bool,
    ) -> Value {
        let origin = url::Url::parse(url)
            .map(|parsed| parsed.origin().ascii_serialization())
            .unwrap_or_default();
        json!({"id":id,"origin":origin,"name":name,"uniqueId":format!("{id}.{}", tab.trim_start_matches("tab-")),"auxData":{"isDefault":default,"type":if default {"default"} else {"isolated"},"frameId":tab}})
    }

    /**
     * A committed navigation, told the way Chrome tells it: the document request's Network
     * events, frameNavigated, contexts replaced, lifecycle through networkIdle. Then the scripts
     * registered for new documents run, and the page's console output so far is reported.
     */
    pub(super) fn navigated(&mut self, tab: &str, page: &Value) {
        let url = page["url"].as_str().unwrap_or("about:blank").to_owned();
        let status = page["status"].as_u64().unwrap_or(200);
        let context = self.new_context(tab);
        let worlds: Vec<String> = self.world_names();
        // The request was announced before the navigation paused, or is announced now.
        let announced = self
            .targets
            .get_mut(tab)
            .and_then(|target| target.announced.take());
        if announced.is_none() {
            self.announce_request(tab, &url);
        }
        let loader = {
            let target = self.targets.entry(tab.to_owned()).or_default();
            target.loader += 1;
            target.context = context;
            target.focused = None;
            target.touched = None;
            target.console_seen = 0;
            target.worlds.clear();
            target.nodes.clear();
            target.page_id = page["page_id"].as_str().unwrap_or_default().to_owned();
            target.loader
        };
        let request_id = loader_id(loader);
        let timestamp = self.timestamp();
        self.emit(Some(tab), "Network.responseReceived", json!({
            "requestId": request_id, "loaderId": request_id, "timestamp": timestamp, "type": "Document", "frameId": tab,
            "hasExtraInfo": false,
            "response": network::response_object(&url, status, "text/html"),
        }));
        let frame = self.frame(tab, &url);
        self.emit(
            Some(tab),
            "Page.frameNavigated",
            json!({"frame":frame,"type":"Navigation"}),
        );
        self.emit(Some(tab), "Runtime.executionContextsCleared", json!({}));
        self.emit(
            Some(tab),
            "Runtime.executionContextCreated",
            json!({"context":Self::context_description(context, tab, &url, "", true)}),
        );
        for name in worlds {
            self.create_world(tab, &name, &url);
        }
        self.emit(
            Some(tab),
            "Network.loadingFinished",
            json!({"requestId":request_id,"timestamp":timestamp,"encodedDataLength":0}),
        );
        self.emit(
            Some(tab),
            "Page.domContentEventFired",
            json!({"timestamp":timestamp}),
        );
        for name in LIFECYCLE {
            self.emit(
                Some(tab),
                "Page.lifecycleEvent",
                json!({"frameId":tab,"loaderId":request_id,"name":name,"timestamp":timestamp}),
            );
            if *name == "load" {
                self.emit(
                    Some(tab),
                    "Page.loadEventFired",
                    json!({"timestamp":timestamp}),
                );
            }
        }
        self.emit(
            Some(tab),
            "Page.frameStoppedLoading",
            json!({"frameId":tab}),
        );
        self.run_new_document_scripts(tab);
        self.report_console(tab);
    }

    /**
     * Network.requestWillBeSent for the next document request of a tab, with the loader id it
     * will carry (the navigation request's id is its loader id, as in Chrome). Announced once:
     * a paused navigation announces before it pauses, the commit does not repeat it.
     */
    pub(super) fn announce_request(&mut self, tab: &str, url: &str) -> String {
        let target = self.targets.entry(tab.to_owned()).or_default();
        let request_id = loader_id(target.loader + 1);
        target.announced = Some(request_id.clone());
        let timestamp = self.timestamp();
        self.emit(
            Some(tab),
            "Page.frameStartedLoading",
            json!({"frameId":tab}),
        );
        self.emit(Some(tab), "Network.requestWillBeSent", json!({
            "requestId": request_id, "loaderId": request_id, "documentURL": url,
            "request": {"url": url, "method": "GET", "headers": self.extra_headers, "initialPriority": "VeryHigh", "referrerPolicy": REFERRER_POLICY},
            "timestamp": timestamp, "wallTime": Self::wall_time(), "initiator": {"type": "other"},
            "redirectHasExtraInfo": false, "type": "Document", "frameId": tab, "hasUserGesture": false,
        }));
        request_id
    }

    /** A navigation that failed before commit: Chrome reports it on the request. */
    fn navigation_failed(&mut self, tab: &str, url: &str, fault: &Fault) -> Value {
        let announced = self
            .targets
            .get_mut(tab)
            .and_then(|target| target.announced.take());
        let request_id = match announced {
            Some(request_id) => request_id,
            None => self.announce_request(tab, url),
        };
        if let Some(target) = self.targets.get_mut(tab) {
            target.loader += 1;
            target.announced = None;
        }
        let timestamp = self.timestamp();
        self.emit(Some(tab), "Network.loadingFailed", json!({"requestId":request_id,"timestamp":timestamp,"type":"Document","errorText":error_text(fault),"canceled":false}));
        self.emit(
            Some(tab),
            "Page.frameStoppedLoading",
            json!({"frameId":tab}),
        );
        json!({"frameId":tab,"loaderId":request_id,"errorText":error_text(fault)})
    }

    /** Distinct world names registered through addScriptToEvaluateOnNewDocument. */
    fn world_names(&self) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for (_, _, world) in &self.new_document_scripts {
            if !world.is_empty() && !names.contains(world) {
                names.push(world.clone());
            }
        }
        names
    }

    /** Announce an isolated world on a tab; it shares the one realm with the main world. */
    fn create_world(&mut self, tab: &str, name: &str, url: &str) -> u64 {
        let id = self.new_context(tab);
        if let Some(target) = self.targets.get_mut(tab) {
            target.worlds.push((id, name.to_owned()));
        }
        self.emit(
            Some(tab),
            "Runtime.executionContextCreated",
            json!({"context":Self::context_description(id, tab, url, name, false)}),
        );
        id
    }

    /** Evaluate the registered scripts in the new document, as Blink does right after commit. */
    fn run_new_document_scripts(&mut self, tab: &str) {
        let scripts: Vec<String> = self
            .new_document_scripts
            .iter()
            .map(|(_, source, _)| source.clone())
            .filter(|source| !source.trim().is_empty())
            .collect();
        for source in scripts {
            let _ = self.devtools(
                tab,
                json!({"kind":"evaluate","expression":source,"options":{"return_by_value":true}}),
            );
        }
    }

    /** Page.getFrameTree: one frame; Mars has no child frames. */
    fn frame_tree(&mut self, session: Option<&str>, _: &Value) -> Result<Value, Fault> {
        let tab = self.tab(session)?;
        let url = self.current_url(&tab);
        Ok(json!({"frameTree":{"frame":self.frame(&tab, &url),"childFrames":[]}}))
    }

    /** The tab's current URL as agents see it. */
    pub(super) fn current_url(&mut self, tab: &str) -> String {
        self.call_tab(tab, "page.wait", json!({"after_revision":0}))
            .ok()
            .and_then(|page| page["url"].as_str().map(str::to_owned))
            .unwrap_or_else(|| "about:blank".into())
    }

    /** Page.navigate through native page.navigate, paused first when Fetch asked for it. */
    fn navigate(&mut self, session: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let tab = self.tab(session)?;
        let url = params["url"]
            .as_str()
            .ok_or_else(|| Fault::invalid("url is required."))?
            .to_owned();
        if url::Url::parse(&url).is_err() {
            return Err(Fault::server(INVALID_URL));
        }
        if self.pause_navigation(&tab, &url)? {
            return Err(Fault::deferred());
        }
        self.perform_navigation(&tab, "page.navigate", json!({"url":url}))
    }

    /** Run a navigation operation and answer as Chrome does: ids on success, errorText on failure. */
    pub(super) fn perform_navigation(
        &mut self,
        tab: &str,
        op: &str,
        params: Value,
    ) -> Result<Value, Fault> {
        let url = params["url"].as_str().unwrap_or_default().to_owned();
        match self.mutate(tab, op, params) {
            Ok(_) => {
                let loader = self.targets.get(tab).map_or(0, |target| target.loader);
                Ok(json!({"frameId":tab,"loaderId":loader_id(loader)}))
            }
            Err(fault) if fault.denied() => Err(fault),
            Err(fault) => Ok(self.navigation_failed(tab, &url, &fault)),
        }
    }

    /** Page.reload through native page.reload. */
    fn reload(&mut self, session: Option<&str>, _: &Value) -> Result<Value, Fault> {
        let tab = self.tab(session)?;
        self.mutate(&tab, "page.reload", json!({}))?;
        Ok(json!({}))
    }

    /**
     * Page.getNavigationHistory: Mars keeps its history's URLs private, so the entries are the
     * current page by URL and the others by position; navigateToHistoryEntry moves by position.
     */
    fn navigation_history(&mut self, session: Option<&str>, _: &Value) -> Result<Value, Fault> {
        let tab = self.tab(session)?;
        let page = self.call_tab(&tab, "page.summary", json!({}))?;
        let history = self.call_tab(&tab, "page.history", json!({}))?;
        let length = history["length"].as_u64().unwrap_or(1).max(1);
        let index = history["index"].as_u64().unwrap_or(0).min(length - 1);
        let entries: Vec<Value> = (0..length)
            .map(|position| {
                let current = position == index;
                json!({"id":position + 1,"url":if current {page["url"].clone()} else {json!("")},"userTypedURL":if current {page["url"].clone()} else {json!("")},"title":if current {page["title"].clone()} else {json!("")},"transitionType":"typed"})
            })
            .collect();
        Ok(json!({"currentIndex":index,"entries":entries}))
    }

    /** Page.navigateToHistoryEntry: back or forward by the entry's distance from the current one. */
    fn navigate_history(&mut self, session: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let tab = self.tab(session)?;
        let history = self.call_tab(&tab, "page.history", json!({}))?;
        let index = history["index"].as_i64().unwrap_or(0);
        let wanted = params["entryId"].as_i64().unwrap_or(index + 1) - 1;
        let op = match wanted.cmp(&index) {
            std::cmp::Ordering::Less => "page.back",
            std::cmp::Ordering::Greater => "page.forward",
            std::cmp::Ordering::Equal => "page.reload",
        };
        for _ in 0..(wanted - index).unsigned_abs().max(1) {
            self.mutate(&tab, op, json!({}))?;
            if op == "page.reload" {
                break;
            }
        }
        Ok(json!({}))
    }

    /**
     * Page.captureScreenshot: a PNG of the viewport, or of a clip. A clip is cut out of the
     * viewport capture (Chrome's clip is in CSS pixels of the page; Mars captures the viewport,
     * so the clip's y is taken relative to the scrolled document when captureBeyondViewport is
     * set, else to the viewport). Other encodings are not produced.
     */
    fn screenshot(&mut self, session: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let tab = self.tab(session)?;
        let format = params["format"].as_str().unwrap_or(PNG);
        if format != PNG {
            return Err(Fault::server("Mars encodes screenshots as PNG only."));
        }
        let shot = self.call_tab(&tab, "page.screenshot", json!({}))?;
        let data = shot["png_base64"].as_str().unwrap_or_default().to_owned();
        let Some(clip) = params.get("clip").filter(|clip| clip.is_object()) else {
            return Ok(json!({"data":data}));
        };
        let (width, height) = (
            clip["width"].as_f64().unwrap_or(0.0),
            clip["height"].as_f64().unwrap_or(0.0),
        );
        if width <= 0.0 {
            return Err(Fault::server("Cannot take screenshot with 0 width."));
        }
        if height <= 0.0 {
            return Err(Fault::server("Cannot take screenshot with 0 height."));
        }
        let scroll = shot["y"].as_f64().unwrap_or(0.0);
        let beyond = params["captureBeyondViewport"].as_bool() == Some(true);
        let x = clip["x"].as_f64().unwrap_or(0.0).max(0.0);
        let y = clip["y"].as_f64().unwrap_or(0.0) - if beyond { scroll } else { 0.0 };
        let png = crate::websocket::decode_base64(&data)
            .ok_or_else(|| Fault::server("The capture could not be decoded."))?;
        let cropped = crop(&png, x, y.max(0.0), width, height)?;
        Ok(json!({"data":crate::websocket::base64(&cropped)}))
    }

    /** Page.createIsolatedWorld: a named world that shares the realm. */
    fn isolated_world(&mut self, session: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let tab = self.tab(session)?;
        let url = self.current_url(&tab);
        let name = params["worldName"].as_str().unwrap_or_default().to_owned();
        let id = self.create_world(&tab, &name, &url);
        Ok(json!({"executionContextId":id}))
    }

    /** Page.addScriptToEvaluateOnNewDocument: kept and run after every commit on this connection. */
    fn new_document_script(
        &mut self,
        session: Option<&str>,
        params: &Value,
    ) -> Result<Value, Fault> {
        let source = params["source"]
            .as_str()
            .or_else(|| params["scriptSource"].as_str())
            .unwrap_or_default()
            .to_owned();
        let world = params["worldName"].as_str().unwrap_or_default().to_owned();
        self.next_script += 1;
        let identifier = self.next_script.to_string();
        self.new_document_scripts
            .push((identifier.clone(), source.clone(), world));
        // Chrome's runImmediately evaluates the script in the documents that already exist.
        if params["runImmediately"].as_bool() == Some(true)
            && !source.trim().is_empty()
            && let Ok(tab) = self.tab(session)
        {
            let _ = self.devtools(
                &tab,
                json!({"kind":"evaluate","expression":source,"options":{"return_by_value":true}}),
            );
        }
        Ok(json!({"identifier":identifier}))
    }

    /** Page.removeScriptToEvaluateOnNewDocument. */
    fn remove_new_document_script(
        &mut self,
        _: Option<&str>,
        params: &Value,
    ) -> Result<Value, Fault> {
        let identifier = params["identifier"].as_str().unwrap_or_default();
        let before = self.new_document_scripts.len();
        self.new_document_scripts
            .retain(|(id, _, _)| id != identifier);
        if self.new_document_scripts.len() == before {
            return Err(Fault::server("Script not found"));
        }
        Ok(json!({}))
    }

    /** Page.setDocumentContent: the markup becomes the document at the current URL. */
    fn set_document_content(
        &mut self,
        session: Option<&str>,
        params: &Value,
    ) -> Result<Value, Fault> {
        let tab = self.tab(session)?;
        let html = params["html"].as_str().unwrap_or_default();
        let url = self.current_url(&tab);
        let body = crate::websocket::base64(html.as_bytes());
        self.perform_navigation(&tab, "page.navigate_intercepted", json!({"url":url,"fulfill":{"status":200,"headers":[["content-type","text/html; charset=utf-8"]],"body_base64":body}}))?;
        Ok(json!({}))
    }

    /** Page.close: the tab, when this lane opened it. */
    fn page_close(&mut self, session: Option<&str>, _: &Value) -> Result<Value, Fault> {
        let tab = self.tab(session)?;
        self.close_tab(&tab)?;
        Ok(json!({}))
    }

    /** Page.getLayoutMetrics: the viewport, its scroll offset and the document's size. */
    fn layout_metrics(&mut self, session: Option<&str>, _: &Value) -> Result<Value, Fault> {
        let tab = self.tab(session)?;
        let nodes = self.call_tab(&tab, "page.nodes", json!({}))?;
        let viewport = &nodes["viewport"];
        let (width, height) = (
            viewport["width"].as_f64().unwrap_or(0.0),
            viewport["height"].as_f64().unwrap_or(0.0),
        );
        let scroll = viewport["scroll_y"].as_f64().unwrap_or(0.0);
        let document_height = nodes["document_height"]
            .as_f64()
            .unwrap_or(height)
            .max(height);
        let layout = json!({"pageX":0,"pageY":scroll,"clientWidth":width,"clientHeight":height});
        let visual = json!({"offsetX":0,"offsetY":0,"pageX":0,"pageY":scroll,"clientWidth":width,"clientHeight":height,"scale":1,"zoom":1});
        let content = json!({"x":0,"y":0,"width":width,"height":document_height});
        Ok(
            json!({"layoutViewport":layout,"visualViewport":visual,"contentSize":content,"cssLayoutViewport":layout,"cssVisualViewport":visual,"cssContentSize":content}),
        )
    }

    /** Page.handleJavaScriptDialog: V9 answers dialogs itself, so none is ever showing. */
    fn dialog(&mut self, _: Option<&str>, _: &Value) -> Result<Value, Fault> {
        Err(Fault::invalid("No dialog is showing"))
    }
}

/** Chrome spells loader ids as hex strings. */
pub(super) fn loader_id(loader: u64) -> String {
    format!("{loader:016X}")
}

/** Chrome's `errorText` for a failed navigation: a net error name. */
fn error_text(fault: &Fault) -> String {
    if fault.message.contains("net::") {
        return fault
            .message
            .split("net::")
            .nth(1)
            .map(|rest| {
                format!(
                    "net::{}",
                    rest.split_whitespace().next().unwrap_or_default()
                )
            })
            .unwrap_or_else(|| "net::ERR_FAILED".into());
    }
    if fault.message.contains("timeout") {
        return "net::ERR_TIMED_OUT".into();
    }
    "net::ERR_FAILED".into()
}

/** Cut a rectangle out of a PNG, bounded to the image; re-encoded as RGBA PNG. */
fn crop(png: &[u8], x: f64, y: f64, width: f64, height: f64) -> Result<Vec<u8>, Fault> {
    let decoder = png::Decoder::new(Cursor::new(png));
    let mut reader = decoder
        .read_info()
        .map_err(|_| Fault::server("The capture could not be decoded."))?;
    let mut buffer = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buffer)
        .map_err(|_| Fault::server("The capture could not be decoded."))?;
    let channels = info.color_type.samples();
    let (full_width, full_height) = (info.width as usize, info.height as usize);
    let left = (x.round() as usize).min(full_width);
    let top = (y.round() as usize).min(full_height);
    let right = ((x + width).round() as usize).clamp(left, full_width);
    let bottom = ((y + height).round() as usize).clamp(top, full_height);
    let (out_width, out_height) = ((right - left).max(1), (bottom - top).max(1));
    let mut out = Vec::with_capacity(out_width * out_height * channels);
    for row in top..top + out_height {
        let start = (row.min(full_height - 1) * full_width + left) * channels;
        let end = start + out_width.min(full_width - left) * channels;
        out.extend_from_slice(&buffer[start..end]);
        out.resize(
            out.len() + (out_width - out_width.min(full_width - left)) * channels,
            0,
        );
    }
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, out_width as u32, out_height as u32);
        encoder.set_color(info.color_type);
        encoder.set_depth(info.bit_depth);
        let mut writer = encoder
            .write_header()
            .map_err(|_| Fault::server("The clip could not be encoded."))?;
        writer
            .write_image_data(&out)
            .map_err(|_| Fault::server("The clip could not be encoded."))?;
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /** A clip cuts the requested box out of the capture and never reads past it. */
    #[test]
    fn clips_crop_within_the_capture() {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, 4, 3);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            let mut pixels = Vec::new();
            for row in 0..3u8 {
                for column in 0..4u8 {
                    pixels.extend_from_slice(&[row, column, 0, 255]);
                }
            }
            writer.write_image_data(&pixels).unwrap();
        }
        let cropped = crop(&bytes, 1.0, 1.0, 2.0, 10.0).unwrap();
        let decoder = png::Decoder::new(Cursor::new(cropped));
        let mut reader = decoder.read_info().unwrap();
        let mut buffer = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buffer).unwrap();
        assert_eq!((info.width, info.height), (2, 2));
        assert_eq!(&buffer[..4], &[1, 1, 0, 255]);
        assert_eq!(&buffer[12..16], &[2, 2, 0, 255]);
    }

    /** Failures become Chrome's net error names. */
    #[test]
    fn errors_become_net_error_text() {
        let fault = Fault::server("navigation_failed: net::ERR_FAILED");
        assert_eq!(error_text(&fault), "net::ERR_FAILED");
        assert_eq!(
            error_text(&Fault::server("The site did not answer: timeout.")),
            "net::ERR_TIMED_OUT"
        );
        assert_eq!(
            error_text(&Fault::server("unsupported_feature: no")),
            "net::ERR_FAILED"
        );
    }
}
