/** Item properties: the ipco property boxes, the ipma links that attach them to items, and the parsers for the ones AVIF decoding reads. */
use crate::boxes::{children, find, full_box};
use crate::constants::{
    ALPHA_URNS, AV1C_HEADER_LEN, AV1C_HIGH_BITDEPTH_BIT, AV1C_MARKER_VERSION, AV1C_MONOCHROME_BIT,
    AV1C_PROFILE_SHIFT, AV1C_SUBSAMPLING_X_BIT, AV1C_SUBSAMPLING_Y_BIT, AV1C_TWELVE_BIT_BIT,
    COLR_NCLX, COLR_PROF, COLR_RICC, FourCc, IMIR_AXIS_MASK, IPCO, IPMA, IPMA_ESSENTIAL_NARROW,
    IPMA_ESSENTIAL_WIDE, IPMA_INDEX_NARROW, IPMA_INDEX_WIDE, IPMA_WIDE_FLAG, IPMA_WIDE_ID_VERSION,
    IROT_ANGLE_MASK, MAX_ITEMS, MAX_PROPERTIES, NCLX_BODY_LEN, NCLX_FULL_RANGE_BIT, PROP_AUXC,
    PROP_AV1C, PROP_CLAP, PROP_COLR, PROP_IMIR, PROP_IROT, PROP_ISPE, PROP_PIXI,
};
use crate::error::{Error, Result};
use crate::reader::Reader;

/** The CICP colour description of an nclx colr box. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Nclx {
    pub(crate) primaries: u16,
    pub(crate) transfer: u16,
    pub(crate) matrix: u16,
    pub(crate) full_range: bool,
}

/** The colr property: CICP codes, an ICC profile (kept only as a fact, since Mars does not use it), or another kind. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Colr {
    Nclx(Nclx),
    Icc,
    Other,
}

/** The AV1 codec configuration: the fields the decoder needs before it reads the sequence header. */
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Av1C {
    pub(crate) profile: u8,
    pub(crate) high_bitdepth: bool,
    pub(crate) twelve_bit: bool,
    pub(crate) monochrome: bool,
    pub(crate) subsampling_x: bool,
    pub(crate) subsampling_y: bool,
    pub(crate) config_obus: Vec<u8>,
}

/** The clean aperture crop: width and height as fractions (numerator, denominator), and the offsets of the crop centre from the image centre. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CleanAperture {
    pub(crate) width: (u32, u32),
    pub(crate) height: (u32, u32),
    pub(crate) horiz_offset: (u32, u32),
    pub(crate) vert_offset: (u32, u32),
}

/** The clap box: eight unsigned 32-bit fields, each fraction's denominator must be non-zero. */
fn parse_clap(body: &[u8]) -> Result<CleanAperture> {
    let mut r = Reader::new(body);
    let mut fraction = || -> Result<(u32, u32)> {
        let (n, d) = (r.u32()?, r.u32()?);
        if d == 0 {
            return Err(Error::Malformed("clap denominator"));
        }
        Ok((n, d))
    };
    Ok(CleanAperture {
        width: fraction()?,
        height: fraction()?,
        horiz_offset: fraction()?,
        vert_offset: fraction()?,
    })
}

/** One property box Mars recognises, or an unknown one kept only so essential flags can refuse it. */
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Property {
    Ispe { width: u32, height: u32 },
    Av1C(Av1C),
    Colr(Colr),
    Pixi { bits: u8 },
    AuxC { alpha: bool },
    Irot { angle: u8 },
    Imir { axis: u8 },
    Clap(CleanAperture),
    Other,
}

/** One property link from ipma: a 1-based index into ipco (0 means none) and whether it is essential. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Slot {
    pub(crate) index: u16,
    pub(crate) essential: bool,
}

/** The property slots of one item, as ipma lists them. */
pub(crate) struct Association {
    pub(crate) item: u32,
    pub(crate) slots: Vec<Slot>,
}

/** Every property in an ipco box, in order, so that ipma indexes resolve to them. */
pub(crate) fn parse_ipco(body: &[u8]) -> Result<Vec<Property>> {
    let boxes = children(body)?;
    if boxes.len() > MAX_PROPERTIES {
        return Err(Error::Limit("property count"));
    }
    boxes
        .iter()
        .map(|b| parse_property(b.kind, b.body))
        .collect()
}

/** Decode one property box by its type; unknown types become Other rather than failing the file. */
fn parse_property(kind: FourCc, body: &[u8]) -> Result<Property> {
    Ok(match kind {
        PROP_ISPE => parse_ispe(body)?,
        PROP_AV1C => Property::Av1C(parse_av1c(body)?),
        PROP_COLR => Property::Colr(parse_colr(body)?),
        PROP_PIXI => parse_pixi(body)?,
        PROP_AUXC => parse_auxc(body)?,
        PROP_IROT => Property::Irot {
            angle: body.first().ok_or(Error::Truncated)? & IROT_ANGLE_MASK,
        },
        PROP_IMIR => Property::Imir {
            axis: body.first().ok_or(Error::Truncated)? & IMIR_AXIS_MASK,
        },
        PROP_CLAP => Property::Clap(parse_clap(body)?),
        _ => Property::Other,
    })
}

