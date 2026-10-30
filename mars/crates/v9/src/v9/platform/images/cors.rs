/** Image request modes are immutable generation data; a DOM attribute alone never grants clean pixels. */
use serde::{Deserialize, Serialize};

/** HTML's three crossorigin states select host-owned fetch behavior and cache identity. */
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ImageCors {
    /// Missing attribute keeps ordinary credentialed no-CORS image loading.
    #[default]
    NoCors,
    /// Empty, anonymous, or invalid attribute requests CORS with same-origin credentials.
    Anonymous,
    /// Explicit use-credentials requests CORS with credentials on every admitted hop.
    UseCredentials,
}

impl ImageCors {
    /** Enumerated attributes match keywords case-insensitively; invalid values are anonymous. */
    pub fn from_attribute(value: Option<&str>) -> Self {
        match value {
            None => Self::NoCors,
            Some(value) if value.eq_ignore_ascii_case("use-credentials") => Self::UseCredentials,
            Some(_) => Self::Anonymous,
        }
    }
}
