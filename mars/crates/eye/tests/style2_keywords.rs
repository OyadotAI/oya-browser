/** CSS-wide keywords (`inherit`, `initial`, `unset`, `revert`) resolve for every computed property. */
use mars_eye::{Document, Node, compute_styles, parse};

/** Styles `markup` with one sheet. */
fn styled(markup: &str, sheet: &str) -> Document {
    compute_styles(&parse(markup).unwrap(), &[sheet.into()]).unwrap()
}
/** The node with `id`. */
fn by_id<'a>(document: &'a Document, id: &str) -> &'a Node {
    document.nodes.iter().find(|n| n.html_id == id).unwrap()
}

#[test]
fn border_radius_inherit_copies_the_parents_corners() {
    // Google's search pill: `.SDkEP{border-radius:inherit}` inside a 26px pill.
    let document = styled(
        "<div id=p><div id=c>x</div></div>",
        "#p{border-radius:26px 26px 0 8px}#c{border-radius:inherit}",
    );
    assert_eq!(by_id(&document, "c").style.radius, [26.0, 26.0, 0.0, 8.0]);
}

#[test]
fn a_single_corner_keyword_copies_only_that_corner() {
    let document = styled(
        "<div id=p><div id=c>x</div></div>",
        "#p{border-radius:10px}#c{border-radius:4px;border-top-left-radius:inherit}",
    );
    assert_eq!(by_id(&document, "c").style.radius, [10.0, 4.0, 4.0, 4.0]);
}

#[test]
fn initial_and_unset_reset_non_inherited_properties() {
    let document = styled(
        "<div id=p><div id=a>a</div><div id=b>b</div></div>",
        "div{border-radius:9px;padding:7px;opacity:.5;z-index:3;aspect-ratio:2}\
         #a{border-radius:initial;padding:initial;opacity:initial;z-index:initial;aspect-ratio:initial}\
         #b{border-radius:unset;padding:unset;opacity:unset;z-index:unset;aspect-ratio:unset}",
    );
    for id in ["a", "b"] {
        let style = &by_id(&document, id).style;
        assert_eq!(style.radius, [0.0; 4], "{id}");
        assert_eq!(style.padding, [0.0; 4], "{id}");
        assert_eq!(style.opacity, 1.0, "{id}");
        assert_eq!(style.z_index, None, "{id}");
        assert_eq!(style.aspect_ratio, None, "{id}");
    }
}

#[test]
fn inherit_works_for_box_flex_grid_and_paint_properties() {
    let document = styled(
        "<div id=p><div id=c>x</div></div>",
        "#p{display:flex;flex-direction:column;justify-content:center;align-items:end;gap:5px;\
         box-sizing:border-box;flex-grow:2;order:4;overflow:hidden;box-shadow:0 1px 2px #000;\
         background-color:#123456;grid-template-columns:1fr 2fr;container-type:inline-size;\
         container-name:card;clip-path:inset(1px);padding:3%;align-self:center}\
         #c{display:inherit;flex-direction:inherit;justify-content:inherit;align-items:inherit;\
         row-gap:inherit;column-gap:inherit;box-sizing:inherit;flex-grow:inherit;order:inherit;\
         overflow:inherit;box-shadow:inherit;background-color:inherit;\
         grid-template-columns:inherit;container:inherit;clip-path:inherit;padding:inherit;\
         align-self:inherit}",
    );
    let (p, c) = (&by_id(&document, "p").style, &by_id(&document, "c").style);
    assert!(c.display_flex);
    assert_eq!(c.flex_direction, p.flex_direction);
    assert_eq!(c.justify_content, p.justify_content);
    assert_eq!(c.align_items, p.align_items);
    assert_eq!((c.row_gap, c.column_gap), (5.0, 5.0));
    assert_eq!(c.box_sizing, p.box_sizing);
    assert_eq!((c.flex_grow, c.order), (2.0, 4));
    assert_eq!(c.overflow, p.overflow);
    assert_eq!(c.shadows, p.shadows);
    assert_eq!(c.background, p.background);
    assert_eq!(c.grid_columns, p.grid_columns);
    assert_eq!(c.container_type, p.container_type);
    assert_eq!(c.container_names, vec!["card".to_string()]);
    assert_eq!(c.clip_path, p.clip_path);
    assert_eq!(c.padding_percent, [3.0; 4]);
    assert_eq!(c.align_self, p.align_self);
}

