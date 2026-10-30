/** Bounded polygon paths retain construction coordinates independently from the saved drawing state. */
use super::{Affine, Allocation, Bitmap, CHANNELS, EDGE, blend};
use crate::protocol::{EngineError, EngineResult};
use serde::{Deserialize, Serialize};
mod arc_to;
mod arcs;
mod curves;
mod raster;
mod round_rect;
mod stroke;
pub use raster::{PathPlan, paint_path, plan_fill};
pub use stroke::{LineCap, LineJoin, StrokeStyle, plan_stroke};

/** Each context admits finite path bookkeeping even when nothing is ever painted. */
const POINTS: usize = 1024;
/** Derived stroke outlines may need several edges for each authored path point. */
const OUTLINE_POINTS: usize = POINTS * 16;
/** Keeps cross products and line intersections representable before clipping to pixels. */
const COORDINATE: f64 = 1.0e12;
/** Two coordinates in canvas bitmap space, after the construction-time transform. */
type Point = [f64; 2];

/** Fill rules apply across all subpaths, including holes and self-intersections. */
#[derive(Clone, Copy, Debug)]
pub enum FillRule {
    Nonzero,
    Evenodd,
}

/** The current path is deliberately outside save/restore; resize and beginPath replace it. */
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Path {
    subpaths: Vec<Subpath>,
}

/** A closed contour retains its start so a following lineTo begins the next subpath there. */
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Subpath {
    points: Vec<Point>,
    closed: bool,
}

impl Path {
    /** Restored paths must satisfy the same finite storage and numeric bounds as live commands. */
    pub fn validate(&self) -> EngineResult<()> {
        if self.subpaths.len() > POINTS || self.count() > POINTS {
            return Err(quota());
        }
        for subpath in &self.subpaths {
            if subpath.points.is_empty() {
                return Err(EngineError::invalid("Empty canvas subpath."));
            }
            for point in &subpath.points {
                checked(*point)?;
            }
        }
        Ok(())
    }

    /** A new subpath records the current transform now, rather than when fill is called. */
    pub fn move_to(&mut self, point: Point, transform: Affine) -> EngineResult<()> {
        let point = checked(transform.map(point))?;
        self.admit(1)?;
        self.subpaths.push(Subpath {
            points: vec![point],
            closed: false,
        });
        Ok(())
    }

    /** An empty path starts at this point; a closed contour resumes from its start point. */
    pub fn line_to(&mut self, point: Point, transform: Affine) -> EngineResult<()> {
        let point = checked(transform.map(point))?;
        let closed = self.subpaths.last().filter(|subpath| subpath.closed);
        let start = closed.map(|subpath| subpath.points[0]);
        self.admit(if start.is_some() { 2 } else { 1 })?;
        if let Some(start) = start {
            self.subpaths.push(Subpath {
                points: vec![start],
                closed: false,
            });
        }
        if let Some(subpath) = self.subpaths.last_mut() {
            subpath.points.push(point);
        } else {
            self.subpaths.push(Subpath {
                points: vec![point],
                closed: false,
            });
        }
        Ok(())
    }

    /** Closing is idempotent and does not modify the drawing state's transform or fill rule. */
    pub fn close(&mut self) {
        if let Some(subpath) = self.subpaths.last_mut() {
            subpath.closed = true;
        }
    }

    /** Rectangle insertion is atomic even when transformed endpoints overflow or the path is full. */
    pub fn rect(&mut self, rect: [f64; 4], transform: Affine) -> EngineResult<()> {
        let [x, y, width, height] = rect;
        let mut points = [
            [x, y],
            [x + width, y],
            [x + width, y + height],
            [x, y + height],
        ];
        for point in &mut points {
            *point = checked(transform.map(*point))?;
        }
        self.admit(points.len())?;
        self.subpaths.push(Subpath {
            points: points.to_vec(),
            closed: true,
        });
        Ok(())
    }

    /** Count points without keeping a second serialized value that could disagree with storage. */
    fn count(&self) -> usize {
        self.subpaths
            .iter()
            .map(|subpath| subpath.points.len())
            .sum()
    }

    /** Quota failure precedes any path mutation, so catch-and-retry sees the original path. */
    fn admit(&self, additional: usize) -> EngineResult<()> {
        if self.count().saturating_add(additional) > POINTS {
            return Err(quota());
        }
        Ok(())
    }
}

/** Numeric overflow is an explicit geometry refusal, distinct from path storage exhaustion. */
fn checked(point: Point) -> EngineResult<Point> {
    if point
        .iter()
        .any(|value| !value.is_finite() || value.abs() > COORDINATE)
    {
        return Err(EngineError::new(
            "canvas_geometry",
            "Canvas path exceeds this build's numeric range.",
            "Use smaller path coordinates and transforms.",
        ));
    }
    Ok(point)
}

/** The interpreter translates this resource refusal into a catchable DOM exception. */
fn quota() -> EngineError {
    EngineError::invalid("QuotaExceededError: Canvas path budget exceeded.")
}
