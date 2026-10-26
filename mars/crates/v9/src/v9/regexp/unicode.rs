/** Unicode property escapes use complete ICU4X data with exact ECMAScript property and value spellings. */
use icu_properties::script::ScriptWithExtensions;
use icu_properties::{CodePointMapData, CodePointSetData, PropertyParser, props::*};

/** Enumerated properties require an explicit allowed property name; lone names allow categories or binary sets. */
pub(super) fn property_ranges(name: &str) -> Option<Vec<(u32, u32)>> {
    if let Some((property, value)) = name.split_once('=') {
        return match property {
            "General_Category" | "gc" => category(value),
            "Script" | "sc" => script(value, false),
            "Script_Extensions" | "scx" => script(value, true),
            _ => None,
        };
    }
    match name {
        "Any" => Some(vec![(0, 0x10ffff)]),
        "ASCII" => Some(vec![(0, 0x7f)]),
        "Assigned" => Some(
            CodePointMapData::<GeneralCategory>::new()
                .iter_ranges_for_value_complemented(GeneralCategory::Unassigned)
                .map(pair)
                .collect(),
        ),
        _ => category(name).or_else(|| binary(name)),
    }
}

/** General categories include the Unicode-defined aggregate aliases, never binary properties or script names. */
fn category(name: &str) -> Option<Vec<(u32, u32)>> {
    let group = match name {
        "L" | "Letter" => GeneralCategoryGroup::Letter,
        "LC" | "Cased_Letter" => GeneralCategoryGroup::CasedLetter,
        "M" | "Mark" | "Combining_Mark" => GeneralCategoryGroup::Mark,
        "N" | "Number" => GeneralCategoryGroup::Number,
        "P" | "Punctuation" | "punct" => GeneralCategoryGroup::Punctuation,
        "S" | "Symbol" => GeneralCategoryGroup::Symbol,
        "Z" | "Separator" => GeneralCategoryGroup::Separator,
        "C" | "Other" => GeneralCategoryGroup::Other,
        _ => PropertyParser::<GeneralCategory>::new()
            .get_strict(name)?
            .into(),
    };
    Some(
        CodePointMapData::<GeneralCategory>::new()
            .iter_ranges_for_group(group)
            .map(pair)
            .collect(),
    )
}

/** Script_Extensions uses actual multi-script membership instead of the single Script property. */
fn script(name: &str, extensions: bool) -> Option<Vec<(u32, u32)>> {
    let script = PropertyParser::<Script>::new().get_strict(name)?;
    Some(if extensions {
        ScriptWithExtensions::new()
            .get_script_extensions_ranges(script)
            .map(pair)
            .collect()
    } else {
        CodePointMapData::<Script>::new()
            .iter_ranges_for_value(script)
            .map(pair)
            .collect()
    })
}

/** Both long and abbreviated binary property names are exact, case-sensitive UCD aliases. */
fn binary(name: &str) -> Option<Vec<(u32, u32)>> {
    macro_rules! properties {($($property:ty),+$(,)?)=>{$(if name.as_bytes()==<$property as BinaryProperty>::NAME||name.as_bytes()==<$property as BinaryProperty>::SHORT_NAME{return Some(CodePointSetData::new::<$property>().iter_ranges().map(pair).collect());})+};}
    properties!(
        Alphabetic,
        AsciiHexDigit,
        BidiControl,
        BidiMirrored,
        CaseIgnorable,
        Cased,
        ChangesWhenCasefolded,
        ChangesWhenCasemapped,
        ChangesWhenLowercased,
        ChangesWhenNfkcCasefolded,
        ChangesWhenTitlecased,
        ChangesWhenUppercased,
        Dash,
        DefaultIgnorableCodePoint,
        Deprecated,
        Diacritic,
        Emoji,
        EmojiComponent,
        EmojiModifierBase,
        EmojiModifier,
        EmojiPresentation,
        ExtendedPictographic,
        Extender,
        GraphemeBase,
        GraphemeExtend,
        HexDigit,
        IdContinue,
        IdStart,
        Ideographic,
        IdsBinaryOperator,
        IdsTrinaryOperator,
        JoinControl,
        LogicalOrderException,
        Lowercase,
        Math,
        NoncharacterCodePoint,
        PatternSyntax,
        PatternWhiteSpace,
        QuotationMark,
        Radical,
        RegionalIndicator,
        SentenceTerminal,
        SoftDotted,
        TerminalPunctuation,
        UnifiedIdeograph,
        Uppercase,
        VariationSelector,
        WhiteSpace,
        XidContinue,
        XidStart
    );
    None
}

/** Existing matcher classes use inclusive ranges and do not need to expand code points individually. */
fn pair(range: std::ops::RangeInclusive<u32>) -> (u32, u32) {
    (*range.start(), *range.end())
}
