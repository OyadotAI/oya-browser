/** A `var()` substitutes tokens, not text, and generated boxes resolve percentage paddings. */
use mars_eye::{DisplayList, StyleSource, compute_styles, parse, render};

/** Styles from the document's own `<style>` elements, then the display list at 1100px. */
fn layout(markup: &str) -> DisplayList {
    let parsed = parse(markup).unwrap();
    let sheets = parsed
        .styles
        .iter()
        .filter_map(|source| match source {
            StyleSource::Inline(css) => Some(css.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    render(&compute_styles(&parsed, &sheets).unwrap(), 1100.0).unwrap()
}

/** `var(--w)minmax(0,1fr)` is a length token then a function token, so the grid keeps two
 * tracks (CNN's story grid wrote it that way; spliced as `200pxminmax` the whole track list
 * was dropped and the text column ran full width under its neighbour). */
#[test]
fn var_next_to_a_function_keeps_both_tracks() {
    let display = layout(
        "<style>body{margin:0}p{margin:0}.g{--w:var(--unset,200px);display:grid;width:600px;\
         grid-template-columns:var(--w)minmax(0,1fr)}</style><div class=g><p>a</p><p>b</p></div>",
    );
    let cells: Vec<_> = display.items.iter().filter(|i| i.tag == "p").collect();
    assert_eq!((cells[0].x, cells[0].width), (0.0, 200.0));
    assert_eq!(
        (cells[1].x, cells[1].y, cells[1].width),
        (200.0, 0.0, 400.0)
    );
}

/** A `::after` with `padding-bottom: 56.25%` sizes a 16:9 image box even when no element of
 * the page has a percentage side of its own (the box stayed 0 tall and its image vanished). */
#[test]
fn generated_percentage_padding_sizes_its_box() {
    let display = layout(
        "<style>body{margin:0}.c{width:160px;display:flex;flex-direction:column}\
         .c::after{content:'';padding-bottom:56.25%;display:block}</style><div class=c></div>",
    );
    let after = display.items.iter().find(|i| i.tag == "::after").unwrap();
    assert_eq!(after.height, 90.0);
}
