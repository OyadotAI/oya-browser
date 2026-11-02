/** Numeric cases pin content boxes, factor weighting and iterative constraint freezing. */
use super::*;

/** Content-box dimensions add padding and border while border-box dimensions include them. */
#[test]
fn box_sizing_controls_the_painted_outer_width() {
    let document = styled(
        "<div id=a></div><div id=b></div>",
        "div{width:100px;padding:10px;border:2px solid black}#b{box-sizing:border-box}",
    );
    let display = render(&document, 500.0).unwrap();
    close(item(&document, &display, "a").width, 124.0);
    close(item(&document, &display, "b").width, 100.0);
}

/** Positive free space follows grow factors after the inter-item gap is reserved. */
#[test]
fn row_growth_preserves_factors_and_gap() {
    let document = styled(
        "<div id=row><div id=a></div><div id=b></div></div>",
        "#row{display:flex;width:400px;gap:20px}#a{flex:1 1 100px;padding:10px 0;min-width:0}#b{flex:3 1 100px;padding:10px 0;min-width:0}",
    );
    let display = render(&document, 600.0).unwrap();
    let a = item(&document, &display, "a");
    let b = item(&document, &display, "b");
    close(a.width, 145.0);
    close(b.width, 235.0);
    close(b.x - a.x - a.width, 20.0);
    close(a.y, b.y);
}

/** Shrink factors multiply the base size rather than dividing negative space equally. */
#[test]
fn negative_space_uses_scaled_shrink_weights() {
    let document = styled(
        "<div id=row><div id=a></div><div id=b></div></div>",
        "#row{display:flex;width:240px}#a{flex:0 1 200px;min-width:0;padding:10px 0}#b{flex:0 2 100px;min-width:0;padding:10px 0}",
    );
    let display = render(&document, 500.0).unwrap();
    close(item(&document, &display, "a").width, 170.0);
    close(item(&document, &display, "b").width, 70.0);
}

/** A max-constrained item freezes before its unused growth is redistributed. */
#[test]
fn maximum_width_freezes_and_redistributes_growth() {
    let document = styled(
        "<div id=row><div id=a></div><div id=b></div></div>",
        "#row{display:flex;width:400px}#a{flex:1 1 100px;max-width:120px;min-width:0;padding:10px 0}#b{flex:1 1 100px;min-width:0;padding:10px 0}",
    );
    let display = render(&document, 600.0).unwrap();
    close(item(&document, &display, "a").width, 120.0);
    close(item(&document, &display, "b").width, 280.0);
}

/** A minimum-constrained item freezes before the remaining shrink is redistributed. */
#[test]
fn minimum_width_freezes_and_redistributes_shrink() {
    let document = styled(
        "<div id=row><div id=a></div><div id=b></div></div>",
        "#row{display:flex;width:200px}#a{flex:0 1 150px;min-width:140px;padding:10px 0}#b{flex:0 1 150px;min-width:0;padding:10px 0}",
    );
    let display = render(&document, 500.0).unwrap();
    close(item(&document, &display, "a").width, 140.0);
    close(item(&document, &display, "b").width, 60.0);
}

/** Automatic minimum size protects text until the author explicitly permits shrinking. */
#[test]
fn explicit_zero_minimum_allows_a_long_word_to_shrink() {
    let body =
        "<div id=row><div id=a>UnbreakableUnbreakableUnbreakable</div><div id=b></div></div>";
    let css = "#row{display:flex;width:180px}#a{flex:1 1 200px}#b{flex:0 0 80px;padding:10px 0}";
    let automatic = styled(body, css);
    let zero = styled(body, &format!("{css}#a{{min-width:0}}"));
    let auto_display = render(&automatic, 500.0).unwrap();
    let zero_display = render(&zero, 500.0).unwrap();
    assert!(item(&automatic, &auto_display, "a").width > item(&zero, &zero_display, "a").width);
    close(item(&zero, &zero_display, "a").width, 100.0);
}

/** Fractional factor totals below one leave the undistributed initial free space available for justification. */
#[test]
fn fractional_grow_factors_do_not_consume_all_remaining_space() {
    let document = styled(
        "<div id=row><div id=a></div><div id=b></div></div>",
        "#row{display:flex;width:400px}#a,#b{flex:0.25 1 100px;min-width:0;padding:10px 0}",
    );
    let display = render(&document, 600.0).unwrap();
    close(item(&document, &display, "a").width, 150.0);
    close(item(&document, &display, "b").width, 150.0);
}
