/** Container checks run without an OS decoder so malformed headers never reach native allocation. */
use super::*;
const GIF: &[u8] = include_bytes!("../../../tests/fixtures/canvas-images/first-frame.gif");
const WEBP: &[u8] = include_bytes!("../../../tests/fixtures/canvas-images/alpha.webp");

#[test]
fn gif_isolation_removes_later_frames_and_requires_a_complete_first_payload() {
    let isolated = gif::first_frame(GIF, 2, 2).unwrap();
    assert!(isolated.len() < GIF.len());
    assert_eq!(isolated.last(), Some(&0x3b));
    assert_eq!(gif::first_frame(&isolated, 2, 2).unwrap(), isolated);
    assert!(gif::first_frame(&isolated[..isolated.len() - 2], 2, 2).is_err());
    assert!(gif::first_frame(GIF, 1, 2).is_err());
}

#[test]
fn webp_admission_checks_compressed_dimensions_and_actual_container_length() {
    assert!(webp::validate(WEBP, 2, 2).is_ok());
    assert!(webp::validate(WEBP, 1, 2).is_err());
    let mut mismatched = WEBP.to_vec();
    mismatched[21] = 0xff;
    assert!(webp::validate(&mismatched, 2, 2).is_err());
    let mut truncated = WEBP.to_vec();
    truncated[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(webp::validate(&truncated, 2, 2).is_err());
}
