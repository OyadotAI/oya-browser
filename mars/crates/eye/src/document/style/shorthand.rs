/** Shorthands expand at their original declaration position before independent cascade winners are chosen. */
use super::{
    flex,
    syntax::{split_top, tokens},
    values::number,
};
/** Clockwise side names. */
pub(super) const SIDES: [&str; 4] = ["top", "right", "bottom", "left"];
/** Clockwise corner names from top-left. */
const CORNERS: [&str; 4] = ["top-left", "top-right", "bottom-right", "bottom-left"];
/** Border keyword styles. */
pub(super) const BORDER_STYLES: [&str; 10] = [
    "none", "hidden", "dotted", "dashed", "solid", "double", "groove", "ridge", "inset", "outset",
];
/** Longhand list of a shorthand, for CSS-wide keywords; None when `key` is not a shorthand. */
pub(super) fn longhands(key: &str) -> Option<Vec<String>> {
    let sides = |format: &dyn Fn(&str) -> String| Some(SIDES.iter().map(|s| format(s)).collect());
    match key {
        "margin" | "padding" => sides(&|side| format!("{key}-{side}")),
        "inset" => sides(&|side| side.to_string()),
        "border-width" | "border-style" | "border-color" => {
            let part = &key["border-".len()..];
            sides(&|side| format!("border-{side}-{part}"))
        }
        "border" => Some(
            SIDES
                .iter()
                .flat_map(|side| ["width", "style", "color"].map(|p| format!("border-{side}-{p}")))
                .collect(),
        ),
        "border-top" | "border-right" | "border-bottom" | "border-left" => Some(
            ["width", "style", "color"]
                .map(|p| format!("{key}-{p}"))
                .into(),
        ),
        "border-radius" => Some(
            CORNERS
                .iter()
                .map(|c| format!("border-{c}-radius"))
                .collect(),
        ),
        "overflow" => Some(vec!["overflow-x".into(), "overflow-y".into()]),
        "animation" => Some(super::animation::LONGHANDS.map(Into::into).into()),
        "mask" => Some(MASK.map(Into::into).into()),
        "grid-area" => Some(vec!["grid-row".into(), "grid-column".into()]),
        "container" => Some(vec!["container-name".into(), "container-type".into()]),
        "grid-template" | "grid" => Some(GRID_TEMPLATE.map(Into::into).into()),
        "list-style" => Some(vec!["list-style-type".into()]),
        "gap" => Some(vec!["row-gap".into(), "column-gap".into()]),
        "flex" => Some(
            ["flex-grow", "flex-shrink", "flex-basis"]
                .map(Into::into)
                .into(),
        ),
        "background" => Some(
            [
                "background-color",
                "background-image",
                "background-size",
                "background-position",
                "background-repeat",
            ]
            .map(Into::into)
            .into(),
        ),
        "font" => Some(
            [
                "font-weight",
                "font-style",
                "font-size",
                "line-height",
                "font-family",
            ]
            .map(Into::into)
            .into(),
        ),
        _ => super::logical::longhands(key),
    }
}
/**
 * Expanded longhands keep one declaration's importance and order. Err(true) is a known
 * shorthand with a bad value; Err(false) never happens for non-shorthands, which pass through.
 */
