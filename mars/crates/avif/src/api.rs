/** The public decode surface: the Image type and the entry points that read the container, parse the AV1 headers, decode and filter each frame, and convert the picture to RGBA. */
use crate::container::{self, Picture};
use crate::error::{Error, Result};
use crate::frame::{FrameHeader, Parsed};
use crate::output::{self, Rect};
use crate::post;
use crate::recon::Plane;
use crate::sequence::Sequence;
use crate::stream;
use crate::tile;

/** A decoded picture: 8-bit RGBA, straight (not premultiplied) alpha, row-major, `width * height * 4` bytes. */
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/** The output width and height, read from the container alone; no coded samples are copied or decoded. */
pub fn dimensions(bytes: &[u8]) -> Result<(u32, u32)> {
    container::open(bytes)?.output_size()
}

/** Decode an AVIF file to 8-bit RGBA. The container, the AV1 headers and every tile are checked; the frame is then deblocked, deringed and restored, and the picture is converted with the colour information it carries. A clean-aperture crop with offsets, and a rotation or mirror, are refused as Unsupported. */
pub fn decode(bytes: &[u8]) -> Result<Image> {
    let picture = container::open(bytes)?;
    let rect = output_rect(&picture)?;
    if let Some(id) = picture.alpha {
        picture.payload(id)?;
    }
    let primary = decode_item(&picture, picture.primary)?;
    let alpha = match picture.alpha {
        Some(id) => Some(decode_item(&picture, id)?.planes),
        None => None,
    };
    let rgba = output::rgba(
        &primary.seq,
        rect,
        &primary.planes,
        alpha.as_ref().map(|planes| &planes[0]),
    )?;
    Ok(Image {
        width: rect.w as u32,
        height: rect.h as u32,
        rgba,
    })
}

/** One decoded item: its sequence header, the frame header that sizes it, and the planes after every post-filter. */
struct Decoded {
    seq: Sequence,
    planes: [Plane; 3],
}

/** Decode one coded item (the primary picture or the alpha picture): parse its headers, decode its tiles, and run the post-filters. */
fn decode_item(picture: &Picture<'_>, id: u32) -> Result<Decoded> {
    let payload = picture.payload(id)?;
    let headers = stream::parse_headers(&payload, &picture.av1c)?;
    match headers.frame {
        Parsed::ShowExisting(_) => Err(Error::Unsupported("show-existing frame")),
        Parsed::Frame(frame) => {
            check_frame_size(&frame, picture)?;
            let groups = stream::tile_groups(&payload, &frame)?;
            let mut state = tile::decode_frame(&headers.seq, &frame, &groups)?;
            let planes = post::filter(&headers.seq, &frame, &mut state);
            Ok(Decoded {
                seq: headers.seq,
                planes,
            })
        }
    }
}

/** The rectangle of the frame that is shown. Only the centred clean-aperture crop is handled; offsets and orientation transforms are not built yet, so those are refused rather than shown wrong. */
fn output_rect(picture: &Picture<'_>) -> Result<Rect> {
    if picture.rotation != 0 || picture.mirror.is_some() {
        return Err(Error::Unsupported("rotation or mirror"));
    }
    let (w, h) = (picture.width, picture.height);
    let Some(clap) = picture.clap else {
        return Ok(Rect {
            x: 0,
            y: 0,
            w: w as usize,
            h: h as usize,
        });
    };
    if clap.horiz_offset.0 != 0 || clap.vert_offset.0 != 0 {
        return Err(Error::Unsupported("clean aperture offset"));
    }
    let cw = container::cropped_side(w, clap.width)? as usize;
    let ch = container::cropped_side(h, clap.height)? as usize;
    Ok(Rect {
        x: centred_offset(w as usize, cw)?,
        y: centred_offset(h as usize, ch)?,
        w: cw,
        h: ch,
    })
}

/** The offset that centres a crop of `cropped` samples in a side of `side` samples. The crop never exceeds the side, so the subtraction is checked rather than trusted. */
fn centred_offset(side: usize, cropped: usize) -> Result<usize> {
    side.checked_sub(cropped)
        .map(|rest| rest / 2)
        .ok_or(Error::Malformed("cropped size"))
}

/** The frame must cover the picture the container declares; a mismatch means the item and its ispe disagree. Superres, which widens the frame after decoding, is not built, so a frame that uses it is refused. */
fn check_frame_size(frame: &FrameHeader, picture: &Picture<'_>) -> Result<()> {
    if frame.size.upscaled_width != picture.width || frame.size.frame_height != picture.height {
        return Err(Error::Malformed("frame size differs from ispe"));
    }
    if frame.size.frame_width != frame.size.upscaled_width {
        return Err(Error::Unsupported("superres"));
    }
    Ok(())
}
