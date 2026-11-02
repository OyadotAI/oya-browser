/** `border-image` parsing, its fetched url, and the nine-slice geometry painters share. */
use mars_eye::{
    BackgroundLayer, BorderImage, BorderImageLength, BorderImageRepeat, DisplayItem, Document,
    NinePiece, background_urls, compute_styles, nine_pieces, parse, render,
};

/** The page width the cases lay out at. */
const VIEWPORT: f64 = 800.0;

/** Styles `markup` with one sheet. */
fn styled(markup: &str, sheet: &str) -> Document {
    compute_styles(&parse(markup).unwrap(), &[sheet.into()]).unwrap()
}
/** The computed border image of the element with id `a`. */
fn image(sheet: &str) -> BorderImage {
    let document = styled("<div id=a></div>", sheet);
    let node = document.nodes.iter().find(|n| n.html_id == "a").unwrap();
    node.style.border_image.clone()
}
/** The display item of the first div. */
fn item(sheet: &str) -> DisplayItem {
    let display = render(&styled("<div></div>", sheet), VIEWPORT).unwrap();
    display.items.into_iter().find(|i| i.tag == "div").unwrap()
}

#[test]
fn the_shorthand_sets_every_part() {
    let image = image("div{border-image:url(\"B.png\") 27 fill / 10px / 2 round space}");
    assert_eq!(
        image.source,
        Some(BackgroundLayer::Image {
            url: "B.png".into()
        })
    );
    assert_eq!(image.slice[0].px, 27.0);
    assert!(image.fill);
    assert!(matches!(image.width[1], BorderImageLength::Length(e) if e.px == 10.0));
    assert_eq!(image.outset[3], BorderImageLength::Number(2.0));
    assert_eq!(
        image.repeat,
        [BorderImageRepeat::Round, BorderImageRepeat::Space]
    );
}

#[test]
fn the_shorthand_resets_what_it_leaves_out() {
    let image = image("div{border-image-slice:5;border-image:linear-gradient(red,red)}");
    assert_eq!(image.slice[0].percent, 100.0);
    assert!(matches!(image.source, Some(BackgroundLayer::Linear { .. })));
}

#[test]
fn slices_and_widths_expand_clockwise() {
    let image = image(
        "div{border-image-slice:40% 30% 20% 10%;border-image-width:1px 5px;\
         border-image-repeat:repeat}",
    );
    let percents: Vec<f64> = image.slice.iter().map(|e| e.percent).collect();
    assert_eq!(percents, [40.0, 30.0, 20.0, 10.0]);
    assert_eq!(image.width[2], image.width[0]);
    assert_eq!(image.width[3], image.width[1]);
    assert_eq!(image.repeat, [BorderImageRepeat::Repeat; 2]);
}

#[test]
fn auto_width_and_calc_parse() {
    let image = image("div{border-image-width:auto calc(50% + 50px)}");
    assert_eq!(image.width[0], BorderImageLength::Auto);
    assert!(
        matches!(image.width[1], BorderImageLength::Length(e) if e.px == 50.0 && e.percent == 50.0)
    );
}

#[test]
fn invalid_values_are_dropped() {
    let image = image(
        "div{border-image-slice:-1;border-image-repeat:repeat-x;\
         border-image-outset:10%;border-image-width:1 2 3 4 5}",
    );
    assert_eq!(image, BorderImage::default());
}

#[test]
fn a_border_image_url_is_fetched_when_no_background_claims_the_identity() {
    let document = styled(
        "<div id=a></div><div id=b></div>",
        "#a{border-image:url(edge.png) 1}#b{background:url(bg.png);border-image:url(edge.png) 1}",
    );
    let urls: Vec<String> = background_urls(&document, "https://site.example/")
        .into_iter()
        .map(|(_, url)| url)
        .collect();
    assert_eq!(
        urls,
        [
            "https://site.example/edge.png",
            "https://site.example/bg.png"
        ]
    );
}

#[test]
fn a_box_without_a_source_has_no_display_border_image() {
    assert!(item("div{border:5px solid}").border_image.is_none());
    let shared = item("div{background:url(bg.png);border-image:url(edge.png) 1}");
    assert!(shared.border_image.is_none());
}

#[test]
fn numbers_multiply_the_border_width() {
    let shown = item("div{border:4px solid;border-image:linear-gradient(red,red) 1 / 2 / 1}");
    let image = shown.border_image.unwrap();
    assert_eq!(image.widths, [Some(8.0); 4]);
    assert_eq!(image.outset, [4.0; 4]);
}

/** The pieces of a 30x30 image sliced at 10 into a 50x50 box with 10px borders. */
fn pieces(sheet: &str) -> Vec<NinePiece> {
    let shown = item(&format!(
        "body{{margin:0}}div{{width:30px;height:30px;{sheet}}}"
    ));
    let image = shown.border_image.as_ref().unwrap();
    let bounds = [shown.x, shown.y, shown.width, shown.height];
    nine_pieces(bounds, image, [30.0, 30.0])
}

#[test]
fn corners_and_edges_without_fill_make_eight_pieces() {
    let all = pieces("border:10px solid;border-image:url(i.png) 10");
    assert_eq!(all.len(), 8);
    assert_eq!(all[0].source, [0.0, 0.0, 10.0, 10.0]);
    assert_eq!(all[0].dest, [0.0, 0.0, 10.0, 10.0]);
    assert_eq!(
        pieces("border:10px solid;border-image:url(i.png) 10 fill").len(),
        9
    );
}

#[test]
fn round_fits_a_whole_number_of_tiles() {
    let all = pieces("border:10px solid;border-image:url(i.png) 10 round");
    let top = all
        .iter()
        .find(|p| p.dest == [10.0, 0.0, 30.0, 10.0])
        .unwrap();
    assert_eq!(top.tile, [10.0, 10.0]);
}

#[test]
fn repeat_centers_the_tiles() {
    let all = pieces("border:10px solid;border-image:url(i.png) 10 / 10px repeat;width:35px");
    let top = all
        .iter()
        .find(|p| p.dest[1] == 0.0 && p.dest[0] == 10.0)
        .unwrap();
    assert_eq!(top.phase[0], (35.0 - 10.0) / 2.0);
}

#[test]
fn space_without_room_for_a_tile_paints_nothing() {
    let all = pieces("border:20px solid;border-image:url(i.png) 10 / 20px space;width:15px");
    assert!(all.iter().all(|p| p.dest[2] != 15.0));
}

#[test]
fn overlapping_widths_scale_down_together() {
    let all = pieces("border:10px solid;border-image:url(i.png) 10 / 40px");
    let corner = all[0];
    assert_eq!(corner.dest, [0.0, 0.0, 25.0, 25.0]);
}
