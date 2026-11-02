/** Test support: a tiny ISOBMFF/HEIF writer that builds AVIF containers with exact field values, so each rule can be broken on purpose. */
/** A plain box: a 32-bit size, the type, then the body. */
pub fn bx(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut out = ((body.len() + 8) as u32).to_be_bytes().to_vec();
    out.extend_from_slice(kind);
    out.extend_from_slice(body);
    out
}

/** A FullBox: a box whose body begins with a version byte and 24 flag bits. */
pub fn fb(kind: &[u8; 4], version: u8, flags: u32, body: &[u8]) -> Vec<u8> {
    let mut inner = vec![version];
    inner.extend_from_slice(&flags.to_be_bytes()[1..]);
    inner.extend_from_slice(body);
    bx(kind, &inner)
}

/** The ftyp box for an AVIF file with the given major brand and compatible brands. */
pub fn ftyp(major: &[u8; 4], compatible: &[&[u8; 4]]) -> Vec<u8> {
    let mut body = major.to_vec();
    body.extend_from_slice(&0u32.to_be_bytes());
    for brand in compatible {
        body.extend_from_slice(*brand);
    }
    bx(b"ftyp", &body)
}

/** An hdlr box naming the handler type. */
pub fn hdlr(handler: &[u8; 4]) -> Vec<u8> {
    let mut body = vec![0u8; 4];
    body.extend_from_slice(handler);
    body.extend_from_slice(&[0u8; 12]);
    body.push(0);
    fb(b"hdlr", 0, 0, &body)
}

/** A pitm box with a 16-bit id, which is version 0. */
pub fn pitm(id: u16) -> Vec<u8> {
    fb(b"pitm", 0, 0, &id.to_be_bytes())
}

/** An infe version 2 entry declaring an item's id and four-character type. */
pub fn infe(id: u16, kind: &[u8; 4]) -> Vec<u8> {
    let mut body = id.to_be_bytes().to_vec();
    body.extend_from_slice(&0u16.to_be_bytes());
    body.extend_from_slice(kind);
    body.push(0);
    fb(b"infe", 2, 0, &body)
}

/** An iinf box holding the given infe entries. */
pub fn iinf(entries: &[Vec<u8>]) -> Vec<u8> {
    let mut body = (entries.len() as u16).to_be_bytes().to_vec();
    for entry in entries {
        body.extend_from_slice(entry);
    }
    fb(b"iinf", 0, 0, &body)
}

/** One iloc entry: the item, its construction method (version 1 layout), and extents of (offset, length). */
pub fn loc(id: u16, method: u16, extents: &[(u32, u32)]) -> Vec<u8> {
    let mut out = id.to_be_bytes().to_vec();
    out.extend_from_slice(&method.to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes()); // data_reference_index
    out.extend_from_slice(&(extents.len() as u16).to_be_bytes());
    for (offset, length) in extents {
        out.extend_from_slice(&offset.to_be_bytes());
        out.extend_from_slice(&length.to_be_bytes());
    }
    out
}

/** An iloc version 1 box with 4-byte offset and length fields and no base offset, holding the given entries. */
pub fn iloc(entries: &[Vec<u8>]) -> Vec<u8> {
    let mut body = vec![0x44, 0x00];
    body.extend_from_slice(&(entries.len() as u16).to_be_bytes());
    for entry in entries {
        body.extend_from_slice(entry);
    }
    fb(b"iloc", 1, 0, &body)
}

/** An ispe box with the picture size. */
pub fn ispe(width: u32, height: u32) -> Vec<u8> {
    let mut body = width.to_be_bytes().to_vec();
    body.extend_from_slice(&height.to_be_bytes());
    fb(b"ispe", 0, 0, &body)
}

/** A valid av1C box with no config OBUs. */
pub fn av1c() -> Vec<u8> {
    bx(b"av1C", &[0x81, 0x00, 0x0c, 0x00])
}

/** An ipma box, version 0 with 8-bit slots, linking each item to its (1-based) property slots. */
pub fn ipma(links: &[(u16, &[u8])]) -> Vec<u8> {
    let mut body = (links.len() as u32).to_be_bytes().to_vec();
    for (id, slots) in links {
        body.extend_from_slice(&id.to_be_bytes());
        body.push(slots.len() as u8);
        body.extend_from_slice(slots);
    }
    fb(b"ipma", 0, 0, &body)
}

/** An iprp box holding a property container and its links. */
pub fn iprp(props: &[Vec<u8>], links: &[u8]) -> Vec<u8> {
    let ipco = bx(b"ipco", &props.concat());
    bx(b"iprp", &[ipco, links.to_vec()].concat())
}

/** A complete single-item AVIF: ftyp, meta, then an mdat holding the payload. */
pub fn single_item(width: u32, height: u32, payload: &[u8]) -> Vec<u8> {
    let props = [ispe(width, height), av1c()];
    let links = ipma(&[(1, &[1, 2])]);
    let header = ftyp(b"avif", &[b"mif1"]);
    let meta_box = |offset: u32| {
        let entries = [loc(1, 0, &[(offset, payload.len() as u32)])];
        fb(
            b"meta",
            0,
            0,
            &[
                hdlr(b"pict"),
                pitm(1),
                iinf(&[infe(1, b"av01")]),
                iloc(&entries),
                iprp(&props, &links),
            ]
            .concat(),
        )
    };
    let meta_len = meta_box(0).len() as u32;
    let offset = header.len() as u32 + meta_len + 8;
    let mut file = header;
    file.extend_from_slice(&meta_box(offset));
    file.extend_from_slice(&bx(b"mdat", payload));
    file
}
