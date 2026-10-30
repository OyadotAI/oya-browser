/** Canonical dash state is finite, independently bounded and copied with drawing state. */
use super::*;

/** Iterable conversion has a fixed bound before odd lists are duplicated. */
pub const DASH_INPUTS: usize = 128;
/** Canonical even lists reserve room for duplication of an odd authored sequence. */
pub const DASH_VALUES: usize = DASH_INPUTS * 2;

/** Dash lengths and offset are measured in the user coordinate system at stroke time. */
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DashStyle {
    /** Alternating ink/gap lengths; an empty or all-zero list draws a solid stroke. */
    pub segments: Vec<f64>,
    /** Positive phase advances into the pattern; negative values wrap in the opposite direction. */
    pub offset: f64,
}

impl DashStyle {
    /** Restored state cannot introduce odd, unbounded, negative or nonfinite native dash sequences. */
    pub fn validate(&self) -> EngineResult<()> {
        if self.segments.len() > DASH_VALUES
            || self.segments.len() % 2 != 0
            || self
                .segments
                .iter()
                .any(|value| !value.is_finite() || *value < 0.0)
            || !self.offset.is_finite()
        {
            return Err(EngineError::invalid("Invalid canvas dash style."));
        }
        Ok(())
    }

    /** Sum overflow is a drawing-time numeric refusal, not an invalid individual setter value. */
    pub(super) fn total(&self) -> EngineResult<f64> {
        let total: f64 = self.segments.iter().sum();
        if !total.is_finite() {
            return Err(EngineError::new(
                "canvas_geometry",
                "Canvas dash period exceeds this build's numeric range.",
                "Use smaller dash lengths.",
            ));
        }
        Ok(total)
    }
}
