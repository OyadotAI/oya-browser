/** Canvas gradients: linear, radial and conic geometry with color stops, evaluated in paint space. */
use super::{GRADIENT_STOPS, TRANSPARENT};
use crate::protocol::{EngineError, EngineResult};
use serde::{Deserialize, Serialize};
use std::f64::consts::TAU;

/** Geometry is stored in the coordinates current at creation; the drawing transform applies at paint time. */
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Geometry {
    /** The color parameter is the projection of a point onto the axis from `from` to `to`. */
    Linear { from: [f64; 2], to: [f64; 2] },
    /** Two circles given as center and radius; the parameter interpolates between them. */
    Radial { start: [f64; 3], end: [f64; 3] },
    /** Angle is clockwise from twelve o'clock, offset by `start` radians, and wraps once per turn. */
    Conic { center: [f64; 2], start: f64 },
}

impl Geometry {
    /** Every coordinate must be finite and radii nonnegative, or painting would read garbage. */
    fn valid(&self) -> bool {
        match self {
            Self::Linear { from, to } => finite(from) && finite(to),
            Self::Radial { start, end } => {
                finite(&start[..]) && finite(&end[..]) && start[2] >= 0.0 && end[2] >= 0.0
            }
            Self::Conic { center, start } => finite(center) && start.is_finite(),
        }
    }
}

/** A color stop at an offset in the unit interval, colored with straight-alpha sRGB. */
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stop {
    /** Position along the gradient parameter, validated to [0, 1] by the caller. */
    pub offset: f64,
    /** The color at exactly this offset. */
    pub color: [u8; 4],
}

/** A geometry plus stops kept sorted by offset; equal offsets keep insertion order. */
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gradient {
    /** Where the color parameter comes from. */
    pub geometry: Geometry,
    /** Sorted stops; an empty list paints nothing. */
    pub stops: Vec<Stop>,
}

impl Gradient {
    /** A new gradient has no stops, so it paints nothing until a stop is added. */
    pub fn new(geometry: Geometry) -> Self {
        Self {
            geometry,
            stops: Vec::new(),
        }
    }

    /** Insert after existing equal offsets, so a later stop at the same offset wins past that point. */
    pub fn add_stop(&mut self, offset: f64, color: [u8; 4]) -> EngineResult<()> {
        if !(0.0..=1.0).contains(&offset) || self.stops.len() >= GRADIENT_STOPS {
            return Err(EngineError::invalid("Invalid canvas gradient stop."));
        }
        let index = self.stops.partition_point(|stop| stop.offset <= offset);
        self.stops.insert(index, Stop { offset, color });
        Ok(())
    }

    /** Serialized gradients must satisfy the same bounds as live insertions. */
    pub fn validate(&self) -> EngineResult<()> {
        let sorted = self
            .stops
            .windows(2)
            .all(|pair| pair[0].offset <= pair[1].offset);
        let offsets = self
            .stops
            .iter()
            .all(|stop| (0.0..=1.0).contains(&stop.offset));
        if !self.geometry.valid() || self.stops.len() > GRADIENT_STOPS || !sorted || !offsets {
            return Err(EngineError::invalid("Invalid canvas gradient."));
        }
        Ok(())
    }

    /** The color a point in paint space reads; points where the parameter is undefined paint nothing. */
    pub fn color_at(&self, point: [f64; 2]) -> [u8; 4] {
        let Some(t) = self.parameter(point) else {
            return TRANSPARENT;
        };
        match self.geometry {
            Geometry::Conic { .. } => wrap(&self.stops, t),
            _ => ramp(&self.stops, t.clamp(0.0, 1.0)),
        }
    }

    /** Linear and radial parameters may leave [0, 1] and extend the end colors; conic already wraps. */
    fn parameter(&self, point: [f64; 2]) -> Option<f64> {
        match &self.geometry {
            Geometry::Linear { from, to } => linear(*from, *to, point),
            Geometry::Radial { start, end } => radial(*start, *end, point),
            Geometry::Conic { center, start } => Some(conic(*center, *start, point)),
        }
    }
}

