/** Named constants for ISOBMFF framing, HEIF and AVIF item types, and the limits that bound untrusted input. */
/** A four-character box, item or brand code. */
pub(crate) type FourCc = [u8; 4];

/** A box size of zero means the box runs to the end of the bytes that contain it. */
pub(crate) const SIZE_TO_END: u32 = 0;
/** A box size of one means the real size follows as a 64-bit largesize field. */
pub(crate) const SIZE_LARGE: u32 = 1;
/** Bytes in the user type that follows a `uuid` box type. */
pub(crate) const UUID_LEN: usize = 16;
/** Bytes in the 24-bit flags of a FullBox. */
pub(crate) const FULL_BOX_FLAGS_LEN: usize = 3;
/** Bytes in a four-character code. */
pub(crate) const FOURCC_LEN: usize = 4;
/** Widest integer field the reader accepts, in bytes. */
pub(crate) const MAX_UINT_WIDTH: usize = 8;
/** Bits in the high nibble of a packed size byte, and the mask for its low nibble. */
pub(crate) const NIBBLE_BITS: u32 = 4;
/** Mask selecting a low nibble. */
pub(crate) const NIBBLE_MASK: u8 = 0x0f;

/** The iloc version that adds the construction method and the extent index field. */
pub(crate) const ILOC_VERSION_WITH_METHOD: u8 = 1;
/** Mask for the construction method, which shares its 16-bit field with 12 reserved bits. */
pub(crate) const ILOC_METHOD_MASK: u16 = 0x000f;

/** Field widths the ISO spec allows for iloc offset, length, base and index fields. */
pub(crate) const ILOC_WIDTHS: [usize; 3] = [0, 4, 8];

/** Box type `uuid`. */
pub(crate) const UUID: FourCc = *b"uuid";
/** Box type `ftyp`, the file type and compatible brands. */
pub(crate) const FTYP: FourCc = *b"ftyp";
/** Box type `meta`, the item table of a HEIF file. */
pub(crate) const META: FourCc = *b"meta";
/** Box type `hdlr`, the handler; HEIF requires `pict`. */
pub(crate) const HDLR: FourCc = *b"hdlr";
/** Box type `pitm`, the primary item. */
pub(crate) const PITM: FourCc = *b"pitm";
/** Box type `iinf`, the item information table. */
pub(crate) const IINF: FourCc = *b"iinf";
/** Box type `infe`, one item information entry. */
pub(crate) const INFE: FourCc = *b"infe";
/** Box type `iloc`, where each item's bytes live. */
pub(crate) const ILOC: FourCc = *b"iloc";
/** Box type `iref`, typed references between items. */
pub(crate) const IREF: FourCc = *b"iref";
/** Box type `iprp`, the item properties container. */
pub(crate) const IPRP: FourCc = *b"iprp";
/** Box type `ipco`, the list of property boxes. */
pub(crate) const IPCO: FourCc = *b"ipco";
/** Box type `ipma`, which properties apply to which items. */
pub(crate) const IPMA: FourCc = *b"ipma";
/** Box type `idat`, item data stored inside `meta`. */
pub(crate) const IDAT: FourCc = *b"idat";

/** Handler type `pict`, the only one a HEIF image uses. */
pub(crate) const PICT: FourCc = *b"pict";
/** Item type `av01`: an AV1 coded image. */
pub(crate) const AV01: FourCc = *b"av01";
/** Reference type `auxl`: an auxiliary image that refers to its primary. */
pub(crate) const AUXL: FourCc = *b"auxl";

/** Major or compatible brand `avif`. */
pub(crate) const BRAND_AVIF: FourCc = *b"avif";
/** Brand `avis`, an AVIF image sequence. */
pub(crate) const BRAND_AVIS: FourCc = *b"avis";
/** Brand `mif1`, the generic HEIF still image brand AVIF files also carry. */
pub(crate) const BRAND_MIF1: FourCc = *b"mif1";
/** Brands that mark a file as AVIF when they are the major brand or a compatible brand. */
pub(crate) const AVIF_BRANDS: [FourCc; 3] = [BRAND_AVIF, BRAND_AVIS, BRAND_MIF1];

