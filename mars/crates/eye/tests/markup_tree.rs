/** Tree construction recovers from everyday malformed markup the way the HTML standard does. */
use mars_eye::constants::{DOCUMENT_DEPTH, HTML_BYTES, STATIC_TEXT_NODE_BYTES};
use mars_eye::{Document, Node, compute_styles, parse, render};

/** The first node with this author id. */
fn by_id<'a>(document: &'a Document, id: &str) -> &'a Node {
    document
        .nodes
        .iter()
        .find(|node| node.html_id == id)
        .unwrap_or_else(|| panic!("no #{id}"))
}

/** The tag of a node's parent, or None at the top level. */
fn parent_tag<'a>(document: &'a Document, node: &Node) -> Option<&'a str> {
    node.parent.map(|id| document.nodes[id].tag.as_str())
}

/** The first node whose own text equals `text`. */
fn with_text<'a>(document: &'a Document, text: &str) -> &'a Node {
    document
        .nodes
        .iter()
        .find(|node| node.text == text)
        .unwrap_or_else(|| panic!("no text {text}"))
}

/** A block start tag closes an open paragraph instead of nesting inside it. */
#[test]
fn block_start_closes_an_open_paragraph() {
    let document = parse("<p id=p>one<div id=d>two</div><p id=q>three<ul id=u><li>x</ul>").unwrap();
    assert_eq!(by_id(&document, "d").parent, None);
    assert_eq!(by_id(&document, "q").parent, None);
    assert_eq!(by_id(&document, "u").parent, None);
}

/** A paragraph inside a button is not closed by a block outside the button scope. */
#[test]
fn paragraph_closing_stops_at_button_scope() {
    let document = parse("<p id=p><button id=b><div id=d>x</div></button></p>").unwrap();
    assert_eq!(parent_tag(&document, by_id(&document, "d")), Some("button"));
}

/** List items close the previous item at the same level, not an outer one. */
#[test]
fn list_items_imply_their_end_tags() {
    let document =
        parse("<ul id=u><li id=a>a<li id=b>b<ul id=inner><li id=c>c<li id=d>d</ul><li id=e>e</ul>")
            .unwrap();
    for id in ["a", "b", "e"] {
        assert_eq!(
            parent_tag(&document, by_id(&document, id)),
            Some("ul"),
            "{id}"
        );
        assert_eq!(
            by_id(&document, id).parent,
            Some(by_id(&document, "u").local_id)
        );
    }
    for id in ["c", "d"] {
        assert_eq!(
            by_id(&document, id).parent,
            Some(by_id(&document, "inner").local_id)
        );
    }
}

/** Definition terms and descriptions close one another. */
#[test]
fn dt_and_dd_imply_their_end_tags() {
    let document =
        parse("<dl id=l><dt id=a>Term<dd id=b>Def<dt id=c>Term 2<dd id=d>Def 2</dl>").unwrap();
    for id in ["a", "b", "c", "d"] {
        assert_eq!(
            by_id(&document, id).parent,
            Some(by_id(&document, "l").local_id),
            "{id}"
        );
    }
}

/** Options and option groups close the open option. */
#[test]
fn options_imply_their_end_tags() {
    let document = parse(
        "<select id=s><option id=a>A<option id=b>B<optgroup id=g><option id=c>C<optgroup id=h><option id=d>D</select>",
    )
    .unwrap();
    let select = by_id(&document, "s").local_id;
    for id in ["a", "b", "g", "h"] {
        assert_eq!(by_id(&document, id).parent, Some(select), "{id}");
    }
    assert_eq!(
        by_id(&document, "c").parent,
        Some(by_id(&document, "g").local_id)
    );
}

/** A heading start tag closes a directly open heading. */
#[test]
fn headings_do_not_nest() {
    let document = parse("<h1 id=a>Title<h2 id=b>Sub</h2>").unwrap();
    assert_eq!(by_id(&document, "b").parent, None);
}

