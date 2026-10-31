/** Media types as AVFoundation names them: a response's MIME type to the uniform type identifier its resource loader expects. */

/// MIME essences and the container type identifier AVFoundation parses them as.
const TYPES: &[(&str, &str)] = &[
    ("video/mp4", "public.mpeg-4"),
    ("audio/mp4", "public.mpeg-4-audio"),
    ("audio/x-m4a", "com.apple.m4a-audio"),
    ("video/x-m4v", "com.apple.m4v-video"),
    ("video/quicktime", "com.apple.quicktime-movie"),
    ("audio/mpeg", "public.mp3"),
    ("audio/mp3", "public.mp3"),
    ("audio/aac", "public.aac-audio"),
    ("audio/wav", "com.microsoft.waveform-audio"),
    ("audio/x-wav", "com.microsoft.waveform-audio"),
    ("audio/wave", "com.microsoft.waveform-audio"),
    ("audio/aiff", "public.aiff-audio"),
    ("audio/x-aiff", "public.aiff-audio"),
    ("audio/flac", "org.xiph.flac"),
    ("video/webm", "org.webmproject.webm"),
    ("audio/webm", "org.webmproject.webm"),
];
/// What an unlabeled or generically labeled resource is assumed to be: MP4 covers most video.
const FALLBACK: &str = "public.mpeg-4";

/** The type identifier for a MIME type, MP4 when the server sent none AVFoundation knows. */
pub(super) fn uti(mime: &str) -> &'static str {
    let essence = mime
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    TYPES
        .iter()
        .find(|(name, _)| *name == essence)
        .map_or(FALLBACK, |(_, uti)| uti)
}

#[cfg(test)]
mod tests {
    /** Known types map exactly, parameters are ignored, and unknown types fall back to MP4. */
    #[test]
    fn mime_types_map_to_container_identifiers() {
        assert_eq!(super::uti("video/mp4"), "public.mpeg-4");
        assert_eq!(super::uti("Audio/MPEG; charset=binary"), "public.mp3");
        assert_eq!(super::uti("application/octet-stream"), "public.mpeg-4");
    }
}
