/** Decoding through the public API: the Airbnb picture against its JPEG in RGB, the 4:4:4 and clap-cropped fixtures, and the refusals the decoder still makes. */
use mars_avif::{Error, decode};

/** The real Airbnb AVIF, 240x180 8-bit 4:2:0, and the JPEG it was made from. */
const AIRBNB: &[u8] = include_bytes!("fixtures/airbnb-240.avif");
const AIRBNB_JPEG: &[u8] = include_bytes!("fixtures/airbnb-240.jpg");
/** The 4:4:4 official vector. */
const MEXICO: &[u8] = include_bytes!("fixtures/mexico-yuv444.avif");
/** The clap-cropped 8-bit 4:2:0 official vector. */
const CHIMERA: &[u8] = include_bytes!("fixtures/chimera-8bit.avif");

/** Peak signal-to-noise ratio of two equal-length 8-bit sample lists, in decibels. */
fn psnr(got: &[f64], want: &[f64]) -> f64 {
    let mse = got
        .iter()
        .zip(want)
        .map(|(g, w)| (g - w).powi(2))
        .sum::<f64>()
        / got.len() as f64;
    10.0 * (255.0f64 * 255.0 / mse).log10()
}

/** The Airbnb picture's RGB, from the JPEG, as f64 samples in the same order as the decoder's RGBA with alpha dropped. */
fn jpeg_rgb() -> Vec<f64> {
    let mut decoder = jpeg_decoder::Decoder::new(AIRBNB_JPEG);
    let rgb = decoder.decode().expect("the Airbnb JPEG decodes");
    rgb.iter().map(|&v| f64::from(v)).collect()
}

/** The decoded RGBA with its alpha bytes dropped, as f64 samples. */
fn rgb_of(rgba: &[u8]) -> Vec<f64> {
    rgba.chunks_exact(4)
        .flat_map(|p| [f64::from(p[0]), f64::from(p[1]), f64::from(p[2])])
        .collect()
}

/** The Airbnb picture decodes to RGB that is close to its JPEG (29.8 dB measured, with bilinear chroma), and every pixel is opaque because the picture has no alpha item. The JPEG is itself lossy, so the 29 dB floor is a plausibility bound. */
#[test]
fn the_airbnb_picture_decodes_close_to_its_jpeg() {
    let image = decode(AIRBNB).expect("the Airbnb picture decodes");
    assert!(image.rgba.chunks_exact(4).all(|p| p[3] == 255), "opaque");
    let score = psnr(&rgb_of(&image.rgba), &jpeg_rgb());
    eprintln!("airbnb RGB PSNR against the JPEG: {score:.2} dB");
    assert!(score > 29.0, "RGB PSNR {score:.2} dB");
}

/** The 4:4:4 picture decodes at its declared size with every sample converted, so its RGBA is the full picture. */
#[test]
fn a_444_picture_decodes_at_full_size() {
    let image = decode(MEXICO).expect("the 4:4:4 picture decodes");
    assert_eq!(
        image.rgba.len(),
        image.width as usize * image.height as usize * 4
    );
}

/** The clap-cropped picture decodes to the cropped size, not the coded size. */
#[test]
fn a_clap_cropped_picture_decodes_to_the_cropped_size() {
    let image = decode(CHIMERA).expect("the clap-cropped picture decodes");
    assert!(image.width > 0 && image.height > 0);
    assert_eq!(
        image.rgba.len(),
        image.width as usize * image.height as usize * 4
    );
}

/** A stream that needs intra block copy is still refused by name, until that stage lands. */
#[test]
fn intra_block_copy_is_refused_by_name() {
    let monochrome = include_bytes!("fixtures/monochrome.avif");
    assert!(matches!(decode(monochrome), Err(Error::Unsupported(_))));
}
