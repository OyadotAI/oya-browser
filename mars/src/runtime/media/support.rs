/**
 * What this machine's decoders play, asked of the OS once: AVFoundation answers for each
 * container and codec family (`isPlayableExtendedMIMEType`), so `canPlayType` and
 * `MediaSource.isTypeSupported` say "probably" only for what will actually play here.
 */
use crate::v9::MediaSupport;
use std::sync::OnceLock;

/// Containers a page may name, with one representative codec string per family to ask about.
const PROBES: &[(&str, &[(&str, &str)])] = &[
    (
        "video/mp4",
        &[
            ("avc1", "avc1.42E01E"),
            ("hvc1", "hvc1.1.6.L93.B0"),
            ("hev1", "hev1.1.6.L93.B0"),
            ("mp4a", "mp4a.40.2"),
            ("av01", "av01.0.04M.08"),
            ("vp09", "vp09.00.10.08"),
            ("ac-3", "ac-3"),
            ("ec-3", "ec-3"),
            ("opus", "opus"),
            ("flac", "flac"),
            ("alac", "alac"),
            ("mp3", "mp3"),
            ("mp4v", "mp4v.20.8"),
        ],
    ),
    (
        "audio/mp4",
        &[
            ("mp4a", "mp4a.40.2"),
            ("ac-3", "ac-3"),
            ("ec-3", "ec-3"),
            ("opus", "opus"),
            ("flac", "flac"),
            ("alac", "alac"),
            ("mp3", "mp3"),
        ],
    ),
    (
        "video/quicktime",
        &[
            ("avc1", "avc1.42E01E"),
            ("hvc1", "hvc1.1.6.L93.B0"),
            ("mp4a", "mp4a.40.2"),
        ],
    ),
    (
        "video/x-m4v",
        &[("avc1", "avc1.42E01E"), ("mp4a", "mp4a.40.2")],
    ),
    ("audio/x-m4a", &[("mp4a", "mp4a.40.2"), ("alac", "alac")]),
    ("audio/mpeg", &[("mp3", "mp3")]),
    ("audio/mp3", &[("mp3", "mp3")]),
    ("audio/aac", &[("mp4a", "mp4a.40.2")]),
    ("audio/wav", &[("1", "1")]),
    ("audio/x-wav", &[("1", "1")]),
    ("audio/aiff", &[]),
    ("audio/flac", &[("flac", "flac")]),
    (
        "video/webm",
        &[
            ("vp8", "vp8"),
            ("vp9", "vp9"),
            ("vp09", "vp09.00.10.08"),
            ("av01", "av01.0.04M.08"),
            ("opus", "opus"),
            ("vorbis", "vorbis"),
        ],
    ),
    ("audio/webm", &[("opus", "opus"), ("vorbis", "vorbis")]),
    (
        "audio/ogg",
        &[("opus", "opus"), ("vorbis", "vorbis"), ("flac", "flac")],
    ),
];

/// Containers a Media Source can stream on this host. None yet: the native player cannot be fed
/// appended segments, so `isTypeSupported` says no rather than accept media it cannot play.
const STREAMS: &[&str] = &[];

/** The probed support, computed on first use and kept for the process. */
pub fn support() -> MediaSupport {
    static SUPPORT: OnceLock<MediaSupport> = OnceLock::new();
    SUPPORT.get_or_init(probe).clone()
}

/** Ask the OS about every container and codec family. */
fn probe() -> MediaSupport {
    let mut support = MediaSupport::default();
    for (container, codecs) in PROBES {
        if !playable(container) {
            continue;
        }
        support.containers.push((*container).into());
        for (family, sample) in *codecs {
            if playable(&format!("{container}; codecs=\"{sample}\"")) {
                support.codecs.push(format!("{container}|{family}"));
            }
        }
        if STREAMS.contains(container) {
            support.streams.push((*container).into());
        }
    }
    support
}

/** AVFoundation's answer for one extended MIME type. */
#[cfg(target_os = "macos")]
fn playable(extended: &str) -> bool {
    use objc2_av_foundation::AVURLAsset;
    use objc2_foundation::NSString;
    // SAFETY: a class method taking an NSString; thread-safe and side-effect free.
    unsafe { AVURLAsset::isPlayableExtendedMIMEType(&NSString::from_str(extended)) }
}

/** Without the OS media frameworks nothing plays, so every answer is "". */
#[cfg(not(target_os = "macos"))]
fn playable(_extended: &str) -> bool {
    false
}