/** Misnested formatting closes back to the matching element, keeping later text outside it. */
#[test]
fn misnested_formatting_closes_back_to_the_match() {
    let document = parse("<p id=p><b id=b>bold<i id=i>both</b>after</i></p>").unwrap();
    assert_eq!(
        by_id(&document, "i").parent,
        Some(by_id(&document, "b").local_id)
    );
    let after = with_text(&document, "after");
    assert_eq!(after.parent, Some(by_id(&document, "p").local_id));
}

/** A link start tag closes an open link, so links never nest. */
#[test]
fn links_do_not_nest() {
    let document = parse("<a id=a href=/one>one<a id=b href=/two>two</a>").unwrap();
    assert_eq!(by_id(&document, "b").parent, None);
}

/** Stray end tags are ignored and cannot close elements outside a table cell. */
#[test]
fn stray_end_tags_are_ignored() {
    let document = parse(
        "<div id=o></span></em><table><tr><td id=c></div>cell</td></tr></table>tail</div></div></p><p id=p>x</p>",
    )
    .unwrap();
    assert_eq!(by_id(&document, "c").text, "cell");
    assert_eq!(
        with_text(&document, "tail").parent,
        Some(by_id(&document, "o").local_id)
    );
    assert_eq!(by_id(&document, "p").parent, None);
}

/** An unmatched ordinary end tag stops at a special element instead of closing through it. */
#[test]
fn unmatched_end_tag_stops_at_special_elements() {
    let document = parse("<span id=s><div id=d>x</span>inside</div>after</span>").unwrap();
    assert_eq!(by_id(&document, "d").text, "xinside");
    assert_eq!(
        with_text(&document, "after").parent,
        Some(by_id(&document, "s").local_id)
    );
}

/** GitHub's custom elements are ordinary containers whose children render. */
#[test]
fn github_custom_elements_render_their_children() {
    let document = parse(include_str!(
        "../../../fixtures/compat/github-custom-elements.html"
    ))
    .unwrap();
    let time = document
        .nodes
        .iter()
        .find(|node| node.tag == "relative-time")
        .unwrap();
    assert_eq!(time.text, "Oct 20, 2026");
    assert_eq!(time.attributes["datetime"], "2026-10-20T12:00:00Z");
    assert!(
        document
            .nodes
            .iter()
            .any(|node| node.tag == "include-fragment")
    );
    assert!(
        document
            .nodes
            .iter()
            .any(|node| node.text == "Copy path" && node.tag == "button")
    );
    assert!(
        !document
            .nodes
            .iter()
            .any(|node| node.text.contains("Hidden template text"))
    );
}

/** Reddit's shreddit elements keep their slotted links and text. */
#[test]
fn reddit_shreddit_elements_keep_slotted_content() {
    let document = parse(include_str!(
        "../../../fixtures/compat/reddit-shreddit.html"
    ))
    .unwrap();
    let post = document
        .nodes
        .iter()
        .find(|node| node.tag == "shreddit-post")
        .unwrap();
    assert_eq!(post.attributes["post-title"], "Mars ships a browser");
    let link = document
        .nodes
        .iter()
        .find(|node| node.href.as_deref() == Some("/r/rust/comments/abc/mars/"))
        .unwrap();
    assert_eq!(link.attributes["slot"], "full-post-link");
    assert!(
        document
            .nodes
            .iter()
            .any(|node| node.text == "Written from scratch in Rust.")
    );
}

/** Custom element children reach the display list. */
#[test]
fn custom_element_text_is_painted() {
    let document = parse("<my-card><card-title>Hello <b>world</b></card-title></my-card>").unwrap();
    let display = render(&compute_styles(&document, &[]).unwrap(), 640.0).unwrap();
    assert!(
        display
            .items
            .iter()
            .any(|item| item.lines.iter().any(|line| line.text.contains("Hello")))
    );
}

