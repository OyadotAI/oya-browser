/** Item tables of `meta`: the primary item (pitm), the item types (iinf/infe) and the typed references between items (iref). */
use crate::boxes::{children, find, full_box};
use crate::constants::{FourCc, IINF, INFE, IREF, MAX_ITEMS, MAX_REFERENCES, PITM};
use crate::error::{Error, Result};
use crate::reader::Reader;

/** One item the file declares: its id and its four-character type (zero for pre-v2 entries, which carry none). */
#[derive(Clone, Copy)]
pub(crate) struct ItemInfo {
    pub(crate) id: u32,
    pub(crate) kind: FourCc,
}

/** A typed reference from one item to others, such as `auxl` from an alpha plane to its picture. */
pub(crate) struct Reference {
    pub(crate) kind: FourCc,
    pub(crate) from: u32,
    pub(crate) to: Vec<u32>,
}

/** The id of the primary item, from pitm; version 0 carries a 16-bit id and later versions 32 bits. */
pub(crate) fn parse_pitm(body: &[u8]) -> Result<u32> {
    let (version, _, rest) = full_box(body)?;
    let mut r = Reader::new(rest);
    if version == 0 {
        Ok(u32::from(r.u16()?))
    } else {
        r.u32()
    }
}

/** Every item in iinf, read from its infe children. */
pub(crate) fn parse_iinf(body: &[u8]) -> Result<Vec<ItemInfo>> {
    let (version, _, rest) = full_box(body)?;
    let mut r = Reader::new(rest);
    let declared = if version == 0 {
        r.u16()? as usize
    } else {
        r.u32()? as usize
    };
    if declared > MAX_ITEMS {
        return Err(Error::Limit("item count"));
    }
    let mut items = Vec::new();
    for entry in children(r.rest())? {
        if entry.kind == INFE {
            items.push(parse_infe(entry.body)?);
        }
    }
    if items.len() > MAX_ITEMS {
        return Err(Error::Limit("item count"));
    }
    Ok(items)
}

/** One infe entry. Versions 0 and 1 have no type; version 2 has a 16-bit id and version 3 a 32-bit id. */
fn parse_infe(body: &[u8]) -> Result<ItemInfo> {
    let (version, _, rest) = full_box(body)?;
    let mut r = Reader::new(rest);
    if version < 2 {
        let id = u32::from(r.u16()?);
        return Ok(ItemInfo { id, kind: [0; 4] });
    }
    let id = if version == 2 {
        u32::from(r.u16()?)
    } else {
        r.u32()?
    };
    r.u16()?; // item_protection_index: protected items are refused later by their missing type.
    Ok(ItemInfo {
        id,
        kind: r.fourcc()?,
    })
}

/** Every typed reference in iref. Version 0 carries 16-bit ids and version 1 32-bit ids. */
pub(crate) fn parse_iref(body: &[u8]) -> Result<Vec<Reference>> {
    let (version, _, rest) = full_box(body)?;
    let mut out = Vec::new();
    for entry in children(rest)? {
        if out.len() == MAX_REFERENCES {
            return Err(Error::Limit("reference count"));
        }
        out.push(parse_reference(entry.kind, entry.body, version)?);
    }
    Ok(out)
}

/** One SingleItemTypeReferenceBox: a source item and the items it points at. */
fn parse_reference(kind: FourCc, body: &[u8], version: u8) -> Result<Reference> {
    let mut r = Reader::new(body);
    let from = read_id(&mut r, version)?;
    let count = r.u16()? as usize;
    if count > MAX_REFERENCES {
        return Err(Error::Limit("reference count"));
    }
    let to = (0..count)
        .map(|_| read_id(&mut r, version))
        .collect::<Result<Vec<_>>>()?;
    Ok(Reference { kind, from, to })
}

/** An item id whose width depends on the box version. */
fn read_id(r: &mut Reader<'_>, version: u8) -> Result<u32> {
    if version == 0 {
        Ok(u32::from(r.u16()?))
    } else {
        r.u32()
    }
}

/** The primary item id of a meta box, or an error if the box has no pitm. */
pub(crate) fn primary_id(children_of_meta: &[crate::boxes::Boxed<'_>]) -> Result<u32> {
    let pitm = find(children_of_meta, PITM).ok_or(Error::Malformed("meta without pitm"))?;
    parse_pitm(pitm.body)
}

/** The iinf and iref boxes of a meta box, parsed, or empty when absent. */
pub(crate) fn tables(
    children_of_meta: &[crate::boxes::Boxed<'_>],
) -> Result<(Vec<ItemInfo>, Vec<Reference>)> {
    let items = match find(children_of_meta, IINF) {
        Some(b) => parse_iinf(b.body)?,
        None => Vec::new(),
    };
    let refs = match find(children_of_meta, IREF) {
        Some(b) => parse_iref(b.body)?,
        None => Vec::new(),
    };
    Ok((items, refs))
}
