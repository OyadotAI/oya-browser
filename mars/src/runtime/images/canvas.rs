/** Decoded canvas sources retain actual image acquisition provenance and bounded shared pixels. */
use super::ImageAsset;
use crate::network::WebResponse;
use crate::protocol::{EngineError, EngineResult};
use crate::runtime::{resources::Redirects, supervisor::Runtime};
use mars_core::canvas::{Bitmap, IMAGE_SOURCE_BYTES};
use std::time::Instant;
use url::Url;
mod cors;
mod decoder;
mod formats;
#[cfg(all(test, target_os = "macos"))]
mod image_formats;

/** Native painting can retain larger or unsupported images without promising canvas pixel access. */
pub(in crate::runtime) fn source(
    asset: Option<&ImageAsset>,
    used: &mut usize,
) -> (Option<Bitmap>, bool) {
    source_decoded(asset, None, used)
}

/** A source decoded during Blob admission reuses those same actual pixels under the delivery budget. */
pub(in crate::runtime) fn source_decoded(
    asset: Option<&ImageAsset>,
    decoded: Option<Bitmap>,
    used: &mut usize,
) -> (Option<Bitmap>, bool) {
    let Some(asset) = asset else {
        return (None, false);
    };
    if asset.decoded == 0 || asset.decoded > IMAGE_SOURCE_BYTES.saturating_sub(*used) as u64 {
        return (None, asset.origin_clean);
    }
    let bitmap = decoded.or_else(|| decoder::decode(asset).ok());
    if let Some(bitmap) = &bitmap {
        if bitmap.pixels.len() > IMAGE_SOURCE_BYTES.saturating_sub(*used) {
            return (None, asset.origin_clean);
        }
        *used += bitmap.pixels.len();
    }
    (bitmap, asset.origin_clean)
}

/** Blob image success requires a complete bounded raster decode, including GIF and WebP containers. */
pub(in crate::runtime) fn decode(asset: &ImageAsset) -> EngineResult<Bitmap> {
    decoder::decode(asset)
}

/** Governed redirects retain credential policy and validate CORS before granting clean image pixels. */
pub(super) fn fetch(
    runtime: &mut Runtime,
    base: &Url,
    initial: &Url,
    bitmap: bool,
    mode: crate::v9::ImageCors,
    deadline: Instant,
) -> EngineResult<(WebResponse, bool)> {
    let mut url = initial.clone();
    let mut flow = cors::ImageFlow::new(base, mode);
    for hops in 0..=crate::constants::REDIRECTS {
        let (request, credentials) = flow.request(&url, bitmap);
        let acquired = runtime.acquire_with(
            request,
            deadline,
            Some(base.clone()),
            false,
            Redirects::Return,
            credentials,
        )?;
        let response = acquired.response;
        flow.admit(&url, &response)?;
        if ![301, 302, 303, 307, 308].contains(&response.status) {
            return Ok((response, flow.origin_clean()));
        }
        if hops == crate::constants::REDIRECTS {
            return Err(EngineError::unsupported("Redirect limit exceeded."));
        }
        let grants = runtime
            .web_origins
            .as_deref()
            .ok_or_else(|| EngineError::unsupported("Images require desktop networking."))?;
        let location = response
            .location
            .as_deref()
            .ok_or_else(|| EngineError::unsupported("Redirect has no Location."))?;
        let next = crate::network::redirect_url(&url, location, grants)?;
        flow.redirect(&url, &next);
        url = next;
    }
    Err(EngineError::unsupported("Redirect limit exceeded."))
}