/** Template content is inert: never a node, so never painted or seen by agents. */
#[test]
fn template_content_is_inert() {
    let document = parse(
        "<template id=t><p>secret</p><template><p>inner</p></template><script>x</script></template><p id=after>after</p>",
    )
    .unwrap();
    assert!(by_id(&document, "t").text.is_empty());
    assert!(
        !document
            .nodes
            .iter()
            .any(|node| ["secret", "inner"].contains(&node.text.as_str()))
    );
    assert_eq!(by_id(&document, "after").parent, None);
    assert!(document.scripts.is_empty());
    assert!(document.diagnostics.iter().any(|d| d.contains("Template")));
}

/** Scripting is on, so noscript content is not rendered, in the head or the body. */
#[test]
fn noscript_content_is_not_rendered() {
    let document = parse(
        "<head><noscript><link rel=stylesheet href=nojs.css></noscript></head><noscript><p>Enable JavaScript</p></noscript><p>ok</p>",
    )
    .unwrap();
    assert!(
        !document
            .nodes
            .iter()
            .any(|node| node.text.contains("Enable"))
    );
    assert!(document.styles.is_empty());
    assert!(document.nodes.iter().any(|node| node.text == "ok"));
}

/** Body content inside the head implicitly ends the head instead of refusing the page. */
#[test]
fn body_content_in_the_head_ends_the_head() {
    let document =
        parse("<html><head><title>T</title><div id=d>visible</div></head><body><p>b</p>").unwrap();
    assert_eq!(by_id(&document, "d").text, "visible");
    assert_eq!(document.title, "T");
}

/** An absolute base address resolves relative links, images and form actions. */
#[test]
fn base_href_resolves_relative_addresses() {
    let document = parse(
        "<head><base href='https://example.org/docs/'></head><a id=a href=page.html>x</a><a id=h href='#top'>t</a><img id=i src=pic.png><form id=f action=send></form>",
    )
    .unwrap();
    assert_eq!(
        by_id(&document, "a").href.as_deref(),
        Some("https://example.org/docs/page.html")
    );
    assert_eq!(
        by_id(&document, "h").href.as_deref(),
        Some("https://example.org/docs/#top")
    );
    assert_eq!(
        by_id(&document, "i").image_src.as_deref(),
        Some("https://example.org/docs/pic.png")
    );
    assert_eq!(
        by_id(&document, "f").attributes["action"],
        "https://example.org/docs/send"
    );
}

/** A relative base is ignored with a diagnostic, leaving addresses as written. */
#[test]
fn relative_base_href_is_ignored() {
    let document = parse("<base href='/sub/'><a id=a href=page>x</a>").unwrap();
    assert_eq!(by_id(&document, "a").href.as_deref(), Some("page"));
    assert!(!document.diagnostics.is_empty());
}

/** Text over the per-node bound is truncated with a diagnostic instead of refusing. */
#[test]
fn oversized_node_text_is_truncated() {
    let document = parse(&format!(
        "<p id=p>{}</p>",
        "a".repeat(STATIC_TEXT_NODE_BYTES + 100)
    ))
    .unwrap();
    assert_eq!(by_id(&document, "p").text.len(), STATIC_TEXT_NODE_BYTES);
    assert!(document.diagnostics.iter().any(|d| d.contains("truncated")));
}

/** Comments, doctypes, processing instructions and bogus comments never refuse. */
#[test]
fn comments_doctypes_and_bogus_markup_are_skipped() {
    let document = parse(
        "<!DOCTYPE html PUBLIC '-//W3C//DTD XHTML 1.0//EN'><?xml version='1.0'?><!-- a -- b --><p id=p>a<!-->b<!--->c<!-- x --!>d</p></ odd><![CDATA[bogus]]><p id=q>e",
    )
    .unwrap();
    // Comments inside an element are `#comment` children, so the text after each is a text node.
    let p = by_id(&document, "p").local_id;
    let text: String = document
        .nodes
        .iter()
        .filter(|n| n.local_id == p || n.parent == Some(p))
        .map(|n| n.text.as_str())
        .collect();
    assert_eq!(text, "abcd");
    assert_eq!(by_id(&document, "q").text, "e");
    assert!(
        parse("<p>x<!-- never closed <p>y")
            .unwrap()
            .nodes
            .iter()
            .all(|n| n.text != "y")
    );
}

