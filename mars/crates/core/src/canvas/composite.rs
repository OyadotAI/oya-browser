/** Bounded Porter–Duff operators share one premultiplied calculation across every Canvas painter. */
use super::*;

/** Only operators that leave transparent-source pixels unchanged are admitted by current raster plans. */
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
            _ => return None,
        })
    }

    /** Factors multiply premultiplied source and destination rather than their straight color channels. */
    fn factors(self, source: f64, destination: f64) -> (f64, f64) {
        match self {
            Self::SourceOver => (1.0, 1.0 - source),
            Self::DestinationOver => (1.0 - destination, 1.0),
            Self::DestinationOut => (0.0, 1.0 - source),
            Self::SourceAtop => (destination, 1.0 - source),
            Self::Xor => (1.0 - destination, 1.0 - source),
        }
    }
}

/** Global alpha and joint geometry/clip coverage scale the source before one premultiplied composite. */
pub fn composite(pixel: &mut [u8], color: [u8; 4], alpha: f64, mode: Composite) {
    if pixel.len() != CHANNELS || !alpha.is_finite() {
        return;
    }
    let source_alpha = f64::from(color[3]) / 255.0 * alpha.clamp(0.0, 1.0);
    if source_alpha == 0.0 {
        return;
    }
    let destination_alpha = f64::from(pixel[3]) / 255.0;
    let (source_factor, destination_factor) = mode.factors(source_alpha, destination_alpha);
    let source = source_alpha * source_factor;
    let destination = destination_alpha * destination_factor;
    let combined = source + destination;
    if combined == 0.0 {
        pixel.fill(0);
        return;
    }
    for channel in 0..3 {
        pixel[channel] = ((f64::from(color[channel]) * source
            + f64::from(pixel[channel]) * destination)
            / combined)
            .round() as u8;
    }
    pixel[3] = (combined * 255.0).round() as u8;
}
