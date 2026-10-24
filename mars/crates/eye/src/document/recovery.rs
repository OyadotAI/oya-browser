/** The tree builder's element categories and scopes from the HTML Living Standard, so implied and
misnested end tags recover the way browsers recover them instead of refusing the page. */
/** Elements the standard calls special: an unmatched ordinary end tag stops at them. */
pub(super) const SPECIAL: [&str; 81] = [
    "address",
    "applet",
    "area",
    "article",
    "aside",
    "base",
    "basefont",
    "bgsound",
    "blockquote",
    "body",
    "br",
    "button",
    "caption",
    "center",
    "col",
    "colgroup",
    "dd",
    "details",
    "dir",
    "div",
    "dl",
    "dt",
    "embed",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "frame",
    "frameset",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "head",
    "header",
    "hgroup",
    "hr",
    "html",
    "iframe",
    "img",
    "input",
    "keygen",
    "li",
    "link",
    "listing",
    "main",
    "marquee",
    "menu",
    "meta",
    "nav",
    "noembed",
    "noframes",
    "noscript",
    "object",
    "ol",
    "p",
    "param",
    "plaintext",
    "pre",
    "script",
    "search",
    "section",
    "select",
    "source",
    "style",
    "summary",
    "table",
    "tbody",
    "td",
    "template",
    "textarea",
    "tfoot",
    "th",
    "thead",
    "title",
    "tr",
    "track",
    "ul",
];
/** Start tags that first close an open paragraph in button scope. */
pub(super) const CLOSES_P: [&str; 39] = [
    "address",
    "article",
    "aside",
    "blockquote",
    "center",
    "details",
    "dialog",
    "dir",
    "div",
    "dl",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "header",
    "hgroup",
    "hr",
    "main",
    "menu",
    "nav",
    "ol",
    "p",
    "pre",
    "search",
    "section",
    "summary",
    "table",
    "ul",
    "li",
    "dd",
    "dt",
    "listing",
];
/** Headings close one another; `</h2>` also closes an open `h3`. */
pub(super) const HEADINGS: [&str; 6] = ["h1", "h2", "h3", "h4", "h5", "h6"];
/** Elements that never have children or an end tag. */
pub(super) const VOID: [&str; 15] = [
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr", "keygen",
];
/** Default scope boundaries: an end tag cannot reach past a cell, caption, table or object. */
pub(super) const DEFAULT_SCOPE: [&str; 9] = [
    "applet", "caption", "html", "table", "td", "th", "marquee", "object", "template",
];
/** Table scope boundaries, so row and cell tags close their own table's structure only. */
pub(super) const TABLE_SCOPE: [&str; 3] = ["html", "table", "template"];
/** Open elements whose content model is table structure; other content is foster parented. */
pub(super) const TABLE_CONTEXT: [&str; 5] = ["table", "tbody", "thead", "tfoot", "tr"];
/** Content that belongs inside table structure rather than being moved out of it. */
pub(super) const TABLE_CONTENT: [&str; 13] = [
    "caption", "colgroup", "col", "tbody", "thead", "tfoot", "tr", "td", "th", "table", "script",
    "style", "form",
];
/** End tags that close only a matching element in default scope and are otherwise ignored. */
pub(super) const SCOPED_END: [&str; 51] = [
    "address",
    "article",
    "aside",
    "blockquote",
    "button",
    "center",
    "details",
    "dialog",
    "dir",
    "div",
    "dl",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "header",
    "hgroup",
    "listing",
    "main",
    "menu",
    "nav",
    "ol",
    "pre",
    "search",
    "section",
    "summary",
    "ul",
    "form",
    "dd",
    "dt",
    "applet",
    "marquee",
    "object",
    "select",
    "a",
    "b",
    "big",
    "code",
    "em",
    "font",
    "i",
    "nobr",
    "s",
    "small",
    "strike",
    "strong",
    "tt",
    "u",
    "caption",
    "colgroup",
    "template",
];
/** Embedded content Eye never loads or runs; each shows a sized placeholder or its fallback. */
pub(super) const EMBEDDED: [&str; 6] = ["iframe", "embed", "object", "video", "audio", "canvas"];
/** Children of embedded media that are resources, not fallback content. */
pub(super) const MEDIA_RESOURCES: [&str; 3] = ["source", "track", "param"];

/// The stack position of the innermost open element named in `targets`, searching from the
/// current node outward and stopping at a scope boundary, as HTML's "has an element in scope".
pub(super) fn in_scope<'a>(
    stack: impl DoubleEndedIterator<Item = (usize, &'a str)>,
    targets: &[&str],
    boundaries: &[&str],
) -> Option<usize> {
    for (position, tag) in stack.rev() {
        if targets.contains(&tag) {
            return Some(position);
        }
        if boundaries.contains(&tag) {
            return None;
        }
    }
    None
}

/// The stack position an ordinary end tag closes: the innermost matching element, unless a
/// special element sits above it, in which case the stray end tag is ignored.
pub(super) fn ordinary_end<'a>(
    stack: impl DoubleEndedIterator<Item = (usize, &'a str)>,
    tag: &str,
) -> Option<usize> {
    for (position, open) in stack.rev() {
        if open == tag {
            return Some(position);
        }
        if SPECIAL.contains(&open) {
            return None;
        }
    }
    None
}

/// The stack position a new list item closes: an open item of the same kind, unless a special
/// element other than address, div or p is open inside it.
pub(super) fn list_item<'a>(
    stack: impl DoubleEndedIterator<Item = (usize, &'a str)>,
    kinds: &[&str],
) -> Option<usize> {
    for (position, open) in stack.rev() {
        if kinds.contains(&open) {
            return Some(position);
        }
        if SPECIAL.contains(&open) && !["address", "div", "p"].contains(&open) {
            return None;
        }
    }
    None
}
