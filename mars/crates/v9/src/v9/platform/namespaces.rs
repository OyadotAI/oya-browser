/**
 * The DOM namespace model's pure rules: the namespace constants, the validate-and-extract steps
 * the DOM standard runs on every namespace-aware name, and the records elements and attributes
 * keep. Everything here is a function of strings, so each rule is checked without a page.
 */
use serde::{Deserialize, Serialize};

/** The HTML namespace: the namespace of every element a parser or createElement makes in HTML. */
pub(in crate::v9) const HTML_NAMESPACE: &str = "http://www.w3.org/1999/xhtml";
/** The SVG namespace, which an `<svg>` element and its parsed children belong to. */
pub(in crate::v9) const SVG_NAMESPACE: &str = "http://www.w3.org/2000/svg";
/** The MathML namespace, which a `<math>` element belongs to. */
pub(in crate::v9) const MATHML_NAMESPACE: &str = "http://www.w3.org/1998/Math/MathML";
/** The namespace the `xml` prefix is bound to, and only it. */
pub(in crate::v9) const XML_NAMESPACE: &str = "http://www.w3.org/XML/1998/namespace";
/** The namespace the `xmlns` prefix is bound to, and only it. */
pub(in crate::v9) const XMLNS_NAMESPACE: &str = "http://www.w3.org/2000/xmlns/";
/** The tag name that places an element in the SVG namespace. */
const SVG_TAG: &str = "svg";
/** The tag name that places an element in the MathML namespace. */
const MATHML_TAG: &str = "math";
/** The prefix that must, and only may, name the XML namespace. */
const XML_PREFIX: &str = "xml";
/** The prefix and name that must, and only may, name the XMLNS namespace. */
const XMLNS_PREFIX: &str = "xmlns";
/** The DOM error a name the grammar rejects raises. */
pub(in crate::v9) const INVALID_CHARACTER_ERROR: &str = "InvalidCharacterError";
/** The DOM error a prefix, namespace pairing the standard forbids raises. */
pub(in crate::v9) const NAMESPACE_ERROR: &str = "NamespaceError";
/** The ASCII characters a name may never contain: whitespace, NUL, the slash and the angle bracket. */
const FORBIDDEN_NAME_CHARS: [char; 3] = ['/', '>', '\0'];
/** The one extra character an attribute local name may not contain, as in `a=b`. */
const ATTRIBUTE_EQUALS: char = '=';

/** Which local-name grammar a name answers to: elements and attributes differ in the standard. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::v9) enum NameKind {
    /** An element's local name, the grammar createElement and createElementNS apply. */
    Element,
    /** An attribute's local name, which allows more than an element's. */
    Attribute,
}

/** A qualified name split at its first colon: an optional prefix and the local name. */
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(in crate::v9) struct QName {
    /** The text before the first colon, when there was one. */
    pub prefix: Option<String>,
    /** The text after the colon, or the whole name when there was no colon. */
    pub local: String,
}

/** The DOM error a rejected name raises, with the message the exception carries. */
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::v9) struct NameError {
    /** The DOM error name, `InvalidCharacterError` or `NamespaceError`. */
    pub name: &'static str,
    /** The message the exception reports, naming the rejected text. */
    pub message: String,
}

/** The namespace, prefix and local name an element carries when a script created it. */
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(in crate::v9) struct ElementNs {
    /** The element's namespace, or None for the null namespace. */
    pub namespace: Option<String>,
    /** The element's prefix, or None when it has none. */
    pub prefix: Option<String>,
    /** The element's local name, which `localName` reports. */
    pub local: String,
}

/** The namespace and local name of an attribute set through a namespace-aware method. */
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(in crate::v9) struct AttrNs {
    /** The attribute's namespace, which is never null for a record kept here. */
    pub namespace: String,
    /** The attribute's prefix, when it had one. */
    pub prefix: Option<String>,
    /** The attribute's local name, which namespace-aware reads match. */
    pub local: String,
}

/** The namespace an element a parser made belongs to: SVG and MathML by tag, HTML otherwise. */
pub(in crate::v9) fn implied_namespace(tag: &str) -> &'static str {
    match tag {
        SVG_TAG => SVG_NAMESPACE,
        MATHML_TAG => MATHML_NAMESPACE,
        _ => HTML_NAMESPACE,
    }
}

