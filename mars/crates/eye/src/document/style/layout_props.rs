/** Box sizing, floats, inline alignment, lists, grid and background geometry longhands. */
use super::{
    AlignItems, BackgroundRepeat, BackgroundSize, Clear, ComputedStyle, Extent, Float, GridArea,
    GridLine, GridTemplate, LineName, ListStyle, ObjectFit, Track, TrackSize, VerticalAlign,
    constants::{ASPECT_RATIO_MAX, GRID_LINE_MAX, GRID_NAME_BYTES, GRID_TRACKS},
    syntax::{split_top, tokens},
    unsupported,
    values::{dimension, number},
};
use crate::{constants::CSS_MAX_SCALAR, document::RenderContext, protocol::EngineResult};

/** Applies one longhand this module owns, or reports that `key` belongs elsewhere. */
pub(super) fn apply(
    style: &mut ComputedStyle,
    key: &str,
    value: &str,
    context: RenderContext,
) -> EngineResult<bool> {
    let font = style.font_size;
    match key {
        "height" => style.height = size(value, font, context, &["auto"])?,
        "min-height" => style.min_height = size(value, font, context, &["auto", "0"])?,
        "max-height" => style.max_height = size(value, font, context, &["none"])?,
        "float" => style.float = float(value)?,
        "clear" => style.clear = clear(value)?,
        "vertical-align" => {
            style.vertical_align = vertical_align(value)?;
            style.vertical_shift = dimension(value, font, context)
                .ok()
                .map(|shift| (shift.px, shift.percent));
        }
        "list-style-type" => style.list_style = list_style(value)?,
        "list-style" => style.list_style = list_shorthand(value),
        "row-gap" | "column-gap" | "grid-row-gap" | "grid-column-gap" => {
            gap(style, key, value, context)?
        }
        "grid-template-columns" => style.grid_columns = template(value, font, context)?,
        "grid-template-rows" => style.grid_rows = template(value, font, context)?,
        "grid-auto-rows" => style.grid_auto_rows = auto_tracks(value, font, context)?,
        "grid-auto-columns" => style.grid_auto_columns = auto_tracks(value, font, context)?,
        "grid-row" => axis(style, 0, value)?,
        "grid-column" => axis(style, 1, value)?,
        "grid-column-start" => style.grid_column.start = line(value)?,
        "grid-row-start" => style.grid_row.start = line(value)?,
        "grid-column-end" => end(&mut style.grid_column, value)?,
        "grid-row-end" => end(&mut style.grid_row, value)?,
        "grid-template-areas" => style.grid_areas = areas(value)?,
        "justify-items" => style.justify_items = justify_items(value)?,
        "background-size" => style.background_size = background_size(value, font, context)?,
        "background-position" => {
            style.background_position = background_position(value, font, context)?
        }
        "background-repeat" => style.background_repeat = background_repeat(value)?,
        "object-fit" => style.object_fit = object_fit(value)?,
        "object-position" => style.object_position = background_position(value, font, context)?,
        "aspect-ratio" => {
            style.aspect_ratio = aspect_ratio(value)?;
            style.aspect_ratio_auto = value.split_whitespace().any(|part| part == "auto");
        }
        "overflow-wrap" => style.overflow_wrap_break = breaking(value, &["break-word", "anywhere"]),
        "word-break" => {
            style.word_break_all = breaking(value, &["break-all", "break-word"]);
            style.word_break_fill = breaking(value, &["break-all"]);
            style.word_break_keep_all = breaking(value, &["keep-all"]);
        }
        "line-break" => style.line_break_anywhere = breaking(value, &["anywhere"]),
        // The end-edge value; a two-value form's second (end) value is the one LTR lines use.
        "text-overflow" => style.text_overflow_ellipsis = tokens(value).last() == Some(&"ellipsis"),
        "container-type" => style.container_type = super::container::kind(value)?,
        "container-name" => style.container_names = super::container::names(value)?,
        // Generated content is read by the cascade for `::before` and `::after` boxes only.
        "content" => {}
        // Read by the cascade from the winning declarations; only `text` changes the paint.
        "background-clip" | "-webkit-background-clip" => {}
        "table-layout" => style.table_layout_fixed = value == "fixed",
        "border-collapse" => style.border_collapse = value == "collapse",
        "caption-side" => style.caption_bottom = value == "bottom",
        "border-spacing" => {
            let first = tokens(value).first().copied().ok_or_else(unsupported)?;
            style.border_spacing = extent(first, font, context)?.px.max(0.0);
        }
        _ => return Ok(false),
    }
    Ok(true)
}
/** Replaced image sizing keywords. */
fn object_fit(value: &str) -> EngineResult<ObjectFit> {
    Ok(match value {
        "fill" => ObjectFit::Fill,
        "contain" => ObjectFit::Contain,
        "cover" => ObjectFit::Cover,
        "none" => ObjectFit::None,
        "scale-down" => ObjectFit::ScaleDown,
        _ => return Err(unsupported()),
    })
}
/** An inherited line-breaking keyword: Some(true) for one of `breaking`, Some(false) for any
other value, None for a CSS-wide keyword that defers to the parent. */
fn breaking(value: &str, breaking: &[&str]) -> Option<bool> {
    let value = value.trim();
    if ["inherit", "unset", "revert", "revert-layer"].contains(&value) {
        return None;
    }
    Some(breaking.contains(&value))
}
/** `auto`, `<ratio>` or `auto && <ratio>`; the ratio is width over height, bounded both ways. */
fn aspect_ratio(value: &str) -> EngineResult<Option<f64>> {
    let ratio = value
        .split_whitespace()
        .filter(|part| *part != "auto")
        .collect::<String>();
    if ratio.is_empty() {
        return Ok(None);
    }
    let (width, height) = ratio.split_once('/').unwrap_or((&ratio, "1"));
    let ratio = number(width)? / number(height)?;
    if !(1.0 / ASPECT_RATIO_MAX..=ASPECT_RATIO_MAX).contains(&ratio) {
        return Err(unsupported());
    }
    Ok(Some(ratio))
}
/** A length or percentage; the listed keywords mean "no constraint". */
fn size(
    value: &str,
    font: f64,
    context: RenderContext,
    none: &[&str],
) -> EngineResult<Option<Extent>> {
    if none.contains(&value) || ["min-content", "max-content", "fit-content"].contains(&value) {
        return Ok(None);
    }
    let extent = extent(value, font, context)?;
    // A negative size is invalid, but a `calc()` mixing signs (`100% - 560px`) is valid and
    // clamps at zero when layout resolves it.
    if extent.px.max(extent.percent) <= 0.0 && extent.px.min(extent.percent) < 0.0 {
        return Err(unsupported());
    }
    Ok(Some(extent))
}
/** Any bounded length, percentage or calc() of both. */
pub(super) fn extent(value: &str, font: f64, context: RenderContext) -> EngineResult<Extent> {
    let dimension = dimension(value, font, context)?;
    let bounded = |v: f64| v.is_finite() && v.abs() <= CSS_MAX_SCALAR;
    if !bounded(dimension.px) || !bounded(dimension.percent) {
        return Err(unsupported());
    }
    Ok(Extent {
        px: dimension.px,
        percent: dimension.percent,
    })
}
/** Float keywords; logical sides map to physical ones for left-to-right text. */
fn float(value: &str) -> EngineResult<Float> {
    Ok(match value {
        "none" => Float::None,
        "left" | "inline-start" => Float::Left,
        "right" | "inline-end" => Float::Right,
        _ => return Err(unsupported()),
    })
}
/** Clear keywords. */
fn clear(value: &str) -> EngineResult<Clear> {
    Ok(match value {
        "none" => Clear::None,
        "left" | "inline-start" => Clear::Left,
        "right" | "inline-end" => Clear::Right,
        "both" => Clear::Both,
        _ => return Err(unsupported()),
    })
}
/** Vertical alignment keywords; lengths and percentages behave as baseline. */
fn vertical_align(value: &str) -> EngineResult<VerticalAlign> {
    Ok(match value {
        "middle" => VerticalAlign::Middle,
        "top" => VerticalAlign::Top,
        "bottom" => VerticalAlign::Bottom,
        "text-top" => VerticalAlign::TextTop,
        "text-bottom" => VerticalAlign::TextBottom,
        "sub" => VerticalAlign::Sub,
        "super" => VerticalAlign::Super,
        _ => VerticalAlign::Baseline,
    })
}
/** Marker kinds; other counter styles fall back to the nearest painted one. */
fn list_style(value: &str) -> EngineResult<ListStyle> {
    Ok(match value {
        "none" => ListStyle::None,
        "disc" => ListStyle::Disc,
        "circle" => ListStyle::Circle,
        "square" => ListStyle::Square,
        "decimal"
        | "decimal-leading-zero"
        | "lower-alpha"
        | "upper-alpha"
        | "lower-latin"
        | "upper-latin"
        | "lower-roman"
        | "upper-roman"
        | "lower-greek" => ListStyle::Decimal,
        _ => return Err(unsupported()),
    })
}
/** The type keyword of a `list-style` shorthand; images and positions are ignored. */
fn list_shorthand(value: &str) -> ListStyle {
    tokens(value)
        .into_iter()
        .find_map(|token| list_style(token).ok())
        .unwrap_or_default()
}
/** Gaps accept any length and percentage; grid layout resolves the percentage part. */
fn gap(
    style: &mut ComputedStyle,
    key: &str,
    value: &str,
    context: RenderContext,
) -> EngineResult<()> {
    let (px, percent) = if value == "normal" {
        (0.0, 0.0)
    } else {
        let extent = extent(value, style.font_size, context)?;
        (extent.px, extent.percent)
    };
    // ponytail: the length part clamps on its own, so `calc(10% - 5px)` loses its negative
    // part; keep it signed when flex layout resolves percentage gaps too.
    let px = px.max(0.0);
    let axis = usize::from(key.contains("row"));
    style.gap_percent[axis] = percent.max(0.0);
    style.gap_set[axis] = value != "normal";
    if axis == 1 {
        style.row_gap = px;
    } else {
        style.column_gap = px;
    }
    Ok(())
}
/** `grid-auto-*` track lists, which take no `subgrid`. */
fn auto_tracks(value: &str, font: f64, context: RenderContext) -> EngineResult<GridTemplate> {
    let template = template(value, font, context)?;
    if template.subgrid {
        return Err(unsupported());
    }
    Ok(template)
}
/** Track lists with lengths, `fr`, `auto`, `minmax()`, `repeat()` and line names skipped. */
fn template(value: &str, font: f64, context: RenderContext) -> EngineResult<GridTemplate> {
    let mut template = GridTemplate::default();
    if value == "none" {
        return Ok(template);
    }
    if let Some(rest) = value.strip_prefix("subgrid")
        && (rest.is_empty() || rest.starts_with([' ', '[', '\t', '\n']))
    {
        return subgrid(rest);
    }
    super::flex::begin_track_exprs();
    for token in split_tracks(value) {
        if let Some(names) = token
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
        {
            // An auto-repeat group counts once here; layout moves the names past it.
            let line = template.tracks.len() + template.repeat.len() + 1;
            let line = u16::try_from(line).map_err(|_| unsupported())?;
            push_names(&mut template.names, names, line);
            continue;
        }
        if let Some(inner) = token
            .strip_prefix("repeat(")
            .and_then(|rest| rest.strip_suffix(')'))
        {
            repeat(inner, font, context, &mut template)?;
        } else {
            template.tracks.push(track(token, font, context)?);
        }
        if template.tracks.len() + template.repeat.len() > GRID_TRACKS {
            return Err(unsupported());
        }
    }
    template.exprs = super::flex::take_track_exprs();
    Ok(template)
}
/**
 * `subgrid` followed by line-name lists, `repeat(<integer>, <lists>)` among them (css-grid-2
 * 7.2, Blink's `ConsumeGridLineNames` in subgrid mode). Line `n` is the `n`th list; any track
 * size or `repeat(auto-fill, ...)` beyond the first makes the declaration invalid.
 */