pub(super) fn expand(key: &str, raw: &str) -> Result<Vec<(String, String)>, bool> {
    if super::declaration::wide(raw) {
        let longhands = longhands(key).unwrap_or_else(|| vec![key.into()]);
        return Ok(longhands
            .into_iter()
            .map(|name| (name, raw.into()))
            .collect());
    }
    let value = raw.to_ascii_lowercase();
    let value = value.as_str();
    let expanded = match key {
        "flex" => flex_values(value),
        "font" => super::font::shorthand(value).ok(),
        "gap" => gap_values(value),
        "margin" | "padding" => four(value).map(|v| sided(&|side| format!("{key}-{side}"), v)),
        "inset" => four(value).map(|v| sided(&|side| side.to_string(), v)),
        "border-width" | "border-style" | "border-color" => {
            let part = &key["border-".len()..];
            four(value).map(|v| sided(&|side| format!("border-{side}-{part}"), v))
        }
        "border" => border_side(value).map(|parts| {
            SIDES
                .iter()
                .flat_map(|side| side_parts(&format!("border-{side}"), &parts))
                .collect()
        }),
        "border-top" | "border-right" | "border-bottom" | "border-left" => {
            border_side(value).map(|parts| side_parts(key, &parts))
        }
        "border-radius" => radius(value),
        "overflow" => overflow(value),
        "list-style" => list_style(value),
        "background" => background(raw),
        "animation" => super::animation::shorthand(raw),
        "mask" => Some(mask(raw)),
        "grid-template" | "grid" => grid_template(value),
        "grid-area" => grid_area(value),
        "container" => super::container::shorthand(value),
        _ => match super::logical::expand(key, value) {
            Some(expanded) => expanded,
            None => return Ok(vec![(key.into(), raw.into())]),
        },
    };
    expanded.ok_or(true)
}
/**
 * `grid-area: row-start / column-start / row-end / column-end` as `grid-row` and `grid-column`;
 * a single name names both axes. It is a shorthand so it competes with those longhands in the
 * cascade by order and specificity, as in CSS.
 */
fn grid_area(value: &str) -> Option<Vec<(String, String)>> {
    let parts = value.split('/').map(str::trim).collect::<Vec<_>>();
    if parts.is_empty() || parts.len() > 4 || parts.iter().any(|part| part.is_empty()) {
        return None;
    }
    let first = parts[0];
    let named = |part: &str| super::layout_props::name(part) && part != "auto";
    let pick = |index: usize, fallback: &str| {
        parts
            .get(index)
            .map_or(fallback.to_string(), |part| part.to_string())
    };
    let column = pick(1, if named(first) { first } else { "auto" });
    let row_end = pick(2, if named(first) { first } else { "auto" });
    let column_end = pick(3, if named(&column) { &column } else { "auto" });
    let line = |start: String, end: String| {
        if named(&start) && start == end {
            start
        } else {
            format!("{start} / {end}")
        }
    };
    Some(vec![
        ("grid-row".into(), line(first.into(), row_end)),
        ("grid-column".into(), line(column.clone(), column_end)),
    ])
}
/** What `grid-template` and `grid` set. */
const GRID_TEMPLATE: [&str; 3] = [
    "grid-template-rows",
    "grid-template-columns",
    "grid-template-areas",
];
/**
 * `rows / columns`, or area strings each followed by an optional row size, then `/ columns`.
 * The `grid` auto-flow forms are not supported and invalidate the declaration.
 */
