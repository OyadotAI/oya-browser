/** Absolute command normalization retains only completed segments and the preceding smooth controls. */
use super::*;

/** Parser state is local scratch, never installed as retained geometry before full admission. */
#[derive(Default)]
pub(super) struct State {
    pub(super) segments: Vec<Segment>,
    current: (f64, f64),
    start: (f64, f64),
    cubic: Option<(f64, f64)>,
    quadratic: Option<(f64, f64)>,
}
impl State {
    /** Convert one complete parameter group, returning its permitted implicit continuation command. */
    pub(super) fn step(&mut self, letter: char, scanner: &mut Scanner<'_>) -> EngineResult<char> {
        let relative = letter.is_ascii_lowercase();
        let offset = if relative { self.current } else { (0.0, 0.0) };
        let (cubic, quadratic) = (self.cubic.take(), self.quadratic.take());
        match letter.to_ascii_uppercase() {
            'M' => {
                let point = point(scanner, offset)?;
                self.push(Segment::Move(point.0, point.1))?;
                (self.current, self.start) = (point, point);
                return Ok(if relative { 'l' } else { 'L' });
            }
            'L' => self.line(point(scanner, offset)?)?,
            'H' => self.line((scanner.number()? + offset.0, self.current.1))?,
            'V' => self.line((self.current.0, scanner.number()? + offset.1))?,
            'C' => {
                let first = point(scanner, offset)?;
                let second = point(scanner, offset)?;
                let end = point(scanner, offset)?;
                self.cubic(first, second, end)?;
            }
            'S' => {
                let first = reflect(cubic, self.current);
                let second = point(scanner, offset)?;
                self.cubic(first, second, point(scanner, offset)?)?;
            }
            'Q' => {
                let control = point(scanner, offset)?;
                self.quadratic(control, point(scanner, offset)?)?;
            }
            'T' => self.quadratic(reflect(quadratic, self.current), point(scanner, offset)?)?,
            'A' => self.arc(scanner, offset)?,
            'Z' => {
                self.push(Segment::Close)?;
                self.current = self.start;
            }
            _ => return Err(malformed()),
        }
        Ok(letter)
    }

    /** Admission and derived-number validation precede each scratch-vector insertion. */
    fn push(&mut self, segment: Segment) -> EngineResult<()> {
        let finite = match segment {
            Segment::Move(x, y) | Segment::Line(x, y) => x.is_finite() && y.is_finite(),
            Segment::Cubic(values) => values.iter().all(|value| value.is_finite()),
            Segment::Close => true,
        };
        if !finite {
            return Err(numeric());
        }
        if self.segments.len() == SVG_PATH_SEGMENTS {
            return Err(quota());
        }
        self.segments.push(segment);
        Ok(())
    }

    /** Smooth cubic continuation reflects this second control only after a C or S command. */
    fn cubic(
        &mut self,
        first: (f64, f64),
        second: (f64, f64),
        end: (f64, f64),
    ) -> EngineResult<()> {
        self.push(Segment::Cubic([
            first.0, first.1, second.0, second.1, end.0, end.1,
        ]))?;
        self.current = end;
        self.cubic = Some(second);
        Ok(())
    }

    /** Quadratic conversion is exact; its original control is retained for the following T command. */
    fn quadratic(&mut self, control: (f64, f64), end: (f64, f64)) -> EngineResult<()> {
        const TWO_THIRDS: f64 = 2.0 / 3.0;
        let first = (
            self.current.0 + TWO_THIRDS * (control.0 - self.current.0),
            self.current.1 + TWO_THIRDS * (control.1 - self.current.1),
        );
        let second = (
            end.0 + TWO_THIRDS * (control.0 - end.0),
            end.1 + TWO_THIRDS * (control.1 - end.1),
        );
        self.push(Segment::Cubic([
            first.0, first.1, second.0, second.1, end.0, end.1,
        ]))?;
        self.current = end;
        self.quadratic = Some(control);
        Ok(())
    }

    /** Endpoint arcs consume every argument before normalization or emission, including compact flags. */
    fn arc(&mut self, scanner: &mut Scanner<'_>, offset: (f64, f64)) -> EngineResult<()> {
        let radii = (scanner.number()?.abs(), scanner.number()?.abs());
        let rotation = scanner.number()?.to_radians();
        let (large, sweep) = (scanner.flag()?, scanner.flag()?);
        let end = point(scanner, offset)?;
        for cubic in arcs::cubics(self.current, radii, rotation, large, sweep, end)? {
            self.push(Segment::Cubic(cubic))?;
        }
        self.current = end;
        Ok(())
    }

    /** Straight lines update the logical point only after complete coordinate validation. */
    fn line(&mut self, end: (f64, f64)) -> EngineResult<()> {
        self.push(Segment::Line(end.0, end.1))?;
        self.current = end;
        Ok(())
    }
}

/** Relative offsets apply equally to every coordinate pair in one parameter group. */
fn point(scanner: &mut Scanner<'_>, offset: (f64, f64)) -> EngineResult<(f64, f64)> {
    let point = (scanner.number()? + offset.0, scanner.number()? + offset.1);
    if !point.0.is_finite() || !point.1.is_finite() {
        return Err(numeric());
    }
    Ok(point)
}

/** Missing prior controls make smooth curves start tangent to the current point. */
fn reflect(previous: Option<(f64, f64)>, current: (f64, f64)) -> (f64, f64) {
    previous.map_or(current, |(x, y)| (2.0 * current.0 - x, 2.0 * current.1 - y))
}
