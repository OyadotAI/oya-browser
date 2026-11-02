/** Text chains (render/text_chains.rs): runs of one inline formatting context that abut or are
separated by whole spaces join into one text the painters draw as one; runs of different
contexts, or justified apart, never join. */
use mars_eye::{DisplayList, compute_styles, parse, render, text_chains};

/** The display list of `markup` at 800px in a 16px serif face. */
fn layout(markup: &str) -> DisplayList {
    let sheet = "body{margin:0;font:16px serif}".to_string();
    let document = compute_styles(&parse(markup).unwrap(), &[sheet]).unwrap();
    render(&document, 800.0).unwrap()
}
/** The joined texts of `markup`'s chains. */
fn joined(markup: &str) -> Vec<String> {
    let display = layout(markup);
    let chains = text_chains(display.items.iter());
    chains.chains.into_iter().map(|chain| chain.text).collect()
}

/** Spans separated by a space join with that space. */
#[test]
fn spans_on_a_line_join() {
    assert_eq!(
        joined("<div><span>Filler Text</span> <span>Filler Text</span></div>"),
        vec!["Filler Text Filler Text".to_string()]
    );
}

/** Table cells side by side are separate formatting contexts and never join. */
#[test]
fn cells_do_not_join() {
    let markup = "<table cellspacing=0 cellpadding=0><tr><td>ab</td><td>cd</td></tr></table>";
    assert!(joined(markup).is_empty());
}

/** Justified words are spread apart, so they are not drawn as one text. */
#[test]
fn justified_words_do_not_join() {
    let markup = "<p style='text-align-last:justify'>Lorem ipsum</p>";
    assert!(joined(markup).is_empty());
}