fn grid_template(value: &str) -> Option<Vec<(String, String)>> {
    let pair = |rows: String, columns: String, areas: String| {
        Some(
            GRID_TEMPLATE
                .into_iter()
                .map(Into::into)
                .zip([rows, columns, areas])
                .collect(),
        )
    };
    if value == "none" {
        return pair("none".into(), "none".into(), "none".into());
    }
    if value.contains("auto-flow") {
        return None;
    }
    let parts = split_top(value, b'/');
    let [rows, columns] = parts.as_slice() else {
        return None;
    };
    if !rows.contains(['"', '\'']) {
        return pair(rows.to_string(), columns.to_string(), "none".into());
    }
    let (mut sizes, mut strings) = (Vec::new(), Vec::new());
    for token in tokens(rows) {
        if token.starts_with(['"', '\'']) {
            strings.push(token);
            sizes.push("auto");
        } else if !token.starts_with('[') {
            *sizes.last_mut()? = token;
        }
    }
    pair(sizes.join(" "), columns.to_string(), strings.join(" "))
}
/** Omitted shorthand basis is zero percent; an explicit third numeric zero is zero pixels. */
fn flex_values(value: &str) -> Option<Vec<(String, String)>> {
    let (grow, shrink, basis) = match value {
        "none" => ("0".into(), "0".into(), "auto".into()),
        "initial" => ("0".into(), "1".into(), "auto".into()),
        "auto" => ("1".into(), "1".into(), "auto".into()),
        _ => parse_flex(value)?,
    };
    Some(vec![
        ("flex-grow".into(), grow),
        ("flex-shrink".into(), shrink),
        ("flex-basis".into(), basis),
    ])
}
/** Blink's factor-first ambiguity rule admits a basis before both factors or after their completed pair. */
fn parse_flex(value: &str) -> Option<(String, String, String)> {
    let (mut grow, mut shrink, mut basis) = (None, None, None);
    for token in value.split_whitespace() {
        if let Ok(factor) = number(token) {
            if grow.is_none() {
                grow = Some(token.to_owned());
            } else if shrink.is_none() {
                shrink = Some(token.to_owned());
            } else if basis.is_none() && factor == 0.0 {
                basis = Some("0px".into());
            } else {
                return None;
            }
        } else if basis.is_none() {
            flex::basis(token).ok()?;
            if grow.is_some() && shrink.is_none() {
                shrink = Some("1".into());
            }
            basis = Some(token.to_owned());
        } else {
            return None;
        }
    }
    if grow.is_none() && shrink.is_none() && basis.is_none() {
        return None;
    }
    Some((
        grow.unwrap_or_else(|| "1".into()),
        shrink.unwrap_or_else(|| "1".into()),
        basis.unwrap_or_else(|| "0%".into()),
    ))
}
/** A single gap sets both axes while two values retain row-then-column order. */
fn gap_values(value: &str) -> Option<Vec<(String, String)>> {
    let (row, column) = match tokens(value).as_slice() {
        [all] => (all.to_string(), all.to_string()),
        [row, column] => (row.to_string(), column.to_string()),
        _ => return None,
    };
    Some(vec![("row-gap".into(), row), ("column-gap".into(), column)])
}
/** One to four clockwise values. */
fn four(value: &str) -> Option<[String; 4]> {
    let values = tokens(value);
    let [a, b, c, d] = match values.as_slice() {
        [a] => [*a; 4],
        [a, b] => [*a, *b, *a, *b],
        [a, b, c] => [*a, *b, *c, *b],
        [a, b, c, d] => [*a, *b, *c, *d],
        _ => return None,
    };
    Some([a, b, c, d].map(Into::into))
}
/** Four clockwise values named per side. */
fn sided(name: &dyn Fn(&str) -> String, values: [String; 4]) -> Vec<(String, String)> {
    SIDES.iter().map(|side| name(side)).zip(values).collect()
}
/** A border side's width, style and color in any order, each optional. */
pub(super) fn border_side(value: &str) -> Option<[String; 3]> {
    let (mut width, mut style, mut color) = (None, None, None);
    for token in tokens(value) {
        if BORDER_STYLES.contains(&token) && style.is_none() {
            style = Some(token);
        } else if width.is_none() && super::border::width(token).is_ok() {
            width = Some(token);
        } else if color.is_none() {
            color = Some(token);
        } else {
            return None;
        }
    }
    Some([
        width.unwrap_or("medium").into(),
        style.unwrap_or("none").into(),
        color.unwrap_or("currentcolor").into(),
    ])
}
/** The three longhands of one side. */
fn side_parts(prefix: &str, [width, style, color]: &[String; 3]) -> Vec<(String, String)> {
    vec![
        (format!("{prefix}-width"), width.clone()),
        (format!("{prefix}-style"), style.clone()),
        (format!("{prefix}-color"), color.clone()),
    ]
}
/** Horizontal radii only; the elliptical part after `/` is approximated away. */
fn radius(value: &str) -> Option<Vec<(String, String)>> {
    let horizontal = value.split('/').next()?;
    let values = four(horizontal)?;
    Some(
        CORNERS
            .iter()
            .map(|corner| format!("border-{corner}-radius"))
            .zip(values)
            .collect(),
    )
}
/** One value sets both axes; two set x then y. */
fn overflow(value: &str) -> Option<Vec<(String, String)>> {
    let (x, y) = match tokens(value).as_slice() {
        [all] => (all.to_string(), all.to_string()),
        [x, y] => (x.to_string(), y.to_string()),
        _ => return None,
    };
    Some(vec![("overflow-x".into(), x), ("overflow-y".into(), y)])
}
/** The final layer's color, every layer's image, and the first layer's size, position and repeat. */
fn background(raw: &str) -> Option<Vec<(String, String)>> {
    let layers = split_top(raw, b',');
    let mut images = Vec::new();
    let mut color = None;
    let mut geometry: Option<(Vec<String>, Vec<String>, Vec<String>)> = None;
    for (index, layer) in layers.iter().enumerate() {
        let mut image = None;
        let (mut position, mut size, mut repeat) = (Vec::new(), Vec::new(), Vec::new());
        let mut after_slash = false;
        for token in tokens(layer) {
            let lower = token.to_ascii_lowercase();
            if lower.starts_with("url(") || lower.contains("gradient(") || lower == "none" {
                image = Some(token.to_string());
            } else if REPEATS.contains(&lower.as_str()) {
                repeat.push(lower);
            } else if index + 1 == layers.len()
                && super::values::contextual_color(&lower, crate::document::Appearance::Light, None)
                    .is_ok()
            {
                color = Some(lower);
            } else {
                for (part, piece) in lower.split('/').enumerate() {
                    if part > 0 {
                        after_slash = true;
                    }
                    if piece.is_empty() {
                        continue;
                    }
                    if after_slash {
                        size.push(piece.to_string());
                    } else {
                        position.push(piece.to_string());
                    }
                }
            }
        }
        if geometry.is_none() && image.as_deref().is_some_and(|image| image != "none") {
            geometry = Some((position, size, repeat));
        }
        images.push(image.unwrap_or_else(|| "none".into()));
    }
    let (position, size, repeat) = geometry.unwrap_or_default();
    let or = |parts: Vec<String>, fallback: &str| {
        if parts.is_empty() {
            fallback.to_string()
        } else {
            parts.join(" ")
        }
    };
    Some(vec![
        (
            "background-color".into(),
            color.unwrap_or_else(|| "transparent".into()),
        ),
        ("background-image".into(), images.join(", ")),
        ("background-size".into(), or(size, "auto")),
        ("background-position".into(), or(position, "0% 0%")),
        ("background-repeat".into(), or(repeat, "repeat")),
    ])
}
/** What the `mask` shorthand sets. */
const MASK: [&str; 4] = ["mask-image", "mask-size", "mask-position", "mask-repeat"];
/** Mask keywords Eye does not model: mode, composite, origin and clip. */
const MASK_IGNORED: [&str; 15] = [
    "alpha",
    "luminance",
    "match-source",
    "add",
    "subtract",
    "intersect",
    "exclude",
    "border-box",
    "padding-box",
    "content-box",
    "margin-box",
    "fill-box",
    "stroke-box",
    "view-box",
    "no-clip",
];
/**
 * The `mask` shorthand parsed like `background` without its color; a value that does not parse
 * still sets the image, so a mask is never lost and its box never paints as a solid fill.
 */
