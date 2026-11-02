/** Range receivers and offsets are validated: splitText past the end is an IndexSizeError, and a
StaticRange refuses Range methods while keeping its own accessors. */
use crate::common;

#[test]
fn split_text_past_the_end_is_an_index_size_error() {
    assert_eq!(
        common::eval(
            "let r = 'no'; try { const t = document.createTextNode('abc'); t.splitText(9); } catch (e) { r = e.name; }",
            "r"
        ),
        "IndexSizeError"
    );
}

#[test]
fn static_range_refuses_range_methods_but_keeps_its_accessors() {
    assert_eq!(
        common::eval(
            "const s = new StaticRange({startContainer: document, startOffset: 0, endContainer: document, endOffset: 1}); \
             let r = [s.startOffset, s.endOffset]; \
             try { s.setStart(document, 0); r.push('no-throw'); } catch (e) { r.push(e.name); } \
             r = r.join('|');",
            "r"
        ),
        "0|1|TypeError"
    );
}