fn subgrid(rest: &str) -> EngineResult<GridTemplate> {
    let mut lists: Vec<Vec<&str>> = Vec::new();
    let mut auto_fill = false;
    for token in split_tracks(rest) {
        if let Some(inner) = token
            .strip_prefix("repeat(")
            .and_then(|rest| rest.strip_suffix(')'))
        {
            let parts = split_top(inner, b',');
            let [count, body] = parts.as_slice() else {
                return Err(unsupported());
            };
            let group = name_lists(body)?;
            let count = count.trim();
            let times = if count == "auto-fill" && !auto_fill {
                // ponytail: an auto-fill group of names repeats once; Blink fills the span.
                auto_fill = true;
                1
            } else {
                let times = number(count)?;
                if times < 1.0 || times.fract() != 0.0 {
                    return Err(unsupported());
                }
                times as usize
            };
            if group.is_empty() || lists.len() + times * group.len() > GRID_TRACKS {
                return Err(unsupported());
            }
            for _ in 0..times {
                lists.extend(group.iter().cloned());
            }
        } else {
            lists.extend(name_lists(token)?);
        }
    }
    let mut template = GridTemplate {
        subgrid: true,
        subgrid_lines: u16::try_from(lists.len()).map_err(|_| unsupported())?,
        ..GridTemplate::default()
    };
    for (index, list) in lists.iter().enumerate() {
        for name in list {
            template.names.push(((*name).to_owned(), index as u16 + 1));
        }
    }
    Ok(template)
}
/** Consecutive `[...]` lists with their names; anything else is not a subgrid line list. */
fn name_lists(value: &str) -> EngineResult<Vec<Vec<&str>>> {
    split_tracks(value)
        .into_iter()
        .map(|token| {
            let names = token
                .strip_prefix('[')
                .and_then(|rest| rest.strip_suffix(']'))
                .ok_or_else(unsupported)?;
            let names = names.split_whitespace().collect::<Vec<_>>();
            if names.iter().all(|name| self::name(name)) {
                Ok(names)
            } else {
                Err(unsupported())
            }
        })
        .collect()
}
/**
 * Tokens that keep parenthesized functions and bracketed names whole. A closing `)` or `]` ends
 * a token too, since minified sheets write `minmax(0,1fr)max(340px,50%)` with no space.
 */
