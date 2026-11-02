/** Untrusted markup never panics or hangs: every input parses or fails with a typed error, fast. */
use mars_eye::{compute_styles, parse, render};
use std::time::{Duration, Instant};

/** Hand-picked truncations and pathologies that end inside a token. */
const EDGES: [&str; 32] = [
    "<",
    "</",
    "<a",
    "<a b",
    "<a b=",
    "<a b='",
    "<a b=\"x",
    "</a",
    "&",
    "&#",
    "&#x",
    "&#x;",
    "&#99999999999999999;",
    "<!--",
    "<!-",
    "<!",
    "<![CDATA[",
    "<?",
    "<table><td><table><td>",
    "<<<>>>",
    "<p></p></p></p>",
    "<svg><![CDATA[x",
    "<template><template>",
    "<iframe>",
    "<script>",
    "<style>",
    "<textarea>",
    "<title>",
    "<noscript>",
    "<select><option><optgroup>",
    "<video><source>",
    "\u{0}<\u{0}a\u{0}>",
];

/** Tokens combined by a fixed-seed generator so the corpus is reproducible. */
const PIECES: [&str; 40] = [
    "<p>",
    "</p>",
    "<div>",
    "</div>",
    "<table>",
    "</table>",
    "<tr>",
    "<td>",
    "</td>",
    "<th colspan=9>",
    "<li>",
    "<ul>",
    "<b>",
    "</b>",
    "<i>",
    "</i>",
    "<a href=x>",
    "</a>",
    "<svg>",
    "<circle/>",
    "<path d='M0 0'/>",
    "</svg>",
    "<template>",
    "</template>",
    "<iframe>",
    "</iframe>",
    "<form>",
    "</form>",
    "<select>",
    "<option>",
    "text",
    "&amp;",
    "&#x",
    "<!--",
    "-->",
    "<",
    ">",
    "<br/>",
    "<video>",
    "<input type=number>",
];

/** A tiny deterministic linear congruential generator. */
fn next(seed: &mut u64) -> usize {
    *seed = seed
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    (*seed >> 33) as usize
}

/** Parsing, styling and rendering one input must return, never panic. */
fn survive(html: &str) {
    if let Ok(document) = parse(html)
        && let Ok(styled) = compute_styles(&document, &[])
    {
        let _ = render(&styled, 640.0);
    }
}

/** Edge inputs ending inside tags, references or raw text never panic. */
#[test]
fn truncated_markup_never_panics() {
    for edge in EDGES {
        survive(edge);
        survive(&format!("<p>{edge}"));
    }
}

/** A reproducible random corpus of tag soup never panics and stays fast. */
#[test]
fn random_tag_soup_never_panics() {
    let mut seed = 7;
    let started = Instant::now();
    for _ in 0..400 {
        let html = (0..next(&mut seed) % 80)
            .map(|_| PIECES[next(&mut seed) % PIECES.len()])
            .collect::<String>();
        survive(&html);
    }
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "{:?}",
        started.elapsed()
    );
}

/** Many unclosed raw-text openers cannot make scanning quadratic. */
#[test]
fn repeated_unclosed_markup_stays_linear() {
    let html = "<a b='x' </ <!x <? &#x ".repeat(40_000);
    let started = Instant::now();
    let _ = parse(&html);
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );
}
