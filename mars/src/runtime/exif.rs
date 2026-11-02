/*! JPEG EXIF orientation (`image-orientation: from-image`, the CSS initial value): a camera
writes its pixels unrotated and records the turn in tag 0x0112 of the APP1 `Exif` block. Chrome
shows the picture upright, so layout swaps the natural size of a quarter-turned image and the
painter turns the pixels.
ponytail: `image-orientation: none` is not read; every JPEG is shown upright. */

/** JPEG marker introducing a segment. */
const MARKER: u8 = 0xff;
/** The APP1 segment that holds EXIF data. */
const APP1: u8 = 0xe1;
/** Start of scan: segments with metadata come before it. */
const SOS: u8 = 0xda;
/** The EXIF header inside APP1. */
const EXIF: &[u8] = b"Exif\0\0";
/** TIFF byte order marks. */
const LITTLE: &[u8] = b"II";
/** Big-endian TIFF byte order. */
const BIG: &[u8] = b"MM";
/** The orientation tag. */
const ORIENTATION: u16 = 0x0112;
/** Bytes per IFD entry. */
const ENTRY: usize = 12;
/** Orientation values 5 to 8 turn the picture a quarter, swapping width and height. */
const FIRST_QUARTER_TURN: u8 = 5;
/** The highest orientation value. */
const LAST: u8 = 8;

/** The EXIF orientation (1 to 8) of JPEG `bytes`; 1, upright, when there is none. */
pub(super) fn orientation(bytes: &[u8]) -> u8 {
    exif_block(bytes)
        .and_then(tiff_orientation)
        .filter(|value| (1..=LAST).contains(value))
        .unwrap_or(1)
}

/** Whether an orientation turns the picture a quarter, so its width and height swap. */
pub(super) fn swaps_axes(orientation: u8) -> bool {
    orientation >= FIRST_QUARTER_TURN
}

/** The TIFF data of the first APP1 `Exif` segment, before the scan starts. */
fn exif_block(bytes: &[u8]) -> Option<&[u8]> {
    let mut at = 2;
    while bytes.get(at) == Some(&MARKER) {
        let kind = *bytes.get(at + 1)?;
        if kind == SOS {
            return None;
        }
        let length = usize::from(u16::from_be_bytes([
            *bytes.get(at + 2)?,
            *bytes.get(at + 3)?,
        ]));
        let body = bytes.get(at + 4..at + 2 + length)?;
        if kind == APP1 && body.starts_with(EXIF) {
            return Some(&body[EXIF.len()..]);
        }
        at += 2 + length;
    }
    None
}

/** The orientation tag of IFD0 in a TIFF block. */
fn tiff_orientation(tiff: &[u8]) -> Option<u8> {
    let little = match tiff.get(..2)? {
        LITTLE => true,
        BIG => false,
        _ => return None,
    };
    let u16_at = |at: usize| {
        let pair = [*tiff.get(at)?, *tiff.get(at + 1)?];
        Some(if little {
            u16::from_le_bytes(pair)
        } else {
            u16::from_be_bytes(pair)
        })
    };
    let u32_at = |at: usize| {
        let quad = [
            *tiff.get(at)?,
            *tiff.get(at + 1)?,
            *tiff.get(at + 2)?,
            *tiff.get(at + 3)?,
        ];
        Some(if little {
            u32::from_le_bytes(quad)
        } else {
            u32::from_be_bytes(quad)
        })
    };
    let ifd = usize::try_from(u32_at(4)?).ok()?;
    let count = usize::from(u16_at(ifd)?);
    (0..count)
        .map(|index| ifd + 2 + index * ENTRY)
        .find(|entry| u16_at(*entry) == Some(ORIENTATION))
        .and_then(|entry| u16_at(entry + 8))
        .and_then(|value| u8::try_from(value).ok())
}

/** `rgba` pixels of a `width` by `height` image shown upright for `orientation`: returns the
turned pixels with their new width and height. */
pub(super) fn upright(
    rgba: &[u8],
    width: u32,
    height: u32,
    orientation: u8,
) -> (Vec<u8>, u32, u32) {
    let (w, h) = (width as usize, height as usize);
    let (out_w, out_h) = if swaps_axes(orientation) {
        (h, w)
    } else {
        (w, h)
    };
    let mut out = vec![0; rgba.len()];
    for y in 0..out_h {
        for x in 0..out_w {
            let (sx, sy) = source(orientation, x, y, w, h);
            let from = (sy * w + sx) * 4;
            let to = (y * out_w + x) * 4;
            out[to..to + 4].copy_from_slice(&rgba[from..from + 4]);
        }
    }
    (out, out_w as u32, out_h as u32)
}

/** The stored pixel shown at upright (`x`, `y`) for an orientation of a `w` by `h` image. */
fn source(orientation: u8, x: usize, y: usize, w: usize, h: usize) -> (usize, usize) {
    match orientation {
        2 => (w - 1 - x, y),
        3 => (w - 1 - x, h - 1 - y),
        4 => (x, h - 1 - y),
        5 => (y, x),
        6 => (y, h - 1 - x),
        7 => (w - 1 - y, h - 1 - x),
        8 => (w - 1 - y, x),
        _ => (x, y),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /** A JPEG start with one APP1 EXIF segment holding orientation `value`, big-endian. */
    fn jpeg_with(value: u8) -> Vec<u8> {
        let mut tiff = b"MM\0\x2a\0\0\0\x08".to_vec();
        tiff.extend([0, 1, 0x01, 0x12, 0, 3, 0, 0, 0, 1, 0, value, 0, 0, 0, 0]);
        let mut body = EXIF.to_vec();
        body.extend(tiff);
        let length = (body.len() + 2) as u16;
        let mut bytes = vec![0xff, 0xd8, MARKER, APP1];
        bytes.extend(length.to_be_bytes());
        bytes.extend(body);
        bytes.extend([MARKER, SOS]);
        bytes
    }

    /** The orientation tag is read from the EXIF block; a JPEG without one is upright. */
    #[test]
    fn reads_the_exif_orientation() {
        assert_eq!(orientation(&jpeg_with(6)), 6);
        assert_eq!(orientation(&[0xff, 0xd8, MARKER, SOS]), 1);
        assert!(swaps_axes(6) && !swaps_axes(3));
    }

    /** Orientation 6 (the camera turned right) is shown turned a quarter clockwise. */
    #[test]
    fn orientation_six_turns_a_quarter_clockwise() {
        // Two pixels side by side, red then blue, become a column with red on top.
        let rgba = [255, 0, 0, 255, 0, 0, 255, 255];
        let (out, w, h) = upright(&rgba, 2, 1, 6);
        assert_eq!((w, h), (1, 2));
        assert_eq!(&out[..4], &[255, 0, 0, 255]);
    }
}