/** An argument that is empty means no value: the empty string is the null namespace or prefix. */
pub(in crate::v9) fn non_empty(text: &str) -> Option<String> {
    (!text.is_empty()).then(|| text.to_string())
}

/** Whether a character may not appear anywhere in a name. */
fn is_forbidden(character: char, kind: NameKind) -> bool {
    let attribute_only = kind == NameKind::Attribute && character == ATTRIBUTE_EQUALS;
    character.is_ascii_whitespace() || FORBIDDEN_NAME_CHARS.contains(&character) || attribute_only
        || character.is_control()
}

/** The element local-name grammar: an ASCII letter then anything allowed, or `:`, `_` or a
non-ASCII character then name characters only. */
fn element_local_is_valid(text: &str) -> bool {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if first.is_ascii_alphabetic() {
        return chars.all(|character| !is_forbidden(character, NameKind::Element));
    }
    let name_start = first == ':' || first == '_' || !first.is_ascii();
    name_start
        && chars.all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(character, '-' | '.' | ':' | '_')
                || !character.is_ascii()
        })
}

/** The attribute local-name grammar: any non-empty text without the forbidden characters. */
fn attribute_local_is_valid(text: &str) -> bool {
    !text.is_empty() && !text.chars().any(|character| is_forbidden(character, NameKind::Attribute))
}

/** Whether a local name (one with no prefix to split off) is valid for its kind. */
fn local_is_valid(text: &str, kind: NameKind) -> bool {
    match kind {
        NameKind::Element => element_local_is_valid(text),
        NameKind::Attribute => attribute_local_is_valid(text),
    }
}

/** An InvalidCharacterError naming the rejected text, as the DOM standard words it. */
fn invalid(qualified: &str) -> NameError {
    NameError {
        name: INVALID_CHARACTER_ERROR,
        message: format!("'{qualified}' is not a valid name."),
    }
}

/** The `Name` check createElement runs on a whole local name: no prefix is split off. */
pub(in crate::v9) fn validate_local_name(local: &str, kind: NameKind) -> Result<(), NameError> {
    if local_is_valid(local, kind) {
        Ok(())
    } else {
        Err(invalid(local))
    }
}

/** Splits a qualified name at its first colon and checks both halves against the grammar. */
fn split_qualified(qualified: &str, kind: NameKind) -> Result<QName, NameError> {
    match qualified.split_once(':') {
        None if local_is_valid(qualified, kind) => Ok(QName {
            prefix: None,
            local: qualified.to_string(),
        }),
        Some((prefix, local))
            if !prefix.is_empty()
                && !prefix.chars().any(|character| is_forbidden(character, kind))
                && local_is_valid(local, kind) =>
        {
            Ok(QName {
                prefix: Some(prefix.to_string()),
                local: local.to_string(),
            })
        }
        _ => Err(invalid(qualified)),
    }
}

/** The namespace pairings the standard forbids: a prefix needs a namespace, `xml` and `xmlns`
name their own namespaces only, and that namespace belongs to `xmlns` names alone. */
fn check_pairing(namespace: Option<&str>, qname: &QName, qualified: &str) -> Result<(), NameError> {
    let prefix = qname.prefix.as_deref();
    let xmlns_name = qualified == XMLNS_PREFIX || prefix == Some(XMLNS_PREFIX);
    let forbidden = (prefix.is_some() && namespace.is_none())
        || (prefix == Some(XML_PREFIX) && namespace != Some(XML_NAMESPACE))
        || (xmlns_name && namespace != Some(XMLNS_NAMESPACE))
        || (namespace == Some(XMLNS_NAMESPACE) && !xmlns_name);
    if forbidden {
        return Err(NameError {
            name: NAMESPACE_ERROR,
            message: format!("'{qualified}' does not match its namespace."),
        });
    }
    Ok(())
}

/** The DOM standard's validate and extract: check the qualified name, split it, then check the
pairing of its prefix with the namespace. Returns the split name, or the error the DOM raises. */
pub(in crate::v9) fn validate_and_extract(
    namespace: Option<&str>,
    qualified: &str,
    kind: NameKind,
) -> Result<QName, NameError> {
    let qname = split_qualified(qualified, kind)?;
    check_pairing(namespace, &qname, qualified)?;
    Ok(qname)
}
