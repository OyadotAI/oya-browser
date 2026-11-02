/** Incremental restyle: a cascade that reuses the page's previous one gives exactly the styles a
fresh full cascade gives, whatever changed between them. */
use mars_eye::{Document, compute_styles, parse};

/** Rules whose matching reads ancestors, siblings, descendants, inheritance and custom properties. */
const SHEET: &str = "
  .open .menu { display: block; color: rgb(1, 2, 3); }
  .menu { display: none; }
  ul > li + li { margin-top: 7px; }
  li ~ li.last { padding-left: 9px; }
  li:nth-child(2n) { background-color: rgb(4, 5, 6); }
  li:first-child { font-weight: 700; }
  .card:has(.badge) { border-top: 3px solid rgb(7, 8, 9); }
  p:empty { height: 11px; }
  .theme { --accent: rgb(10, 11, 12); }
  .uses { color: var(--accent, rgb(0, 0, 0)); }
  input:checked + label { font-style: italic; }
  input:disabled { opacity: 0.5; }
  .big { font-size: 30px; }
";

/** The page before each change. */
const BEFORE: &str = r#"<div id="root" class="theme">
  <nav class="bar"><div class="menu">Menu</div></nav>
  <ul><li>a</li><li>b</li><li class="last">c</li></ul>
  <div class="card"><span>x</span></div>
  <p></p><p>text</p>
  <span class="uses">accent</span>
  <input type="checkbox" id="c"><label for="c">check</label>
  <input id="d">
  <section><em>inherits</em></section>
</div>"#;

/** Each change, as the page after it: attributes, siblings, descendants, text and inheritance. */
const AFTER: [&str; 9] = [
    // An ancestor's class changes what a descendant combinator matches.
    r#"<div id="root" class="theme">
  <nav class="bar open"><div class="menu">Menu</div></nav>
  <ul><li>a</li><li>b</li><li class="last">c</li></ul>
  <div class="card"><span>x</span></div>
  <p></p><p>text</p>
  <span class="uses">accent</span>
  <input type="checkbox" id="c"><label for="c">check</label>
  <input id="d">
  <section><em>inherits</em></section>
</div>"#,
    // A sibling inserted before others shifts nth-child, + and ~ matches.
    r#"<div id="root" class="theme">
  <nav class="bar"><div class="menu">Menu</div></nav>
  <ul><li>new</li><li>a</li><li>b</li><li class="last">c</li></ul>
  <div class="card"><span>x</span></div>
  <p></p><p>text</p>
  <span class="uses">accent</span>
  <input type="checkbox" id="c"><label for="c">check</label>
  <input id="d">
  <section><em>inherits</em></section>
</div>"#,
    // A descendant appears, so :has() starts matching its ancestor.
    r#"<div id="root" class="theme">
  <nav class="bar"><div class="menu">Menu</div></nav>
  <ul><li>a</li><li>b</li><li class="last">c</li></ul>
  <div class="card"><span>x</span><i class="badge">1</i></div>
  <p></p><p>text</p>
  <span class="uses">accent</span>
  <input type="checkbox" id="c"><label for="c">check</label>
  <input id="d">
  <section><em>inherits</em></section>
</div>"#,
    // Text makes an :empty element non-empty.
    r#"<div id="root" class="theme">
  <nav class="bar"><div class="menu">Menu</div></nav>
  <ul><li>a</li><li>b</li><li class="last">c</li></ul>
  <div class="card"><span>x</span></div>
  <p>now</p><p>text</p>
  <span class="uses">accent</span>
  <input type="checkbox" id="c"><label for="c">check</label>
  <input id="d">
  <section><em>inherits</em></section>
</div>"#,
    // A custom property on an ancestor changes a descendant's var().
    r#"<div id="root" class="theme" style="--accent: rgb(90, 91, 92)">
  <nav class="bar"><div class="menu">Menu</div></nav>
  <ul><li>a</li><li>b</li><li class="last">c</li></ul>
  <div class="card"><span>x</span></div>
  <p></p><p>text</p>
  <span class="uses">accent</span>
  <input type="checkbox" id="c"><label for="c">check</label>
  <input id="d">
  <section><em>inherits</em></section>
</div>"#,
    // Form state drives :checked + sibling and :disabled.
    r#"<div id="root" class="theme">
  <nav class="bar"><div class="menu">Menu</div></nav>
  <ul><li>a</li><li>b</li><li class="last">c</li></ul>
  <div class="card"><span>x</span></div>
  <p></p><p>text</p>
  <span class="uses">accent</span>
  <input type="checkbox" id="c" checked><label for="c">check</label>
  <input id="d" disabled>
  <section><em>inherits</em></section>
</div>"#,
    // An inherited property changes on an ancestor whose children did not change.
    r#"<div id="root" class="theme">
  <nav class="bar"><div class="menu">Menu</div></nav>
  <ul><li>a</li><li>b</li><li class="last">c</li></ul>
  <div class="card"><span>x</span></div>
  <p></p><p>text</p>
  <span class="uses">accent</span>
  <input type="checkbox" id="c"><label for="c">check</label>
  <input id="d">
  <section class="big"><em>inherits</em></section>
</div>"#,
    // A node is removed from the middle.
    r#"<div id="root" class="theme">
  <nav class="bar"><div class="menu">Menu</div></nav>
  <ul><li>a</li><li class="last">c</li></ul>
  <div class="card"><span>x</span></div>
  <p></p><p>text</p>
  <span class="uses">accent</span>
  <input type="checkbox" id="c"><label for="c">check</label>
  <input id="d">
  <section><em>inherits</em></section>
</div>"#,
    // Nothing changes: every style is reused.
    BEFORE,
];

