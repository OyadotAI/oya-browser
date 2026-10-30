/** Image lifecycle work uses the existing governed fetch/decoder and returns only inert outcomes to V9. */
use super::{IMAGE_REQUEST_WALL, INSERTED_IMAGE_WALL, Page, Runtime};
use crate::constants::{IMAGE_COUNT, NODES, URL_BYTES, VM_DETACHED_NODES};
use crate::protocol::{EngineError, EngineResult};
use crate::v9::{ImageAdoption, ImageCors, ImageRequest, ImageResponse};
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

/** The source selection inputs ensure a cached decode cannot be reused for another candidate. */
type Source = super::super::images::ImageSelection;

/** Per-page queues and native decode admission remain bounded even for detached Image objects. */
#[derive(Clone, Default)]
pub(in crate::runtime) struct Loading {
    /** Only the most recent queued generation per image needs to start a fetch. */
    pub pending: Vec<ImageRequest>,
    /** Images managed by the lifecycle must not also enter the legacy paint-only fetch path. */
    pub managed: BTreeSet<usize>,
    /** Hidden and detached decoded pixels stay owned and budgeted without entering the painted snapshot. */
    pub hidden: BTreeMap<usize, super::super::images::ImageAsset>,
    /** Genuine previously decoded source selections, including the navigation's initial assets. */
    decoded: BTreeMap<usize, (Source, (u32, u32))>,
    /** Monotonic page-wide request, transferred-byte and decoded-byte admission. */
    budget: Option<super::super::images::Budget>,
    /** Monotonic delivery admission bounds pixels retained in the independent V9 image-source store. */
    canvas_bytes: usize,
}

impl Loading {
    /** A sandbox result is checked before it can queue any privileged native work. */
    pub(super) fn queue(&mut self, requests: Vec<ImageRequest>) -> EngineResult<()> {
        if requests.len() > IMAGE_COUNT
            || requests
                .iter()
                .any(|r| r.generation == 0 || r.source.len() > URL_BYTES || r.url.len() > URL_BYTES)
        {
            return Err(EngineError::invalid("Invalid image request batch."));
        }
        for request in requests {
            self.managed.insert(request.node);
            self.hidden.remove(&request.node);
            self.pending.retain(|old| old.node != request.node);
            self.pending.push(request);
        }
        if self.managed.len() > NODES + VM_DETACHED_NODES {
            return Err(EngineError::limit(
                "Image lifecycle nodes",
                NODES + VM_DETACHED_NODES,
            ));
        }
        Ok(())
    }
}

/** Exact selection inputs from the current owned document, never a page-supplied native handle. */
fn source(page: &Page, node: &crate::document::Node, viewport: f64) -> Option<Source> {
    let (url, density) = super::super::images::selected_source(&page.document, node, viewport)?;
    let base = url::Url::parse(&page.url).ok()?;
    let cors = ImageCors::from_attribute(node.attributes.get("crossorigin").map(String::as_str));
    Source::new(&base, &url, density, cors)
}

/** Multiple adoptions in one task may name intermediate identities absent from the final projection. */
fn valid_adoptions(page: &Page, rows: &[ImageAdoption], saved: &crate::v9::VmState) -> bool {
    if rows.len() > crate::constants::STATEMENTS {
        return false;
    }
    let mapping: BTreeMap<_, _> = rows.iter().map(|row| (row.from, row.to)).collect();
    mapping.len() == rows.len()
        && rows.iter().all(|row| {
            row.from != row.to
                && adoption_target(row.to, &mapping).is_some_and(|id| {
                    page.document
                        .nodes
                        .iter()
                        .find(|node| node.local_id == id)
                        .map_or_else(|| saved.is_detached_image(id), |node| node.tag == "img")
                })
        })
}

/** A cycle or a chain longer than the admitted batch cannot name a genuine final DOM identity. */
fn adoption_target(mut node: usize, mapping: &BTreeMap<usize, usize>) -> Option<usize> {
    for _ in 0..=mapping.len() {
        match mapping.get(&node) {
            Some(next) => node = *next,
            None => return Some(node),
        }
    }
    None
}

impl Runtime {
    /** Commit only validated DOM adoption associations; no page-supplied bytes or dimensions enter this path. */
    pub(super) fn adopt_image_associations(
        &self,
        page: &mut Page,
        adoptions: Vec<ImageAdoption>,
        saved: &crate::v9::VmState,
    ) -> EngineResult<()> {
        if !valid_adoptions(page, &adoptions, saved) {
            return Err(EngineError::invalid("Invalid image adoption batch."));
        }
        for row in adoptions {
            let state = &mut page.scripting.image_loading;
            if state.managed.remove(&row.from) {
                state.managed.insert(row.to);
            }
            for request in &mut state.pending {
                if request.node == row.from {
                    request.node = row.to;
                }
            }
            if let Some(decoded) = state.decoded.remove(&row.from) {
                state.decoded.insert(row.to, decoded);
            }
            if let Some(asset) = state.hidden.remove(&row.from) {
                state.hidden.insert(row.to, asset);
            }
            if let Some(asset) = page.images.remove(&row.from) {
                state.hidden.insert(row.to, asset);
            }
            self.attach_adopted_image(page, row.to);
        }
        Ok(())
    }

