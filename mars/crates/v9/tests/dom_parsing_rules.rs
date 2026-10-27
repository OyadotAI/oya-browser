/*! DOM parsing rules in V9: the main document's settled encoding, LegacyNullToEmptyString on the
innerHTML and outerHTML setters, outerHTML on a parentless element, the SyntaxError for a bad
insertAdjacentHTML position, the input-stream newline rule in markup, and the required argument
of createContextualFragment. */
mod common;
use common::{environment, failure, global, host, load, page};
use mars_v9::{Execution, ModuleInput, PageInput, StorageState};

/** Run a script against the test page whose main document settled `charset`. */
fn run_with_charset(charset: &str, source: &str) -> Execution {
    let host = host();
    let environment = environment();
    let mut document = page();
    document.charset = charset.into();
    mars_v9::load_page(PageInput {
        sources: &[source.to_string()],
        source_keys: &[],
        document,
        storage: StorageState::default(),
        host: &host,
        environment: &environment,
        modules: ModuleInput::NONE,
    })
    .expect("page loads")
}

/** Asserts a script ran cleanly and returns one of its globals. */
fn value_of(execution: &Execution, name: &str) -> String {
    assert!(execution.failed.is_empty(), "{:?}", failure(execution));
    global(execution, name)
}

/** The main document reports the encoding its parser settled, through all three accessors. */
#[test]
fn main_document_reports_the_settled_charset() {
    let source =
        "let a = document.characterSet; let b = document.charset; let c = document.inputEncoding;";
    let execution = run_with_charset("windows-1252", source);
    assert_eq!(value_of(&execution, "a"), "windows-1252");
    assert_eq!(value_of(&execution, "b"), "windows-1252");
    assert_eq!(value_of(&execution, "c"), "windows-1252");
}

/** A page whose parse settled no encoding reports UTF-8, the default decoding. */
#[test]
fn main_document_without_a_settled_charset_reports_utf8() {
    let execution = run_with_charset("", "let a = document.characterSet;");
    assert_eq!(value_of(&execution, "a"), "UTF-8");
}

/** innerHTML maps null to the empty string, but stringifies undefined as the text "undefined". */
#[test]
fn inner_html_null_is_empty_and_undefined_is_text() {
    let source = "let holder = document.createElement('div'); \
        holder.innerHTML = null; let afterNull = holder.innerHTML; \
        holder.innerHTML = undefined; let afterUndefined = holder.innerHTML;";
    let execution = load(source);
    assert_eq!(value_of(&execution, "afterNull"), "");
    assert_eq!(value_of(&execution, "afterUndefined"), "undefined");
}

/** outerHTML maps null to the empty string, so assigning null removes the element it replaces. */
#[test]
fn outer_html_null_replaces_the_element_with_nothing() {
    let source = "let wrap = document.createElement('div'); \
        let child = document.createElement('span'); wrap.appendChild(child); \
        child.outerHTML = null; let left = wrap.innerHTML;";
    let execution = load(source);
    assert_eq!(value_of(&execution, "left"), "");
}

/** Setting outerHTML on an element with no parent has nothing to replace, so it changes nothing. */
#[test]
fn outer_html_on_a_parentless_element_is_a_no_op() {
    let source = "let lone = document.createElement('p'); lone.outerHTML = '<b>x</b>'; \
        let kept = lone.outerHTML;";
    let execution = load(source);
    assert_eq!(value_of(&execution, "kept"), "<p></p>");
}

/** An unknown insertAdjacentHTML position throws a DOMException named SyntaxError. */
#[test]
fn insert_adjacent_html_bad_position_is_a_syntax_error() {
    let source = "let name = ''; try { document.body.insertAdjacentHTML('sideways', '<b>x</b>'); } \
        catch (error) { name = error.name; }";
    let execution = load(source);
    assert_eq!(value_of(&execution, "name"), "SyntaxError");
}

/** A carriage return in markup reads as LF in attribute values, per the input-stream newline rule. */
#[test]
fn carriage_return_in_markup_reads_as_newline() {
    let source = r#"let holder = document.createElement('div');
        holder.innerHTML = '<i title="a\rb"></i>';
        let title = holder.firstChild.getAttribute('title');"#;
    let execution = load(source);
    assert_eq!(value_of(&execution, "title"), "a\nb");
}

/** The document element's parent is the document, which outerHTML cannot replace, so it throws NoModificationAllowedError. */
#[test]
fn outer_html_on_the_document_element_throws() {
    let source = "let name = ''; try { document.documentElement.outerHTML = '<html></html>'; } \
        catch (error) { name = error.name; }";
    let execution = load(source);
    assert_eq!(value_of(&execution, "name"), "NoModificationAllowedError");
}

/** createContextualFragment requires its markup argument, so calling it bare throws a TypeError. */
#[test]
fn create_contextual_fragment_requires_its_argument() {
    let source = "let name = ''; try { document.createRange().createContextualFragment(); } \
        catch (error) { name = error.name; }";
    let execution = load(source);
    assert_eq!(value_of(&execution, "name"), "TypeError");
}
