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
/// How much media stays behind the playhead before eviction takes it, in seconds.
pub const KEEP_BEHIND: f64 = 30.0;
