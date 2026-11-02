/** AVIF semantics over the boxes: the brand check, the primary and alpha items, and the properties that size, colour and orient the picture. */
use crate::boxes::{children, find};
use crate::constants::{
    AUXL, AV01, AVIF_BRANDS, FOURCC_LEN, FTYP, IROT_ODD_TURNS, MAX_DIMENSION, MAX_PICTURE_PIXELS,
    META,
};
use crate::error::{Error, Result};
use crate::meta::{self, Meta};
use crate::properties::{Av1C, CleanAperture, Colr, Property};
use crate::reader::Reader;

/** An AVIF file whose container has been read and checked. Item bytes are copied out only on request. */
// The colour, bit depth and mirror fields are read by the AV1 sample stage, which is not built yet.
#[allow(dead_code)]
pub(crate) struct Picture<'a> {
    file: &'a [u8],
    meta: Meta<'a>,
    pub(crate) primary: u32,
    pub(crate) alpha: Option<u32>,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) av1c: Av1C,
    pub(crate) colr: Option<Colr>,
    pub(crate) bit_depth: Option<u8>,
    pub(crate) rotation: u8,
    pub(crate) mirror: Option<u8>,
    pub(crate) clap: Option<CleanAperture>,
}

impl Picture<'_> {
    /** The size the picture is output at: the clean-aperture crop comes first, then a quarter-turn rotation swaps width and height. A crop that leaves a side empty is refused. */
    pub(crate) fn output_size(&self) -> Result<(u32, u32)> {
        let (width, height) = match self.clap {
            Some(c) => (
                cropped_side(self.width, c.width)?,
                cropped_side(self.height, c.height)?,
            ),
            None => (self.width, self.height),
        };
        Ok(if IROT_ODD_TURNS.contains(&self.rotation) {
            (height, width)
        } else {
            (width, height)
        })
    }

    /** The coded bytes of one item, such as the primary or alpha AV1 payload. */
    pub(crate) fn payload(&self, id: u32) -> Result<Vec<u8>> {
        self.meta.payload(self.file, id)
    }
}

/** Read the container of an AVIF file and check that it describes a picture this crate can handle. */
pub(crate) fn open(file: &[u8]) -> Result<Picture<'_>> {
    let top = children(file)?;
    check_brand(
        find(&top, FTYP)
            .ok_or(Error::Malformed("missing ftyp"))?
            .body,
    )?;
    let meta = meta::parse(
        find(&top, META)
            .ok_or(Error::Malformed("missing meta"))?
            .body,
    )?;
    let primary = meta.primary;
    let info = meta
        .item(primary)
        .ok_or(Error::Malformed("undeclared primary item"))?;
    if info.kind != AV01 {
        return Err(Error::Unsupported("primary item type"));
    }
    let props = meta.properties_of(primary)?;
    let (width, height) = size(&props)?;
    check_size(width, height)?;
    let picture = Picture {
        file,
        primary,
        alpha: alpha_item(&meta, primary),
        width,
        height,
        av1c: av1c(&props)?,
        colr: find_prop(&props, |p| match p {
            Property::Colr(c) => Some(*c),
            _ => None,
        }),
        bit_depth: find_prop(&props, |p| match p {
            Property::Pixi { bits } => Some(*bits),
            _ => None,
        }),
        rotation: find_prop(&props, |p| match p {
            Property::Irot { angle } => Some(*angle),
            _ => None,
        })
        .unwrap_or(0),
        mirror: find_prop(&props, |p| match p {
            Property::Imir { axis } => Some(*axis),
            _ => None,
        }),
        clap: find_prop(&props, |p| match p {
            Property::Clap(c) => Some(*c),
            _ => None,
        }),
        meta,
    };
    Ok(picture)
}

/** A clean-aperture side: the fraction's integer part, never wider than the decoded side. A zero denominator or a crop that leaves no samples is Malformed, so no later step divides by it or builds an empty picture. */
pub(crate) fn cropped_side(side: u32, fraction: (u32, u32)) -> Result<u32> {
    let whole = fraction
        .0
        .checked_div(fraction.1)
        .ok_or(Error::Malformed("clap denominator"))?;
    match whole.min(side) {
        0 => Err(Error::Malformed("cropped size")),
        cropped => Ok(cropped),
    }
}

/** The first property that `pick` extracts a value from, if any. */
fn find_prop<T>(props: &[&Property], pick: impl Fn(&Property) -> Option<T>) -> Option<T> {
    props.iter().find_map(|p| pick(p))
}

/** The ftyp box must name AVIF as its major brand or one of its compatible brands. */
fn check_brand(body: &[u8]) -> Result<()> {
    let mut r = Reader::new(body);
    let major = r.fourcc()?;
    r.u32()?; // minor_version has no bearing on what the decoder does.
    let is_avif = |brand: &[u8]| AVIF_BRANDS.iter().any(|b| b[..] == *brand);
    let compatible_ok = r.rest().chunks_exact(FOURCC_LEN).any(is_avif);
    if is_avif(&major) || compatible_ok {
        Ok(())
    } else {
        Err(Error::Malformed("not an AVIF file"))
    }
}

/** The ispe size of the primary item, which every AVIF picture must declare. */
fn size(props: &[&Property]) -> Result<(u32, u32)> {
    find_prop(props, |p| match p {
        Property::Ispe { width, height } => Some((*width, *height)),
        _ => None,
    })
    .ok_or(Error::Malformed("primary item without ispe"))
}

/** The av1C configuration of the primary item, which every AVIF picture must declare. */
fn av1c(props: &[&Property]) -> Result<Av1C> {
    find_prop(props, |p| match p {
        Property::Av1C(c) => Some(c.clone()),
        _ => None,
    })
    .ok_or(Error::Malformed("primary item without av1C"))
}

/** Refuse zero sides, sides past the AV1 level limit, and areas whose RGBA buffer would be unreasonable. */
fn check_size(width: u32, height: u32) -> Result<()> {
    if width == 0 || height == 0 {
        return Err(Error::Malformed("picture size"));
    }
    if width > MAX_DIMENSION || height > MAX_DIMENSION {
        return Err(Error::Limit("picture side"));
    }
    if u64::from(width) * u64::from(height) > MAX_PICTURE_PIXELS {
        return Err(Error::Limit("picture area"));
    }
    Ok(())
}

/** The alpha plane: an AV1 item with an alpha auxC whose auxl reference points at the primary item. */
fn alpha_item(meta: &Meta<'_>, primary: u32) -> Option<u32> {
    meta.references
        .iter()
        .filter(|r| r.kind == AUXL && r.to.contains(&primary))
        .map(|r| r.from)
        .find(|id| is_alpha(meta, *id))
}

/** Whether an item is an alpha auxiliary: AV1-coded and tagged with an alpha auxC property. */
fn is_alpha(meta: &Meta<'_>, id: u32) -> bool {
    let coded = meta.item(id).is_some_and(|i| i.kind == AV01);
    coded
        && meta.properties_of(id).is_ok_and(|props| {
            props
                .iter()
                .any(|p| matches!(p, Property::AuxC { alpha: true }))
        })
}
