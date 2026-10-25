/*!
 * BCP 47 / UTS 35 Unicode locale identifiers, as ECMA-402 reads them (V8's intl-objects.cc
 * delegates this to ICU's LocaleBuilder; here it is ported from the UTS 35 grammar directly):
 * the structural-validity check, canonical form (case, subtag order, extension order, the
 * `true` value dropped) and the small CLDR alias and likely-subtag tables the embedded locales
 * need. Every Intl constructor and `Intl.getCanonicalLocales` goes through `canonicalize`.
 */

/** A `-t-` extension: the source language and the fields. */
pub(in crate::v9) type Transform = (Option<String>, Vec<(String, String)>);

/** A parsed Unicode locale identifier, all subtags lowercase as stored. */
#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::v9) struct Tag {
    pub language: String,
    pub script: Option<String>,
    pub region: Option<String>,
    pub variants: Vec<String>,
    /** The `-u-` extension's attributes. */
    pub attributes: Vec<String>,
    /** The `-u-` extension's keywords, key then value ("" for a bare key). */
    pub keywords: Vec<(String, String)>,
    /** Whether a `-u-` extension was present at all. */
    pub unicode: bool,
    /** The `-t-` extension: its source language and fields, already lowercase text. */
    pub transform: Option<Transform>,
    /** Other extensions, by singleton, each its subtags joined. */
    pub others: Vec<(char, String)>,
    /** The `-x-` private use subtags joined. */
    pub private: Option<String>,
}

/** Regular grandfathered tags and their replacements (CLDR languageAlias). */
const GRANDFATHERED: &[(&str, &str)] = &[
    ("art-lojban", "jbo"),
    ("cel-gaulish", "xtg"),
    ("zh-guoyu", "zh"),
    ("zh-hakka", "hak"),
    ("zh-xiang", "hsn"),
    ("sgn-gr", "gss"),
    ("sgn-de", "gsg"),
];

/** Language aliases (CLDR supplementalMetadata), a replacement possibly carrying a script. */
const LANGUAGE_ALIASES: &[(&str, &str)] = &[
    ("cmn", "zh"),
    ("in", "id"),
    ("iw", "he"),
    ("ji", "yi"),
    ("jw", "jv"),
    ("mo", "ro"),
    ("sh", "sr-latn"),
    ("tl", "fil"),
    ("aar", "aa"),
    ("heb", "he"),
    ("ces", "cs"),
    ("deu", "de"),
    ("eng", "en"),
    ("fra", "fr"),
    ("spa", "es"),
    ("zho", "zh"),
    ("arb", "ar"),
    ("swh", "sw"),
    ("cnr", "sr-me"),
];

/** Region aliases (CLDR territoryAlias) whose replacement is a single region. */
const REGION_ALIASES: &[(&str, &str)] = &[
    ("dd", "de"),
    ("bu", "mm"),
    ("fx", "fr"),
    ("tp", "tl"),
    ("yd", "ye"),
    ("zr", "cd"),
    ("su", "ru"),
    ("uk", "gb"),
    ("554", "nz"),
    ("280", "de"),
    ("810", "ru"),
];

/** Unicode extension value aliases (CLDR bcp47 data), by key. */
const VALUE_ALIASES: &[(&str, &str, &str)] = &[
    ("ca", "ethiopic-amete-alem", "ethioaa"),
    ("ca", "islamicc", "islamic-civil"),
    ("ks", "primary", "level1"),
    ("ks", "tertiary", "level3"),
    ("ms", "imperial", "uksystem"),
    ("tz", "cnckg", "cnsha"),
    ("tz", "eire", "iedub"),
    ("tz", "est", "papty"),
    ("tz", "gmt0", "gmt"),
    ("tz", "uct", "utc"),
    ("tz", "zulu", "utc"),
    ("rg", "no23", "no50"),
    ("sd", "no23", "no50"),
    ("sd", "cn11", "cnbj"),
    ("rg", "cn11", "cnbj"),
    ("sd", "cz10a", "cz110"),
    ("rg", "cz10a", "cz110"),
];

