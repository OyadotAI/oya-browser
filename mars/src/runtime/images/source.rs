/** Image fetch identity includes its immutable CORS policy and owning origin, independently of clean pixels. */
use crate::v9::ImageCors;
use url::Url;

/** Native decoded provenance matches the same source selection used by initial, detached and adopted images. */
#[derive(Clone, Debug, PartialEq)]
pub(in crate::runtime) struct Selection {
    /// Absolute selected URL; responsive source density does not change its acquisition policy.
    pub url: String,
    /// Density-corrected natural dimensions belong to this selection.
    pub density: f64,
    /// Changing crossorigin invalidates an earlier selection even when URL text is restored.
    pub cors: ImageCors,
    /// CORS available-image entries are bound to the document origin that passed admission.
    pub origin: Option<String>,
}

impl Selection {
    /** Resolve an owned source against its document rather than trusting a script-provided clean flag. */
    pub fn new(base: &Url, source: &str, density: f64, cors: ImageCors) -> Option<Self> {
        Some(Self {
            url: base.join(source.trim()).ok()?.to_string(),
            density,
            cors,
            origin: origin(base, cors),
        })
    }
}

/** Higher-layer image reuse cannot promote no-CORS bytes or reuse anonymous grants with credentials. */
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct FetchKey {
    url: String,
    cors: ImageCors,
    origin: Option<String>,
}

impl FetchKey {
    /** Include the resolved URL and the same CORS principal as the document's available-image list. */
    pub(super) fn new(base: &Url, source: &str, cors: ImageCors) -> Self {
        Self {
            url: base
                .join(source.trim())
                .map_or_else(|_| source.trim().to_string(), |url| url.to_string()),
            cors,
            origin: origin(base, cors),
        }
    }
}

/** No-CORS images retain conservative taint; CORS cache grants are scoped to the requesting origin. */
fn origin(base: &Url, cors: ImageCors) -> Option<String> {
    (cors != ImageCors::NoCors).then(|| base.origin().ascii_serialization())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_url_reuse_requires_the_same_mode_and_cors_principal() {
        let base = Url::parse("https://a.test/page").unwrap();
        let other = Url::parse("https://b.test/page").unwrap();
        let url = "https://cdn.test/pixel";
        let anonymous = FetchKey::new(&base, url, ImageCors::Anonymous);
        assert_eq!(
            anonymous,
            FetchKey::new(&base, " https://cdn.test/pixel ", ImageCors::Anonymous)
        );
        assert_ne!(anonymous, FetchKey::new(&base, url, ImageCors::NoCors));
        assert_ne!(
            anonymous,
            FetchKey::new(&base, url, ImageCors::UseCredentials)
        );
        assert_ne!(anonymous, FetchKey::new(&other, url, ImageCors::Anonymous));
    }

    #[test]
    fn decoded_selection_retains_mode_and_origin_beyond_attribute_mutations() {
        let base = Url::parse("https://a.test/page").unwrap();
        let anonymous = Selection::new(&base, "/pixel", 2.0, ImageCors::Anonymous).unwrap();
        assert_eq!(anonymous.url, "https://a.test/pixel");
        assert_eq!(anonymous.origin.as_deref(), Some("https://a.test"));
        assert_ne!(
            Some(anonymous),
            Selection::new(&base, "/pixel", 2.0, ImageCors::NoCors)
        );
    }
}
