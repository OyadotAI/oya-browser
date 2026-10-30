/** Scanline supersampling fills arbitrary polygon contours with bounded edge and pixel work. */
use super::*;
use crate::canvas::clip::{GRID as SAMPLES, SAMPLES as SAMPLE_COUNT, offset};

/** A scanline crossing carries direction so coincident contours obey either fill rule. */
type Crossing = (f64, i32);
/** Each nonhorizontal edge contributes at most one crossing to a sample row. */
type Edge = [Point; 2];

/** Plans retain no script callbacks and reserve scratch before mutable pixel ownership is acquired. */
#[derive(Debug)]
pub struct PathPlan {
    size: [u32; 2],
    edges: Vec<Edge>,
    bounds: [u32; 4],
    rule: FillRule,
    work: usize,
    clip: Option<ClipMask>,
    mode: Composite,
    _allocation: Allocation,
}

impl PathPlan {
    /** These operators preserve transparent-source pixels, so path coverage and clip plans remain valid. */
    pub fn with_composite(mut self, mode: Composite) -> Self {
        self.mode = mode;
        self
    }

    /** Native dash preparation is charged together with raster work before backing pixels become mutable. */
    pub(super) fn add_work(mut self, additional: usize) -> EngineResult<Self> {
        self.work = self.work.checked_add(additional).ok_or_else(quota)?;
        Ok(self)
    }

    /** Charge edge sorting, scanline stepping and coverage before painting starts. */
    pub fn work(&self) -> usize {
        self.work
    }
    /** Fully clipped and degenerate contours do not require a bitmap copy. */
    pub fn is_empty(&self) -> bool {
        self.bounds[0] == self.bounds[2] || self.bounds[1] == self.bounds[3]
    }

    /** Attach immutable clip samples while keeping existing public paint entry points stable. */
    pub fn with_clip(mut self, clip: Option<&ClipMask>) -> EngineResult<Self> {
        if let Some(clip) = clip {
            clip.validate(self.size)?;
        }
        let pixels =
            (self.bounds[2] - self.bounds[0]) as usize * (self.bounds[3] - self.bounds[1]) as usize;
        self.work = self.work - pixels * usize::from(self.clip.is_some())
            + pixels * usize::from(clip.is_some());
        self.clip = clip.cloned();
        Ok(self)
    }

    /** Clip construction also zeroes a full mask and may compare it with its previous immutable value. */
    pub fn clip_work(&self) -> usize {
        self.work + self.size[0] as usize * self.size[1] as usize * 2
    }

    /** Build the intersection completely before the caller installs a new saved-state mask reference. */
    pub fn clip_mask(&self) -> EngineResult<ClipMask> {
        let mut mask = ClipMask::zeroed(self.size)?;
        let mut crossings = Vec::with_capacity(self.edges.len());
        let storage = mask.storage()?;
        for y in self.bounds[1]..self.bounds[3] {
            let coverage = coverage_row(self, y, &mut crossings);
            for x in self.bounds[0]..self.bounds[2] {
                ClipMask::write(storage, self.size[0], x, y, covered(self, &coverage, x, y));
            }
        }
        Ok(mask)
    }
}

/** Open subpaths are implicitly closed for filling without changing the stored current path. */
pub fn plan_fill(size: [u32; 2], path: &Path, rule: FillRule) -> EngineResult<PathPlan> {
    path.validate()?;
    contours(
        size,
        path.subpaths
            .iter()
            .map(|subpath| subpath.points.as_slice()),
        rule,
    )
}

/** Stroke outlines share the same fill engine; consistently oriented contours form their union. */
pub(super) fn contours<'a>(
    size: [u32; 2],
    contours: impl Iterator<Item = &'a [Point]>,
    rule: FillRule,
) -> EngineResult<PathPlan> {
    if size.iter().any(|edge| *edge > EDGE) {
        return Err(EngineError::invalid("Invalid canvas path dimensions."));
    }
    let allocation = Allocation::new(
        OUTLINE_POINTS * (std::mem::size_of::<Edge>() + std::mem::size_of::<Crossing>()),
    )?;
    let mut edges = Vec::new();
    for points in contours {
        append_edges(&mut edges, points)?;
    }
    let bounds = bounds(size, &edges);
    let width = (bounds[2] - bounds[0]) as usize;
    let height = (bounds[3] - bounds[1]) as usize;
    let sort_work = edges.len() * (edges.len().max(1).ilog2() as usize + 2);
    let work = if width == 0 {
        0
    } else {
        height * SAMPLES * (width * SAMPLES + sort_work)
    };
    Ok(PathPlan {
        size,
        edges,
        bounds,
        rule,
        work,
        clip: None,
        mode: Composite::default(),
        _allocation: allocation,
    })
}