fn split_tracks(value: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let (mut depth, mut start) = (0usize, None);
    for (index, c) in value.char_indices() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            _ => {}
        }
        if c.is_whitespace() && depth == 0 {
            if let Some(begin) = start.take() {
                parts.push(&value[begin..index]);
            }
        } else if start.is_none() {
            start = Some(index);
        }
        if matches!(c, ')' | ']')
            && depth == 0
            && let Some(begin) = start.take()
        {
            parts.push(&value[begin..index + 1]);
        }
    }
    if let Some(begin) = start {
        parts.push(&value[begin..]);
    }
    parts
}
/** `repeat(count, tracks)` expands in place; auto-fill and auto-fit are kept for layout. */
fn repeat(
    inner: &str,
    font: f64,
    context: RenderContext,
    template: &mut GridTemplate,
) -> EngineResult<()> {
    let parts = split_top(inner, b',');
    let [count, body] = parts.as_slice() else {
        return Err(unsupported());
    };
    // The group's tracks, and its line names numbered from 1 at the line before its first track.
    let mut tracks = Vec::new();
    let mut names = Vec::new();
    for token in split_tracks(body) {
        match token
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
        {
            Some(list) => {
                let line = u16::try_from(tracks.len() + 1).map_err(|_| unsupported())?;
                push_names(&mut names, list, line);
            }
            None => tracks.push(track(token, font, context)?),
        }
    }
    let count = count.trim();
    if ["auto-fill", "auto-fit"].contains(&count) {
        template.repeat_at = template.tracks.len();
        template.repeat = tracks;
        template.repeat_fit = count == "auto-fit";
        template.repeat_names = names;
        return Ok(());
    }
    let times = number(count)? as usize;
    if times == 0 || times * tracks.len() > GRID_TRACKS {
        return Err(unsupported());
    }
    for _ in 0..times {
        let before = template.tracks.len() + template.repeat.len();
        for (name, offset) in &names {
            let line = u16::try_from(before).map_err(|_| unsupported())? + offset;
            push_names(&mut template.names, name, line);
        }
        template.tracks.extend_from_slice(&tracks);
    }
    Ok(())
}
/** Adds the valid names of a `[...]` list at `line`, up to `GRID_TRACKS` names in all. */
fn push_names(names: &mut Vec<(String, u16)>, list: &str, line: u16) {
    for name in list.split_whitespace().filter(|name| self::name(name)) {
        if names.len() < GRID_TRACKS {
            names.push((name.to_owned(), line));
        }
    }
}
/** One track: a size or `minmax(min, max)`; `fit-content()` behaves as auto. */
fn track(token: &str, font: f64, context: RenderContext) -> EngineResult<Track> {
    if let Some(inner) = token
        .strip_prefix("minmax(")
        .and_then(|rest| rest.strip_suffix(')'))
    {
        let parts = split_top(inner, b',');
        let [min, max] = parts.as_slice() else {
            return Err(unsupported());
        };
        let content = track_content(min.trim(), max.trim(), font, context);
        let min = match track_size(min.trim(), font, context)? {
            TrackSize::Fr(_) => TrackSize::Auto,
            other => other,
        };
        return Ok(Track {
            min,
            max: track_size(max.trim(), font, context)?,
            content,
        });
    }
    let size = track_size(token, font, context)?;
    let min = match size {
        TrackSize::Fr(_) => TrackSize::Auto,
        other => other,
    };
    let content = track_content(token, token, font, context);
    Ok(Track {
        min,
        max: size,
        content,
    })
}
/** The content keywords of a track's two bounds; `fit-content(L)` has an `auto` minimum and a
 * max-content maximum limited to `L`. */