#[test]
fn border_keywords_survive_border_resolution() {
    let document = styled(
        "<div id=p><div id=c>x</div></div>",
        "#p{border:4px solid #ff0000}#c{border-width:inherit;border-style:inherit;border-color:inherit}",
    );
    let c = &by_id(&document, "c").style;
    assert_eq!(c.border_widths, [4.0; 4]);
    assert_eq!(c.border, 4.0);
    assert_eq!(c.border_color.as_deref(), Some("#ff0000"));
}

#[test]
fn unset_inherits_inherited_properties_and_revert_acts_like_unset() {
    let document = styled(
        "<div id=p><span id=a>a</span><span id=b>b</span></div>",
        "#p{color:#00ff00;letter-spacing:2px;border-radius:5px}span{color:#0000ff;letter-spacing:0;border-radius:1px}\
         #a{color:unset;letter-spacing:unset;border-radius:unset}#b{color:revert;border-radius:revert}",
    );
    let (a, b) = (&by_id(&document, "a").style, &by_id(&document, "b").style);
    assert_eq!(a.color.as_deref(), Some("#00ff00"));
    assert_eq!(a.letter_spacing, 2.0);
    assert_eq!(a.radius, [0.0; 4]);
    assert_eq!(b.color.as_deref(), Some("#00ff00"));
    assert_eq!(b.radius, [0.0; 4]);
}

#[test]
fn keywords_never_make_a_hidden_subtree_visible_or_a_masked_box_actionable() {
    let document = styled(
        "<div id=p><a id=a href=/x>a</a></div>",
        "#p{visibility:hidden}#a{visibility:initial;pointer-events:initial}",
    );
    let a = &by_id(&document, "a").style;
    assert!(a.hidden && !a.interactive);
}

#[test]
fn inherit_matches_declaring_the_parents_value_for_every_supported_property() {
    let cases = [
        ("width", "40px"),
        ("max-width", "50%"),
        ("min-width", "10px"),
        ("height", "30px"),
        ("min-height", "5px"),
        ("max-height", "90px"),
        ("margin", "1px 2px 3% auto"),
        ("padding", "1px 2% 3px 4px"),
        ("top", "3px"),
        ("left", "4px"),
        ("position", "relative"),
        ("z-index", "2"),
        ("display", "inline-block"),
        ("float", "left"),
        ("clear", "both"),
        ("vertical-align", "middle"),
        ("opacity", "0.5"),
        ("overflow", "hidden"),
        ("transform", "translateX(3px)"),
        ("box-shadow", "1px 1px 2px #000000"),
        ("background-color", "#abcdef"),
        ("background-image", "linear-gradient(#000000,#ffffff)"),
        ("background-size", "cover"),
        ("background-position", "left top"),
        ("background-repeat", "no-repeat"),
        ("border-radius", "3px 4px 5px 6px"),
        ("border", "2px dashed #00ff00"),
        ("border-left", "5px solid #0000ff"),
        ("flex", "2 3 40px"),
        ("flex-flow", "column wrap"),
        ("justify-content", "space-between"),
        ("align-items", "center"),
        ("align-self", "end"),
        ("justify-items", "center"),
        ("justify-self", "end"),
        ("order", "-2"),
        ("gap", "4px 6px"),
        ("grid-template-columns", "1fr 2fr"),
        ("grid-template-rows", "10px auto"),
        ("grid-column", "2 / span 2"),
        ("grid-row", "1"),
        ("grid-template-areas", "\"a b\""),
        ("box-sizing", "border-box"),
        ("aspect-ratio", "16 / 9"),
        ("clip", "rect(1px, 2px, 3px, 4px)"),
        ("clip-path", "inset(1px 2px)"),
        ("container", "card / inline-size"),
        ("mask-image", "url(m.svg)"),
        ("list-style-type", "square"),
        ("text-decoration", "none"),
        ("border-collapse", "collapse"),
        ("border-spacing", "3px"),
        ("pointer-events", "none"),
    ];
    for (property, value) in cases {
        let markup = "<div id=p><div id=c>x</div></div>";
        let inherited = styled(
            markup,
            &format!("#p{{{property}:{value}}}#c{{{property}:inherit}}"),
        );
        let declared = styled(
            markup,
            &format!("#p{{{property}:{value}}}#c{{{property}:{value}}}"),
        );
        assert_eq!(
            by_id(&inherited, "c").style,
            by_id(&declared, "c").style,
            "{property}: inherit"
        );
    }
}
