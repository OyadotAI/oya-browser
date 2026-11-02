/** The meta box of a HEIF file: handler check, item tables, locations and properties, plus the lookups that resolve an item to its bytes and properties. */
use crate::boxes::{Boxed, children, find, full_box};
use crate::constants::{HDLR, IDAT, ILOC, IPRP, LOCATION_IDAT, PICT};
use crate::error::{Error, Result};
use crate::iloc::{self, Location};
use crate::items::{self, ItemInfo, Reference};
use crate::properties::{self, Association, Property};
use crate::reader::Reader;

/** The parsed meta box. Its idat, when present, borrows from the file so item bytes can be copied out later. */
pub(crate) struct Meta<'a> {
    pub(crate) primary: u32,
    pub(crate) items: Vec<ItemInfo>,
    pub(crate) references: Vec<Reference>,
    pub(crate) locations: Vec<Location>,
    pub(crate) properties: Vec<Property>,
    pub(crate) links: Vec<Association>,
    pub(crate) idat: Option<&'a [u8]>,
}

/** Parse the body of a meta box. HEIF requires the `pict` handler, a primary item and an iloc. */
pub(crate) fn parse(body: &[u8]) -> Result<Meta<'_>> {
    let (_, _, rest) = full_box(body)?;
    let kids = children(rest)?;
    check_handler(&kids)?;
    let primary = items::primary_id(&kids)?;
    let (items, references) = items::tables(&kids)?;
    let iloc_box = find(&kids, ILOC).ok_or(Error::Malformed("meta without iloc"))?;
    let locations = iloc::parse(iloc_box.body)?;
    let (properties, links) = match find(&kids, IPRP) {
        Some(b) => properties::parse_iprp(&children(b.body)?)?,
        None => (Vec::new(), Vec::new()),
    };
    let idat = find(&kids, IDAT).map(|b| b.body);
    Ok(Meta {
        primary,
        items,
        references,
        locations,
        properties,
        links,
        idat,
    })
}

/** The hdlr box must name the `pict` handler; any other handler is not a still image. */
fn check_handler(kids: &[Boxed<'_>]) -> Result<()> {
    let hdlr = find(kids, HDLR).ok_or(Error::Malformed("meta without hdlr"))?;
    let (_, _, rest) = full_box(hdlr.body)?;
    let mut r = Reader::new(rest);
    r.u32()?; // pre_defined carries nothing the decoder reads.
    if r.fourcc()? == PICT {
        Ok(())
    } else {
        Err(Error::Malformed("handler is not pict"))
    }
}

impl Meta<'_> {
    /** The declared item with this id, if the file declares one. */
    pub(crate) fn item(&self, id: u32) -> Option<ItemInfo> {
        self.items.iter().copied().find(|i| i.id == id)
    }

    /** The properties attached to an item. An essential property Mars does not understand refuses the item. */
    pub(crate) fn properties_of(&self, id: u32) -> Result<Vec<&Property>> {
        let mut out = Vec::new();
        for link in self.links.iter().filter(|l| l.item == id) {
            for slot in link.slots.iter().filter(|s| s.index != 0) {
                let prop = self
                    .properties
                    .get(usize::from(slot.index) - 1)
                    .ok_or(Error::Malformed("property index"))?;
                if slot.essential && *prop == Property::Other {
                    return Err(Error::Unsupported("essential item property"));
                }
                out.push(prop);
            }
        }
        Ok(out)
    }

    /** The coded bytes of an item, copied from the file or from idat as its location says. */
    pub(crate) fn payload(&self, file: &[u8], id: u32) -> Result<Vec<u8>> {
        let location = self
            .locations
            .iter()
            .find(|l| l.id == id)
            .ok_or(Error::Malformed("item without location"))?;
        let base = if location.method == LOCATION_IDAT {
            self.idat.ok_or(Error::Malformed("idat missing"))?
        } else {
            file
        };
        iloc::read(base, location)
    }
}
