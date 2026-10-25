/** Foreign content rules the html/syntax suite pins (math-parse01, math-parse03): HTML start tags
break out of SVG and MathML unless an integration point is current, and a self-closing MathML
start tag closes its element at once. Checked through the fragment parser scripts reach. */
use mars_eye::parse_fragment;

/** Each fragment node as `tag<parent tag`, in preorder; the top level is `-`. Text inside an
element is the element's own text, so only top-level text is a node. */
fn tree(html: &str) -> Vec<String> {
    let nodes = parse_fragment("div", html, 10).unwrap().nodes;
    nodes
        .iter()
        .map(|node| {
            let parent = node
                .parent
                .and_then(|id| nodes.iter().find(|n| n.local_id == id))
                .map_or("-", |n| n.tag.as_str());
            format!("{}<{parent}", node.tag)
        })
        .collect()
}

/** `<math><mrow/><mi/></math>` holds two siblings, and `<math/>` holds nothing. */
#[test]
fn self_closing_mathml_closes_at_once() {
    assert_eq!(
        tree("<math><mrow/><mi/></math>"),
        ["math<-", "mrow<math", "mi<math"]
    );
    assert_eq!(tree("<math/>x"), ["math<-", "#text<-"]);
}

/** An HTML start tag inside MathML (not in a text integration point) ends the formula. */
#[test]
fn html_start_tag_breaks_out_of_mathml() {
    assert_eq!(
        tree("<math><mrow><p>x</p><mi>y</mi></mrow></math>"),
        ["math<-", "mrow<math", "p<-", "mi<-"]
    );
}

/** mi, mtext and an HTML annotation-xml are integration points: HTML stays inside them, and
self-closing there is ignored for HTML elements. */
#[test]
fn integration_points_keep_html_inside() {
    assert_eq!(
        tree("<math><mi><span>x</span></mi></math>"),
        ["math<-", "mi<math", "span<mi"]
    );
    assert_eq!(
        tree("<math><annotation-xml encoding=text/html><div></div></annotation-xml><mn/></math>"),
        [
            "math<-",
            "annotation-xml<math",
            "div<annotation-xml",
            "mn<math"
        ]
    );
    assert_eq!(
        tree("<math><annotation-xml><foo/><bar/></annotation-xml></math>"),
        [
            "math<-",
            "annotation-xml<math",
            "foo<annotation-xml",
            "bar<annotation-xml"
        ]
    );
}

/** An HTML start tag inside an open SVG drawing closes the drawing instead of being skipped
with everything after it; `font` breaks out only with color, face or size. */
#[test]
fn html_start_tag_breaks_out_of_svg() {
    assert_eq!(
        tree("<svg><rect></rect><p>x</p>"),
        ["svg<-", "rect<svg", "p<-"]
    );
    assert_eq!(tree("<svg><g><b>y</b>"), ["svg<-", "g<svg", "b<-"]);
    assert_eq!(tree("<svg><font>z</font></svg>"), ["svg<-"]);
}