/** A lone `<` and other text that cannot open markup stay literal text. */
#[test]
fn literal_less_than_signs_stay_text() {
    let document = parse("<p id=p>1 < 2 and 3 <= 4 <3 and a<</p>").unwrap();
    assert_eq!(by_id(&document, "p").text, "1 < 2 and 3 <= 4 <3 and a<");
}

/** Named, legacy, numeric and C1 character references decode as HTML defines them. */
#[test]
fn character_references_decode() {
    let document = parse(
        "<p id=p>caf&eacute; &mdash; &hellip; &copy 2026 &AMP; &#x1F600; &#128; &#0; &bogus; &amp</p>",
    )
    .unwrap();
    assert_eq!(
        by_id(&document, "p").text,
        "café — … © 2026 & 😀 € \u{fffd} &bogus; &"
    );
}

/** In attributes a legacy reference before `=` stays literal, so query strings survive. */
#[test]
fn attribute_references_keep_query_strings() {
    let document =
        parse("<a id=a href='?a=1&copy=2&amp;b=3&lt' title=\"&quot;q&quot;\">x</a>").unwrap();
    let link = by_id(&document, "a");
    assert_eq!(link.href.as_deref(), Some("?a=1&copy=2&b=3<"));
    assert_eq!(link.attributes["title"], "\"q\"");
}

/** Line breaks, word-break opportunities and rules are void, and `</br>` is a break. */
#[test]
fn void_breaks_and_rules_do_not_open() {
    let document = parse("<p id=p>a<br>b<wbr>c</br>d<hr id=r></p>").unwrap();
    let breaks = document
        .nodes
        .iter()
        .filter(|node| node.tag == "br")
        .count();
    assert_eq!(breaks, 2);
    assert_eq!(by_id(&document, "r").parent, None);
    assert!(document.nodes.iter().any(|node| node.text == "d"));
}

/** Self-closing syntax on an ordinary HTML element is ignored, as browsers ignore it. */
#[test]
fn self_closing_flag_is_ignored_on_html_elements() {
    let document = parse("<div id=d /><span id=s>inside</span>").unwrap();
    assert_eq!(
        by_id(&document, "s").parent,
        Some(by_id(&document, "d").local_id)
    );
}

/** Inline handlers stay on every element for V9 to run, and the body's are forwarded to the
window (`<body onload>`), as in HTML. */
#[test]
fn inline_event_handlers_are_kept() {
    let document = parse(
        "<body onload=boot()><div id=d onclick=steal() onmouseover=x()>d</div><img id=i src=a.png onerror=pwn()><button id=b onclick=go()>Go</button>",
    )
    .unwrap();
    let div = by_id(&document, "d");
    assert_eq!(div.attributes.get("onmouseover").map(String::as_str), Some("x()"));
    assert_eq!(div.handler.as_deref(), Some("steal()"));
    assert_eq!(by_id(&document, "i").attributes.get("onerror").map(String::as_str), Some("pwn()"));
    assert_eq!(
        document
            .formatting
            .body
            .attributes
            .get("onload")
            .map(String::as_str),
        Some("boot()")
    );
    assert_eq!(by_id(&document, "b").handler.as_deref(), Some("go()"));
    assert!(
        !document
            .diagnostics
            .iter()
            .any(|d| d.contains("event handler"))
    );
}

/** `javascript:` links and images are inert: no navigation target is ever produced. */
#[test]
fn javascript_urls_are_inert() {
    let document = parse(
        "<a id=a href='javascript:alert(1)'>a</a><a id=b href=' JaVa\tScRiPt:void(0)'>b</a><img id=i src='javascript:x'>",
    )
    .unwrap();
    assert!(by_id(&document, "a").href.is_none());
    assert!(by_id(&document, "b").href.is_none());
    assert!(by_id(&document, "i").image_src.is_none());
}