fn track_content(
    min: &str,
    max: &str,
    font: f64,
    context: RenderContext,
) -> super::visual_model::TrackContent {
    use super::visual_model::ContentKeyword;
    let keyword = |token: &str| match token {
        "min-content" => ContentKeyword::MinContent,
        "max-content" => ContentKeyword::MaxContent,
        _ => ContentKeyword::Auto,
    };
    let fit_limit = max
        .strip_prefix("fit-content(")
        .and_then(|rest| rest.strip_suffix(')'))
        .and_then(|limit| extent(limit.trim(), font, context).ok());
    let min = if min.starts_with("fit-content(") {
        ContentKeyword::Auto
    } else {
        keyword(min)
    };
    super::visual_model::TrackContent {
        min,
        max: keyword(max),
        fit_limit,
    }
}
/** A single track size keyword, flexible factor or length. */
fn track_size(token: &str, font: f64, context: RenderContext) -> EngineResult<TrackSize> {
    if ["auto", "min-content", "max-content"].contains(&token) || token.starts_with("fit-content(")
    {
        return Ok(TrackSize::Auto);
    }
    if let Some(factor) = token.strip_suffix("fr") {
        return Ok(TrackSize::Fr(number(factor)?));
    }
    match extent(token, font, context) {
        Ok(length) => Ok(TrackSize::Length(length)),
        Err(error) => mixed(token, font, context)
            .or_else(|| super::flex::track_expr_size(token, font, context))
            .ok_or(error),
    }
}
/** Two-argument `max()` or `min()` mixing lengths and percentages, kept for the grid to decide. */
fn mixed(token: &str, font: f64, context: RenderContext) -> Option<TrackSize> {
    let (larger, inner) = match token.strip_suffix(')')? {
        rest if rest.starts_with("max(") => (true, &rest[4..]),
        rest if rest.starts_with("min(") => (false, &rest[4..]),
        _ => return None,
    };
    let parts = split_top(inner, b',');
    let [a, b] = parts.as_slice() else {
        return None;
    };
    let (a, b) = (
        extent(a.trim(), font, context).ok()?,
        extent(b.trim(), font, context).ok()?,
    );
    Some(if larger {
        TrackSize::Larger(a, b)
    } else {
        TrackSize::Smaller(a, b)
    })
}
/** `start / end`, `span n`, `start / span n` or a single line. */
fn placement(value: &str) -> EngineResult<GridLine> {
    let mut parts = value.split('/');
    let first = parts.next().unwrap_or("").trim();
    let mut placed = GridLine::default();
    if let Some(span) = span(first)? {
        placed.span = span;
    } else {
        placed.start = line(first)?;
    }
    if let Some(second) = parts.next() {
        end(&mut placed, second.trim())?;
    }
    Ok(placed)
}
/** `span n`, or None for a non-span value. A named span (`span a`, `span 2 a`) is a span of 1
 * here, which is what it places as with no definite line (css-grid-2 8.3.1); its name and count
 * are kept by `line_name`. */
