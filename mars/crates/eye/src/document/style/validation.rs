/** Private computed styles must satisfy the same bounds before render as styles parsed from CSS. */
use super::{
    BackgroundLayer, ColorScheme, ColorStop, ComputedStyle, FlexBasis,
    constants::{
        BACKGROUND_LAYERS, GRADIENT_STOPS, LETTER_SPACING_MAX, SHADOWS, TRANSFORM_MAX, WEIGHT_MAX,
        WEIGHT_MIN,
    },
};
use crate::document::Appearance;
use crate::{
    constants::{
        CSS_MAX_BORDER_WIDTH, CSS_MAX_FONT_SIZE, CSS_MAX_SCALAR, CSS_MIN_FONT_SIZE, URL_BYTES,
    },
    protocol::{EngineError, EngineResult},
};
impl ComputedStyle {
    /// Reject forged geometry and unsupported resolved combinations before native measurement or publication.
    pub fn validate(&self) -> EngineResult<()> {
        let mut lengths = self
            .padding
            .iter()
            .copied()
            .chain(self.width)
            .chain(self.max_width)
            .chain(self.min_width)
            .chain([
                self.flex_grow,
                self.flex_shrink,
                self.row_gap,
                self.column_gap,
            ]);
        if matches!(
            (self.color_scheme, self.used_scheme),
            (ColorScheme::Normal | ColorScheme::Light, Appearance::Dark)
                | (ColorScheme::Dark, Appearance::Light)
        ) {
            return Err(invalid());
        }
        let advance = self.line_height.used(self.font_size);
        // Zero-size text may have a zero line; every other line advances.
        let empty = advance == 0.0 && self.font_size == 0.0;
        if !advance.is_finite() || (advance <= 0.0 && !empty) || advance > CSS_MAX_SCALAR {
            return Err(invalid());
        }
        if lengths.any(|value| !scalar(value))
            || self
                .margin
                .iter()
                .any(|value| !value.is_finite() || value.abs() > CSS_MAX_SCALAR)
            || !(0.0..=1.0).contains(&self.opacity)
            || !self.transition.valid()
            || self.rotation_radians.is_some_and(|angle| {
                !angle.is_finite() || angle.abs() > super::constants::TRANSFORM_MAX
            })
            || self.rotation_animations.len() > super::constants::ANIMATIONS
            || !self
                .rotation_animations
                .iter()
                .all(super::RotationAnimation::valid)
            || self.opacity_animations.len() > super::constants::ANIMATIONS
            || !self
                .opacity_animations
                .iter()
                .all(super::OpacityAnimation::valid)
            || !self.font_size.is_finite()
            || !(self.font_size == 0.0
                || (CSS_MIN_FONT_SIZE..=CSS_MAX_FONT_SIZE).contains(&self.font_size))
            || !self.border.is_finite()
            || !(0.0..=CSS_MAX_BORDER_WIDTH).contains(&self.border)
            || self.font_families.len() > super::font_face::FONT_FAMILY_NAMES
            || self
                .font_families
                .iter()
                .any(|name| name.len() > super::font_face::FONT_NAME_BYTES)
            || self.percent_width && self.width.is_none()
            || self.display_flex && self.inline
            || self.display_grid && (self.inline || self.display_flex)
            || !self.layout_ok()
            || ![
                "left",
                "center",
                "right",
                "start",
                "end",
                "webkit-center",
                "justify",
            ]
            .contains(&self.align.as_str())
        {
            return Err(invalid());
        }
        if let FlexBasis::Pixels(value) | FlexBasis::Percent(value) = self.flex_basis
            && !scalar(value)
        {
            return Err(invalid());
        }
        let colors = [&self.color, &self.background, &self.border_color]
            .into_iter()
            .chain(&self.border_colors)
            .flatten()
            .chain(self.shadows.iter().map(|shadow| &shadow.color))
            .chain(
                self.backgrounds
                    .iter()
                    .flat_map(stops)
                    .map(|stop| &stop.color),
            );
        for value in colors {
            if !super::colors::literal(value) {
                return Err(invalid());
            }
        }
        self.validate_visual()
    }
}
impl ComputedStyle {
    /** Visual painter data stays finite, bounded and within its count limits. */
    fn validate_visual(&self) -> EngineResult<()> {
        let signed = |value: f64| value.is_finite() && value.abs() <= CSS_MAX_SCALAR;
        let shadows_ok = self.shadows.len() <= SHADOWS
            && self.shadows.iter().all(|shadow| {
                [shadow.x, shadow.y, shadow.spread].into_iter().all(signed) && scalar(shadow.blur)
            });
        let layers_ok = self.backgrounds.len() <= BACKGROUND_LAYERS
            && self.backgrounds.iter().all(|layer| match layer {
                BackgroundLayer::Linear { angle, stops, .. } => {
                    angle.is_finite() && stops_ok(stops)
                }
                BackgroundLayer::Radial { stops, shape, .. } => {
                    let extents = match shape.size {
                        super::RadialSize::Explicit(x, y) => {
                            [x, y, shape.center[0], shape.center[1]]
                        }
                        _ => [
                            shape.center[0],
                            shape.center[1],
                            shape.center[0],
                            shape.center[1],
                        ],
                    };
                    stops_ok(stops) && extents.iter().all(|e| signed(e.px) && signed(e.percent))
                }
                BackgroundLayer::Image { url } => url_ok(url),
            })
            && self.mask.as_ref().is_none_or(|mask| {
                url_ok(&mask.url)
                    && mask
                        .position
                        .iter()
                        .all(|e| signed(e.px) && signed(e.percent))
            });
        if !shadows_ok
            || !layers_ok
            || !self.border_image.valid()
            || !self.inset.iter().flatten().copied().all(signed)
            || !self.radius.iter().copied().all(scalar)
            || !self
                .border_widths
                .iter()
                .all(|w| w.is_finite() && (0.0..=CSS_MAX_BORDER_WIDTH).contains(w))
            || !(WEIGHT_MIN..=WEIGHT_MAX).contains(&self.font_weight)
            || !(self.letter_spacing.is_finite() && self.letter_spacing.abs() <= LETTER_SPACING_MAX)
            || self
                .transform
                .is_some_and(|m| !m.iter().all(|v| v.is_finite() && v.abs() <= TRANSFORM_MAX))
        {
            return Err(invalid());
        }
        Ok(())
    }
}
/** An authored image URL: present, bounded and free of control characters. */
fn url_ok(url: &str) -> bool {
    !url.is_empty() && url.len() <= URL_BYTES && !url.chars().any(char::is_control)
}
/** A gradient's stops, empty for other layers. */
fn stops(layer: &BackgroundLayer) -> &[ColorStop] {
    match layer {
        BackgroundLayer::Linear { stops, .. } | BackgroundLayer::Radial { stops, .. } => stops,
        BackgroundLayer::Image { .. } => &[],
    }
}
/** Stop counts stay bounded, and positions finite and within `GRADIENT_STOP_REACH` lines (CSS
 * allows stops before 0% and past 100%; the painter clips the ramp to the line). */
