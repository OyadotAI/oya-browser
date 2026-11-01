/** Canvas text placement: baselines, alignment, direction and the metrics measureText reports. */
use serde::{Deserialize, Serialize};

/** A hanging baseline sits this fraction of the font's ascent above the alphabetic baseline. */
pub const HANGING_RATIO: f64 = 0.8;

/** The textBaseline attribute, in the spelling scripts read back. */
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Baseline {
    /** Top of the em box. */
    Top,
    /** The hanging baseline. */
    Hanging,
    /** Middle of the em box. */
    Middle,
    /** The default: the alphabetic baseline, where fillText has always drawn. */
    #[default]
    Alphabetic,
    /** Ideographic baseline, below the alphabetic one by the font's descent. */
    Ideographic,
    /** Bottom of the em box. */
    Bottom,
}

/** The textAlign attribute; start and end resolve against the direction. */
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Align {
    /** The default: the leading edge of the text, left in ltr. */
    #[default]
    Start,
    /** The trailing edge of the text, right in ltr. */
    End,
    /** Always the left edge. */
    Left,
    /** Always the right edge. */
    Right,
    /** Centered on the anchor. */
    Center,
}

/** The direction attribute; inherit takes the canvas element's computed direction. */
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    /** Left to right. */
    Ltr,
    /** Right to left. */
    Rtl,
    /** The default: whatever the canvas element computes. */
    #[default]
    Inherit,
}

/** Ascent and descent of the font box, both positive, measured from the alphabetic baseline. */
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FontMetrics {
    /** Distance from the alphabetic baseline up to the top of the font box. */
    pub ascent: f64,
    /** Distance from the alphabetic baseline down to the bottom of the font box. */
    pub descent: f64,
}

impl FontMetrics {
    /** The em box is as tall as the font size, centered on the font box's own extent. */
    pub fn em_ascent(self, size: f64) -> f64 {
        (size + self.ascent - self.descent) / 2.0
    }
    /** Whatever part of the em box lies below the alphabetic baseline. */
    pub fn em_descent(self, size: f64) -> f64 {
        size - self.em_ascent(size)
    }
}

/** Every value measureText reports, all relative to the textBaseline line the way the spec states them. */
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextMeasure {
    /** Advance width of the run. */
    pub width: f64,
    /** Distance from the alignment anchor to the left edge of the run. */
    pub actual_left: f64,
    /** Distance from the alignment anchor to the right edge of the run. */
    pub actual_right: f64,
    /** Font-box ascent above the textBaseline line. */
    pub actual_ascent: f64,
    /** Font-box descent below the textBaseline line. */
    pub actual_descent: f64,
    /** Same as the actual ascent, because glyph ink is not measured; the font box stands in for it. */
    pub font_ascent: f64,
    /** Same as the actual descent, for the same reason. */
    pub font_descent: f64,
    /** Top of the em box above the textBaseline line. */
    pub em_ascent: f64,
    /** Bottom of the em box below the textBaseline line. */
    pub em_descent: f64,
    /** Hanging baseline, measured from the textBaseline line. */
    pub hanging: f64,
    /** Alphabetic baseline, measured from the textBaseline line. */
    pub alphabetic: f64,
    /** Ideographic baseline, measured from the textBaseline line. */
    pub ideographic: f64,
}

/** Baseline offset: how far above the alphabetic baseline the named line sits; negative means below. */
pub fn baseline_offset(baseline: Baseline, metrics: FontMetrics, size: f64) -> f64 {
    match baseline {
        Baseline::Top => metrics.em_ascent(size),
        Baseline::Hanging => metrics.ascent * HANGING_RATIO,
        Baseline::Middle => (metrics.em_ascent(size) - metrics.em_descent(size)) / 2.0,
        Baseline::Alphabetic => 0.0,
        Baseline::Ideographic => -metrics.descent,
        Baseline::Bottom => -metrics.em_descent(size),
    }
}

/** Horizontal shift from the anchor to the start of the run, after resolving start and end. */
pub fn align_shift(align: Align, width: f64, rtl: bool) -> f64 {
    let resolved = match (align, rtl) {
        (Align::Start, false) | (Align::End, true) => Align::Left,
        (Align::Start, true) | (Align::End, false) => Align::Right,
        (other, _) => other,
    };
    match resolved {
        Align::Center => -width / 2.0,
        Align::Right => -width,
        _ => 0.0,
    }
}

/** Derive the measureText record from the host's width and font box for the current placement. */
pub fn measure(
    size: f64,
    metrics: FontMetrics,
    width: f64,
    placement: (Baseline, Align, bool),
) -> TextMeasure {
    let (baseline, align, rtl) = placement;
    let shift = align_shift(align, width, rtl);
    let here = baseline_offset(baseline, metrics, size);
    let above = |line: f64| line - here;
    TextMeasure {
        width,
        actual_left: -shift,
        actual_right: shift + width,
        actual_ascent: above(metrics.ascent),
        actual_descent: metrics.descent + here,
        font_ascent: above(metrics.ascent),
        font_descent: metrics.descent + here,
        em_ascent: above(metrics.em_ascent(size)),
        em_descent: metrics.em_descent(size) + here,
        hanging: above(baseline_offset(Baseline::Hanging, metrics, size)),
        alphabetic: above(0.0),
        ideographic: above(baseline_offset(Baseline::Ideographic, metrics, size)),
    }
}

/** Keyword tables give each attribute one spelling for parsing and for the getter. */
pub fn lookup<T: Copy>(table: &[(&str, T)], text: &str) -> Option<T> {
    table
        .iter()
        .find(|(name, _)| *name == text)
        .map(|(_, value)| *value)
}

/** The canonical spelling of a value in its keyword table. */
pub fn keyword<T: Copy + PartialEq>(table: &[(&'static str, T)], value: T) -> &'static str {
    table
        .iter()
        .find(|(_, candidate)| *candidate == value)
        .map_or("", |(name, _)| *name)
}

/** The textBaseline keywords. */
pub const BASELINES: [(&str, Baseline); 6] = [
    ("top", Baseline::Top),
    ("hanging", Baseline::Hanging),
    ("middle", Baseline::Middle),
    ("alphabetic", Baseline::Alphabetic),
    ("ideographic", Baseline::Ideographic),
    ("bottom", Baseline::Bottom),
];
/** The textAlign keywords. */
pub const ALIGNS: [(&str, Align); 5] = [
    ("start", Align::Start),
    ("end", Align::End),
    ("left", Align::Left),
    ("right", Align::Right),
    ("center", Align::Center),
];
/** The direction keywords. */
pub const DIRECTIONS: [(&str, Direction); 3] = [
    ("ltr", Direction::Ltr),
    ("rtl", Direction::Rtl),
    ("inherit", Direction::Inherit),
];
