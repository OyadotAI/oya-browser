/** Embedded CSS image payloads retain their separate limits through parsing and restored style validation. */
use mars_eye::{background_urls, compute_styles, parse};

#[test]
fn long_embedded_background_images_survive_style_validation() {
    let source = format!("data:image/png;base64,{}", "A".repeat(9000));
    let document = parse("<div id='image'>visible content</div>").unwrap();
    let styled = compute_styles(
        &document,
        &[format!("div{{background-image:url('{source}')}}")],
    )
    .unwrap();
    let urls = background_urls(&styled, "https://example.test/");
    assert_eq!(urls.len(), 1);
    assert_eq!(urls[0].1, source);
    assert!(
        styled
            .nodes
            .iter()
            .all(|node| node.style.validate().is_ok())
    );
}

#[test]
fn oversized_network_backgrounds_are_ignored_without_losing_content() {
    let source = format!("https://example.test/{}", "x".repeat(9000));
    let document = parse("<div id='image'>visible content</div>").unwrap();
    let styled = compute_styles(
        &document,
        &[format!("div{{background-image:url('{source}')}}")],
    )
    .unwrap();
    assert!(background_urls(&styled, "https://example.test/").is_empty());
    assert!(styled.nodes.iter().any(|node| node.html_id == "image"));
}