    /** Already decoded detached pixels attach to the same image without a second fetch or load event. */
    fn attach_adopted_image(&self, page: &mut Page, id: usize) {
        let Some(node) = page.document.nodes.iter().find(|node| node.local_id == id) else {
            return;
        };
        let Some((identity, size)) = page.scripting.image_loading.decoded.get(&id) else {
            return;
        };
        if source(page, node, self.render_context.width).as_ref() != Some(identity) {
            return;
        }
        let size = *size;
        let asset = page
            .scripting
            .image_loading
            .hidden
            .get(&id)
            .or_else(|| page.images.get(&id));
        let drawing = asset
            .filter(|a| a.mime == "image/svg+xml")
            .and_then(|a| std::str::from_utf8(&a.bytes).ok())
            .map(super::super::images::svg_url);
        let node = page
            .document
            .nodes
            .iter_mut()
            .find(|node| node.local_id == id)
            .expect("adopted image");
        node.image_size = Some(size);
        node.text.clear();
        if let Some(drawing) = drawing {
            node.image_src = Some(drawing);
        }
        self.refresh_image_visibility(page);
    }

    /** Source removal cancels queued work and clears pixels without fabricating a decoding event. */
    pub(super) fn discard_removed_image_sources(&self, page: &mut Page) {
        let removed: Vec<_> = page
            .document
            .nodes
            .iter()
            .filter(|node| {
                node.tag == "img"
                    && page
                        .scripting
                        .image_loading
                        .managed
                        .contains(&node.local_id)
                    && !node.attributes.contains_key("src")
                    && !node.attributes.contains_key("srcset")
            })
            .map(|node| node.local_id)
            .collect();
        for node in removed {
            page.scripting
                .image_loading
                .pending
                .retain(|request| request.node != node);
            self.clear_failed_image(page, node);
        }
    }

    /** Parser and DOMContentLoaded image cohorts must finish before the document load event. */
    pub(in crate::runtime) fn settle_images_before_load(
        &mut self,
        page: &mut Page,
        deadline: Instant,
    ) -> EngineResult<bool> {
        let mut changed = false;
        for _ in 0..super::SETTLE_STEPS {
            if page.scripting.image_loading.pending.is_empty() {
                return Ok(changed);
            }
            if Instant::now() >= deadline {
                return Ok(changed);
            }
            let responses = self.perform_image_requests(page, deadline)?;
            if responses.iter().any(|response| {
                page.document
                    .nodes
                    .iter()
                    .any(|node| node.local_id == response.request.node)
            }) {
                self.prepare_display(page)?;
                changed = true;
            }
            changed |= self
                .script_step(
                    page,
                    serde_json::json!({"kind":"images","responses":responses}),
                )?
                .changed;
        }
        Ok(changed)
    }

    /** Initial image metadata already came through the real decoder before page scripts ran. */
    pub(super) fn initialize_image_loading(&self, page: &mut Page) {
        let mut decoded = BTreeMap::new();
        for node in page.document.nodes.iter().filter(|node| node.tag == "img") {
            let selected = source(page, node, self.render_context.width);
            let actual = page
                .images
                .get(&node.local_id)
                .and_then(|asset| asset.image_source.as_ref());
            if let (Some(size), Some(selected), Some(actual)) = (node.image_size, selected, actual)
                && &selected == actual
            {
                decoded.insert(node.local_id, (selected, size));
            }
        }
        let state = &mut page.scripting.image_loading;
        state.budget = Some(super::super::images::Budget::spent(&page.images));
        state.decoded = decoded;
    }

    /** Decode a bounded batch before returning its completions as a separate V9 task. */
    pub(super) fn perform_image_requests(
        &mut self,
        page: &mut Page,
        until: Instant,
    ) -> EngineResult<Vec<ImageResponse>> {
        page.document = self.styled(
            &page.document,
            &page.resolved_stylesheets,
            self.render_context,
        )?;
        page.styled = true;
        let mut budget = page
            .scripting
            .image_loading
            .budget
            .take()
            .unwrap_or_else(|| super::super::images::Budget::spent(&page.images));
        budget.include_assets(&page.images, &page.scripting.image_loading.hidden);
        let result = self.decode_image_batch(
            page,
            until.min(Instant::now() + INSERTED_IMAGE_WALL),
            &mut budget,
        );
        page.scripting.image_loading.budget = Some(budget);
        result
    }

