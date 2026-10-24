/** Visually hidden clips paint nothing but keep their text, and `@container` width queries apply. */
use mars_eye::{
    DisplayList, Document, Node, RenderContext, agent_visible, compute_styles,
    compute_styles_with_context, parse, render,
};

/** Styles `markup` with one sheet. */
fn styled(markup: &str, sheet: &str) -> Document {
    compute_styles(&parse(markup).unwrap(), &[sheet.into()]).unwrap()
}
/** The node with `id`. */
fn by_id<'a>(document: &'a Document, id: &str) -> &'a Node {
    document.nodes.iter().find(|n| n.html_id == id).unwrap()
}
/** Every item painted for `id`. */
fn items<'a>(
    document: &Document,
    display: &'a DisplayList,
    id: &str,
) -> Vec<&'a mars_eye::DisplayItem> {
    let local = by_id(document, id).local_id;
    display
        .items
        .iter()
        .filter(|item| item.local_id == local)
        .collect()
}

/** Apple's global navigation label: visible link, visually hidden text inside it. */
const APPLE: &str = ".globalnav-link-text{position:absolute;clip:rect(1px,1px,1px,1px);\
                     clip-path:inset(0 0 99.9% 99.9%)}a{display:inline-block;width:40px;height:20px}";

#[test]
fn a_visually_hidden_label_paints_nothing() {
    let document = styled(
        "<a id=link href=/store><span id=label class=globalnav-link-text>Store</span></a>",
        APPLE,
    );
    let display = render(&document, 800.0).unwrap();
    let label = items(&document, &display, "label");
    assert!(!label.is_empty());
    for item in label {
        let clip = item.clip.expect("clipped");
        assert!(clip.width * clip.height < 0.5, "{clip:?}");
        assert!(!agent_visible(item));
    }
}

#[test]
fn the_hidden_label_stays_readable_and_the_link_stays_actionable() {
    let document = styled(
        "<a id=link href=/store><span id=label class=globalnav-link-text>Store</span></a>",
        APPLE,
    );
    let (link, label) = (by_id(&document, "link"), by_id(&document, "label"));
    assert!(!label.hidden() && label.text.contains("Store"));
    assert!(
        !label.style.interactive,
        "the clipped label itself is not a target"
    );
    assert!(link.style.interactive);
    let display = render(&document, 800.0).unwrap();
    let painted = items(&document, &display, "link");
    assert!(painted.iter().any(|item| agent_visible(item)));
    let index = document
        .nodes
        .iter()
        .position(|n| n.html_id == "label")
        .unwrap();
    assert_eq!(
        document.labels()[index],
        "Store",
        "agents still read the text"
    );
}

#[test]
fn clip_rect_only_applies_to_absolute_boxes_and_inset_clips_partially() {
    let document = styled(
        "<div id=a>a</div><div id=b>b</div>",
        "div{width:100px;height:40px}#a{clip:rect(0,10px,10px,0)}#b{clip-path:inset(10px 20%)}",
    );
    let display = render(&document, 800.0).unwrap();
    assert!(items(&document, &display, "a")[0].clip.is_none());
    let b = items(&document, &display, "b")[0];
    let clip = b.clip.unwrap();
    assert_eq!((clip.x - b.x, clip.y - b.y), (20.0, 10.0));
    assert_eq!((clip.width, clip.height), (60.0, 20.0));
}

#[test]
fn unsupported_clip_shapes_still_withdraw_actions() {
    let document = styled(
        "<a id=a href=/x>a</a>",
        "a{clip-path:polygon(0 0,1px 0,0 1px)}",
    );
    assert!(!by_id(&document, "a").style.interactive);
}

/** YouTube's home grid: three columns once its container is at least 890px wide. */
const YOUTUBE: &str = "#home{container-type:inline-size}\
                       #media{display:grid;grid-template-columns:repeat(2,1fr)}\
                       @container (min-width:890px){#media{grid-template-columns:repeat(3,1fr)}}";

/** Styles at a viewport width. */
fn styled_at(markup: &str, sheet: &str, width: f64) -> Document {
    let context = RenderContext {
        width,
        ..RenderContext::default()
    };
    compute_styles_with_context(&parse(markup).unwrap(), &[sheet.into()], context).unwrap()
}

#[test]
fn container_width_queries_apply_against_the_containers_content_box() {
    let markup = "<div id=home><div id=media><i>1</i><i>2</i><i>3</i></div></div>";
    let wide = styled_at(markup, YOUTUBE, 1400.0);
    let narrow = styled_at(markup, YOUTUBE, 700.0);
    assert_eq!(by_id(&wide, "media").style.grid_columns.tracks.len(), 3);
    assert_eq!(by_id(&narrow, "media").style.grid_columns.tracks.len(), 2);
}

#[test]
fn named_queries_pick_the_nearest_container_with_that_name() {
    let sheet = "#outer{container:page/inline-size;width:1000px}#inner{container-type:inline-size;width:300px}\
                 @container page (width > 900px){#x{order:1}}\
                 @container (width > 900px){#x{z-index:5}}";
    let document = styled_at(
        "<div id=outer><div id=inner><p id=x>x</p></div></div>",
        sheet,
        1200.0,
    );
    let x = &by_id(&document, "x").style;
    assert_eq!(x.order, 1, "the named outer container is 1000px wide");
    assert_eq!(
        x.z_index, None,
        "the nearest unnamed container is 300px wide"
    );
}

#[test]
fn a_query_without_a_container_applies_nothing_but_masks_its_hiding_rules() {
    let document = styled(
        "<a id=a href=/x>a</a><p id=p>p</p>",
        "@container (min-width:1px){a{display:none}p{order:3}}",
    );
    let a = &by_id(&document, "a").style;
    assert!(!a.hidden && !a.interactive);
    assert_eq!(by_id(&document, "p").style.order, 0);
}

#[test]
fn undecidable_container_queries_still_mask_their_hiding_rules() {
    let document = styled(
        "<div id=c><a id=a href=/x>a</a></div>",
        "#c{container-type:size}@container (min-height:1px){a{display:none}}\
         @container style(--x: 1){#c a{visibility:hidden}}",
    );
    let a = &by_id(&document, "a").style;
    assert!(!a.hidden, "the unknown rule does not apply");
    assert!(!a.interactive, "but what it could hide is masked");
}

#[test]
fn a_hiding_rule_under_a_true_query_hides_and_under_a_false_one_does_not() {
    let markup = "<div id=c><a id=a href=/x>a</a></div>";
    let sheet = "#c{container-type:inline-size;width:500px}\
                 @container (max-width:600px){a{display:none}}";
    let hidden = styled(markup, sheet);
    assert!(by_id(&hidden, "a").style.hidden);
    let shown = styled(markup, &sheet.replace("max-width:600px", "max-width:400px"));
    let a = &by_id(&shown, "a").style;
    assert!(!a.hidden && a.interactive);
}

#[test]
fn hostile_container_preludes_never_panic() {
    for prelude in [
        "",
        "(",
        "((((min-width:1px",
        "none (width>1px)",
        "a b c (width>1px)",
        "(width >)",
        "(1px < width < )",
        "(min-width: 1e999px)",
    ] {
        let sheet =
            format!("#c{{container-type:inline-size}}@container {prelude}{{a{{display:none}}}}");
        let _ = styled("<div id=c><a href=/x>a</a></div>", &sheet);
    }
}