/** Unsupported SVG content is dropped while the supported drawing subset stays; `defs`,
`symbol` and paint servers (gradients, patterns and their stops) stay for references. */
#[test]
fn unsupported_svg_content_is_dropped() {
    let document = parse(
        "<svg width=16 height=16><title>Icon</title><pattern id=pt /><defs><linearGradient id=g><stop/></linearGradient></defs><path id=p d='M0 0'/></svg><p id=after>after</p>",
    )
    .unwrap();
    assert!(!document.nodes.iter().any(|node| node.tag == "title"));
    assert_eq!(parent_tag(&document, by_id(&document, "p")), Some("svg"));
    assert_eq!(by_id(&document, "after").parent, None);
}

/** Markup nested past the depth bound flattens into siblings and still renders. */
#[test]
fn deep_nesting_flattens_instead_of_refusing() {
    let html = format!(
        "{}deep{}",
        "<div>".repeat(DOCUMENT_DEPTH * 3),
        "</div>".repeat(DOCUMENT_DEPTH * 3)
    );
    // The document service runs on a SERVICE_STACK_BYTES thread; so does this test.
    std::thread::Builder::new()
        .stack_size(mars_eye::constants::SERVICE_STACK_BYTES)
        .spawn(move || {
            let document = parse(&html).unwrap();
            assert!(document.nodes.iter().any(|node| node.text == "deep"));
            render(&compute_styles(&document, &[]).unwrap(), 640.0).unwrap();
        })
        .unwrap()
        .join()
        .unwrap();
}

/** A linked sheet's media condition travels with it, so the cascade decides when it applies. */
#[test]
fn conditional_stylesheets_carry_their_media() {
    let document = parse(
        "<link rel=stylesheet href=print.css media=print><link rel=stylesheet href=a.css><p>x</p>",
    )
    .unwrap();
    assert!(matches!(
        &document.styles[..],
        [
            mars_eye::StyleSource::LinkedMedia { href, media },
            mars_eye::StyleSource::Linked(_),
        ] if href == "print.css" && media == "print"
    ));
}

/** An SVG `image` stays inside its drawing and never swallows the rest of the page. */
#[test]
fn svg_image_keeps_the_rest_of_the_page() {
    for html in [
        "<svg><image href=x></image></svg><input id=q>",
        "<svg><image href=x></svg><input id=q>",
        "<div><svg><foreignObject><p>a</svg></div><input id=q>",
    ] {
        let document = parse(html).unwrap();
        let input = by_id(&document, "q");
        assert_eq!(input.parent, None, "{html}");
    }
}

/** An inline sheet's media attribute is kept as an `@media` condition for the cascade to evaluate. */
#[test]
fn inline_style_media_becomes_a_media_rule() {
    let document =
        parse("<style media=print>p{color:red}</style><style media=all>a{}</style><p>x</p>")
            .unwrap();
    let sheets = document
        .styles
        .iter()
        .map(|source| match source {
            mars_eye::StyleSource::Inline(css) => css.as_str(),
            mars_eye::StyleSource::Linked(href)
            | mars_eye::StyleSource::LinkedMedia { href, .. } => href.as_str(),
        })
        .collect::<Vec<_>>();
    assert_eq!(sheets, ["@media print {\np{color:red}\n}", "a{}"]);
    let styled = compute_styles(&document, &[sheets[0].into()]).unwrap();
    assert!(styled.nodes[0].style.color.is_none());
}

/** The source byte bound still refuses, since it is a hard bound and not markup support. */
#[test]
fn html_byte_bound_still_refuses() {
    let error = parse(&"a".repeat(HTML_BYTES + 1)).unwrap_err();
    assert_eq!(error.code, "limit_exceeded");
}

/** Diagnostics stay deduplicated however often a page repeats the same recovery. */
#[test]
fn diagnostics_are_deduplicated() {
    let document = parse(&"<input type=date>".repeat(50)).unwrap();
    let handler = document
        .diagnostics
        .iter()
        .filter(|d| d.contains("plain text"))
        .count();
    assert_eq!(handler, 1);
}
