/** Retained paths copy independent contours and defer context transforms until drawing. */
use super::*;

impl Path {
    /** Native retained-object budgets count actual geometry rather than serialized metadata. */
    pub fn point_count(&self) -> usize {
        self.count()
    }

    /** An empty path has no current point, even if a later operation will create one. */
    pub fn is_empty(&self) -> bool {
        self.subpaths.is_empty()
    }

    /** Transform a bounded independent copy without modifying its source on overflow. */
    pub fn transformed(&self, transform: Affine) -> EngineResult<Self> {
        self.validate()?;
        let mut result = self.clone();
        for subpath in &mut result.subpaths {
            for point in &mut subpath.points {
                *point = checked(transform.map(*point))?;
            }
        }
        Ok(result)
    }

    /** Append independent subpaths and establish the final logical point for subsequent commands. */
    pub fn add_path(&mut self, source: &Self, transform: Affine) -> EngineResult<()> {
        if source.is_empty() {
            return Ok(());
        }
        self.admit(source.count().saturating_add(1))?;
        let mut copy = source.transformed(transform)?;
        let last = copy.subpaths.last().expect("nonempty source");
        let point = if last.closed {
            last.points[0]
        } else {
            *last.points.last().expect("validated contour")
        };
        copy.subpaths.push(Subpath {
            points: vec![point],
            closed: false,
        });
        self.subpaths.extend(copy.subpaths);
        Ok(())
    }
}
