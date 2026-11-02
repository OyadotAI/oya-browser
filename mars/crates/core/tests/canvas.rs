/** Graphics contracts reject forged storage and preserve transactional candidate pixels. */
use mars_core::canvas::{Bitmap, EDGE, PAGE_BYTES};

#[test]
fn dimensions_preserve_reflected_integer_rules() {
    for (text, expected) in [
        ("4junk", Some(4)),
        (" +7tail", Some(7)),
        ("-0", Some(0)),
        ("-1", None),
        ("-+0", None),
        ("2147483648", None),
        ("", None),
    ] {
        assert_eq!(mars_core::canvas::dimension(text), expected, "{text}");
    }
}

#[test]
fn excessive_dimensions_never_overflow_or_allocate() {
    let bitmap = Bitmap::new(u32::MAX, u32::MAX).unwrap();
    assert!(bitmap.pixels.is_empty());
    assert!(bitmap.validate().is_ok());
}
#[test]
fn forged_lengths_are_rejected_before_painting() {
    let bitmap: Bitmap =
        serde_json::from_str("{\"width\":1,\"height\":1,\"pixels\":[0,0,0]}").unwrap();
    assert!(bitmap.validate().is_err());
}
#[test]
fn serialized_pixel_arrays_are_bounded_during_decode() {
    let count = EDGE as usize * EDGE as usize * 4 + 1;
    let payload = format!(
        "{{\"width\":512,\"height\":512,\"pixels\":[{}0]}}",
        "0,".repeat(count - 1)
    );
    assert!(serde_json::from_str::<Bitmap>(&payload).is_err());
}
#[test]
fn maximum_page_pixels_fit_the_existing_vm_wire_budget() {
    let bitmap = Bitmap::new(EDGE, EDGE).unwrap();
    let surfaces = vec![bitmap; PAGE_BYTES / (EDGE as usize * EDGE as usize * 4)];
    assert!(serde_json::to_vec(&surfaces).unwrap().len() < mars_core::constants::VM_STATE_BYTES);
}
