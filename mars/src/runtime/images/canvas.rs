/** Decoded canvas sources retain actual image acquisition provenance and bounded shared pixels. */
use super::ImageAsset;
use crate::network::{Destination, WebRequest, WebResponse};
use crate::protocol::{EngineError, EngineResult};
use crate::runtime::{resources::Redirects, supervisor::Runtime};
use mars_core::canvas::{Bitmap, IMAGE_SOURCE_BYTES};
use std::time::Instant;
use url::Url;
mod decoder;
mod formats;
#[cfg(all(test, target_os = "macos"))]
mod image_formats;

/** Native painting can retain larger or unsupported images without promising canvas pixel access. */
pub(in crate::runtime) fn source(
    asset: Option<&ImageAsset>,
    used: &mut usize,
) -> (Option<Bitmap>, bool) {
    let Some(asset) = asset else {
        return (None, false);
    };
    if asset.decoded == 0 || asset.decoded > IMAGE_SOURCE_BYTES.saturating_sub(*used) as u64 {
        return (None, asset.origin_clean);
    }
    let bitmap = decoder::decode(asset).ok();
    if let Some(bitmap) = &bitmap {
        if bitmap.pixels.len() > IMAGE_SOURCE_BYTES.saturating_sub(*used) {
            return (None, asset.origin_clean);
        }
        *used += bitmap.pixels.len();
    }
    (bitmap, asset.origin_clean)
}

/** Every redirect hop is observed before marking a no-CORS image origin-clean. */
pub(super) fn fetch(
    runtime: &mut Runtime,
    base: &Url,
    initial: &Url,
    bitmap: bool,
    deadline: Instant,
) -> EngineResult<(WebResponse, bool)> {
    let mut url = initial.clone();
    let mut origin_clean = true;
    for hops in 0..=crate::constants::REDIRECTS {
        origin_clean &= url.origin() == base.origin();
        let request = WebRequest {
            destination: Destination::Image,
            image: bitmap,
            ..WebRequest::get(url.as_str())
        };
        let acquired = runtime.acquire_with(
            request,
            deadline,
            Some(base.clone()),
            false,
            Redirects::Return,
            true,
        )?;
        let response = acquired.response;
        origin_clean &=
            Url::parse(&response.url).is_ok_and(|final_url| final_url.origin() == base.origin());
        if ![301, 302, 303, 307, 308].contains(&response.status) {
            return Ok((response, origin_clean));
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
        url = crate::network::redirect_url(&url, location, grants)?;
    }
    Err(EngineError::unsupported("Redirect limit exceeded."))
}
