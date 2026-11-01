/*!
 * Numbers of the stream presenters: CoreMedia's timing and status conventions, how far ahead of the playhead media
 * is handed to the decoder and the renderer, and the AAC description the audio renderer is opened with, including
 * the MPEG-4 elementary stream descriptor layout the OS decoder reads its configuration from.
 */

/// Ticks per second of the timestamps handed to CoreMedia: fine enough for 90 kHz MPEG clocks.
pub const TIME_SCALE: i32 = 90_000;
/// CoreMedia's and VideoToolbox's OSStatus for success.
pub const NO_ERROR: i32 = 0;
/// How far ahead of the playhead samples are sent to the decoder and the audio renderer, in seconds.
pub const LOOKAHEAD: f64 = 1.0;
/// AudioFormatID of MPEG-4 AAC: the four characters `aac `.
pub const FORMAT_AAC: u32 = 0x6161_6320;
/// The audio object type of AAC-LC (`mp4a.40.2`), the one profile the renderer is opened for.
pub const AAC_LC: u8 = 2;
/// PCM frames in every AAC-LC access unit.
pub const AAC_FRAMES_PER_PACKET: u32 = 1024;
/// The audio object type sits in the top five bits of an AudioSpecificConfig's first byte.
pub const AOT_SHIFT: u32 = 3;
/// MPEG-4 descriptor tags: ES, DecoderConfig, DecoderSpecificInfo and SLConfig.
pub const DESC_ES: u8 = 0x03;
pub const DESC_DECODER_CONFIG: u8 = 0x04;
pub const DESC_DECODER_SPECIFIC: u8 = 0x05;
pub const DESC_SL_CONFIG: u8 = 0x06;
/// DecoderConfig's object type indication for MPEG-4 audio, and its stream type byte for an audio stream.
pub const OBJECT_TYPE_MPEG4_AUDIO: u8 = 0x40;
pub const STREAM_TYPE_AUDIO: u8 = 0x15;
/// DecoderConfig's buffer size (3 bytes) and maximum and average bit rates (4 bytes each), left at zero.
pub const DECODER_CONFIG_RATES: usize = 11;
/// SLConfig's predefined value for MP4 files.
pub const SL_MP4: u8 = 0x02;
/// A descriptor's length is written as four 7-bit groups, each but the last flagged with the high bit.
pub const LENGTH_GROUP_BITS: u32 = 7;
pub const LENGTH_GROUP_MASK: u32 = 0x7f;
pub const LENGTH_CONTINUES: u8 = 0x80;
pub const LENGTH_GROUPS: u32 = 4;
