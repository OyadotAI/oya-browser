/** `url()` masks keep their image and geometry for the painter, and still withdraw agent actions. */
use mars_eye::{Document, Node, background_urls, compute_styles, parse};

/** Styles `markup` with one sheet. */
fn styled(markup: &str, sheet: &str) -> Document {
    compute_styles(&parse(markup).unwrap(), &[sheet.into()]).unwrap()
}
/** The node with `id`. */
fn by_id<'a>(document: &'a Document, id: &str) -> &'a Node {
    document.nodes.iter().find(|n| n.html_id == id).unwrap()
}

#[test]
fn mask_image_keeps_its_url_and_geometry() {
    let document = styled(
        "<span id=a>a</span>",
        "span{background-color:#333;-webkit-mask-image:url(/Icon.svg);mask-size:20px 20px;\
         mask-position:center;mask-repeat:no-repeat}",
    );
    let a = &by_id(&document, "a").style;
    let mask = a.mask.as_ref().unwrap();
    assert_eq!(mask.url, "/Icon.svg");
    assert_eq!(format!("{:?}", mask.repeat), "NoRepeat");
    assert_eq!(mask.position[0].percent, 50.0);
    assert!(a.masked && !a.interactive);
}

#[test]
fn the_mask_shorthand_parses_like_background_and_ignores_modes() {
    let document = styled(
        "<span id=a>a</span>",
        "span{mask:url(\"i.svg\") no-repeat 50% 50%/contain alpha border-box}",
    );
    let mask = by_id(&document, "a").style.mask.clone().unwrap();
    assert_eq!(mask.url, "i.svg");
    assert_eq!(format!("{:?}", mask.repeat), "NoRepeat");
    assert_eq!(format!("{:?}", mask.size), "Contain");
}

#[test]
fn a_gradient_mask_masks_without_a_url() {
    let document = styled(
        "<a id=a href=/x>a</a>",
        "a{mask-image:linear-gradient(#000,#0000)}",
    );
    let a = &by_id(&document, "a").style;
    assert!(a.masked && a.mask.is_none() && !a.interactive);
}

#[test]
fn mask_urls_are_listed_for_fetching() {
    let document = styled("<span id=a>a</span>", "span{mask:url(/m.svg)}");
    let urls = background_urls(&document, "https://example.com/page");
    assert_eq!(
        urls,
        vec![(
            by_id(&document, "a").local_id,
            "https://example.com/m.svg".into()
        )]
    );
}

#[test]
fn css_escapes_in_a_quoted_url_are_decoded() {
    let document = styled(
        "<span id=a>a</span>",
        r#"span{mask-image:url("data:image/svg+xml;utf8,<svg a=\"1\" b=\31 0>")}"#,
    );
    let mask = by_id(&document, "a").style.mask.clone().unwrap();
    assert_eq!(mask.url, r#"data:image/svg+xml;utf8,<svg a="1" b=10>"#);
}

#[test]
fn a_pseudo_elements_mask_is_listed_under_its_element() {
    let document = styled(
        "<label id=a>145 languages</label>",
        "label::after{content:'';display:inline-block;width:12px;height:12px;\
         background-color:#36c;mask-image:url(/chevron.svg)}",
    );
    let urls = background_urls(&document, "https://example.com/page");
    assert_eq!(
        urls,
        vec![(
            by_id(&document, "a").local_id,
            "https://example.com/chevron.svg".into()
        )]
    );
}

#[test]
fn data_url_backgrounds_are_listed_for_local_decoding() {
    let document = styled(
        "<span id=a></span>",
        "span{display:block;width:50px;height:50px;\
         background:#000c url(\"data:image/svg+xml,%3Csvg%3E%3C/svg%3E\") center no-repeat}",
    );
    let urls = background_urls(&document, "https://example.com/page");
    assert_eq!(urls.len(), 1);
    assert!(urls[0].1.starts_with("data:image/svg+xml,"));
}
