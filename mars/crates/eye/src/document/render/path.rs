/** Owned SVG path-data parsing for the full SVG 2 path grammar, in user units. */
use crate::{
    constants::SVG_PATH_SEGMENTS,
    protocol::{EngineError, EngineResult},
};
use std::f64::consts::FRAC_PI_2;

/** User-space drawing step with every coordinate already absolute; quadratics are cubics. */
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Segment {
    Move(f64, f64),
    Line(f64, f64),
    Cubic([f64; 6]),
    Close,
}

/** Every SVG 2 command parses; malformed data fails so the caller drops just that shape. */
pub(super) fn parse(data: &str) -> EngineResult<Vec<Segment>> {
    let mut scanner = Scanner { rest: data };
    let mut state = State::default();
    let mut command = None;
    while let Some(next) = scanner.command_or_number()? {
        let letter = match next {
            Token::Command(letter) => letter,
            Token::Number => command.ok_or_else(malformed)?,
        };
        // SVG path data must open with a moveto; native paths have no current point before one.
        if command.is_none() && !matches!(letter, 'M' | 'm') {
            return Err(malformed());
        }
        command = Some(state.step(letter, &mut scanner)?);
        if state.segments.len() > SVG_PATH_SEGMENTS {
            return Err(EngineError::limit("SVG path segments", SVG_PATH_SEGMENTS));
        }
    }
    Ok(state.segments)
}

/** Current point, subpath start and the last control points resolve relative and smooth commands. */
#[derive(Default)]
struct State {
    segments: Vec<Segment>,
    current: (f64, f64),
    start: (f64, f64),
    /** The previous C or S second control point, which S reflects. */
    cubic: Option<(f64, f64)>,
    /** The previous Q or T control point, which T reflects. */
    quadratic: Option<(f64, f64)>,
}
impl State {
    /** Executes one command and returns the command an implicit repeat continues with. */
    fn step(&mut self, letter: char, scanner: &mut Scanner<'_>) -> EngineResult<char> {
        let relative = letter.is_ascii_lowercase();
        let (dx, dy) = if relative { self.current } else { (0.0, 0.0) };
        let mut point =
            || -> EngineResult<(f64, f64)> { Ok((scanner.number()? + dx, scanner.number()? + dy)) };
        let (cubic, quadratic) = (self.cubic.take(), self.quadratic.take());
        match letter.to_ascii_uppercase() {
            'M' => {
                let point = point()?;
                self.segments.push(Segment::Move(point.0, point.1));
                (self.current, self.start) = (point, point);
                return Ok(if relative { 'l' } else { 'L' });
            }
            'L' => self.line(point()?),
            'H' => self.line((scanner.number()? + dx, self.current.1)),
            'V' => self.line((self.current.0, scanner.number()? + dy)),
            'C' => {
                let (first, second, end) = (point()?, point()?, point()?);
                self.cubic(first, second, end);
            }
            'S' => {
                let first = reflect(cubic, self.current);
                let (second, end) = (point()?, point()?);
                self.cubic(first, second, end);
            }
            'Q' => {
                let (control, end) = (point()?, point()?);
                self.quadratic(control, end);
            }
            'T' => {
                let control = reflect(quadratic, self.current);
                self.quadratic(control, point()?);
            }
            'A' => self.arc(scanner, (dx, dy))?,
            'Z' => {
                self.segments.push(Segment::Close);
                self.current = self.start;
            }
            _ => return Err(malformed()),
        }
        Ok(letter)
    }
    /** One cubic Bezier, remembering its second control for a following S. */
    fn cubic(&mut self, first: (f64, f64), second: (f64, f64), end: (f64, f64)) {
        self.segments.push(Segment::Cubic([
            first.0, first.1, second.0, second.1, end.0, end.1,
        ]));
        self.current = end;
        self.cubic = Some(second);
    }
    /** Reads one arc's radii, rotation, flags and end point, then appends its cubics. */
    fn arc(&mut self, scanner: &mut Scanner<'_>, (dx, dy): (f64, f64)) -> EngineResult<()> {
        let radii = (scanner.number()?.abs(), scanner.number()?.abs());
        let rotation = scanner.number()?.to_radians();
        let (large, sweep) = (scanner.flag()?, scanner.flag()?);
        let end = (scanner.number()? + dx, scanner.number()? + dy);
        for cubic in arc_cubics(self.current, radii, rotation, large, sweep, end) {
            self.segments.push(Segment::Cubic(cubic));
        }
        self.current = end;
        Ok(())
    }
    /** Straight segments share one current-point update. */
    fn line(&mut self, point: (f64, f64)) {
        self.segments.push(Segment::Line(point.0, point.1));
        self.current = point;
    }
    /** A quadratic is exactly the cubic whose controls sit two thirds toward its control point. */
    fn quadratic(&mut self, control: (f64, f64), end: (f64, f64)) {
        let (x0, y0) = self.current;
        let two_thirds = 2.0 / 3.0;
        self.segments.push(Segment::Cubic([
            x0 + two_thirds * (control.0 - x0),
            y0 + two_thirds * (control.1 - y0),
            end.0 + two_thirds * (control.0 - end.0),
            end.1 + two_thirds * (control.1 - end.1),
            end.0,
            end.1,
        ]));
        self.current = end;
        self.quadratic = Some(control);
    }
}

