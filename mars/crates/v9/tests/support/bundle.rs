/** Native d8 bundle containers supply unchanged Script/module parts and ordered module entrypoints. */
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
/** The parser uses the exact byte markers recognized by pinned d8 TryExecuteBundle. */
const PREFIX: &str = "// JS_BUNDLE_";
const SCRIPT: &str = "// JS_BUNDLE_SCRIPT";
const MODULE: &str = "// JS_BUNDLE_MODULE:";
const ENTRY: &str = "// JS_BUNDLE_MODULE_ENTRYPOINT";
/** Every part retains its literal contents; the source wrapper itself is native container metadata. */
#[derive(Clone)]
pub(super) enum Item {
    Script(String),
    Module(String),
}
/** All module records are gathered before any execution, so later declarations satisfy earlier imports. */
#[derive(Clone, Default)]
pub(super) struct Bundle {
    pub items: Vec<Item>,
    pub modules: BTreeMap<String, String>,
}
/** Whitespace and ordinary comments may precede the first marker, but executable source makes this a normal file. */
fn start(source: &str) -> Option<usize> {
    let mut offset = 0;
    while offset < source.len() {
        let rest = &source[offset..];
        if rest.starts_with(PREFIX) {
            return Some(offset);
        }
        let first = rest.chars().next()?;
        if first.is_ascii_whitespace() {
            offset += first.len_utf8();
        } else if rest.starts_with("//") {
            offset += rest.find('\n')? + 1;
        } else if rest.starts_with("/*") {
            offset += rest.find("*/")? + 2;
        } else {
            return None;
        }
    }
    None
}
/** Filesystem-like module names are normalized lexically without touching or trusting any host path. */
fn module_url(root: &Path, name: &str) -> Result<String, String> {
    if name.starts_with("http://") || name.starts_with("https://") || name.starts_with("data:") {
        return url::Url::parse(name)
            .map(String::from)
            .map_err(|e| e.to_string());
    }
    let name = name.replace('\\', "/");
    let raw = root.join(name);
    let mut path = PathBuf::new();
    for component in raw.components() {
        match component {
            Component::ParentDir => {
                path.pop();
            }
            Component::CurDir => {}
            other => path.push(other.as_os_str()),
        }
    }
    url::Url::from_file_path(path)
        .map(String::from)
        .map_err(|_| "Invalid bundle module path".into())
}
/** Header spelling, CRLF trimming, anonymous naming and next-marker searching follow d8's native container parser. */
pub(super) fn parse(source: &str, root: &Path) -> Result<Option<Bundle>, String> {
    let Some(mut offset) = start(source) else {
        return Ok(None);
    };
    let mut bundle = Bundle::default();
    let mut anonymous = 0;
    while offset < source.len() {
        let newline = source[offset..].find('\n').map(|index| offset + index);
        let header = source[offset..newline.unwrap_or(source.len())]
            .strip_suffix('\r')
            .unwrap_or(&source[offset..newline.unwrap_or(source.len())]);
        let body_start = newline.map_or(source.len(), |index| index + 1);
        let end = source[body_start..]
            .find(PREFIX)
            .map_or(source.len(), |index| body_start + index);
        let body = source[body_start..end].to_string();
        offset = end;
        if header == SCRIPT {
            bundle.items.push(Item::Script(body));
        } else if let Some(name) = header.strip_prefix(MODULE) {
            bundle.modules.insert(module_url(root, name)?, body);
        } else if let Some(rest) = header.strip_prefix(ENTRY) {
            let name = match rest.strip_prefix(':') {
                Some(name) => name.to_string(),
                None => {
                    let name = format!("entrypoint_{anonymous}.mjs");
                    anonymous += 1;
                    name
                }
            };
            let url = module_url(root, &name)?;
            bundle.modules.insert(url.clone(), body);
            bundle.items.push(Item::Module(url));
        } else {
            eprintln!("Warning: Unknown bundle marker: {header}");
        }
    }
    Ok(Some(bundle))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    /** Script parts preserve directives independently and modules are collected before entrypoint execution. */
    fn native_container_preserves_content_and_order() {
        let result=parse("// comment\n/* preface */\n// JS_BUNDLE_SCRIPT\r\n'use strict';\n// JS_BUNDLE_MODULE_ENTRYPOINT\nimport './later.mjs';\n// JS_BUNDLE_MODULE:later.mjs\nexport const x=42;",Path::new("/checkout")).unwrap().unwrap();
        assert!(matches!(&result.items[0],Item::Script(source)if source=="'use strict';\n"));
        assert!(
            matches!(&result.items[1],Item::Module(url)if url=="file:///checkout/entrypoint_0.mjs")
        );
        assert_eq!(
            result.modules["file:///checkout/later.mjs"],
            "export const x=42;"
        );
    }
    #[test]
    /** A marker embedded after executable code never changes a normal file into a native bundle. */
    fn executable_preamble_is_not_a_bundle() {
        assert!(
            parse(
                "const x=1;\n// JS_BUNDLE_SCRIPT\nthrow 42",
                Path::new("/checkout")
            )
            .unwrap()
            .is_none()
        );
    }
}
