/*!
 * Media constants: the byte quotas a Media Source's buffers may hold, and how much media stays behind the playhead
 * before eviction takes it. Chrome's figures are the reference for the quotas: about 150 MB of video and 12 MB of
 * audio per SourceBuffer, which is what a page's append loop is written against.
 */

/// Video bytes one SourceBuffer may hold before an append is refused with QuotaExceededError.
pub const VIDEO_QUOTA_BYTES: usize = 150 * 1024 * 1024;
/// Audio bytes one SourceBuffer may hold before an append is refused with QuotaExceededError.
pub const AUDIO_QUOTA_BYTES: usize = 12 * 1024 * 1024;
/// Stored bytes across every SourceBuffer of every Media Source a player holds, so no page can grow the process without end.
pub const TOTAL_QUOTA_BYTES: usize = 512 * 1024 * 1024;
/// Media Source players one page may hold. Each holds up to TOTAL_QUOTA_BYTES, so this bounds the page's total.
pub const MAX_MSE_PLAYERS: usize = 4;
/// How much media stays behind the playhead before eviction takes it, in seconds.
pub const KEEP_BEHIND: f64 = 30.0;
/// The deepest reordering a decoder may hold: H.264 allows 16 reference frames to be reordered past one another.
pub const MAX_REORDER_DEPTH: usize = 16;
/// Decoded pictures a stream may hold waiting for their time, each a full BGRA frame: reorder depth plus a few.
pub const MAX_PENDING_PICTURES: usize = MAX_REORDER_DEPTH + 4;
/// The largest picture the stream decoder opens, the platform's 4K: a decoded BGRA picture of it is 38 MB,
/// and the SPS of a hostile stream can declare any size.
pub const MAX_PICTURE_WIDTH: u32 = 4096;
pub const MAX_PICTURE_HEIGHT: u32 = 2304;
/// Decoded bytes a stream may hold waiting for their time, beside the picture count: three 4K pictures.
pub const PENDING_PICTURE_BYTES: usize = 128 * 1024 * 1024;
