/**
 * Fragmented MP4 demuxing for Mars's Media Source path. It reads an initialization segment (moov: tracks, codecs,
 * edit lists, track defaults) and a media segment (moof and mdat: one sample table per track fragment) and reports
 * each sample's byte range, decode and presentation time, duration and sync flag. It does not decode anything.
 * It is std only, and every length read from untrusted bytes is bounded, so bad input returns an Error.
 */
mod boxes;
mod constants;
mod error;
mod fragment;
mod init;

pub use boxes::{BoxSpan, complete_boxes};
pub use error::{Error, Result};
pub use fragment::{Sample, parse_segment};
pub use init::{Track, TrackKind, parse_init};