fn stops_ok(stops: &[ColorStop]) -> bool {
    let reach = super::paint::GRADIENT_STOP_REACH;
    stops.len() <= GRADIENT_STOPS
        && stops.iter().all(|stop| {
            stop.position
                .is_none_or(|p| (-reach..=1.0 + reach).contains(&p))
                && stop
                    .px
                    .is_none_or(|px| px.is_finite() && px.abs() <= CSS_MAX_SCALAR)
        })
}
/** CSS scalars share one finite nonnegative budget independently of their source spelling. */
fn scalar(value: f64) -> bool {
    value.is_finite() && (0.0..=CSS_MAX_SCALAR).contains(&value)
}
/** Value-free failures do not disclose private author declarations. */
fn invalid() -> EngineError {
    EngineError::invalid("Computed style values are outside supported bounds.")
}
impl ComputedStyle {
    /** Sizes, grid tracks and background geometry stay finite and bounded. */
    fn layout_ok(&self) -> bool {
        let extent = |e: &super::Extent| {
            [e.px, e.percent]
                .iter()
                .all(|v| v.is_finite() && v.abs() <= CSS_MAX_SCALAR)
        };
        let track_size = |size: &super::TrackSize| match size {
            super::TrackSize::Length(e) => extent(e),
            super::TrackSize::Fr(f) => scalar(*f),
            super::TrackSize::Auto => true,
            // Resolved at layout against the template's bounded expression list.
            super::TrackSize::Expr(_) => true,
            super::TrackSize::Larger(a, b) | super::TrackSize::Smaller(a, b) => {
                extent(a) && extent(b)
            }
        };
        let template = |t: &super::GridTemplate| {
            t.tracks.len() + t.repeat.len() <= super::constants::GRID_TRACKS
                && t.exprs.len() <= super::constants::GRID_TRACKS
                && t.repeat_at <= t.tracks.len().max(usize::from(t.subgrid_lines))
                && usize::from(t.subgrid_fill) <= super::constants::GRID_TRACKS
                && t.repeat_names.len() <= super::constants::GRID_TRACKS
                && t.repeat_names.iter().all(|(_, line)| {
                    // A subgrid's auto-fill names number its lists, a track group's its lines.
                    let group = t.repeat.len().max(usize::from(t.subgrid_fill));
                    *line >= 1 && usize::from(*line) <= group + 1
                })
                && t.tracks
                    .iter()
                    .chain(&t.repeat)
                    .all(|track| track_size(&track.min) && track_size(&track.max))
        };
        let size = match self.background_size {
            super::BackgroundSize::Explicit(w, h) => w.iter().chain(h.iter()).all(extent),
            _ => true,
        };
        [self.height, self.min_height, self.max_height]
            .iter()
            .flatten()
            .all(extent)
            && template(&self.grid_columns)
            && template(&self.grid_rows)
            && self.areas_ok()
            && self.added_ok()
            && size
            && self.background_position.iter().all(extent)
            && self.object_position.iter().all(extent)
            && self.generated.len() <= 2
            && self.generated.iter().all(|generated| {
                generated.content.chars().count() <= super::content::GENERATED_CHARS
                    && generated.style.generated.is_empty()
                    && generated.style.validate().is_ok()
            })
    }
    /** Self alignment, order, percentage sides, aspect ratio, clips and container names. */
    fn added_ok(&self) -> bool {
        use super::constants::{ASPECT_RATIO_MAX, CONTAINER_NAMES, GRID_NAME_BYTES, ORDER_MAX};
        let signed = |v: &f64| v.is_finite() && v.abs() <= CSS_MAX_SCALAR;
        let clip = |insets: &Option<[super::Extent; 4]>| {
            insets
                .iter()
                .flatten()
                .all(|e| signed(&e.px) && signed(&e.percent))
        };
        i64::from(self.order).abs() <= ORDER_MAX
            && self.padding_percent.iter().all(|p| scalar(*p))
            && self.margin_percent.iter().all(signed)
            && self.inset_percent.iter().all(signed)
            && self.radius_percent.iter().all(|p| scalar(*p))
            && self.radius_px.iter().all(signed)
            && self
                .radius_y
                .iter()
                .flatten()
                .all(|(px, percent)| signed(px) && signed(percent))
            && self.translate_percent.iter().all(signed)
            && self
                .aspect_ratio
                .is_none_or(|r| (1.0 / ASPECT_RATIO_MAX..=ASPECT_RATIO_MAX).contains(&r))
            && clip(&self.clip_rect)
            && clip(&self.clip_path)
            && self.container_names.len() <= CONTAINER_NAMES
            && self
                .container_names
                .iter()
                .all(|name| !name.is_empty() && name.len() <= GRID_NAME_BYTES)
    }
    /** Named grid areas stay inside the track bound with short names. */
    fn areas_ok(&self) -> bool {
        use super::constants::{GRID_NAME_BYTES, GRID_TRACKS};
        let fits = |start: u16, count: u16| {
            count > 0 && usize::from(start) + usize::from(count) <= GRID_TRACKS
        };
        self.grid_areas.len() <= GRID_TRACKS * GRID_TRACKS
            && self.grid_areas.iter().all(|area| {
                area.name.len() <= GRID_NAME_BYTES
                    && fits(area.row, area.rows)
                    && fits(area.column, area.columns)
            })
            && self
                .grid_area_names
                .iter()
                .flatten()
                .chain(
                    self.grid_line_names
                        .iter()
                        .flatten()
                        .flatten()
                        .map(|line| &line.name),
                )
                .all(|name| name.len() <= GRID_NAME_BYTES)
            && self
                .grid_line_names
                .iter()
                .flatten()
                .flatten()
                .all(|line| line.nth.abs() <= super::constants::GRID_LINE_MAX)
    }
}
