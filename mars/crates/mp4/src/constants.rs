/** Box types, flag bits and size limits of the fragmented MP4 subset Mars demuxes. */
/** A box type: four ASCII bytes. */
pub(crate) type Kind = [u8; 4];

pub(crate) const MOOV: Kind = *b"moov";
pub(crate) const TRAK: Kind = *b"trak";
pub(crate) const TKHD: Kind = *b"tkhd";
pub(crate) const MDIA: Kind = *b"mdia";
pub(crate) const MDHD: Kind = *b"mdhd";
pub(crate) const HDLR: Kind = *b"hdlr";
pub(crate) const MINF: Kind = *b"minf";
pub(crate) const STBL: Kind = *b"stbl";
pub(crate) const STSD: Kind = *b"stsd";
pub(crate) const EDTS: Kind = *b"edts";
pub(crate) const ELST: Kind = *b"elst";
pub(crate) const MVEX: Kind = *b"mvex";
pub(crate) const TREX: Kind = *b"trex";
pub(crate) const MOOF: Kind = *b"moof";
pub(crate) const TRAF: Kind = *b"traf";
pub(crate) const TFHD: Kind = *b"tfhd";
pub(crate) const TFDT: Kind = *b"tfdt";
pub(crate) const TRUN: Kind = *b"trun";
pub(crate) const AVCC: Kind = *b"avcC";
pub(crate) const ESDS: Kind = *b"esds";
pub(crate) const AVC1: Kind = *b"avc1";
pub(crate) const AVC3: Kind = *b"avc3";
pub(crate) const MP4A: Kind = *b"mp4a";
pub(crate) const VIDE: Kind = *b"vide";
pub(crate) const SOUN: Kind = *b"soun";

/// Header sizes: a plain box is size + type; a large-size box adds a 64-bit size.
pub(crate) const HEADER: usize = 8;
pub(crate) const HEADER_LARGE: usize = 16;
/// The size field value meaning "a 64-bit size follows", and the one meaning "to the end of the file".
pub(crate) const SIZE_LARGE: u32 = 1;
pub(crate) const SIZE_TO_END: u32 = 0;

/// Full-box version 1 uses 64-bit times and offsets.
pub(crate) const VERSION_LARGE: u8 = 1;

/// The fixed part of a VisualSampleEntry and an AudioSampleEntry before their child boxes.
pub(crate) const VISUAL_ENTRY_FIELDS: usize = 78;
pub(crate) const AUDIO_ENTRY_FIELDS: usize = 28;
/// Offsets, within an audio sample entry's body, of its channel count (16 bits) and its sample rate (16.16 fixed).
pub(crate) const AUDIO_CHANNELS_AT: usize = 16;
pub(crate) const AUDIO_RATE_AT: usize = 24;

/// tfhd flags: which optional fields follow the track id.
pub(crate) const TFHD_BASE_OFFSET: u32 = 0x1;
pub(crate) const TFHD_SAMPLE_DESCRIPTION: u32 = 0x2;
pub(crate) const TFHD_DEFAULT_DURATION: u32 = 0x8;
pub(crate) const TFHD_DEFAULT_SIZE: u32 = 0x10;
pub(crate) const TFHD_DEFAULT_FLAGS: u32 = 0x20;
/// tfhd: the base is the start of the moof, not the previous fragment's end.
pub(crate) const TFHD_BASE_IS_MOOF: u32 = 0x20000;

/// trun flags: which fields each sample carries, and whether the first sample's flags are separate.
pub(crate) const TRUN_DATA_OFFSET: u32 = 0x1;
pub(crate) const TRUN_FIRST_FLAGS: u32 = 0x4;
pub(crate) const TRUN_DURATION: u32 = 0x100;
pub(crate) const TRUN_SIZE: u32 = 0x200;
pub(crate) const TRUN_FLAGS: u32 = 0x400;
pub(crate) const TRUN_COMPOSITION: u32 = 0x800;

/// Sample flags: bit 16 set means the sample depends on others (not a sync sample).
pub(crate) const SAMPLE_NON_SYNC: u32 = 0x0001_0000;

/// The most samples one run may declare; a larger count is refused rather than allocated.
pub(crate) const SAMPLE_LIMIT: u32 = 1 << 20;

/// The most samples one media segment may yield in total, across all its moofs, trafs and truns. Each run is
/// bounded by SAMPLE_LIMIT alone, so without this a segment of many small truns could still allocate without end.
pub(crate) const SAMPLES_PER_SEGMENT: usize = 1 << 20;

/// The descriptor tags inside an esds box.
pub(crate) const DESC_ES: u8 = 0x03;
pub(crate) const DESC_DECODER_CONFIG: u8 = 0x04;
pub(crate) const DESC_DECODER_SPECIFIC: u8 = 0x05;
/// The fixed bytes of an ES_Descriptor before its optional fields, and of a DecoderConfigDescriptor.
pub(crate) const ES_FIXED: usize = 3;
pub(crate) const DECODER_CONFIG_FIXED: usize = 13;
/// ES_Descriptor flag bits that add optional fields: stream dependence (2 bytes), URL (length byte), OCR (2 bytes).
pub(crate) const ES_STREAM_DEPENDENCE: u8 = 0x80;
pub(crate) const ES_URL: u8 = 0x40;
pub(crate) const ES_OCR: u8 = 0x20;
/// An AAC audio object type is the high five bits of the DecoderSpecificInfo's first byte.
pub(crate) const AOT_SHIFT: u32 = 3;
/** The bits of a 16.16 fixed-point field below the integer part. */
pub(crate) const FIXED_INTEGER_SHIFT: u32 = 16;
/// A descriptor length's continuation bit and value mask (one to four bytes).
pub(crate) const LENGTH_MORE: u8 = 0x80;
pub(crate) const LENGTH_MASK: u8 = 0x7f;
/// A descriptor's length field is at most four bytes; ES dependence and OCR fields are two bytes each.
pub(crate) const LENGTH_BYTES_MAX: usize = 4;
pub(crate) const ES_SKIP_BYTES: usize = 2;
