/** Round-five rules from the inline builder: a `local` background paints with the scrolled
 * contents, page fonts are requested for pseudo-elements, unit-only elements and fallback
 * families, a first letter's background images are fetched, an SVG image paints its root
 * background, spread shadow corners follow corner shaping, and selector edge cases. */
use mars_eye::{
    DisplayList, background_urls, compute_styles, font_requests, parse, render_scrolled,
    spread_radius,
};

/** The page base the url cases resolve against. */
const BASE: &str = "https://example.test/";

/** The display list of `markup` styled by `sheet`, with `#s` scrolled by `offset`. */
fn scrolled(markup: &str, sheet: &str, offset: [f64; 2]) -> (DisplayList, usize) {
    let document = compute_styles(&parse(markup).unwrap(), &[sheet.to_string()]).unwrap();
    let local = document
        .nodes
        .iter()
        .find(|n| n.html_id == "s")
        .unwrap()
        .local_id;
    let offsets = mars_eye::ScrollOffsets::from([(local, offset)]);
    let context = mars_eye::RenderContext {
        width: 800.0,
        height: 600.0,
        ..Default::default()
    };
    (
        render_scrolled(&document, context, &offsets).unwrap(),
        local,
    )
}

/** A scrolled `local` box clips its background to the padding box, and a `content-box` clip
 * also to the scrolled contents' content box within it. */
#[test]
fn local_background_clips_with_the_scrolled_contents() {
    let markup = "<div id=s><div style='height:500px'></div></div>";
    let sheet = |clip: &str| {
        format!(
            "body{{margin:0}} #s{{width:200px;height:200px;padding:40px;border:10px solid;\
             overflow:hidden;background:green local {clip}}}"
        )
    };
    let clip = |name: &str| {
        let (list, local) = scrolled(markup, &sheet(name), [0.0, 15.0]);
        let item = list
            .items
            .iter()
            .find(|item| item.local_id == local)
            .unwrap();
        (item.background_clip, item.background_rect)
    };
    assert_eq!(clip("border-box"), ([10.0; 4], None));
    // The content-box clip is the padding box plus the scrolled content box's rectangle.
    assert_eq!(
        clip("content-box"),
        ([10.0; 4], Some([50.0, 35.0, 200.0, 255.0]))
    );
}

/** The faces each case's page asks for, by `@font-face` family. */
fn requested(markup: &str, sheet: &str) -> Vec<String> {
    let document = compute_styles(&parse(markup).unwrap(), &[sheet.to_string()]).unwrap();
    let faces = &document.formatting.font_faces;
    font_requests(&document)
        .into_iter()
        .map(|request| faces[request.face].family.clone())
        .collect()
}

/** A font named only by a `::first-letter` rule, or only measured by an empty box's `ch`
 * width, is requested; a character the first family's range leaves out comes from the next. */
#[test]
fn fonts_are_requested_for_pseudos_units_and_fallbacks() {
    let face = |family: &str, range: &str| {
        format!("@font-face{{font-family:{family};src:url({family}.woff);{range}}}")
    };
    let letter = format!("{} p::first-letter{{font-family:L}}", face("L", ""));
    assert_eq!(requested("<p>Text</p>", &letter), ["l"]);
    let unit = format!("{} div{{font-family:U;width:5ch}}", face("U", ""));
    assert_eq!(requested("<div></div>", &unit), ["u"]);
    let fallback = format!(
        "{}{} div{{font-family:A,I}}",
        face("A", "unicode-range:U+0-7F;"),
        face("I", "")
    );
    assert_eq!(requested("<div>a\u{6C34}</div>", &fallback), ["a", "i"]);
}

/** A `::first-letter` background image is fetched under its element's identity. */
#[test]
fn first_letter_background_images_are_fetched() {
    let document = compute_styles(
        &parse("<p>Text</p>").unwrap(),
        &["p::first-letter{background-image:url(a.png)}".into()],
    )
    .unwrap();
    let urls = background_urls(&document, BASE);
    assert_eq!(urls.len(), 1);
    assert!(urls[0].1.ends_with("/a.png"));
}

/** An SVG image's root inline `background` fills the image under its drawing. */
#[test]
fn svg_image_paints_its_root_background() {
    let markup = "<svg xmlns='http://www.w3.org/2000/svg' style='background: green'></svg>";
    let paths = mars_eye::VectorPath::from_image(markup, [0.0, 0.0, 10.0, 10.0]);
    assert_eq!(paths[0].fill.as_deref(), Some("green"));
    let bare = "<svg xmlns='http://www.w3.org/2000/svg'></svg>";
    assert!(mars_eye::VectorPath::from_image(bare, [0.0, 0.0, 10.0, 10.0]).is_empty());
}

/** A spread keeps square corners square, adds itself to a large radius, shapes a small one by
 * the cubic ratio, and keeps a circle a circle. */
#[test]
fn spread_shadow_corners_follow_corner_shaping() {
    let size = [100.0, 100.0];
    assert_eq!(spread_radius([0.0; 2], size, 10.0), [0.0; 2]);
    assert_eq!(spread_radius([20.0; 2], size, 10.0), [30.0; 2]);
    let shaped = spread_radius([5.0; 2], [20.0, 20.0], 20.0)[0];
    assert!((shaped - 17.6171875).abs() < 0.01, "{shaped}");
    assert_eq!(spread_radius([1.0; 2], [2.0, 2.0], 149.0), [150.0; 2]);
    assert_eq!(spread_radius([20.0; 2], size, -30.0), [0.0; 2]);
}

/** Whether `#t` takes the green background of the rule `selector`. */
fn green(markup: &str, selector: &str) -> bool {
    let sheet = format!("{selector}{{background:green}}");
    let document = compute_styles(&parse(markup).unwrap(), &[sheet]).unwrap();
    let node = document.nodes.iter().find(|n| n.html_id == "t").unwrap();
    node.style.background.as_deref() == Some("green")
}

/** `[|name]` matches an HTML attribute, `[ | name]` and An+B with a space inside a term are
 * invalid, and no selector matches a text node. */
#[test]
fn selector_edge_cases() {
    assert!(green("<p id=t data-x></p>", "[|data-x]"));
    assert!(!green("<p id=t data-x></p>", "[ | data-x]"));
    assert!(green("<p id=t></p>", ":nth-child(n + 1)"));
    assert!(!green("<p id=t></p>", ":nth-child(1 n)"));
    assert!(!green("<p id=t></p>", ":nth-child(+ 1)"));
    let document = compute_styles(
        &parse("<p><b>a</b>b</p>").unwrap(),
        &["*{background:green}".into()],
    )
    .unwrap();
    let text = document.nodes.iter().find(|n| n.tag == "#text").unwrap();
    assert_eq!(text.style.background, None);
}
