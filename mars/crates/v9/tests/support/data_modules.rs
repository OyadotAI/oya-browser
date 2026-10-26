/** The pinned d8 shell treats its exact data-module prefix as literal JavaScript source, not a web data-URL decoder. */

/** d8.cc's kDataURLPrefix intentionally excludes MIME parameters and base64 transport. */
const PREFIX: &str = "data:text/javascript,";

/** Native d8 feeds the unchanged suffix to its module compiler, including percent signs, hashes and newlines. */
pub(super) fn source(url: &str) -> Option<String> {
    url.strip_prefix(PREFIX).map(str::to_string)
}
