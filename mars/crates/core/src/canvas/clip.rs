/** Immutable sample masks preserve exact intersection on the rasterizer's bounded antialiasing grid. */
use super::{EDGE, Pixels};
use crate::protocol::{EngineError, EngineResult};
use serde::{Deserialize, Serialize};

/** The path and clipped-rectangle rasterizers share this sample grid. */
pub(crate) const GRID: usize = 4;
/** Every pixel retains individual samples so repeated intersection cannot darken a fractional edge. */
pub(crate) const SAMPLES: usize = GRID * GRID;
/** A sixteen-sample mask is encoded as two little-endian bytes per pixel. */
const BYTES: usize = std::mem::size_of::<u16>();

/** Clones share immutable, process-accounted storage; public users cannot mutate admitted masks. */
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClipMask {
    width: u32,
    height: u32,
    samples: Pixels,
}

impl ClipMask {
    /** Validate dimensions and exact storage before any sampling or saved-state restoration. */
    pub fn validate(&self, size: [u32; 2]) -> EngineResult<()> {
        if size != [self.width, self.height] || Self::length(size) != Some(self.samples.len()) {
            return Err(EngineError::invalid("Invalid canvas clipping mask."));
        }
        Ok(())
    }

    /** Retained page admission counts storage once per mask-pool entry, not once per saved reference. */
    pub fn bytes(&self) -> usize {
        self.samples.len()
    }

    /** Native glyph ink supplies pixel coverage, which is multiplied by the current clipping coverage. */
    pub fn coverage(&self, x: u32, y: u32) -> f64 {
        f64::from(self.bits(x, y).count_ones()) / SAMPLES as f64
    }

    /** Size is known before allocation and obeys the same canvas edge bound as every painter. */
    pub(crate) fn length([width, height]: [u32; 2]) -> Option<usize> {
        if width > EDGE || height > EDGE {
            return None;
        }
        Some(width as usize * height as usize * BYTES)
    }

    /** Uncovered samples start at zero; allocation failure occurs before any clipping state changes. */
    pub(crate) fn zeroed(size: [u32; 2]) -> EngineResult<Self> {
        let length = Self::length(size)
            .ok_or_else(|| EngineError::invalid("Invalid canvas clip dimensions."))?;
        Ok(Self {
            width: size[0],
            height: size[1],
            samples: Pixels::zeroed(length)?,
        })
    }

    /** Outside the admitted mask no samples are visible, including degenerate empty canvases. */
    pub(crate) fn bits(&self, x: u32, y: u32) -> u16 {
        if self.width > EDGE || self.height > EDGE || x >= self.width || y >= self.height {
            return 0;
        }
        let at = (y as usize * self.width as usize + x as usize) * BYTES;
        self.samples
            .get(at..at + BYTES)
            .map_or(0, |bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    /** Only a fresh mask under construction can obtain mutable backing. */
    pub(crate) fn storage(&mut self) -> EngineResult<&mut [u8]> {
        self.samples.make_mut()
    }

    /** Encode one admitted sample word without retaining platform-specific integer storage. */
    pub(crate) fn write(storage: &mut [u8], width: u32, x: u32, y: u32, bits: u16) {
        let at = (y as usize * width as usize + x as usize) * BYTES;
        storage[at..at + BYTES].copy_from_slice(&bits.to_le_bytes());
    }
}

/** Shared sample centers keep masks, paths and clipped rectangles aligned exactly. */
pub(crate) fn offset(sample: usize) -> f64 {
    (sample as f64 + 0.5) / GRID as f64
}