/** A smooth command's first control mirrors the previous one through the current point. */
fn reflect(previous: Option<(f64, f64)>, current: (f64, f64)) -> (f64, f64) {
    previous.map_or(current, |(x, y)| (2.0 * current.0 - x, 2.0 * current.1 - y))
}

/// SVG 2 Appendix B.2.5 and F.6: an endpoint arc becomes at most four cubics, one per quarter
/// turn. Zero radii draw a line and a coincident end draws nothing, as the specification says.
fn arc_cubics(
    from: (f64, f64),
    radii: (f64, f64),
    rotation: f64,
    large: bool,
    sweep: bool,
    to: (f64, f64),
) -> Vec<[f64; 6]> {
    if from == to {
        return Vec::new();
    }
    if radii.0 == 0.0 || radii.1 == 0.0 {
        return vec![[from.0, from.1, to.0, to.1, to.0, to.1]];
    }
    let ellipse = Ellipse::new(from, radii, rotation, large, sweep, to);
    let pieces = (ellipse.delta.abs() / FRAC_PI_2).ceil().max(1.0);
    let step = ellipse.delta / pieces;
    // Each piece's controls sit k along the tangents, with k = 4/3 tan(step / 4).
    let k = 4.0 / 3.0 * (step / 4.0).tan();
    (0..pieces as usize)
        .map(|index| {
            let (a, b) = (
                ellipse.start + step * index as f64,
                ellipse.start + step * (index + 1) as f64,
            );
            let (p0, d0) = (ellipse.point(a), ellipse.tangent(a));
            let (p1, d1) = (ellipse.point(b), ellipse.tangent(b));
            [
                p0.0 + k * d0.0,
                p0.1 + k * d0.1,
                p1.0 - k * d1.0,
                p1.1 - k * d1.1,
                p1.0,
                p1.1,
            ]
        })
        .collect()
}

