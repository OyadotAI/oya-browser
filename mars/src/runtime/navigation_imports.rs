/** `@import`: the sheets a stylesheet imports, fetched as linked sheets and cascaded before it. */
use super::{Gaps, Runtime};
use crate::protocol::EngineResult;
use std::time::Instant;

/// Levels of nested imports followed; deeper `@import`s are ignored, so a cycle ends here.
const IMPORT_DEPTH: usize = 4;

impl Runtime {
    /**
     * `css` with each sheet its leading `@import` rules name (resolved against `base`) placed
     * before it, wrapped in `@media` when the import names media, as the cascade orders an
     * imported sheet's rules before its importer's. An import that cannot be loaded is skipped
     * like a linked sheet; imports past IMPORT_DEPTH levels are ignored.
     */
    pub(super) fn with_imports(
        &mut self,
        base: &url::Url,
        css: String,
        deadline: Instant,
        (gaps, depth): (&mut Gaps, usize),
    ) -> EngineResult<String> {
        let imports = crate::document::css_imports(&css);
        if imports.is_empty() || depth >= IMPORT_DEPTH {
            return Ok(css);
        }
        let mut out = String::new();
        for (href, media) in imports {
            let Some(sheet) = self.linked_sheet(base, &href, deadline, (gaps, depth + 1))? else {
                continue;
            };
            if media.is_empty() {
                out.push_str(&sheet);
            } else {
                out.push_str(&format!("@media {media} {{\n{sheet}\n}}"));
            }
            out.push('\n');
        }
        out.push_str(&css);
        Ok(out)
    }
}
