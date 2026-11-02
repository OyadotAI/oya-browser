/** Bounded Porter–Duff operators share one premultiplied calculation across every Canvas painter. */
use super::*;

/** Porter–Duff modes declare whether transparent source outside geometry changes the destination. */
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Composite {
    /** Paint the source over the destination. */
    #[default]
    SourceOver,
    /** Paint the source behind the destination's existing alpha. */
    DestinationOver,
    /** Remove destination coverage in proportion to source alpha. */
    DestinationOut,
    /** Paint source color within destination alpha, retaining that destination alpha. */
    SourceAtop,
    /** Keep each image only where the other image is transparent. */
    Xor,
    /** Replace the output inside the current clip with the source, including transparent source. */
    Copy,
    /** Keep source only within destination alpha. */
    SourceIn,
    /** Keep source only outside destination alpha. */
    SourceOut,
    /** Keep destination only within source alpha. */
    DestinationIn,
    /** Keep destination over source, limited to source alpha. */
    DestinationAtop,
    /** Add premultiplied colors and alpha, clamping before applying fractional clipping. */
    Lighter,
}

impl Composite {
    /** Binding getters expose canonical names instead of unsupported expando strings. */
    pub fn name(self) -> &'static str {
        match self {
            Self::SourceOver => "source-over",
            Self::DestinationOver => "destination-over",
            Self::DestinationOut => "destination-out",
            Self::SourceAtop => "source-atop",
            Self::Xor => "xor",
            Self::Lighter => "lighter",
            Self::DestinationAtop => "destination-atop",
            Self::DestinationIn => "destination-in",
            Self::SourceOut => "source-out",
            Self::SourceIn => "source-in",
            Self::Copy => "copy",
        }
    }

    /** Unsupported operations remain unaccepted until every painter has their required semantics. */
    pub fn named(name: &str) -> Option<Self> {
        Some(match name {
            "source-over" => Self::SourceOver,
            "destination-over" => Self::DestinationOver,
            "destination-out" => Self::DestinationOut,
            "source-atop" => Self::SourceAtop,
            "xor" => Self::Xor,
            "lighter" => Self::Lighter,
            "destination-atop" => Self::DestinationAtop,
            "destination-in" => Self::DestinationIn,
            "source-out" => Self::SourceOut,
            "source-in" => Self::SourceIn,
            "copy" => Self::Copy,
            _ => return None,
        })
    }

    /** These modes composite transparent source across the entire clipped destination. */
    pub fn affects_uncovered(self) -> bool {
        matches!(
            self,
            Self::Copy
                | Self::SourceIn
                | Self::SourceOut
                | Self::DestinationIn
                | Self::DestinationAtop
        )
    }

    /** Factors multiply premultiplied source and destination rather than their straight color channels. */
    fn factors(self, source: f64, destination: f64) -> (f64, f64) {
        match self {
            Self::SourceOver => (1.0, 1.0 - source),
            Self::DestinationOver => (1.0 - destination, 1.0),
            Self::DestinationOut => (0.0, 1.0 - source),
            Self::SourceAtop => (destination, 1.0 - source),
            Self::Xor => (1.0 - destination, 1.0 - source),
            Self::Copy => (1.0, 0.0),
            Self::SourceIn => (destination, 0.0),
            Self::SourceOut => (1.0 - destination, 0.0),
            Self::DestinationIn => (0.0, source),
            Self::DestinationAtop => (1.0 - destination, source),
            Self::Lighter => (1.0, 1.0),
        }
    }
}

/** Fixed per-pixel arithmetic is charged independently from geometry and image sampling. */
pub const COMPOSITE_WORK: usize = 4;

/** Unclipped callers retain the stable public compositing entry point. */
pub fn composite(pixel: &mut [u8], color: [u8; 4], alpha: f64, mode: Composite) {
    composite_coverage(pixel, color, alpha, mode, 1.0, 1.0);
}

/** Joint geometry/clip coverage and clip-only coverage preserve transparent-source and saturating operators. */
pub fn composite_coverage(
    pixel: &mut [u8],
    color: [u8; 4],
    alpha: f64,
    mode: Composite,
    covered: f64,
    clip: f64,
) {
    if pixel.len() != CHANNELS || ![alpha, covered, clip].iter().all(|value| value.is_finite()) {
        return;
    }
    let clip = clip.clamp(0.0, 1.0);
    let covered = covered.clamp(0.0, clip);
    if clip == 0.0 || (covered == 0.0 && !mode.affects_uncovered()) {
        return;
    }
    let source_alpha = f64::from(color[3]) / 255.0 * alpha.clamp(0.0, 1.0);
    if source_alpha == 0.0 && !mode.affects_uncovered() {
        return;
    }
    let destination_alpha = f64::from(pixel[3]) / 255.0;
    let (source_factor, destination_factor) = mode.factors(source_alpha, destination_alpha);
    let source = source_alpha * source_factor;
    let destination = destination_alpha * destination_factor;
    let untouched = 1.0
        - if mode.affects_uncovered() {
            clip
        } else {
            covered
        };
    let combined = (source + destination).min(1.0) * covered + destination_alpha * untouched;
    if combined == 0.0 {
        pixel.fill(0);
        return;
    }
    for channel in 0..3 {
        let result = (f64::from(color[channel]) * source + f64::from(pixel[channel]) * destination)
            .min(255.0);
        pixel[channel] = ((result * covered
            + f64::from(pixel[channel]) * destination_alpha * untouched)
            / combined)
            .round() as u8;
    }
    pixel[3] = (combined * 255.0).round() as u8;
}

/** Composite a whole layer through one operator and clip; transparent layer pixels follow the operator's own rules. */
pub fn composite_layer(
    destination: &mut Bitmap,
    layer: &Bitmap,
    clip: Option<&ClipMask>,
    alpha: f64,
    mode: Composite,
) -> EngineResult<()> {
    destination.validate()?;
    layer.validate()?;
    if (destination.width, destination.height) != (layer.width, layer.height) {
        return Err(EngineError::invalid(
            "Canvas layer size differs from its target.",
        ));
    }
    let width = destination.width as usize;
    let pixels = destination.pixels.make_mut()?;
    for (index, (pixel, source)) in pixels
        .chunks_exact_mut(CHANNELS)
        .zip(layer.pixels.chunks_exact(CHANNELS))
        .enumerate()
    {
        let (x, y) = ((index % width) as u32, (index / width) as u32);
        let clip_coverage = clip.map_or(1.0, |clip| clip.coverage(x, y));
        let color = [source[0], source[1], source[2], source[3]];
        composite_coverage(pixel, color, alpha, mode, 1.0, clip_coverage);
    }
    Ok(())
}