/** Variant aliases (CLDR variantAlias): a variant replaced by another. */
const VARIANT_ALIASES: &[(&str, &str)] = &[("heploc", "alalc97"), ("polytoni", "polyton")];

/** Keys whose value `yes` means `true` (CLDR bcp47 boolean types). */
const BOOLEAN_KEYS: &[&str] = &["kb", "kc", "kh", "kk", "kn"];

/** Likely subtags (CLDR likelySubtags) for the embedded locales, key to full tag. */
const LIKELY: &[(&str, &str)] = &[
    ("und", "en-Latn-US"),
    ("jbo", "jbo-Latn-001"),
    ("en", "en-Latn-US"),
    ("en-Shaw", "en-Shaw-GB"),
    ("de", "de-Latn-DE"),
    ("fr", "fr-Latn-FR"),
    ("es", "es-Latn-ES"),
    ("it", "it-Latn-IT"),
    ("pt", "pt-Latn-BR"),
    ("nl", "nl-Latn-NL"),
    ("sv", "sv-Latn-SE"),
    ("da", "da-Latn-DK"),
    ("fi", "fi-Latn-FI"),
    ("nb", "nb-Latn-NO"),
    ("no", "no-Latn-NO"),
    ("pl", "pl-Latn-PL"),
    ("cs", "cs-Latn-CZ"),
    ("hu", "hu-Latn-HU"),
    ("ro", "ro-Latn-RO"),
    ("tr", "tr-Latn-TR"),
    ("id", "id-Latn-ID"),
    ("vi", "vi-Latn-VN"),
    ("ru", "ru-Cyrl-RU"),
    ("uk", "uk-Cyrl-UA"),
    ("bg", "bg-Cyrl-BG"),
    ("sr", "sr-Cyrl-RS"),
    ("el", "el-Grek-GR"),
    ("he", "he-Hebr-IL"),
    ("ar", "ar-Arab-EG"),
    ("fa", "fa-Arab-IR"),
    ("hi", "hi-Deva-IN"),
    ("th", "th-Thai-TH"),
    ("ja", "ja-Jpan-JP"),
    ("ko", "ko-Kore-KR"),
    ("zh", "zh-Hans-CN"),
    ("zh-TW", "zh-Hant-TW"),
    ("zh-HK", "zh-Hant-HK"),
    ("zh-MO", "zh-Hant-MO"),
    ("zh-Hant", "zh-Hant-TW"),
    ("und-Latn", "en-Latn-US"),
    ("und-Cyrl", "ru-Cyrl-RU"),
    ("und-Cyrl-RO", "bg-Cyrl-RO"),
    ("und-Thai", "th-Thai-TH"),
    ("und-Arab", "ar-Arab-EG"),
    ("und-Hans", "zh-Hans-CN"),
    ("und-Hant", "zh-Hant-TW"),
    ("und-Jpan", "ja-Jpan-JP"),
    ("und-Kore", "ko-Kore-KR"),
    ("und-419", "es-Latn-419"),
    ("und-150", "en-Latn-150"),
    ("und-AT", "de-Latn-AT"),
    ("und-AQ", "en-Latn-AQ"),
    ("und-RO", "ro-Latn-RO"),
    ("und-DE", "de-Latn-DE"),
    ("und-FR", "fr-Latn-FR"),
    ("und-JP", "ja-Jpan-JP"),
    ("und-CN", "zh-Hans-CN"),
    ("und-TW", "zh-Hant-TW"),
];

/** Whether every char is an ASCII letter / alphanumeric and the length is in range. */
fn alpha(s: &str, min: usize, max: usize) -> bool {
    (min..=max).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphabetic())
}
fn alnum(s: &str, min: usize, max: usize) -> bool {
    (min..=max).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphanumeric())
}

