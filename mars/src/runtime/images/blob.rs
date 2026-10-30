/** Blob images use immutable generation-owned bytes and never populate the URL-only acquisition cache. */
use super::*;
use crate::v9::{BlobUrlSource, ImageRequest};
use mars_core::canvas::Bitmap;
use url::Url;

#[cfg(test)]
mod tests;

/** Every Blob spelling must use local acquisition even when its capture is absent or invalid. */
pub(in crate::runtime) fn is_blob(address: &str) -> bool {
    Url::parse(address).is_ok_and(|url| url.scheme() == "blob")
}

/** Recheck actual snapshot size on the native queue, independently of serialized V9 counters. */
pub(in crate::runtime) fn snapshot_bytes(request: &ImageRequest) -> EngineResult<usize> {
    let Some(source) = &request.blob_source else {
        return Ok(0);
    };
    source.validate(&request.url)?;
    if source.bytes.len() > IMAGE_BYTES {
        return Err(invalid());
    }
    Ok(source
        .bytes
        .len()
        .saturating_add(request.url.len())
        .saturating_add(source.mime_type.len())
        .saturating_add(source.origin.len()))
}

impl super::super::supervisor::Runtime {
    /** Local capture proves provenance; current document policy and full decoding still gate success. */
    pub(in crate::runtime) fn read_blob_image(
        &mut self,
        base: &Url,
        policies: &[String],
        selected: &str,
        request: &ImageRequest,
        budget: &mut Budget,
    ) -> EngineResult<Option<(Fetched, Bitmap)>> {
        self.publish(|_| ())?;
        let Some(source) = &request.blob_source else {
            return Ok(None);
        };
        if selected != request.url || !budget.request() {
            return Ok(None);
        }
        let Some((fetched, bitmap)) = decode(base, policies, selected, source) else {
            return Ok(None);
        };
        Ok(budget.admit(&fetched).then_some((fetched, bitmap)))
    }
}

/** Raster sniffing and real decode reject unsupported SVG, malformed headers and unavailable formats. */
fn decode(
    base: &Url,
    policies: &[String],
    address: &str,
    source: &BlobUrlSource,
) -> Option<(Fetched, Bitmap)> {
    let url = Url::parse(address).ok()?;
    if source.validate(address).is_err()
        || source.bytes.len() > IMAGE_BYTES
        || source.origin != base.origin().ascii_serialization()
        || !super::super::csp::allows_image(policies, base, &url)
    {
        return None;
    }
    let mime = sniffed(&source.bytes)?;
    let size = dimensions(mime, &source.bytes).ok()?;
    let asset = ImageAsset {
        mime: mime.into(),
        bytes: source.bytes.clone(),
        decoded: u64::from(size.width) * u64::from(size.height) * 4,
        image_source: None,
        origin_clean: true,
    };
    let bitmap = canvas::decode(&asset).ok()?;
    let dimensions = (bitmap.width, bitmap.height);
    Some((Fetched::Raster(asset, dimensions), bitmap))
}
