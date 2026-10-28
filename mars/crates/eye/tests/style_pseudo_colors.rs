/** `::placeholder` and `::marker` colors reach the native field and the list marker run. Sites
restyle both (24 and 11 of 52 measured pages), and Chrome's defaults stood out against them. */
use mars_eye::{compute_styles, parse, render};

/** The styled document for `markup` under `css`. */
fn styled(markup: &str, css: &str) -> mars_eye::Document {
    compute_styles(&parse(markup).unwrap(), &[css.to_owned()]).unwrap()
}

/** An input's `::placeholder` color is kept on its style; the legacy WebKit name works too. */
#[test]
fn placeholder_color_reaches_the_input() {
    for rule in ["input::placeholder{color:#ff0000}", "input::-webkit-input-placeholder{color:#ff0000}"] {
        let document = styled("<input id=t placeholder=Search>", rule);
        let node = document.nodes.iter().find(|n| n.html_id == "t").unwrap();
        assert_eq!(node.style.placeholder_color.as_deref(), Some("#ff0000"), "{rule}");
    }
}

/** Without a rule the native default stays. */
#[test]
fn an_unstyled_placeholder_keeps_the_default() {
    let document = styled("<input id=t placeholder=Search>", "input{color:#000000}");
    let node = document.nodes.iter().find(|n| n.html_id == "t").unwrap();
    assert_eq!(node.style.placeholder_color, None);
}

/** A list item's marker run takes its `::marker` color; the item's text keeps its own. */
#[test]
fn marker_color_paints_the_bullet_only() {
    let document = styled("<ul><li id=t>Item</li></ul>", "li::marker{color:#00ff00}li{color:#000000}");
    let display = render(&document, 800.0).unwrap();
    let lines: Vec<_> = display.items.iter().flat_map(|item| item.lines.iter()).collect();
    let bullet = lines.iter().find(|line| line.text == "\u{2022}").expect("bullet run");
    assert_eq!(bullet.color.as_deref(), Some("#00ff00"));
    let text = lines.iter().find(|line| line.text.contains("Item")).expect("item text");
    assert_eq!(text.color, None);
}
