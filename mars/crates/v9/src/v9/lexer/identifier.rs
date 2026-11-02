/** Identifier grammar uses Unicode ID_Start and ID_Continue with ECMAScript's explicit additions. */
use icu_properties::{
    CodePointSetData,
    props::{IdContinue, IdStart},
};

/** Digits, marks and replacement characters cannot start a name, including inside a Unicode escape. */
pub(super) fn identifier_start(character: char) -> bool {
    if character.is_ascii() {
        character.is_ascii_alphabetic() || matches!(character, '$' | '_')
    } else {
        CodePointSetData::new::<IdStart>().contains(character)
    }
}

/** Joining controls are explicitly permitted after the first identifier character. */
pub(super) fn identifier_continue(character: char) -> bool {
    if character.is_ascii() {
        character.is_ascii_alphanumeric() || matches!(character, '$' | '_')
    } else {
        matches!(character, '\u{200c}' | '\u{200d}')
            || CodePointSetData::new::<IdContinue>().contains(character)
    }
}
