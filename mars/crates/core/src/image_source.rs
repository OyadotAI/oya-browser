/** Image source text admission separates embedded payloads from network address limits. */
use crate::constants::{IMAGE_BYTES, IMAGE_TOTAL_BYTES, URL_BYTES};

/** Percent-escaped base64 needs at most four source bytes per decoded image byte. */
pub const DATA_URL_BYTES: usize = 4 * IMAGE_BYTES + URL_BYTES;
/** Image lifecycle text shares the existing page encoded-image budget. */
pub const SOURCE_TOTAL_BYTES: usize = IMAGE_TOTAL_BYTES;
/** One maximum inline source and its resolved URL fit a response-task batch. */
pub const REQUEST_TEXT_BYTES: usize = 2 * DATA_URL_BYTES;
/** URL normalization may percent-escape UTF-8; retained pending text remains independently bounded. */
pub const QUEUED_TEXT_BYTES: usize = 4 * SOURCE_TOTAL_BYTES;
const _: () = assert!(
    6 * REQUEST_TEXT_BYTES + crate::constants::VM_STATE_BYTES < crate::constants::FRAME_BYTES
);

/** This admits text only: native decoders still prove MIME, encoded size and decoded pixel bounds. */
pub fn valid_url_text(source: &str) -> bool {
    if source.len() <= URL_BYTES {
        return true;
    }
    if source.len() > DATA_URL_BYTES {
        return false;
    }
    let source = source.trim();
    source
        .get(..11)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("data:image/"))
        && source.find(',').is_some_and(|comma| comma <= URL_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_image_text_has_its_own_envelope_without_widening_network_addresses() {
        let payload = "A".repeat(URL_BYTES);
        assert!(valid_url_text(&format!("data:image/png;base64,{payload}")));
        assert!(valid_url_text(&format!("DATA:image/svg+xml,{payload}")));
        assert!(!valid_url_text(&format!("https://example.test/{payload}")));
        assert!(!valid_url_text(&format!("data:text/html,{payload}")));
    }

    #[test]
    fn embedded_metadata_and_payload_remain_bounded() {
        assert!(!valid_url_text(&format!(
            "data:image/png;{},A",
            "x".repeat(URL_BYTES)
        )));
        assert!(!valid_url_text(&format!(
            "data:image/png,{}",
            "A".repeat(DATA_URL_BYTES)
        )));
    }
}
