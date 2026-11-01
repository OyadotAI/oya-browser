/** Raw-text and RCDATA elements follow the WHATWG tokenizer: script, style, xmp, iframe, noembed,
 * noframes and plaintext keep their content literal, while textarea and title decode references. */
use crate::common;
use common::eval;

/** An xmp's content is raw text: references stay literal and tags are text, not elements. */
#[test]
fn xmp_inner_html_is_literal_raw_text() {
    let source = "const d=document.createElement('div');d.innerHTML='<xmp>&amp;<b>x</b></xmp>';\
        const x=d.firstChild;const r=x.textContent+'|'+x.childNodes.length+'|'+x.children.length;";
    assert_eq!(eval(source, "r"), "&amp;<b>x</b>|1|0");
}

/** An iframe's content is raw text, so a reference inside it does not decode. */
#[test]
fn iframe_inner_html_keeps_references_literal() {
    let source = "const d=document.createElement('div');d.innerHTML='<iframe>&amp;<b>x</b></iframe>';\
        const x=d.firstChild;const r=x.textContent+'|'+x.children.length;";
    assert_eq!(eval(source, "r"), "&amp;<b>x</b>|0");
}

/** noembed and noframes are raw text too; their markup is not parsed into elements. */
#[test]
fn noembed_and_noframes_inner_html_are_raw_text() {
    let source = "const d=document.createElement('div');\
        d.innerHTML='<noembed>&lt;i&gt;</noembed><noframes>&eacute;<p>y</p></noframes>';\
        const r=d.children.length+'|'+d.firstChild.children.length+'|'+d.firstChild.textContent+'|'+d.lastChild.textContent;";
    assert_eq!(eval(source, "r"), "2|0|&lt;i&gt;|&eacute;<p>y</p>");
}

/** plaintext has no end tag: everything after it, markup included, is literal text. */
#[test]
fn plaintext_runs_to_the_end_as_literal_text() {
    let source = "const d=document.createElement('div');d.innerHTML='<plaintext>&amp;<b>x</b>';\
        const x=d.firstChild;const r=x.textContent+'|'+x.children.length;";
    assert_eq!(eval(source, "r"), "&amp;<b>x</b>|0");
}

/** title is RCDATA: its references decode, and its tags stay text. */
#[test]
fn title_inner_html_decodes_references_as_rcdata() {
    let source = "const d=document.createElement('div');d.innerHTML='<title>&amp;<b>x</b></title>';\
        const x=d.firstChild;const r=x.textContent+'|'+x.children.length;";
    assert_eq!(eval(source, "r"), "&<b>x</b>|0");
}
