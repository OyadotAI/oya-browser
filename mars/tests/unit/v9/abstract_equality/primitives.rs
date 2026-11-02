/** Primitive equality cases distinguish language conversion from blanket numeric conversion. */
use super::*;

/** Every supported primitive type participates symmetrically, including the nonnumeric nullish pair. */
#[test]
fn primitive_pair_matrix_preserves_nullish_boolean_number_and_string_rules() {
    let values = [
        "undefined",
        "null",
        "false",
        "true",
        "0",
        "1",
        "''",
        "'1'",
        "'x'",
        "NaN",
    ];
    let rows = [
        "1100000000",
        "1100000000",
        "0010101000",
        "0001010100",
        "0010101000",
        "0001010100",
        "0010101000",
        "0001010100",
        "0000000010",
        "0000000000",
    ];
    for (left, row) in values.iter().zip(rows) {
        for (right, expected) in values.iter().zip(row.bytes()) {
            assert_pair(left, right, expected == b'1');
        }
    }
}

/** Loose inequality negates precisely one successful comparison, while strict operators retain their meanings. */
fn assert_pair(left: &str, right: &str, expected: bool) {
    assert_eq!(
        result("", &format!("({left})==({right})")),
        expected.to_string()
    );
    assert_eq!(
        result("", &format!("({left})!=({right})")),
        (!expected).to_string()
    );
}

/** Numeric strings use complete ECMAScript whitespace and numeric grammar, including invalid-to-NaN cases. */
#[test]
fn numeric_string_conversion_uses_existing_complete_string_numeric_literal_rules() {
    for (string, number, expected) in [
        ("'\\u0009\\u000a\\u00a0\\ufeff'", "0", true),
        ("'  -2.5e1  '", "-25", true),
        ("'0x10'", "16", true),
        ("'0o10'", "8", true),
        ("'0b10'", "2", true),
        ("'Infinity'", "Infinity", true),
        ("'-Infinity'", "-Infinity", true),
        ("'+Infinity'", "Infinity", true),
        ("'1x'", "1", false),
        ("'+0x10'", "16", false),
        ("'1_0'", "10", false),
        ("'NaN'", "NaN", false),
        ("'\\ud800'", "0", false),
    ] {
        assert_pair(string, number, expected);
        assert_pair(number, string, expected);
    }
}

/** Matching strings compare raw UTF-16 units without normalization or numeric reinterpretation. */
#[test]
fn strings_nan_signed_zero_and_strict_comparisons_retain_their_distinctions() {
    for (left, right, expected) in [
        ("'01'", "'1'", false),
        ("''", "'0'", false),
        ("'\\ud800'", "'\\ud800'", true),
        ("'\\ud800'", "'\\ud801'", false),
        ("'🦀'", "'\\ud83e\\udd80'", true),
        ("'é'", "'e\\u0301'", false),
        ("NaN", "NaN", false),
        ("0", "-0", true),
        ("'-0'", "0", true),
        ("Infinity", "Infinity", true),
    ] {
        assert_pair(left, right, expected);
    }
    assert_eq!(
        result("", "('2'==2)+' '+('2'===2)+' '+('2'!=2)+' '+('2'!==2)"),
        "true false false true"
    );
}

/** Equality keeps tier three, left associativity, relational binding and lazy logical operands. */
#[test]
fn precedence_left_association_and_short_circuiting_remain_ecmascript_ordered() {
    for (expression, expected) in [
        ("2=='2'==true", "true"),
        ("0==0=='0'", "false"),
        ("1<2=='1'", "true"),
        ("false||2=='2'&&'kept'", "kept"),
        ("true||({}==0)", "true"),
        ("false&&({}==0)", "false"),
        ("1+1=='2'", "true"),
        ("!false==1", "true"),
    ] {
        assert_eq!(result("", expression), expected, "{expression}");
    }
}

/** Each operand runs once in source order before a successful equality or inequality is returned. */
#[test]
fn side_effecting_operands_run_left_then_right_exactly_once() {
    let source = "var trail='';function left(){trail+='L';return '2';}function right(){trail+='R';return 2;}const equal=left()==right();const different=left()!=right();";
    assert_eq!(
        result(source, "equal+' '+different+' '+trail"),
        "true false LRLR"
    );
}