/** The center parameterization of one endpoint arc (SVG 2 Appendix B.2.4 and B.2.5). */
struct Ellipse {
    center: (f64, f64),
    radii: (f64, f64),
    /** Cosine and sine of the x-axis rotation. */
    axis: (f64, f64),
    /** The start angle and the signed sweep, in radians. */
    start: f64,
    delta: f64,
}
impl Ellipse {
    /** Out-of-range radii scale up until the end point is reachable, per B.2.5. */
    fn new(
        from: (f64, f64),
        radii: (f64, f64),
        rotation: f64,
        large: bool,
        sweep: bool,
        to: (f64, f64),
    ) -> Self {
        let (sin, cos) = rotation.sin_cos();
        let (hx, hy) = ((from.0 - to.0) / 2.0, (from.1 - to.1) / 2.0);
        let (x1, y1) = (cos * hx + sin * hy, -sin * hx + cos * hy);
        let lambda = (x1 / radii.0).powi(2) + (y1 / radii.1).powi(2);
        let scale = lambda.sqrt().max(1.0);
        let (rx, ry) = (radii.0 * scale, radii.1 * scale);
        let numerator = (rx * ry).powi(2) - (rx * y1).powi(2) - (ry * x1).powi(2);
        let denominator = (rx * y1).powi(2) + (ry * x1).powi(2);
        let sign = if large == sweep { -1.0 } else { 1.0 };
        let root = sign * (numerator / denominator).max(0.0).sqrt();
        let (cx1, cy1) = (root * rx * y1 / ry, -root * ry * x1 / rx);
        let center = (
            cos * cx1 - sin * cy1 + (from.0 + to.0) / 2.0,
            sin * cx1 + cos * cy1 + (from.1 + to.1) / 2.0,
        );
        let start = ((y1 - cy1) / ry).atan2((x1 - cx1) / rx);
        let end = ((-y1 - cy1) / ry).atan2((-x1 - cx1) / rx);
        let mut delta = end - start;
        if sweep && delta < 0.0 {
            delta += 2.0 * std::f64::consts::PI;
        } else if !sweep && delta > 0.0 {
            delta -= 2.0 * std::f64::consts::PI;
        }
        Self {
            center,
            radii: (rx, ry),
            axis: (cos, sin),
            start,
            delta,
        }
    }
    /** The point at parametric angle `t`. */
    fn point(&self, t: f64) -> (f64, f64) {
        let (x, y) = (self.radii.0 * t.cos(), self.radii.1 * t.sin());
        self.rotate(x, y, self.center)
    }
    /** The derivative at `t`, the direction the cubic controls follow. */
    fn tangent(&self, t: f64) -> (f64, f64) {
        let (x, y) = (-self.radii.0 * t.sin(), self.radii.1 * t.cos());
        self.rotate(x, y, (0.0, 0.0))
    }
    /** Applies the x-axis rotation and then an offset. */
    fn rotate(&self, x: f64, y: f64, offset: (f64, f64)) -> (f64, f64) {
        let (cos, sin) = self.axis;
        (cos * x - sin * y + offset.0, sin * x + cos * y + offset.1)
    }
}

/** A command letter, or the start of a number that implicitly repeats the previous command. */
enum Token {
    Command(char),
    Number,
}

/** Compact SVG numbers such as `1.5-1` and `.6.3` split exactly where the grammar says. */
struct Scanner<'a> {
    rest: &'a str,
}
impl Scanner<'_> {
    /** Separators are whitespace and commas. */
    fn skip(&mut self) {
        self.rest = self
            .rest
            .trim_start_matches(|c: char| c.is_ascii_whitespace() || c == ',');
    }
    /** Peeks whether the next token is a command letter or a number, consuming only the letter. */
    fn command_or_number(&mut self) -> EngineResult<Option<Token>> {
        self.skip();
        let Some(next) = self.rest.chars().next() else {
            return Ok(None);
        };
        if next.is_ascii_alphabetic() && !matches!(next, 'e' | 'E') {
            self.rest = &self.rest[1..];
            return Ok(Some(Token::Command(next)));
        }
        if next.is_ascii_digit() || matches!(next, '-' | '+' | '.') {
            return Ok(Some(Token::Number));
        }
        Err(malformed())
    }
    /** An arc flag is exactly one `0` or `1`, which may abut the next number (`a1 1 0 00 1 1`). */
    fn flag(&mut self) -> EngineResult<bool> {
        self.skip();
        let flag = match self.rest.as_bytes().first() {
            Some(b'0') => false,
            Some(b'1') => true,
            _ => return Err(malformed()),
        };
        self.rest = &self.rest[1..];
        Ok(flag)
    }
    /** One number: sign, digits, at most one point, optional exponent. */
    fn number(&mut self) -> EngineResult<f64> {
        self.skip();
        let bytes = self.rest.as_bytes();
        let mut end = usize::from(matches!(bytes.first(), Some(b'-' | b'+')));
        let mut point = false;
        while let Some(byte) = bytes.get(end) {
            match byte {
                b'0'..=b'9' => {}
                b'.' if !point => point = true,
                b'e' | b'E'
                    if bytes
                        .get(end + 1)
                        .is_some_and(|b| b.is_ascii_digit() || *b == b'-' || *b == b'+') =>
                {
                    end += 2;
                    while bytes.get(end).is_some_and(u8::is_ascii_digit) {
                        end += 1;
                    }
                    break;
                }
                _ => break,
            }
            end += 1;
        }
        let value = self.rest[..end].parse::<f64>().map_err(|_| malformed())?;
        self.rest = &self.rest[end..];
        value.is_finite().then_some(value).ok_or_else(malformed)
    }
}

/** Malformed path data fails closed rather than drawing a truncated shape. */
fn malformed() -> EngineError {
    EngineError::unsupported("This page uses malformed SVG path data.")
}
