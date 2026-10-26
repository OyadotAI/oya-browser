/**
 * Import maps (`<script type=importmap>`) and module specifier resolution, as the HTML standard
 * defines them: URL-like specifiers resolve against their referrer, bare ones only through a map,
 * the most specific scope wins, and a prefix key (`"lib/"`) maps everything under it. Several
 * maps merge in document order, an earlier map's entries winning.
 */
use crate::constants::IMPORT_MAP_ENTRIES;
use serde_json::{Map, Value};
use url::Url;

/** One specifier map: keys longest first, so the most specific prefix matches first. A None
target blocks its key, as an invalid address does. */
type Specifiers = Vec<(String, Option<Url>)>;

/** Every import map the page declared, merged. */
#[derive(Default)]
pub(super) struct ImportMap {
    /** The opt-in d8 shell resolves bare filenames against their referrer; browser import maps stay strict. */
    #[cfg(feature = "conformance")]
    pub(super) d8: bool,
    /** The top-level `imports`. */
    imports: Specifiers,
    /** Scope prefix and its map, most specific scope first. */
    scopes: Vec<(String, Specifiers)>,
    /** Subresource Integrity metadata by module URL. */
    integrity: Vec<(String, String)>,
    /** Entries kept so far, bounded by IMPORT_MAP_ENTRIES. */
    entries: usize,
}

impl ImportMap {
    /** Merge one map's JSON; text that is not a JSON object is reported and changes nothing. */
    pub fn merge(&mut self, text: &str, base: &Url) -> Result<(), String> {
        let value: Value = serde_json::from_str(text).map_err(|_| invalid())?;
        let Value::Object(map) = value else {
            return Err(invalid());
        };
        if let Some(Value::Object(imports)) = map.get("imports") {
            let parsed = self.specifiers(imports, base);
            merge_into(&mut self.imports, parsed);
        }
        if let Some(Value::Object(scopes)) = map.get("scopes") {
            for (prefix, imports) in scopes {
                let (Ok(scope), Value::Object(imports)) = (base.join(prefix), imports) else {
                    continue;
                };
                let parsed = self.specifiers(imports, base);
                match self
                    .scopes
                    .iter_mut()
                    .find(|(known, _)| *known == scope.as_str())
                {
                    Some((_, known)) => merge_into(known, parsed),
                    None => self.scopes.push((scope.into(), parsed)),
                }
            }
            self.scopes.sort_by(|a, b| b.0.cmp(&a.0));
        }
        if let Some(Value::Object(integrity)) = map.get("integrity") {
            for (key, metadata) in integrity {
                let (Some(url), Value::String(metadata)) = (url_like(key, base), metadata) else {
                    continue;
                };
                if self.entries < IMPORT_MAP_ENTRIES
                    && !self
                        .integrity
                        .iter()
                        .any(|(known, _)| *known == url.as_str())
                {
                    self.entries += 1;
                    self.integrity.push((url.into(), metadata.clone()));
                }
            }
        }
        Ok(())
    }
    /** One specifier map, normalized and bounded. */
    fn specifiers(&mut self, map: &Map<String, Value>, base: &Url) -> Specifiers {
        let mut parsed = Specifiers::new();
        for (key, target) in map {
            if key.is_empty() || self.entries >= IMPORT_MAP_ENTRIES {
                continue;
            }
            self.entries += 1;
            let key = url_like(key, base).map_or_else(|| key.clone(), String::from);
            let target = match target {
                Value::String(address) => url_like(address, base),
                _ => None,
            }
            .filter(|url| !key.ends_with('/') || url.as_str().ends_with('/'));
            parsed.push((key, target));
        }
        parsed.sort_by(|a, b| b.0.cmp(&a.0));
        parsed
    }
    /** Resolve a specifier imported by code whose base URL is `base`. */
    pub fn resolve(&self, specifier: &str, base: &Url) -> Result<Url, String> {
        let as_url = url_like(specifier, base);
        let normalized = as_url.as_ref().map_or(specifier, Url::as_str);
        for (scope, imports) in &self.scopes {
            let applies = base.as_str() == scope
                || scope.ends_with('/') && base.as_str().starts_with(scope.as_str());
            if applies && let Some(found) = matched(imports, normalized, as_url.is_some()) {
                return found;
            }
        }
        if let Some(found) = matched(&self.imports, normalized, as_url.is_some()) {
            return found;
        }
        #[cfg(feature = "conformance")]
        if as_url.is_none() && self.d8 {
            return base.join(specifier).map_err(|error| error.to_string());
        }
        as_url.ok_or_else(|| {
            format!(
                "Failed to resolve module specifier \"{specifier}\". Relative references must start with either \"/\", \"./\", or \"../\"."
            )
        })
    }
    /** The integrity metadata the maps give a module URL. */
    pub fn integrity(&self, url: &str) -> Option<String> {
        self.integrity
            .iter()
            .find(|(known, _)| known == url)
            .map(|(_, metadata)| metadata.clone())
    }
}

/** The import map error browsers report. */
fn invalid() -> String {
    "An import map was ignored: it is not a JSON object.".into()
}

/** Add entries an earlier map does not already define, keeping the longest-first order. */
fn merge_into(known: &mut Specifiers, parsed: Specifiers) {
    for (key, target) in parsed {
        if !known.iter().any(|(existing, _)| *existing == key) {
            known.push((key, target));
        }
    }
    known.sort_by(|a, b| b.0.cmp(&a.0));
}

/** An exact key, or the longest prefix key under which the specifier falls. */
fn matched(imports: &Specifiers, normalized: &str, url_like: bool) -> Option<Result<Url, String>> {
    let blocked = || Err(format!("The import map blocks \"{normalized}\"."));
    for (key, target) in imports {
        if key == normalized {
            return Some(target.clone().map_or_else(blocked, Ok));
        }
        let prefix = key.ends_with('/') && normalized.starts_with(key.as_str());
        if prefix && (!url_like || normalized.starts_with("http")) {
            let Some(target) = target else {
                return Some(blocked());
            };
            let rest = &normalized[key.len()..];
            return Some(
                target
                    .join(rest)
                    .ok()
                    .filter(|url| url.as_str().starts_with(target.as_str()))
                    .map_or_else(blocked, Ok),
            );
        }
    }
    None
}

/** A specifier that is an absolute URL or starts with `/`, `./` or `../`, resolved. */
fn url_like(specifier: &str, base: &Url) -> Option<Url> {
    if ["/", "./", "../"]
        .iter()
        .any(|prefix| specifier.starts_with(prefix))
    {
        return base.join(specifier).ok();
    }
    Url::parse(specifier).ok()
}