/** Projection onto the axis; a zero-length axis has no parameter, so nothing is painted. */
fn linear(from: [f64; 2], to: [f64; 2], point: [f64; 2]) -> Option<f64> {
    let [dx, dy] = [to[0] - from[0], to[1] - from[1]];
    let length = dx * dx + dy * dy;
    (length > 0.0).then(|| ((point[0] - from[0]) * dx + (point[1] - from[1]) * dy) / length)
}

/** The largest parameter whose circle contains the point with nonnegative radius, per the canvas radial rule. */
fn radial(start: [f64; 3], end: [f64; 3], point: [f64; 2]) -> Option<f64> {
    let [x0, y0, r0] = start;
    let [x1, y1, r1] = end;
    let (dx, dy, dr) = (x1 - x0, y1 - y0, r1 - r0);
    let (px, py) = (point[0] - x0, point[1] - y0);
    let a = dx * dx + dy * dy - dr * dr;
    let b = px * dx + py * dy + r0 * dr;
    let c = px * px + py * py - r0 * r0;
    radial_roots(a, b, c)
        .into_iter()
        .flatten()
        .find(|w| r0 + w * dr >= 0.0)
}

/** Roots of a*w^2 - 2*b*w + c = 0, largest first; a zero leading term degenerates to a linear root. */
fn radial_roots(a: f64, b: f64, c: f64) -> [Option<f64>; 2] {
    if a == 0.0 {
        return [(b != 0.0).then(|| c / (2.0 * b)), None];
    }
    let discriminant = b * b - a * c;
    if discriminant < 0.0 {
        return [None, None];
    }
    let root = discriminant.sqrt();
    let (first, second) = ((b + root) / a, (b - root) / a);
    [Some(first.max(second)), Some(first.min(second))]
}

/** Clockwise angle from twelve o'clock, as a fraction of one turn after the start angle. */
fn conic(center: [f64; 2], start: f64, point: [f64; 2]) -> f64 {
    let (dx, dy) = (point[0] - center[0], point[1] - center[1]);
    (dx.atan2(-dy) - start).rem_euclid(TAU) / TAU
}

/** Clamped ramp: before the first stop and after the last, the end colors extend. */
fn ramp(stops: &[Stop], t: f64) -> [u8; 4] {
    if stops.is_empty() {
        return TRANSPARENT;
    }
    let next = stops.partition_point(|stop| stop.offset <= t);
    match next {
        0 => stops[0].color,
        n if n == stops.len() => stops[n - 1].color,
        n => between(&stops[n - 1], &stops[n], t),
    }
}

/** Conic ramps wrap: the segment from the last stop back around to the first stop closes the turn. */
fn wrap(stops: &[Stop], t: f64) -> [u8; 4] {
    let (Some(first), Some(last)) = (stops.first(), stops.last()) else {
        return TRANSPARENT;
    };
    if t >= last.offset || t < first.offset {
        let position = if t < first.offset { t + 1.0 } else { t };
        let span = first.offset + 1.0 - last.offset;
        let weight = if span > 0.0 {
            (position - last.offset) / span
        } else {
            0.0
        };
        return mix(last.color, first.color, weight);
    }
    ramp(stops, t)
}

/** Interpolate within one segment; a zero-width segment is a hard edge that takes the later color. */
fn between(from: &Stop, to: &Stop, t: f64) -> [u8; 4] {
    let span = to.offset - from.offset;
    let weight = if span > 0.0 {
        (t - from.offset) / span
    } else {
        1.0
    };
    mix(from.color, to.color, weight)
}

/** Channels interpolate as stored straight-alpha values, matching Chrome's canvas gradients. */
fn mix(from: [u8; 4], to: [u8; 4], weight: f64) -> [u8; 4] {
    let weight = weight.clamp(0.0, 1.0);
    std::array::from_fn(|i| {
        (f64::from(from[i]) + (f64::from(to[i]) - f64::from(from[i])) * weight).round() as u8
    })
}

/** Every coordinate in a slice is a finite number. */
fn finite(values: &[f64]) -> bool {
    values.iter().all(|value| value.is_finite())
}