/** `unicode_language_subtag`: 2-3 or 5-8 letters. */
pub(in crate::v9) fn is_language(s: &str) -> bool {
    alpha(s, 2, 3) || alpha(s, 5, 8)
}
/** `unicode_script_subtag`: four letters. */
pub(in crate::v9) fn is_script(s: &str) -> bool {
    alpha(s, 4, 4)
}
/** `unicode_region_subtag`: two letters or three digits. */
pub(in crate::v9) fn is_region(s: &str) -> bool {
    alpha(s, 2, 2) || (s.len() == 3 && s.bytes().all(|b| b.is_ascii_digit()))
}
/** `unicode_variant_subtag`: 5-8 alphanumerics, or a digit and three alphanumerics. */
pub(in crate::v9) fn is_variant(s: &str) -> bool {
    alnum(s, 5, 8) || (s.len() == 4 && s.as_bytes()[0].is_ascii_digit() && alnum(s, 4, 4))
}
/** `type`: one or more 3-8 alphanumeric subtags joined by hyphens. */
pub(in crate::v9) fn is_type(s: &str) -> bool {
    !s.is_empty() && s.split('-').all(|p| alnum(p, 3, 8))
}

/** The `unicode_language_id` at the front of `parts`: language, script, region, variants. */
fn language_id(parts: &[String], at: &mut usize, tag: &mut Tag) -> Option<()> {
    let first = parts.get(*at)?;
    if !is_language(first) {
        return None;
    }
    tag.language = first.clone();
    *at += 1;
    if parts.get(*at).is_some_and(|p| is_script(p)) {
        tag.script = Some(parts[*at].clone());
        *at += 1;
    }
    if parts.get(*at).is_some_and(|p| is_region(p)) {
        tag.region = Some(parts[*at].clone());
        *at += 1;
    }
    while let Some(part) = parts.get(*at).filter(|p| is_variant(p)) {
        if tag.variants.contains(part) {
            return None;
        }
        tag.variants.push(part.clone());
        *at += 1;
    }
    Some(())
}

/** The subtags of one extension: everything up to the next singleton. */
fn extension_run(parts: &[String], at: &mut usize) -> Vec<String> {
    let start = *at;
    while parts.get(*at).is_some_and(|p| p.len() > 1) {
        *at += 1;
    }
    parts[start..*at].to_vec()
}

/** A `-u-` extension's attributes then keywords. */
fn unicode_ext(run: &[String], tag: &mut Tag) -> Option<()> {
    let mut i = 0;
    while i < run.len() && alnum(&run[i], 3, 8) {
        tag.attributes.push(run[i].clone());
        i += 1;
    }
    while i < run.len() {
        let key = &run[i];
        let b = key.as_bytes();
        if key.len() != 2 || !b[0].is_ascii_alphanumeric() || !b[1].is_ascii_alphabetic() {
            return None;
        }
        i += 1;
        let start = i;
        while i < run.len() && alnum(&run[i], 3, 8) {
            i += 1;
        }
        tag.keywords.push((key.clone(), run[start..i].join("-")));
    }
    (!run.is_empty()).then_some(())
}

/** A `-t-` extension: an optional source language then fields keyed alpha-digit. */
fn transform_ext(run: &[String], tag: &mut Tag) -> Option<()> {
    let mut i = 0;
    let mut tlang = None;
    if run.first().is_some_and(|p| is_language(p)) {
        let mut inner = Tag::default();
        language_id(run, &mut i, &mut inner)?;
        tlang = Some(run[..i].join("-"));
    }
    let mut fields = Vec::new();
    while i < run.len() {
        let key = run[i].as_bytes();
        if key.len() != 2 || !key[0].is_ascii_alphabetic() || !key[1].is_ascii_digit() {
            return None;
        }
        let start = i + 1;
        i += 1;
        while i < run.len() && alnum(&run[i], 3, 8) {
            i += 1;
        }
        if i == start {
            return None;
        }
        fields.push((run[start - 1].clone(), run[start..i].join("-")));
    }
    if tlang.is_none() && fields.is_empty() {
        return None;
    }
    tag.transform = Some((tlang, fields));
    Some(())
}

