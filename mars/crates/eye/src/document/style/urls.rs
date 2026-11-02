/** Background image addresses: linked sheets rebased to absolute URLs, and the per-element list the app fetches. */
use super::BackgroundLayer;
use crate::constants::URL_BYTES;
use crate::document::Document;

/** Background images one page asks the app to fetch, in document order: the page's whole image
budget (IMAGE_COUNT, whose total bytes are bounded too), so pages and references with a few dozen
background boxes are not left half painted. */
pub const BACKGROUND_IMAGES: usize = crate::constants::IMAGE_COUNT;
/** `url()` tokens one sheet rewrites; later ones stay as authored. */
const SHEET_URLS: usize = 4096;

/// Rewrites relative `url()` references in a linked sheet against the sheet's own URL, so they
/// keep meaning once the sheet's text is cascaded with the document's. Call it on each fetched
/// linked sheet before passing the sheets to `compute_styles`.
pub fn rebase_urls(css: &str, sheet_url: &str) -> String {
    let Ok(base) = url::Url::parse(sheet_url) else {
        return css.to_owned();
    };
    let lower = css.to_ascii_lowercase();
    let mut out = String::with_capacity(css.len());
    let mut at = 0;
    for _ in 0..SHEET_URLS {
        let Some(found) = lower[at..].find("url(") else {
            break;
        };
        let open = at + found + "url(".len();
        let Some(close) = css[open..].find(')').map(|end| open + end) else {
            break;
        };
        let inner = css[open..close].trim().trim_matches(['"', '\'']);
        out.push_str(&css[at..open]);
        match rebased(&base, inner) {
            Some(absolute) => {
                out.push('"');
                out.push_str(&absolute);
                out.push('"');
            }
            None => out.push_str(&css[open..close]),
        }
        at = close;
    }
    out.push_str(&css[at..]);
    out
}
/** An absolute http(s) URL for a relative reference; data and absolute URLs stay as written. */
fn rebased(base: &url::Url, reference: &str) -> Option<String> {
    if reference.is_empty()
        || reference.contains(['"', '\'', '\\'])
        || reference.len() > URL_BYTES
        || url::Url::parse(reference).is_ok()
    {
        return None;
    }
    let joined = base.join(reference).ok()?;
    (["http", "https"].contains(&joined.scheme()) && joined.as_str().len() <= URL_BYTES)
        .then(|| joined.to_string())
}
/// The first background image of each element that paints one, as an absolute http(s) URL
/// resolved against the document URL `base`, keyed by the element's local identity. Images are
/// skipped (they paint their own bytes under the same key). An element with a `url()` mask lists
/// the mask instead (icons are usually a mask over a background color), so the bytes stored
/// under its identity are the mask whenever `style.mask` is set. An element without a
/// background image lists its `border-image` url, and one with neither lists its `::before` or
/// `::after` image instead. At most BACKGROUND_IMAGES entries.
/// The app fetches each like an `<img>` and stores the bytes under that identity; painters draw a
/// background `Image` layer with the bytes stored under its item's identity.
pub fn background_urls(document: &Document, base: &str) -> Vec<(usize, String)> {
    /** The first image layer's URL. */
    fn first(layers: &[BackgroundLayer]) -> Option<&String> {
        layers.iter().find_map(|layer| match layer {
            BackgroundLayer::Image { url } => Some(url),
            _ => None,
        })
    }
    let Ok(base) = url::Url::parse(base) else {
        return Vec::new();
    };
    document
        .nodes
        .iter()
        .filter(|node| node.tag != "img" && !node.hidden())
        .filter_map(|node| {
            // A `::before` or `::after` shares its element's identity, so its image is fetched
            // under that key when the element paints none (LinkedIn's lazy-loaded "Join your
            // colleagues" street scene is a `::after` background).
            // ponytail: one image per identity; an element and its pseudo with two images keep
            // the element's.
            // A pseudo's `url()` mask counts like its background (Wikipedia's dropdown chevron
            // is a `::after` mask over its color).
            // A `border-image` url comes next: its painter draws the bytes under this identity
            // only when the element has no mask or background image of its own.
            let image = first(super::layer_urls::keyed_layers(&node.style))
                .or_else(|| super::border_image::url(&node.style))
                .or_else(|| {
                    node.style.generated.iter().find_map(|generated| {
                        let style = &generated.style;
                        (!style.hidden)
                            .then(|| style.mask.as_ref().map(|mask| &mask.url))
                            .flatten()
                            .or_else(|| first(&style.backgrounds))
                    })
                });
            let url = node.style.mask.as_ref().map(|mask| &mask.url).or(image)?;
            fetchable(&base, url).map(|url| (node.local_id, url))
        })
        // Further image layers and the canvas's follow, under keys of their own (layer_urls.rs).
        .chain(
            super::layer_urls::extra_layers(document)
                .into_iter()
                .filter_map(|(key, url)| fetchable(&base, url).map(|url| (key, url))),
        )
        .take(BACKGROUND_IMAGES)
        .collect()
}
/** `url` resolved against `base` when the app may fetch it: http(s), or `data:` decoded
 * locally in the app within the same image budget. */
fn fetchable(base: &url::Url, url: &str) -> Option<String> {
    if !mars_core::image_source::valid_url_text(url) {
        return None;
    }
    let joined = base.join(url.trim()).ok()?;
    (["http", "https", "data"].contains(&joined.scheme())
        && mars_core::image_source::valid_url_text(joined.as_str()))
    .then(|| joined.to_string())
}
