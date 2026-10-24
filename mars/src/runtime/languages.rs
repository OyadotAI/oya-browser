/** The coordinator reads preferred languages once, in one place, for navigator.languages. */
use crate::constants::{LANGUAGE_TAG_BYTES, NAVIGATOR_LANGUAGES};

/** Hermetic runs and live comparisons pick the list with a comma-separated override. */
const OVERRIDE_ENV: &str = "MARS_LANGUAGES";
/** Used when neither the override nor the OS yields a usable tag. */
const DEFAULT_LANGUAGE: &str = "en-US";

/// Override first, then the OS list, then the default; V9 refuses malformed tags, so drop them here.
pub(super) fn preferred() -> Vec<String> {
    let requested = std::env::var(OVERRIDE_ENV)
        .ok()
        .map(|list| list.split(',').map(|tag| tag.trim().to_owned()).collect())
        .unwrap_or_else(system);
    usable(requested)
}

/// Keeps well-formed tags within V9's bounds and never returns an empty list.
pub(super) fn usable(tags: Vec<String>) -> Vec<String> {
    let mut tags: Vec<String> = tags
        .into_iter()
        .filter(|tag| well_formed(tag))
        .take(NAVIGATOR_LANGUAGES)
        .collect();
    if tags.is_empty() {
        tags.push(DEFAULT_LANGUAGE.into());
    }
    tags
}

/** ASCII letters, digits and hyphens within the tag length bound, as V9 validates them. */
fn well_formed(tag: &str) -> bool {
    !tag.is_empty()
        && tag.len() <= LANGUAGE_TAG_BYTES
        && tag
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

/** macOS reports the user's ordered list exactly as Safari and Electron see it. */
#[cfg(target_os = "macos")]
fn system() -> Vec<String> {
    objc2_foundation::NSLocale::preferredLanguages()
        .iter()
        .map(|tag| tag.to_string())
        .collect()
}

/** Elsewhere the POSIX locale is the only portable source, e.g. `fr_FR.UTF-8` becomes `fr-FR`. */
#[cfg(not(target_os = "macos"))]
fn system() -> Vec<String> {
    std::env::var("LANG")
        .ok()
        .and_then(|locale| {
            locale
                .split(['.', '@'])
                .next()
                .map(|tag| tag.replace('_', "-"))
        })
        .filter(|tag| tag != "C" && tag != "POSIX")
        .into_iter()
        .collect()
}
