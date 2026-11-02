/** Exact UTF-16 behavior is observable without routing lone surrogates through the UTF-8 DOM boundary. */
use super::*;
/** String length and canonical indexed reads count code units rather than Unicode characters. */
#[test]
fn length_and_indices_preserve_utf16_units() {
    let loaded=load("const text='A😀B';const length=text.length;const high=text[1];const low=text['2'];const missing=text[4];",&[]).unwrap();
    assert_eq!(units(&loaded.state, "high"), vec![0xd83d]);
    assert_eq!(units(&loaded.state, "low"), vec![0xde00]);
    assert_eq!(
        loaded.state.globals()["length"].value,
        Some(JsValue::Number(4.0))
    );
    assert_eq!(
        loaded.state.globals()["missing"].value,
        Some(JsValue::Undefined)
    );
    assert_eq!(result("", "'abc'['01']===undefined"), "true");
}
/** All specified whitespace trims at both edges while interior characters and non-whitespace remain intact. */
#[test]
fn trim_uses_exact_ecmascript_whitespace() {
    let spaces = "\t\n\u{b}\u{c}\r \u{a0}\u{1680}\u{2000}\u{2001}\u{2002}\u{2003}\u{2004}\u{2005}\u{2006}\u{2007}\u{2008}\u{2009}\u{200a}\u{2028}\u{2029}\u{202f}\u{205f}\u{3000}\u{feff}";
    let value = mars::v9::JsString::from_utf8(&format!("{spaces}a b{spaces}")).unwrap();
    assert_eq!(value.trimmed().unwrap().to_dom().unwrap(), "a b");
    let escaped = spaces
        .chars()
        .map(|character| format!("\\u{:04x}", u32::from(character)))
        .collect::<String>();
    assert_eq!(
        result("", &format!("'{escaped}a b{escaped}'.trim()")),
        "a b"
    );
    assert_eq!(result("", "'\\u0085a\\u0085'.trim()"), "\u{85}a\u{85}");
    assert_eq!(result("", "'\\u200ba\\u200b'.trim()"), "\u{200b}a\u{200b}");
    assert_eq!(result("", "'   '.trim()"), "");
}
/** Trimming does not decode, replace or discard surrogate units adjacent to whitespace. */
#[test]
fn trim_preserves_lone_surrogates_and_interior_whitespace() {
    let loaded = load("const value=' \\ud800 a \\udfff '.trim();", &[]).unwrap();
    assert_eq!(
        units(&loaded.state, "value"),
        vec![0xd800, 32, 97, 32, 0xdfff]
    );
}
/** Empty literal separators emit individual UTF-16 units, including surrogate halves. */
#[test]
fn empty_separator_splits_code_units_and_empty_input() {
    let loaded=load("const parts='😀'.split('');const high=parts[0];const low=parts[1];const count=parts.length;",&[]).unwrap();
    assert_eq!(units(&loaded.state, "high"), vec![0xd83d]);
    assert_eq!(units(&loaded.state, "low"), vec![0xde00]);
    assert_eq!(
        loaded.state.globals()["count"].value,
        Some(JsValue::Number(2.0))
    );
    assert_eq!(result("", "''.split('').length"), "0");
    assert_eq!(result("", "''.split(',').length"), "1");
}
/** Literal separators consume nonoverlapping matches and retain adjacent and trailing empty fields. */
#[test]
fn literal_split_preserves_empty_fields_and_nonoverlapping_matches() {
    assert_eq!(
        result(
            "const values=',a,,'.split(',');",
            "values.length+' '+values[0]+' '+values[1]+' '+values[2]+' '+values[3]"
        ),
        "4  a  "
    );
    assert_eq!(
        result(
            "const values='aaaaa'.split('aa');",
            "values.length+' '+values[0]+' '+values[1]+' '+values[2]"
        ),
        "3   a"
    );
    assert_eq!(result("", "'abc'.split('missing')[0]"), "abc");
    assert_eq!(result("", "'a,b'.split()[0]"), "a,b");
    assert_eq!(result("", "'a,b'.split(undefined)[0]"), "a,b");
}
/** Matching separators may themselves contain otherwise unrepresentable UTF-16 units. */
#[test]
fn literal_split_matches_lone_surrogate_separators() {
    let loaded=load("const values='a\\ud800b\\ud800'.split('\\ud800');const first=values[0];const second=values[1];const tail=values[2];",&[]).unwrap();
    assert_eq!(units(&loaded.state, "first"), vec![97]);
    assert_eq!(units(&loaded.state, "second"), vec![98]);
    assert!(units(&loaded.state, "tail").is_empty());
}