/** Parse a structurally valid Unicode locale identifier, or None (ECMA-402 IsStructurallyValidLanguageTag). */
pub(in crate::v9) fn parse(text: &str) -> Option<Tag> {
    if !text.is_ascii() || text.is_empty() {
        return None;
    }
    let parts: Vec<String> = text.split('-').map(str::to_ascii_lowercase).collect();
    let mut tag = Tag::default();
    let mut at = 0;
    language_id(&parts, &mut at, &mut tag)?;
    let mut seen = Vec::new();
    while at < parts.len() {
        let single = &parts[at];
        if single.len() != 1 || !single.as_bytes()[0].is_ascii_alphanumeric() {
            return None;
        }
        let c = single.as_bytes()[0] as char;
        at += 1;
        if c == 'x' {
            let rest = &parts[at..];
            if rest.is_empty() || !rest.iter().all(|p| alnum(p, 1, 8)) {
                return None;
            }
            tag.private = Some(rest.join("-"));
            break;
        }
        if seen.contains(&c) {
            return None;
        }
        seen.push(c);
        let run = extension_run(&parts, &mut at);
        match c {
            'u' => {
                tag.unicode = true;
                unicode_ext(&run, &mut tag)?;
            }
            't' => transform_ext(&run, &mut tag)?,
            _ if !run.is_empty() && run.iter().all(|p| alnum(p, 2, 8)) => {
                tag.others.push((c, run.join("-")));
            }
            _ => return None,
        }
    }
    Some(tag)
}