fn mask(raw: &str) -> Vec<(String, String)> {
    let kept: Vec<&str> = tokens(raw)
        .into_iter()
        .filter(|t| !MASK_IGNORED.contains(&t.trim_end_matches(',').to_ascii_lowercase().as_str()))
        .collect();
    match background(&kept.join(" ")) {
        Some(parts) => parts
            .into_iter()
            .filter_map(|(name, value)| {
                Some((name.strip_prefix("background-")?.to_string(), value))
            })
            .filter(|(name, _)| name != "color")
            .map(|(name, value)| (format!("mask-{name}"), value))
            .collect(),
        None => vec![("mask-image".into(), raw.into())],
    }
}
/** Tiling keywords inside a `background` shorthand layer. */
const REPEATS: [&str; 6] = [
    "repeat",
    "no-repeat",
    "repeat-x",
    "repeat-y",
    "space",
    "round",
];
/** The marker type of a `list-style` shorthand; `none` anywhere means no marker, as when only images are named. */
fn list_style(value: &str) -> Option<Vec<(String, String)>> {
    const POSITIONS: [&str; 2] = ["inside", "outside"];
    let kind = tokens(value)
        .into_iter()
        .find(|token| !POSITIONS.contains(token) && !token.starts_with("url("))
        .unwrap_or("disc");
    Some(vec![("list-style-type".into(), kind.to_string())])
}