/** Property `ispe`: the image size. */
pub(crate) const PROP_ISPE: FourCc = *b"ispe";
/** Property `av1C`: the AV1 codec configuration. */
pub(crate) const PROP_AV1C: FourCc = *b"av1C";
/** Property `colr`: colour information. */
pub(crate) const PROP_COLR: FourCc = *b"colr";
/** Property `pixi`: bits per channel. */
pub(crate) const PROP_PIXI: FourCc = *b"pixi";
/** Property `auxC`: which auxiliary image type an item is. */
pub(crate) const PROP_AUXC: FourCc = *b"auxC";
/** Property `irot`: a quarter-turn counter-clockwise rotation. */
pub(crate) const PROP_IROT: FourCc = *b"irot";
/** Property `imir`: a mirror across an axis. */
pub(crate) const PROP_IMIR: FourCc = *b"imir";
/** Property `clap`: a clean aperture crop, which Mars does not apply yet. */
pub(crate) const PROP_CLAP: FourCc = *b"clap";

/** Colour type `nclx`: CICP primaries, transfer, matrix and a range flag. */
pub(crate) const COLR_NCLX: FourCc = *b"nclx";
/** Colour type `rICC`: an embedded ICC profile. */
pub(crate) const COLR_RICC: FourCc = *b"rICC";
/** Colour type `prof`: an embedded ICC profile. */
pub(crate) const COLR_PROF: FourCc = *b"prof";
/** Bytes of the nclx body after its type: three 16-bit codes and a range byte. */
pub(crate) const NCLX_BODY_LEN: usize = 7;
/** Bit in the nclx range byte that marks full-range samples. */
pub(crate) const NCLX_FULL_RANGE_BIT: u8 = 0x80;

/** Alpha auxiliary type URNs; either one marks an auxC property as the alpha plane. */
pub(crate) const ALPHA_URNS: [&[u8]; 2] = [
    b"urn:mpeg:mpegB:cicp:systems:auxiliary:alpha",
    b"urn:mpeg:hevc:2015:auxid:1",
];

/** Location method 0: the bytes sit in the file at the iloc offset. */
pub(crate) const LOCATION_FILE: u8 = 0;
/** Location method 1: the bytes sit in the `idat` box of `meta`. */
pub(crate) const LOCATION_IDAT: u8 = 1;

/** The first byte of an av1C box: marker bit set and version 1. */
pub(crate) const AV1C_MARKER_VERSION: u8 = 0x81;
/** Bytes of fixed av1C fields before the optional configOBUs. */
pub(crate) const AV1C_HEADER_LEN: usize = 4;
/** Bit positions in the second and third av1C bytes. */
pub(crate) const AV1C_HIGH_BITDEPTH_BIT: u32 = 6;
pub(crate) const AV1C_TWELVE_BIT_BIT: u32 = 5;
pub(crate) const AV1C_MONOCHROME_BIT: u32 = 4;
pub(crate) const AV1C_SUBSAMPLING_X_BIT: u32 = 3;
pub(crate) const AV1C_SUBSAMPLING_Y_BIT: u32 = 2;
/** Bits of seq_profile at the top of the second av1C byte. */
pub(crate) const AV1C_PROFILE_SHIFT: u32 = 5;

/** Mask for the rotation angle in an irot body. */
pub(crate) const IROT_ANGLE_MASK: u8 = 0x03;
/** Mask for the mirror axis in an imir body. */
pub(crate) const IMIR_AXIS_MASK: u8 = 0x01;
/** The rotation angle that swaps width and height. */
pub(crate) const IROT_ODD_TURNS: [u8; 2] = [1, 3];

/** ipma flag bit: property indexes are 16 bits wide instead of 8. */
pub(crate) const IPMA_WIDE_FLAG: u32 = 1;
/** ipma essential bit and index mask for the 8-bit form. */
pub(crate) const IPMA_ESSENTIAL_NARROW: u8 = 0x80;
pub(crate) const IPMA_INDEX_NARROW: u8 = 0x7f;
/** ipma essential bit and index mask for the 16-bit form. */
pub(crate) const IPMA_ESSENTIAL_WIDE: u16 = 0x8000;
pub(crate) const IPMA_INDEX_WIDE: u16 = 0x7fff;
/** The entry version at which ipma item ids widen to 32 bits. */
pub(crate) const IPMA_WIDE_ID_VERSION: u8 = 1;

/** Largest item count, property count, extent count or reference count the reader accepts. */
pub(crate) const MAX_BOXES: usize = 4096;
pub(crate) const MAX_ITEMS: usize = 4096;
pub(crate) const MAX_EXTENTS: usize = 4096;
pub(crate) const MAX_PROPERTIES: usize = 4096;
pub(crate) const MAX_REFERENCES: usize = 4096;
/** Largest side length the AV1 levels allow. */
pub(crate) const MAX_DIMENSION: u32 = 16384;
/** Largest picture the AV1 level 6 limits allow; bounds the RGBA buffer a decode allocates. */
pub(crate) const MAX_PICTURE_PIXELS: u64 = 35_651_584;