/** All generated edges are validated before scanline arithmetic or COW can begin. */
fn append_edges(edges: &mut Vec<Edge>, points: &[Point]) -> EngineResult<()> {
    if points.len() < 3 {
        return Ok(());
    }
    if edges.len().saturating_add(points.len()) > OUTLINE_POINTS {
        return Err(quota());
    }
    for point in points {
        checked(*point)?;
    }
    for index in 0..points.len() {
        let edge = [points[index], points[(index + 1) % points.len()]];
        if edge[0][1] != edge[1][1] {
            edges.push(edge);
        }
    }
    Ok(())
}

/** Clip finite bounding coordinates before converting to bitmap indices. */
fn bounds(size: [u32; 2], edges: &[Edge]) -> [u32; 4] {
    if edges.is_empty() {
        return [0; 4];
    }
    let points = || edges.iter().flat_map(|edge| edge.iter());
    let minimum = |axis| {
        points()
            .map(|point| point[axis])
            .fold(f64::INFINITY, f64::min)
    };
    let maximum = |axis| {
        points()
            .map(|point| point[axis])
            .fold(f64::NEG_INFINITY, f64::max)
    };
    [
        minimum(0).floor().clamp(0.0, size[0] as f64) as u32,
        minimum(1).floor().clamp(0.0, size[1] as f64) as u32,
        maximum(0).ceil().clamp(0.0, size[0] as f64) as u32,
        maximum(1).ceil().clamp(0.0, size[1] as f64) as u32,
    ]
}

/** One composite per pixel prevents self-overlap from multiplying global alpha. */
pub fn paint_path(
    bitmap: &mut Bitmap,
    plan: &PathPlan,
    color: [u8; 4],
    alpha: f64,
) -> EngineResult<()> {
    bitmap.validate()?;
    if plan.size != [bitmap.width, bitmap.height]
        || !alpha.is_finite()
        || !(0.0..=1.0).contains(&alpha)
    {
        return Err(EngineError::invalid("Invalid canvas path paint state."));
    }
    if plan.is_empty() {
        return Ok(());
    }
    let mut crossings = Vec::with_capacity(plan.edges.len());
    let width = bitmap.width as usize;
    let pixels = bitmap.pixels.make_mut()?;
    for y in plan.bounds[1]..plan.bounds[3] {
        let coverage = coverage_row(plan, y, &mut crossings);
        for x in plan.bounds[0]..plan.bounds[2] {
            let at = (y as usize * width + x as usize) * CHANNELS;
            let coverage = covered(plan, &coverage, x, y).count_ones();
            if coverage == 0 {
                continue;
            }
            composite(
                &mut pixels[at..at + CHANNELS],
                color,
                alpha * f64::from(coverage) / SAMPLE_COUNT as f64,
                plan.mode,
            );
        }
    }
    Ok(())
}

/** Half-open vertical intervals count shared vertices exactly once for both winding rules. */
fn scanline(
    plan: &PathPlan,
    y: f64,
    row_sample: usize,
    crossings: &mut Vec<Crossing>,
    coverage: &mut [u16; EDGE as usize],
) {
    crossings.clear();
    for &[from, to] in &plan.edges {
        if y < from[1].min(to[1]) || y >= from[1].max(to[1]) {
            continue;
        }
        let x = from[0] + (y - from[1]) / (to[1] - from[1]) * (to[0] - from[0]);
        crossings.push((x, if to[1] > from[1] { 1 } else { -1 }));
    }
    crossings.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
    let (mut cursor, mut winding) = (0, 0i32);
    for x in plan.bounds[0]..plan.bounds[2] {
        for sample in 0..SAMPLES {
            let at = x as f64 + offset(sample);
            while cursor < crossings.len() && crossings[cursor].0 <= at {
                winding += crossings[cursor].1;
                cursor += 1;
            }
            let inside = match plan.rule {
                FillRule::Nonzero => winding != 0,
                FillRule::Evenodd => winding % 2 != 0,
            };
            coverage[x as usize] |= u16::from(inside) << (row_sample * SAMPLES + sample);
        }
    }
}

/** Preserve individual sample bits through fill planning and clip intersections. */
fn coverage_row(plan: &PathPlan, y: u32, crossings: &mut Vec<Crossing>) -> [u16; EDGE as usize] {
    let mut coverage = [0; EDGE as usize];
    for sample in 0..SAMPLES {
        scanline(
            plan,
            y as f64 + offset(sample),
            sample,
            crossings,
            &mut coverage,
        );
    }
    coverage
}

/** An absent mask means the initial unbounded region, while an all-zero mask is an empty clip. */
fn covered(plan: &PathPlan, coverage: &[u16; EDGE as usize], x: u32, y: u32) -> u16 {
    coverage[x as usize] & plan.clip.as_ref().map_or(u16::MAX, |clip| clip.bits(x, y))
}
