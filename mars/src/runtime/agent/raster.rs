/**
 * page.screenshot without a window: a PNG painted from the same display list the native window
 * draws, so a headless or attached agent can see layout. It paints boxes, borders, images and
 * text runs as bars (there is no font rasterizer outside AppKit); the desktop window answers
 * page.screenshot itself with real pixels.
 */
use super::super::supervisor::Runtime;
use crate::document::{DisplayItem, agent_visible};
use crate::protocol::{EngineError, EngineResult, parameters};
use serde::Deserialize;
use serde_json::{Value, json};

/// Longest edge a screenshot may have, bounding its memory to a few megabytes.
const SHOT_EDGE: f64 = 2048.0;
/// Text bars cover the middle of each line box, roughly where glyph ink sits.
const INK_TOP: f64 = 0.25;
/// Share of the font size a text bar is tall.
const INK_HEIGHT: f64 = 0.55;
/// Images paint as a neutral placeholder of their box.
const IMAGE_FILL: [u8; 4] = [0xd0, 0xd4, 0xda, 0xff];
/// The canvas when the page names no background.
const WHITE: [u8; 4] = [0xff, 0xff, 0xff, 0xff];
/// Text and borders when the page names no color.
const BLACK: [u8; 4] = [0, 0, 0, 0xff];

/** Optionally clip the shot to one element's box. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Shot {
    #[serde(default)]
    node_id: Option<String>,
}

/** A window onto document coordinates, one pixel per logical point. */
struct Canvas {
    left: f64,
    top: f64,
    width: usize,
    height: usize,
    pixels: Vec<u8>,
}

impl Canvas {
    /** A blank RGB canvas over a document rectangle. */
    fn new(left: f64, top: f64, width: f64, height: f64, background: [u8; 4]) -> Self {
        let (width, height) = (
            width.clamp(1.0, SHOT_EDGE) as usize,
            height.clamp(1.0, SHOT_EDGE) as usize,
        );
        let pixels = background[..3].repeat(width * height);
        Self {
            left,
            top,
            width,
            height,
            pixels,
        }
    }
    /** Blend one color over a document rectangle, clipped to the canvas and an optional clip. */
    fn fill(&mut self, rect: [f64; 4], clip: Option<[f64; 4]>, color: [u8; 4]) {
        let [mut x0, mut y0, mut x1, mut y1] =
            [rect[0], rect[1], rect[0] + rect[2], rect[1] + rect[3]];
        if let Some([cx, cy, cw, ch]) = clip {
            (x0, y0, x1, y1) = (x0.max(cx), y0.max(cy), x1.min(cx + cw), y1.min(cy + ch));
        }
        let column = |x: f64, edge: usize| ((x - self.left).round().max(0.0) as usize).min(edge);
        let row = |y: f64, edge: usize| ((y - self.top).round().max(0.0) as usize).min(edge);
        let alpha = u32::from(color[3]);
        for y in row(y0, self.height)..row(y1, self.height) {
            for x in column(x0, self.width)..column(x1, self.width) {
                let at = (y * self.width + x) * 3;
                for (pixel, over) in self.pixels[at..at + 3].iter_mut().zip(color) {
                    let (under, over) = (u32::from(*pixel), u32::from(over));
                    *pixel = ((over * alpha + under * (255 - alpha)) / 255) as u8;
                }
            }
        }
    }
    /** Paint one display item: background, border, image placeholder and text runs. */
    fn item(&mut self, item: &DisplayItem, foreground: [u8; 4]) {
        let clip = item
            .clip
            .map(|clip| [clip.x, clip.y, clip.width, clip.height]);
        let fade = |mut color: [u8; 4]| {
            color[3] = (f64::from(color[3]) * item.opacity.clamp(0.0, 1.0)) as u8;
            color
        };
        let bounds = [item.x, item.y, item.width, item.height];
        if let Some(color) = item.background.as_deref().and_then(rgba) {
            self.fill(bounds, clip, fade(color));
        }
        if item.tag == "img" {
            self.fill(bounds, clip, fade(IMAGE_FILL));
        }
        self.border(
            item,
            clip,
            fade(item.border_color.as_deref().and_then(rgba).unwrap_or(BLACK)),
        );
        let ink = fade(item.color.as_deref().and_then(rgba).unwrap_or(foreground));
        for line in &item.lines {
            let top = line.y + item.font_size * INK_TOP;
            self.fill(
                [line.x, top, line.width, item.font_size * INK_HEIGHT],
                clip,
                ink,
            );
        }
    }
    /** Uniform borders paint as four strips inside the box. */
    fn border(&mut self, item: &DisplayItem, clip: Option<[f64; 4]>, color: [u8; 4]) {
        let [top, right, bottom, left] = if item.border > 0.0 {
            [item.border; 4]
        } else {
            item.border_widths
        };
        let (x, y, w, h) = (item.x, item.y, item.width, item.height);
        for strip in [
            [x, y, w, top],
            [x + w - right, y, right, h],
            [x, y + h - bottom, w, bottom],
            [x, y, left, h],
        ] {
            if strip[2] > 0.0 && strip[3] > 0.0 {
                self.fill(strip, clip, color);
            }
        }
    }
    /** Encode as an 8-bit RGB PNG. */
    fn png(&self) -> EngineResult<Vec<u8>> {
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, self.width as u32, self.height as u32);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .and_then(|mut writer| writer.write_image_data(&self.pixels))
            .map_err(|_| EngineError::invalid("The screenshot could not be encoded."))?;
        Ok(bytes)
    }
}