fn span(value: &str) -> EngineResult<Option<u16>> {
    let Some(count) = value.strip_prefix("span") else {
        return Ok(None);
    };
    if line_name(value).is_some() {
        return Ok(Some(1));
    }
    let count = count.trim();
    let count = if count.is_empty() {
        1.0
    } else {
        number(count)?
    };
    Ok(Some((count as u16).clamp(1, GRID_TRACKS as u16)))
}
/** One side's named line: `name`, `name <integer>` (either order), or `span` with a name and
 * an optional positive count; None for a number, `auto` or anything invalid. */
fn line_name(value: &str) -> Option<LineName> {
    let mut parts = value.split_whitespace().peekable();
    let span = parts.next_if_eq(&"span").is_some();
    let (mut ident, mut nth) = (None, None);
    for part in parts {
        match part.parse::<i32>() {
            Ok(n) if nth.is_none() && n != 0 && (!span || n > 0) => nth = Some(n),
            Err(_) if ident.is_none() && !["auto", "span"].contains(&part) && name(part) => {
                ident = Some(part)
            }
            _ => return None,
        }
    }
    let limit = if span {
        GRID_TRACKS as i32
    } else {
        GRID_LINE_MAX
    };
    Some(LineName {
        name: ident?.to_owned(),
        nth: nth.unwrap_or(i32::from(span)).clamp(-limit, limit),
        span,
    })
}
/** A numeric line, or None for `auto` and named lines. */
fn line(value: &str) -> EngineResult<Option<i32>> {
    let value = value.trim();
    if value == "auto" || value.chars().any(|c| c.is_ascii_alphabetic()) {
        return Ok(None);
    }
    let number: i32 = value.parse().map_err(|_| unsupported())?;
    Ok((number != 0).then_some(number.clamp(-GRID_LINE_MAX, GRID_LINE_MAX)))
}
/** An end line or span after a start. */
fn end(placed: &mut GridLine, value: &str) -> EngineResult<()> {
    if let Some(span) = span(value)? {
        placed.span = span;
    } else {
        placed.end = line(value)?;
    }
    Ok(())
}
/** `grid-row` or `grid-column`: a named area of the container, or line placement. */
fn axis(style: &mut ComputedStyle, index: usize, value: &str) -> EngineResult<()> {
    let named = value != "auto" && name(value);
    let line = if named {
        GridLine::default()
    } else {
        placement(value)?
    };
    style.grid_area_names[index] = named.then(|| value.into());
    // Named lines (`name-start / name-end`, `a 2`, `span a`); numbers and spans stay in `line`.
    let mut parts = value.split('/').map(str::trim);
    style.grid_line_names[index] =
        [parts.next(), parts.next()].map(|part| part.filter(|_| !named).and_then(line_name));
    if index == 0 {
        style.grid_row = line;
    } else {
        style.grid_column = line;
    }
    Ok(())
}
/** Item alignment keywords inside grid areas. */
fn justify_items(value: &str) -> EngineResult<AlignItems> {
    Ok(match value {
        "start" | "flex-start" | "self-start" | "left" => AlignItems::Start,
        "center" => AlignItems::Center,
        "end" | "flex-end" | "self-end" | "right" => AlignItems::End,
        "stretch" | "normal" | "legacy" => AlignItems::Stretch,
        _ => return Err(unsupported()),
    })
}
/** `mask-size`, `mask-position` or `mask-repeat` into the mask layer, parsed as their background twins. */
pub(super) fn mask_geometry(
    layer: &mut super::MaskLayer,
    key: &str,
    value: &str,
    font: f64,
    context: RenderContext,
) -> EngineResult<()> {
    match key {
        "mask-size" => layer.size = background_size(value, font, context)?,
        "mask-position" => layer.position = background_position(value, font, context)?,
        _ => layer.repeat = background_repeat(value)?,
    }
    Ok(())
}
/** `cover`, `contain`, or one or two lengths of the first layer. */
fn background_size(value: &str, font: f64, context: RenderContext) -> EngineResult<BackgroundSize> {
    let first = split_top(value, b',')
        .into_iter()
        .next()
        .unwrap_or_default();
    let parts = tokens(first.trim());
    let one = |token: &str| -> EngineResult<Option<Extent>> {
        if token == "auto" {
            Ok(None)
        } else {
            extent(token, font, context).map(Some)
        }
    };
    Ok(match parts.as_slice() {
        ["cover"] => BackgroundSize::Cover,
        ["contain"] => BackgroundSize::Contain,
        [width] => BackgroundSize::Explicit(one(width)?, None),
        [width, height] => BackgroundSize::Explicit(one(width)?, one(height)?),
        _ => return Err(unsupported()),
    })
}
/** Keyword, length and edge-offset positions of the first layer; see `paint::position`. */
pub(super) fn background_position(
    value: &str,
    font: f64,
    context: RenderContext,
) -> EngineResult<[Extent; 2]> {
    super::paint::position(value, font, context)
}
/** Tiling keywords of the first layer; `space` and `round` tile like repeat. */
fn background_repeat(value: &str) -> EngineResult<BackgroundRepeat> {
    let first = split_top(value, b',')
        .into_iter()
        .next()
        .unwrap_or_default();
    Ok(match tokens(first.trim()).as_slice() {
        ["no-repeat"] | ["no-repeat", "no-repeat"] => BackgroundRepeat::NoRepeat,
        ["repeat-x"] | ["repeat", "no-repeat"] => BackgroundRepeat::RepeatX,
        ["repeat-y"] | ["no-repeat", "repeat"] => BackgroundRepeat::RepeatY,
        ["repeat"] | ["repeat", "repeat"] => BackgroundRepeat::Repeat,
        [one] => BackgroundRepeat::Axes(repeat_style(one)?, repeat_style(one)?),
        [x, y] => BackgroundRepeat::Axes(repeat_style(x)?, repeat_style(y)?),
        _ => return Err(unsupported()),
    })
}
/** One `background-repeat` axis keyword. */
fn repeat_style(value: &str) -> EngineResult<super::RepeatStyle> {
    use super::RepeatStyle;
    Ok(match value {
        "repeat" => RepeatStyle::Repeat,
        "no-repeat" => RepeatStyle::NoRepeat,
        "space" => RepeatStyle::Space,
        "round" => RepeatStyle::Round,
        _ => return Err(unsupported()),
    })
}
/** A single custom identifier such as an area name. */
pub(super) fn name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= GRID_NAME_BYTES
        && value
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        && !value.starts_with(|c: char| c.is_ascii_digit())
        // CSS Syntax: `-` followed by a digit starts a number (`-1`), not an identifier.
        && !value
            .strip_prefix('-')
            .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
}
/**
 * `grid-template-areas`: one quoted string per row, equal cell counts, and every name one
 * filled rectangle; anything else is invalid, as in CSS. `.` cells belong to no area.
 */
