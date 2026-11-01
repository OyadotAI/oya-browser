/** Header tests on real files: the sequence and frame headers of each fixture must agree with the container's ispe and av1C. */
use crate::av1::KEY_FRAME;
use crate::container::{self, Picture};
use crate::frame::Parsed;
use crate::stream::{self, Headers};

/** The Airbnb CDN AVIF: 240x180, 8-bit 4:2:0. */
const AIRBNB: &[u8] = include_bytes!("../tests/fixtures/airbnb-240.avif");
/** Official AVIF test vectors: a clap-cropped 8-bit 4:2:0 file, 4:4:4, monochrome, and a still picture. */
const CHIMERA_8BIT: &[u8] = include_bytes!("../tests/fixtures/chimera-8bit.avif");
const MEXICO_444: &[u8] = include_bytes!("../tests/fixtures/mexico-yuv444.avif");
const MONOCHROME: &[u8] = include_bytes!("../tests/fixtures/monochrome.avif");
const STILL: &[u8] = include_bytes!("../tests/fixtures/still-picture.avif");

/** The layout a fixture is expected to carry: bit depth, monochrome, and the two chroma subsampling flags. */
#[derive(Clone, Copy)]
struct Layout {
    bit_depth: u8,
    mono: bool,
    ss_x: bool,
    ss_y: bool,
}

/** Parse a fixture's container and headers, returning the picture and the headers. */
fn open(bytes: &[u8]) -> (Picture<'_>, Headers) {
    let picture = container::open(bytes).unwrap();
    let payload = picture.payload(picture.primary).unwrap();
    let headers = stream::parse_headers(&payload, &picture.av1c).unwrap();
    (picture, headers)
}

/** Assert one fixture's headers agree with its ispe, its av1C and the expected layout. */
fn check(bytes: &[u8], expect: Layout) {
    let (picture, h) = open(bytes);
    let Parsed::Frame(frame) = &h.frame else {
        panic!("a still picture leads with a frame, not show-existing");
    };
    assert_eq!(
        frame.size.upscaled_width, picture.width,
        "width against ispe"
    );
    assert_eq!(
        frame.size.frame_height, picture.height,
        "height against ispe"
    );
    assert_eq!(frame.frame_type, KEY_FRAME);
    assert!(frame.show_frame);
    assert_eq!(h.seq.color.bit_depth, expect.bit_depth, "bit depth");
    assert_eq!(h.seq.color.mono_chrome, expect.mono, "monochrome");
    assert_eq!(h.seq.color.subsampling_x, expect.ss_x, "subsampling x");
    assert_eq!(h.seq.color.subsampling_y, expect.ss_y, "subsampling y");
    assert_eq!(picture.av1c.monochrome, expect.mono, "av1C monochrome");
    assert_eq!(
        picture.av1c.subsampling_x, expect.ss_x,
        "av1C subsampling x"
    );
    assert_eq!(
        picture.av1c.subsampling_y, expect.ss_y,
        "av1C subsampling y"
    );
    assert_eq!(
        picture.av1c.high_bitdepth,
        expect.bit_depth > 8,
        "av1C high bitdepth"
    );
}

#[test]
fn the_airbnb_headers_match_its_container() {
    check(
        AIRBNB,
        Layout {
            bit_depth: 8,
            mono: false,
            ss_x: true,
            ss_y: true,
        },
    );
    let (picture, _) = open(AIRBNB);
    assert_eq!((picture.width, picture.height), (240, 180));
}

#[test]
fn the_clap_cropped_vector_headers_match_its_container() {
    check(
        CHIMERA_8BIT,
        Layout {
            bit_depth: 8,
            mono: false,
            ss_x: true,
            ss_y: true,
        },
    );
}

#[test]
fn the_444_vector_headers_match_its_container() {
    check(
        MEXICO_444,
        Layout {
            bit_depth: 8,
            mono: false,
            ss_x: false,
            ss_y: false,
        },
    );
}

#[test]
fn the_monochrome_vector_headers_match_its_container() {
    check(
        MONOCHROME,
        Layout {
            bit_depth: 8,
            mono: true,
            ss_x: true,
            ss_y: true,
        },
    );
}

#[test]
fn the_still_picture_vector_headers_match_its_container() {
    check(
        STILL,
        Layout {
            bit_depth: 8,
            mono: false,
            ss_x: true,
            ss_y: true,
        },
    );
}

#[test]
fn the_frame_header_ends_inside_the_frame_obu_payload() {
    let (picture, h) = open(AIRBNB);
    let payload = picture.payload(picture.primary).unwrap();
    let Parsed::Frame(frame) = h.frame else {
        panic!("expected a frame header");
    };
    assert!(frame.header_bytes > 0 && frame.header_bytes < payload.len());
}