    /**
     * An ordinary decode/network failure becomes error; authority and native service failures
     * propagate. The batch's document images are fetched together first, so it costs about one
     * round trip; each image then started gets at least IMAGE_REQUEST_WALL, and once the slice
     * has passed the rest wait for the next task instead of failing, as a browser's queued image
     * loads wait rather than error.
     */
    fn decode_image_batch(
        &mut self,
        page: &mut Page,
        deadline: Instant,
        budget: &mut super::super::images::Budget,
    ) -> EngineResult<Vec<ImageResponse>> {
        let count = page.scripting.image_loading.pending.len().min(IMAGE_COUNT);
        let requests: Vec<_> = page
            .scripting
            .image_loading
            .pending
            .drain(..count)
            .collect();
        self.prefetch_batch(page, &requests)?;
        let mut responses = Vec::with_capacity(count);
        let mut requests = requests.into_iter();
        for request in requests.by_ref() {
            let started = Instant::now();
            if !responses.is_empty() && started >= deadline {
                // Past the slice: keep this one and the rest pending, in order.
                let later = std::iter::once(request).chain(requests.by_ref());
                let queued = std::mem::take(&mut page.scripting.image_loading.pending);
                page.scripting.image_loading.pending = later.chain(queued).collect();
                break;
            }
            let until = deadline.max(started + IMAGE_REQUEST_WALL);
            let dimensions = self.decode_image_request(page, &request, until, budget)?;
            let asset = page
                .images
                .get(&request.node)
                .or_else(|| page.scripting.image_loading.hidden.get(&request.node))
                .cloned();
            let (bitmap, origin_clean) = super::super::images::canvas::source(
                asset.as_ref().filter(|_| dimensions.is_some()),
                &mut page.scripting.image_loading.canvas_bytes,
            );
            responses.push(ImageResponse {
                request,
                dimensions,
                bitmap,
                origin_clean,
            });
        }
        Ok(responses)
    }

    /** Fetch the batch's in-document images in parallel; reads then find the answers waiting. */
    fn prefetch_batch(&mut self, page: &Page, requests: &[ImageRequest]) -> EngineResult<()> {
        let wanted: BTreeSet<usize> = requests.iter().map(|request| request.node).collect();
        let url = page.url.clone();
        self.prefetch_images(
            &page.document,
            &url,
            Instant::now() + IMAGE_REQUEST_WALL,
            &|node| wanted.contains(&node.local_id),
            wanted.len(),
        )
    }

    /** Reuse only a matching actual decode; source changes otherwise pass through the same decoder. */
    fn decode_image_request(
        &mut self,
        page: &mut Page,
        request: &ImageRequest,
        deadline: Instant,
        budget: &mut super::super::images::Budget,
    ) -> EngineResult<Option<(u32, u32)>> {
        let node = page
            .document
            .nodes
            .iter()
            .find(|node| node.local_id == request.node && node.tag == "img");
        let selected = if let Some(node) = node {
            if node.attributes.get("src").map_or("", String::as_str) != request.source
                || ImageCors::from_attribute(node.attributes.get("crossorigin").map(String::as_str))
                    != request.cors
            {
                return Ok(None);
            }
            source(page, node, self.render_context.width)
        } else if request.source.trim().is_empty() {
            None
        } else {
            url::Url::parse(&page.url)
                .ok()
                .and_then(|base| Source::new(&base, &request.url, 1.0, request.cors))
        };
        let Some(selected) = selected else {
            self.clear_failed_image(page, request.node);
            return Ok(None);
        };
        if node.is_some()
            && let Some((_, size)) = page
                .scripting
                .image_loading
                .decoded
                .get(&request.node)
                .filter(|(old, _)| *old == selected)
        {
            return Ok(Some(*size));
        }
        let base = url::Url::parse(&page.url)
            .map_err(|_| EngineError::invalid("Invalid image base URL."))?;
        let mut notes = Vec::new();
        if let Some(mut fetched) = self.read_image_mode(
            &base,
            &selected.url,
            selected.cors,
            deadline,
            budget,
            &mut notes,
        )? {
            if let super::super::images::Fetched::Raster(asset, _) = &mut fetched {
                asset.image_source = Some(selected.clone());
            }
            let size = self.commit_image_decode(page, request.node, fetched, selected.density);
            page.scripting
                .image_loading
                .decoded
                .insert(request.node, (selected, size));
            return Ok(Some(size));
        }
        self.clear_failed_image(page, request.node);
        Ok(None)
    }

    /** A failed replacement must not leave the previous image painted or report its old dimensions. */
    fn clear_failed_image(&self, page: &mut Page, node: usize) {
        page.images.remove(&node);
        page.scripting.image_loading.hidden.remove(&node);
        page.scripting.image_loading.decoded.remove(&node);
        if let Some(node) = page
            .document
            .nodes
            .iter_mut()
            .find(|entry| entry.local_id == node)
        {
            node.image_size = None;
            node.text = node.image_alt.clone();
        }
    }
}
