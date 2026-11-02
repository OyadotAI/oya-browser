/** Canonical methods retain conversion order and receiver rules through ordinary call adaptation. */
use super::*;
/** Generic supported primitive receivers convert while nullish receivers fail before other conversions. */
#[test]
fn generic_string_methods_convert_supported_primitive_receivers() {
    assert_eq!(
        result("const trim=''.trim;", "trim.call(123)+' '+trim.call(true)"),
        "123 true"
    );
    assert_eq!(
        result(
            "const split=''.split;const values=split.call(123,2);",
            "values.length+' '+values[0]+' '+values[1]"
        ),
        "2 1 3"
    );
    for source in [
        "''.trim.call(null);",
        "''.trim.call(undefined);",
        "''.split.call(null,{},{});",
    ] {
        assert_eq!(
            execute(source).unwrap_err().code,
            "script_execution_error",
            "{source}"
        );
    }
}
/** split converts the limit before the separator, as the specification and V8 order them. */
#[test]
fn split_conversion_order_matches_pinned_v8() {
    assert_eq!(
        result(
            "let order='';const separator={toString(){order+='s';return 'b';}};const limit={valueOf(){order+='l';return 2;}};const parts='abc'.split(separator,limit);",
            "order+' '+parts.join('|')"
        ),
        "ls a|c"
    );
}
/** Separator primitives use JavaScript rendering and undefined retains the omitted-separator shortcut. */
#[test]
fn split_separator_primitives_use_javascript_string_conversion() {
    assert_eq!(
        result(
            "const values='anullb'.split(null);",
            "values[0]+' '+values[1]"
        ),
        "a b"
    );
    assert_eq!(
        result(
            "const values='atrueb'.split(true);",
            "values[0]+' '+values[1]"
        ),
        "a b"
    );
    assert_eq!(
        result("const values='a2b'.split(2);", "values[0]+' '+values[1]"),
        "a b"
    );
}
/** Canonical methods remain true callable values with fixed metadata and ordinary adapters. */
#[test]
fn extracted_and_bound_string_methods_keep_identity_and_metadata() {
    assert_eq!(
        result(
            "const trim=''.trim;const bound=trim.bind(' a ');",
            "bound()+' '+trim.name+' '+trim.length+' '+''.split.length"
        ),
        "a trim 0 2"
    );
    assert_eq!(
        result("", "'a'.trim==='b'.trim&&'a'.split==='b'.split"),
        "true"
    );
    assert_eq!(
        result(
            "const split=''.split;const values=split.apply('a,b',[',']);",
            "values[1]"
        ),
        "b"
    );
}
/** Objects and functions convert through ToString wherever a string method needs a string. */
#[test]
fn object_and_function_receivers_convert_through_to_string() {
    assert_eq!(
        result(
            "",
            "''.trim.call({})+'/'+'a b'.split({toString(){return ' ';}}).length+'/'+'abc'.split(',',{valueOf(){return 0;}}).length"
        ),
        "[object Object]/2/0"
    );
}
/** Host objects carry their browser toStringTag, so String(document) names its interface. */
#[test]
fn host_objects_report_their_interface_tag() {
    use mars::{
        document::{parse, script_document},
        v9::{StorageState, load_document_scripts},
    };
    let document = script_document(&parse("<p id=x>t</p>").unwrap(), "strings").unwrap();
    let loaded = load_document_scripts(
        &["document.getElementById('x').textContent=String(document)+' '+String(document.getElementById('x'))+' '+Object.prototype.toString.call(window);".into()],
        document,
        StorageState::default(),
    )
    .unwrap();
    assert_eq!(
        loaded.changes[0].text().unwrap(),
        "[object HTMLDocument] [object HTMLParagraphElement] [object Window]"
    );
}