/** The ispe box: a FullBox holding the picture width and height. */
fn parse_ispe(body: &[u8]) -> Result<Property> {
    let (_, _, rest) = full_box(body)?;
    let mut r = Reader::new(rest);
    Ok(Property::Ispe {
        width: r.u32()?,
        height: r.u32()?,
    })
}

/** The av1C box: four fixed bytes of codec configuration, then the optional configOBUs. */
pub(crate) fn parse_av1c(body: &[u8]) -> Result<Av1C> {
    let mut r = Reader::new(body);
    let head = r.take(AV1C_HEADER_LEN)?;
    if head[0] != AV1C_MARKER_VERSION {
        return Err(Error::Malformed("av1C marker"));
    }
    let flags = head[2];
    let bit = |n: u32| (flags >> n) & 1 == 1;
    Ok(Av1C {
        profile: head[1] >> AV1C_PROFILE_SHIFT,
        high_bitdepth: bit(AV1C_HIGH_BITDEPTH_BIT),
        twelve_bit: bit(AV1C_TWELVE_BIT_BIT),
        monochrome: bit(AV1C_MONOCHROME_BIT),
        subsampling_x: bit(AV1C_SUBSAMPLING_X_BIT),
        subsampling_y: bit(AV1C_SUBSAMPLING_Y_BIT),
        config_obus: r.rest().to_vec(),
    })
}

/** The colr box. An nclx carries CICP codes and a range flag; an ICC profile is recorded but its bytes are not read. */
fn parse_colr(body: &[u8]) -> Result<Colr> {
    let mut r = Reader::new(body);
    match r.fourcc()? {
        COLR_NCLX => {
            let fields = r.take(NCLX_BODY_LEN)?;
            Ok(Colr::Nclx(Nclx {
                primaries: u16::from_be_bytes([fields[0], fields[1]]),
                transfer: u16::from_be_bytes([fields[2], fields[3]]),
                matrix: u16::from_be_bytes([fields[4], fields[5]]),
                full_range: fields[6] & NCLX_FULL_RANGE_BIT != 0,
            }))
        }
        COLR_RICC | COLR_PROF => Ok(Colr::Icc),
        _ => Ok(Colr::Other),
    }
}

/** The pixi box: the bit depth of each channel. Mars reads the first channel's depth. */
fn parse_pixi(body: &[u8]) -> Result<Property> {
    let (_, _, rest) = full_box(body)?;
    let mut r = Reader::new(rest);
    let channels = r.byte()?;
    if channels == 0 {
        return Err(Error::Malformed("pixi channel count"));
    }
    Ok(Property::Pixi { bits: r.byte()? })
}

/** The auxC box: an auxiliary type string. Only the two alpha URNs make the item an alpha plane. */
fn parse_auxc(body: &[u8]) -> Result<Property> {
    let (_, _, rest) = full_box(body)?;
    let end = rest.iter().position(|b| *b == 0).unwrap_or(rest.len());
    let kind = &rest[..end];
    Ok(Property::AuxC {
        alpha: ALPHA_URNS.contains(&kind),
    })
}

/** ipma: which property slots apply to which items. Its flags choose 8-bit or 16-bit index fields. */
pub(crate) fn parse_ipma(body: &[u8]) -> Result<Vec<Association>> {
    let (version, flags, rest) = full_box(body)?;
    let mut r = Reader::new(rest);
    let count = r.u32()? as usize;
    if count > MAX_ITEMS {
        return Err(Error::Limit("ipma item count"));
    }
    let wide = flags & IPMA_WIDE_FLAG != 0;
    (0..count)
        .map(|_| parse_association(&mut r, version, wide))
        .collect()
}

/** One ipma entry: an item id, then its property slots. */
fn parse_association(r: &mut Reader<'_>, version: u8, wide: bool) -> Result<Association> {
    let item = if version < IPMA_WIDE_ID_VERSION {
        u32::from(r.u16()?)
    } else {
        r.u32()?
    };
    let entries = usize::from(r.byte()?);
    let mut slots = Vec::with_capacity(entries);
    for _ in 0..entries {
        slots.push(if wide {
            let v = r.u16()?;
            Slot {
                index: v & IPMA_INDEX_WIDE,
                essential: v & IPMA_ESSENTIAL_WIDE != 0,
            }
        } else {
            let v = r.byte()?;
            Slot {
                index: u16::from(v & IPMA_INDEX_NARROW),
                essential: v & IPMA_ESSENTIAL_NARROW != 0,
            }
        });
    }
    Ok(Association { item, slots })
}

/** The iprp box's ipco and ipma children, parsed; an absent ipma means no item has properties. */
pub(crate) fn parse_iprp(
    iprp_children: &[crate::boxes::Boxed<'_>],
) -> Result<(Vec<Property>, Vec<Association>)> {
    let props = match find(iprp_children, IPCO) {
        Some(b) => parse_ipco(b.body)?,
        None => Vec::new(),
    };
    let links = match find(iprp_children, IPMA) {
        Some(b) => parse_ipma(b.body)?,
        None => Vec::new(),
    };
    Ok((props, links))
}