/** Replace aliased language, region and variant subtags (UTS 35 annex C, the simple cases). */
fn replace_aliases(tag: &mut Tag) {
    let base = std::iter::once(tag.language.as_str())
        .chain(tag.variants.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join("-");
    if let Some((_, to)) = GRANDFATHERED.iter().find(|(from, _)| *from == base) {
        tag.language = (*to).into();
        tag.variants.clear();
    }
    if tag.language == "sgn"
        && let Some(region) = &tag.region
        && let Some((_, to)) = GRANDFATHERED
            .iter()
            .find(|(from, _)| *from == format!("sgn-{region}"))
    {
        tag.language = (*to).into();
        tag.region = None;
    }
    if let Some((_, to)) = LANGUAGE_ALIASES.iter().find(|(f, _)| *f == tag.language) {
        let mut pieces = to.split('-');
        tag.language = pieces.next().unwrap_or_default().into();
        for piece in pieces {
            if is_script(piece) && tag.script.is_none() {
                tag.script = Some(piece.into());
            } else if is_region(piece) && tag.region.is_none() {
                tag.region = Some(piece.into());
            }
        }
    }
    if let Some(region) = &tag.region
        && let Some((_, to)) = REGION_ALIASES.iter().find(|(f, _)| f == region)
    {
        tag.region = Some((*to).into());
    }
    for variant in &mut tag.variants {
        if let Some((_, to)) = VARIANT_ALIASES.iter().find(|(f, _)| f == variant) {
            *variant = (*to).into();
        }
    }
    if tag.language == "hy" && tag.variants.iter().any(|v| v == "arevmda") {
        tag.language = "hyw".into();
        tag.variants.retain(|v| v != "arevmda");
    }
    if tag.language == "hy" {
        tag.variants.retain(|v| v != "arevela");
    }
    if tag.variants.iter().any(|v| v == "hepburn") && tag.variants.iter().any(|v| v == "alalc97") {
        tag.variants.retain(|v| v != "hepburn");
    }
}

/** Canonical keyword value: aliases resolved and `true` (or a boolean `yes`) dropped. */
fn canonical_value(key: &str, value: &str) -> String {
    if let Some((_, _, to)) = VALUE_ALIASES
        .iter()
        .find(|(k, from, _)| *k == key && *from == value)
    {
        return (*to).into();
    }
    if value == "true" || (value == "yes" && BOOLEAN_KEYS.contains(&key)) {
        return String::new();
    }
    value.into()
}

/** The tag in canonical form (UTS 35 3.2.1 plus the alias replacement ECMA-402 asks for). */
pub(in crate::v9) fn canonicalize_tag(mut tag: Tag) -> Tag {
    replace_aliases(&mut tag);
    tag.variants.sort();
    tag.variants.dedup();
    tag.attributes.sort();
    tag.attributes.dedup();
    let mut keywords: Vec<(String, String)> = Vec::new();
    for (key, value) in std::mem::take(&mut tag.keywords) {
        if !keywords.iter().any(|(k, _)| *k == key) {
            let value = canonical_value(&key, &value);
            keywords.push((key, value));
        }
    }
    keywords.sort_by(|a, b| a.0.cmp(&b.0));
    tag.keywords = keywords;
    if let Some((tlang, fields)) = &mut tag.transform {
        if let Some(lang) = tlang.as_mut()
            && let Some(parsed) = parse(lang)
        {
            let canon = canonicalize_tag(parsed);
            *lang = base_name(&canon).to_ascii_lowercase();
        }
        fields.sort_by(|a, b| a.0.cmp(&b.0));
        for (key, value) in fields.iter_mut() {
            if key == "m0" && value == "names" {
                *value = "prprname".into();
            }
        }
    }
    tag.others.sort();
    tag
}

/** `unicode_language_id` in canonical case. */
pub(in crate::v9) fn base_name(tag: &Tag) -> String {
    let mut out = tag.language.clone();
    if let Some(script) = &tag.script {
        out.push('-');
        let mut chars = script.chars();
        if let Some(first) = chars.next() {
            out.push(first.to_ascii_uppercase());
            out.push_str(chars.as_str());
        }
    }
    if let Some(region) = &tag.region {
        out.push('-');
        out.push_str(&region.to_ascii_uppercase());
    }
    for variant in &tag.variants {
        out.push('-');
        out.push_str(variant);
    }
    out
}

/** The `-u-` extension text without its singleton, or None. */
fn unicode_text(tag: &Tag) -> Option<String> {
    if tag.attributes.is_empty() && tag.keywords.is_empty() {
        return None;
    }
    let mut parts: Vec<String> = tag.attributes.clone();
    for (key, value) in &tag.keywords {
        parts.push(key.clone());
        if !value.is_empty() {
            parts.push(value.clone());
        }
    }
    Some(parts.join("-"))
}

/** The whole tag as text, extensions in singleton order and private use last. */
pub(in crate::v9) fn to_text(tag: &Tag) -> String {
    let mut extensions: Vec<(char, String)> = tag.others.clone();
    if let Some(text) = unicode_text(tag) {
        extensions.push(('u', text));
    }
    if let Some((tlang, fields)) = &tag.transform {
        let mut parts: Vec<String> = tlang.iter().cloned().collect();
        for (key, value) in fields {
            parts.push(format!("{key}-{value}"));
        }
        extensions.push(('t', parts.join("-")));
    }
    extensions.sort();
    let mut out = base_name(tag);
    for (single, text) in extensions {
        out.push_str(&format!("-{single}-{text}"));
    }
    if let Some(private) = &tag.private {
        out.push_str("-x-");
        out.push_str(private);
    }
    out
}

/** Canonicalize tag text, or None when it is not structurally valid. */
pub(in crate::v9) fn canonicalize(text: &str) -> Option<String> {
    parse(text).map(|tag| to_text(&canonicalize_tag(tag)))
}

/** The likely full `language-Script-REGION` for a tag's base (UTS 35 Add Likely Subtags). */
pub(in crate::v9) fn maximize(tag: &Tag) -> Tag {
    let canon = base_name(&Tag {
        variants: Vec::new(),
        ..tag.clone()
    });
    let pieces: Vec<&str> = canon.split('-').collect();
    let lang = pieces[0];
    let script = tag.script.as_ref().map(|_| pieces[1]);
    let region = tag.region.as_ref().map(|_| *pieces.last().unwrap_or(&""));
    let mut keys = Vec::new();
    let fallback = if lang == "und" { None } else { Some("und") };
    for l in std::iter::once(lang).chain(fallback) {
        if l != lang {
            if let Some(s) = script {
                keys.push(format!("{l}-{s}"));
            }
            continue;
        }
        if let (Some(s), Some(r)) = (script, region) {
            keys.push(format!("{l}-{s}-{r}"));
        }
        if let Some(r) = region {
            keys.push(format!("{l}-{r}"));
        }
        if let Some(s) = script {
            keys.push(format!("{l}-{s}"));
        }
        keys.push(l.to_string());
    }
    let found = keys
        .iter()
        .find_map(|key| LIKELY.iter().find(|(k, _)| k == key).map(|(_, v)| *v));
    let mut out = tag.clone();
    if let Some(full) = found {
        let mut parts = full.split('-');
        let (l, s, r) = (parts.next(), parts.next(), parts.next());
        if out.language == "und" {
            out.language = l.unwrap_or("und").into();
        }
        out.script = out.script.or(s.map(str::to_ascii_lowercase));
        out.region = out.region.or(r.map(str::to_ascii_lowercase));
    }
    out
}

/** The shortest tag that maximizes to the same thing (UTS 35 Remove Likely Subtags). */
pub(in crate::v9) fn minimize(tag: &Tag) -> Tag {
    let max = maximize(tag);
    let base = |script: bool, region: bool| Tag {
        language: max.language.clone(),
        script: if script { max.script.clone() } else { None },
        region: if region { max.region.clone() } else { None },
        ..Tag::default()
    };
    let same = |t: &Tag| {
        let m = maximize(t);
        (m.language == max.language) && m.script == max.script && m.region == max.region
    };
    let pick = [(false, false), (false, true), (true, false)]
        .into_iter()
        .map(|(s, r)| base(s, r))
        .find(same)
        .unwrap_or_else(|| base(true, true));
    Tag {
        language: pick.language,
        script: pick.script,
        region: pick.region,
        ..max
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_case_order_and_aliases() {
        let c = |s: &str| canonicalize(s).unwrap();
        assert_eq!(c("DE-de"), "de-DE");
        assert_eq!(c("CMN-hANS"), "zh-Hans");
        assert_eq!(
            c("cmn-hans-cn-u-ca-t-ca-x-t-u"),
            "zh-Hans-CN-t-ca-u-ca-x-t-u"
        );
        assert_eq!(c("sl-rozaj-biske-1994"), "sl-1994-biske-rozaj");
        assert_eq!(c("sgn-GR"), "gss");
        assert_eq!(c("art-lojban"), "jbo");
        assert_eq!(c("und-u-kb-yes"), "und-u-kb");
        assert_eq!(c("en-u-ca-gregory-ca-buddhist"), "en-u-ca-gregory");
    }

    #[test]
    fn structurally_invalid_tags_are_rejected() {
        for bad in [
            "",
            "i-klingon",
            "en-GB-oed",
            "x-private",
            "en-u",
            "de-a-b",
            "en-t",
            "sl-rozaj-rozaj",
            "en-u-ca-a-u-ca",
            "abcd",
            "en_US",
        ] {
            assert!(parse(bad).is_none(), "{bad} should be invalid");
        }
        assert!(parse("da-u-attr-co").is_some());
    }

    #[test]
    fn likely_subtags_round_trip() {
        let max = |s: &str| to_text(&maximize(&parse(s).unwrap()));
        let min = |s: &str| to_text(&minimize(&parse(s).unwrap()));
        assert_eq!(max("en"), "en-Latn-US");
        assert_eq!(max("und-Cyrl-RO"), "bg-Cyrl-RO");
        assert_eq!(max("en-Arab"), "en-Arab-US");
        assert_eq!(min("en-Latn-US"), "en");
        assert_eq!(min("en-Arab-US"), "en-Arab");
        assert_eq!(min("und-Latn-AQ"), "en-AQ");
    }
}