/** The page's styles, cascaded on this thread (so after any earlier cascade here). */
fn styled(html: &str) -> Document {
    compute_styles(&parse(html).unwrap(), &[SHEET.to_owned()]).unwrap()
}

/** Every node's computed style, in node order, as comparable text. */
fn styles(document: &Document) -> Vec<String> {
    document
        .nodes
        .iter()
        .map(|node| format!("{} {} {:?}", node.local_id, node.tag, node.style))
        .collect()
}

/** After every kind of change, reusing the previous cascade equals styling from scratch. */
#[test]
fn incremental_restyle_equals_a_full_cascade() {
    for (case, after) in AFTER.iter().enumerate() {
        let incremental = std::thread::spawn(move || {
            styled(BEFORE);
            styles(&styled(after))
        })
        .join()
        .unwrap();
        let full = std::thread::spawn(move || styles(&styled(after)))
            .join()
            .unwrap();
        assert_eq!(incremental.len(), full.len(), "case {case}");
        for (index, (a, b)) in incremental.iter().zip(&full).enumerate() {
            assert_eq!(a, b, "case {case}, node {index}");
        }
    }
}

/** A changed sheet list never reuses: the same page under new rules styles from scratch. */
#[test]
fn changed_sheets_style_from_scratch() {
    let incremental = std::thread::spawn(|| {
        styled(BEFORE);
        let other = ".menu { display: block; color: rgb(50, 60, 70); }".to_owned();
        styles(&compute_styles(&parse(BEFORE).unwrap(), &[other]).unwrap())
    })
    .join()
    .unwrap();
    let full = std::thread::spawn(|| {
        let other = ".menu { display: block; color: rgb(50, 60, 70); }".to_owned();
        styles(&compute_styles(&parse(BEFORE).unwrap(), &[other]).unwrap())
    })
    .join()
    .unwrap();
    assert_eq!(incremental, full);
}

/** Rules that depend on a container's measured size, which a second pass resolves. */
const CONTAINER_SHEET: &str = "
  .box { container-type: inline-size; }
  .wide { width: 400px; }
  .narrow { width: 200px; }
  @container (min-width: 300px) { .inner { color: rgb(1, 1, 1); } }
  .inner { font-size: 5cqw; }
";

/** A container page whose container is `class`, with an unrelated sibling at `other`. */
fn container_page(class: &str, other: &str) -> String {
    format!(r#"<div class="box {class}"><p class="inner">x</p></div><p class="{other}">y</p>"#)
}

/** Container queries: resizing the container restyles what reads it, and an unrelated change
still equals a full cascade. */
#[test]
fn container_query_passes_reuse_only_with_unchanged_sizes() {
    let sheets = || vec![CONTAINER_SHEET.to_owned()];
    for (before, after) in [
        (container_page("wide", "a"), container_page("narrow", "a")),
        (container_page("wide", "a"), container_page("wide", "b")),
        (container_page("wide", "a"), container_page("wide", "a")),
    ] {
        let after_full = after.clone();
        let incremental = std::thread::spawn(move || {
            compute_styles(&parse(&before).unwrap(), &sheets()).unwrap();
            styles(&compute_styles(&parse(&after).unwrap(), &sheets()).unwrap())
        })
        .join()
        .unwrap();
        let full = std::thread::spawn(move || {
            styles(&compute_styles(&parse(&after_full).unwrap(), &sheets()).unwrap())
        })
        .join()
        .unwrap();
        assert_eq!(incremental, full);
    }
}
