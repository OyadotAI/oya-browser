/** `vertical-align` on a box Blink's StyleAdjuster blockifies (absolutely positioned or floated)
never shifts the text inside it: such a box is not an inline box, so its runs sit on its own
line boxes' baselines (WPT css/CSS2/linebox/vertical-align-sub-001). */
use mars_eye::{compute_styles, parse, render};

/** Top of the first text run inside `#s` for a span styled `style`. */
fn run_top(style: &str) -> f64 {
    let markup = format!(
        "<body style='margin:0'><div style='font:16px sans-serif'>\
         <span id=s style='{style}'>q</span></div>"
    );
    let document = compute_styles(&parse(&markup).unwrap(), &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    let span = document.nodes.iter().find(|n| n.html_id == "s").unwrap();
    let item = display
        .items
        .iter()
        .find(|i| {
            !i.lines.is_empty()
                && (i.local_id == span.local_id
                    || document
                        .nodes
                        .iter()
                        .any(|n| n.local_id == i.local_id && n.parent == Some(span.local_id)))
        })
        .unwrap();
    item.lines.first().map_or(item.y, |line| line.y)
}

/** An absolutely positioned span ignores its own `vertical-align: sub`. */
#[test]
fn absolute_box_ignores_its_vertical_align() {
    let plain = run_top("position:absolute");
    let sub = run_top("position:absolute;vertical-align:sub");
    assert!((plain - sub).abs() < 0.01, "{plain} vs {sub}");
}

/** A floated span ignores its own `vertical-align: super`. */
#[test]
fn floated_box_ignores_its_vertical_align() {
    let plain = run_top("float:left");
    let raised = run_top("float:left;vertical-align:super");
    assert!((plain - raised).abs() < 0.01, "{plain} vs {raised}");
}

/** An in-flow inline span still drops for `sub`. */
#[test]
fn inline_box_still_shifts() {
    assert!(run_top("vertical-align:sub") > run_top("") + 1.0);
}