impl Runtime {
    /** page.screenshot: the visible viewport, or one element's box, as an inline PNG. */
    pub(super) fn screenshot(&mut self, params: Value) -> EngineResult<Value> {
        let input: Shot = parameters(params)?;
        let page = self.current()?;
        let display = page.display.as_ref().ok_or_else(|| {
            EngineError::new("no_page", "The page has no layout yet.", "Navigate first.")
        })?;
        let mut area = [
            0.0,
            self.scroll_y,
            self.render_context.width,
            self.render_context.height,
        ];
        if let Some(node_id) = &input.node_id {
            let item = display
                .items
                .iter()
                .find(|item| &page.node_id(item.local_id) == node_id)
                .ok_or_else(|| {
                    EngineError::new(
                        "invalid_target",
                        "No painted box has that node_id.",
                        "Use a node_id from page_find on the current page.",
                    )
                })?;
            area = [item.x, item.y, item.width, item.height];
        }
        let background = display
            .canvas_background
            .as_deref()
            .and_then(rgba)
            .unwrap_or(WHITE);
        let foreground = display
            .canvas_foreground
            .as_deref()
            .and_then(rgba)
            .unwrap_or(BLACK);
        let mut canvas = Canvas::new(area[0], area[1], area[2], area[3], background);
        for item in display.items.iter().filter(|item| agent_visible(item)) {
            canvas.item(item, foreground);
        }
        let png = canvas.png()?;
        Ok(
            json!({"page_id":page.id(),"revision":page.revision,"layout_generation":page.layout_generation,"x":area[0],"y":area[1],"width":canvas.width,"height":canvas.height,"unit":"logical_points","render":"wireframe","mime":"image/png","png_base64":base64(&png)}),
        )
    }
}

/** A painter literal as RGBA: a palette name, `#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa`. */
fn rgba(value: &str) -> Option<[u8; 4]> {
    let hex = match value {
        "black" => "000000",
        "white" => "ffffff",
        "red" => "ff0000",
        "green" => "008000",
        "blue" => "0000ff",
        "gray" | "grey" => "808080",
        "navy" => "000080",
        "teal" => "008080",
        "transparent" => "00000000",
        other => other.strip_prefix('#')?,
    };
    if !hex.is_ascii() {
        return None;
    }
    let hex = if hex.len() <= 4 {
        hex.chars().flat_map(|c| [c, c]).collect()
    } else {
        hex.to_owned()
    };
    let hex = if hex.len() == 6 { hex + "ff" } else { hex };
    if hex.len() != 8 {
        return None;
    }
    let byte = |at: usize| u8::from_str_radix(&hex[at..at + 2], 16).ok();
    Some([byte(0)?, byte(2)?, byte(4)?, byte(6)?])
}

/** Standard padded base64, the encoding MCP image content uses. */
pub(crate) fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, b)| n | u32::from(*b) << (16 - 8 * i));
        for i in 0..4 {
            out.push(if i <= chunk.len() {
                ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char
            } else {
                '='
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{base64, rgba};

    /** Base64 matches the RFC 4648 test vectors, padding included. */
    #[test]
    fn base64_matches_rfc_vectors() {
        for (input, output) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(input.as_bytes()), output);
        }
    }

    /** Painter literals parse in every spelling the display list carries. */
    #[test]
    fn colors_parse_names_and_hex() {
        assert_eq!(rgba("#fff"), Some([255, 255, 255, 255]));
        assert_eq!(rgba("#10203040"), Some([16, 32, 48, 64]));
        assert_eq!(rgba("navy"), Some([0, 0, 128, 255]));
        assert_eq!(rgba("rgb(1,2,3)"), None);
    }
}
