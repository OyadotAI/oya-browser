/** One admission rule for parser metadata and untrusted document/environment restoration. */
use super::MetadataElement;
use crate::{
    constants::{METADATA_BYTES, METADATA_ELEMENTS, NODES},
    protocol::{EngineError, EngineResult},
};
use std::collections::BTreeMap;

/** Attribute names consume storage too, including empty-valued boolean attributes. */
pub fn metadata_attribute_bytes(attributes: &BTreeMap<String, String>) -> usize {
    attributes.iter().fold(0usize, |used, (name, value)| {
        used.saturating_add(name.len()).saturating_add(value.len())
    })
}

/** One complete record has a single byte cost in Eye and V9, with overflow failing admission. */
pub fn metadata_element_bytes(element: &MetadataElement) -> usize {
    metadata_attribute_bytes(&element.attributes)
        .saturating_add(element.tag.len())
        .saturating_add(element.text.len())
}

/** Validate imported records before bootstrap copies them into detached DOM state. */
pub fn validate_metadata<'a>(
    metadata: &[MetadataElement],
    script_attributes: impl IntoIterator<Item = &'a BTreeMap<String, String>>,
) -> EngineResult<()> {
    if metadata.len() > METADATA_ELEMENTS {
        return Err(EngineError::limit("metadata elements", METADATA_ELEMENTS));
    }
    let mut bytes = 0usize;
    for element in metadata {
        validate_element(element)?;
        bytes = admit(bytes, metadata_element_bytes(element))?;
    }
    for attributes in script_attributes {
        validate_attributes(attributes)?;
        bytes = admit(bytes, metadata_attribute_bytes(attributes))?;
    }
    Ok(())
}

/** Metadata can only describe parser-owned nonlayout tags and bounded source positions. */
fn validate_element(element: &MetadataElement) -> EngineResult<()> {
    if !["meta", "link", "style", "title", "base", "script"].contains(&element.tag.as_str())
        || element.parent.is_some_and(|parent| parent >= NODES)
        || element.next > NODES
    {
        return Err(EngineError::invalid("Invalid document metadata record."));
    }
    validate_attributes(&element.attributes)
}

/** Nonempty names ensure map punctuation is bounded by the charged string bytes. */
fn validate_attributes(attributes: &BTreeMap<String, String>) -> EngineResult<()> {
    if attributes.keys().any(String::is_empty) {
        return Err(EngineError::invalid("Invalid document metadata attribute."));
    }
    Ok(())
}

/** Restore paths refuse excess data rather than silently truncating an already-built document. */
fn admit(used: usize, bytes: usize) -> EngineResult<usize> {
    used.checked_add(bytes)
        .filter(|total| *total <= METADATA_BYTES)
        .ok_or_else(|| EngineError::limit("metadata bytes", METADATA_BYTES))
}
