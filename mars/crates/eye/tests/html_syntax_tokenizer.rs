/** Tokenizer and in-body rules the html/syntax suite pins, checked through the fragment parser
scripts reach with innerHTML: the bogus comment and comment states, U+0000 handling, input
stream newline normalization and the dropped newline after `<pre>`. */
use mars_eye::parse_fragment;

/** Each fragment node as `tag|text|value`, in preorder. */
fn nodes(html: &str) -> Vec<String> {
    parse_fragment("div", html, 10)
        .unwrap()
        .nodes
        .into_iter()
        .map(|node| format!("{}|{}|{}", node.tag, node.text, node.value))
        .collect()
}

/** The first node's attribute `name`. */
fn attribute(html: &str, name: &str) -> String {
    parse_fragment("div", html, 10).unwrap().nodes[0].attributes[name].clone()
}

/** `<!x>`, `<?x>` and `</#x>` are bogus comments holding everything to the next `>`, and a
truncated declaration at the end of input is still one. */
#[test]
fn bogus_comments_become_comments() {
    assert_eq!(
        nodes("Abc<!d>Hi"),
        ["#text|Abc|", "#comment||d", "#text|Hi|"]
    );
    assert_eq!(nodes("Abc<!d"), ["#text|Abc|", "#comment||d"]);
    assert_eq!(nodes("<?COMMENT?>"), ["#comment||?COMMENT?"]);
    assert_eq!(nodes("</ COMMENT >"), ["#comment|| COMMENT "]);
    assert_eq!(nodes("<![CDATA[x]]>"), ["#comment||[CDATA[x]]"]);
    assert_eq!(nodes("<!-\0>"), ["#comment||-\u{fffd}"]);
}

/** `</>` is dropped and `</` at the end of input is text. */
#[test]
fn stray_end_tag_openers() {
    assert_eq!(nodes("a</>b"), ["#text|ab|"]);
    assert_eq!(nodes("a</"), ["#text|a</|"]);
}

/** The comment end states keep stray dashes and `--!` as data and end at `-->` or `--!>`. */
#[test]
fn comment_states_keep_stray_dashes() {
    assert_eq!(nodes("<!----->x"), ["#comment||-", "#text|x|"]);
    assert_eq!(nodes("<!-- a -- b -->"), ["#comment|| a -- b "]);
    assert_eq!(nodes("<!--a--!-->"), ["#comment||a--!"]);
    assert_eq!(nodes("<!--a--!>x"), ["#comment||a", "#text|x|"]);
    assert_eq!(nodes("<!-->x"), ["#comment||", "#text|x|"]);
    assert_eq!(nodes("<!--->x"), ["#comment||", "#text|x|"]);
    assert_eq!(nodes("<!--a-->"), ["#comment||a"]);
    assert_eq!(nodes("<!--a\0"), ["#comment||a\u{fffd}"]);
}

/** U+0000 vanishes from body text after character references resolve, and becomes U+FFFD in
attribute values before they resolve. */
#[test]
fn null_characters() {
    assert_eq!(nodes("a\0b"), ["#text|ab|"]);
    assert_eq!(nodes("&\0auml;"), ["#text|&auml;|"]);
    assert_eq!(nodes("&auml\0;"), ["#text|\u{e4};|"]);
    assert_eq!(
        attribute("<span title='&\0auml;'>", "title"),
        "&\u{fffd}auml;"
    );
    assert_eq!(
        attribute("<span title='&auml\0;'>", "title"),
        "\u{e4}\u{fffd};"
    );
}

/** CRLF and lone CR become LF in text and attributes, while a `&#x0D;` reference stays CR. */
#[test]
fn newlines_are_normalized() {
    assert_eq!(nodes("a\rb\r\nc\nd"), ["#text|a\nb\nc\nd|"]);
    assert_eq!(attribute("<input value='a\rb\nc'>", "value"), "a\nb\nc");
    assert_eq!(nodes("a&#x0D;b"), ["#text|a\rb|"]);
}

/** A newline right after a `<pre>` or `<listing>` start tag is not content; a second one is. */
#[test]
fn newline_after_pre_is_dropped() {
    assert_eq!(nodes("<pre>\nx</pre>"), ["pre|x|"]);
    assert_eq!(nodes("<listing>\n\nx</listing>"), ["listing|\nx|"]);
}