fn areas(value: &str) -> EngineResult<Vec<GridArea>> {
    if value == "none" {
        return Ok(Vec::new());
    }
    let mut grid: Vec<Vec<&str>> = Vec::new();
    for row in tokens(value) {
        let inner = row
            .strip_prefix(['"', '\''])
            .and_then(|rest| rest.strip_suffix(['"', '\'']))
            .ok_or_else(unsupported)?;
        grid.push(inner.split_whitespace().collect());
    }
    let width = grid.first().map_or(0, Vec::len);
    if grid.is_empty() || width == 0 || width > GRID_TRACKS || grid.len() > GRID_TRACKS {
        return Err(unsupported());
    }
    if grid.iter().any(|row| row.len() != width) {
        return Err(unsupported());
    }
    let mut out: Vec<GridArea> = Vec::new();
    for (r, row) in grid.iter().enumerate() {
        for (c, cell) in row.iter().enumerate() {
            if cell.starts_with('.') || out.iter().any(|area| area.name == *cell) {
                continue;
            }
            out.push(rectangle(&grid, cell, r, c)?);
        }
    }
    Ok(out)
}
/** The rectangle a name fills from its first cell; a name that is not one rectangle is invalid. */
fn rectangle(grid: &[Vec<&str>], cell: &str, row: usize, column: usize) -> EngineResult<GridArea> {
    if !name(cell) {
        return Err(unsupported());
    }
    let columns = grid[row][column..]
        .iter()
        .take_while(|c| **c == cell)
        .count();
    let rows = grid[row..]
        .iter()
        .take_while(|line| line[column] == cell)
        .count();
    let count = grid.iter().flatten().filter(|c| **c == cell).count();
    let filled = grid[row..row + rows]
        .iter()
        .all(|line| line[column..column + columns].iter().all(|c| *c == cell));
    if count != rows * columns || !filled {
        return Err(unsupported());
    }
    let index = |n: usize| n as u16;
    Ok(GridArea {
        name: cell.into(),
        row: index(row),
        column: index(column),
        rows: index(rows),
        columns: index(columns),
    })
}
