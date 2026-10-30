/** Bounded IntersectionObserver dictionaries support viewport roots, px/% margins and thresholds. */
use super::*;

/** Percentages resolve against the undilated root width for every side, including vertical sides. */
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Margin {
    pub value: f64,
    pub percent: bool,
}

/** Unsupported clipping roots and visibility tracking fail explicitly instead of fabricating visibility. */
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Options {
    pub margins: [Margin; 4],
    pub thresholds: Vec<f64>,
}

impl Options {
    /** The getter expands CSS shorthand into its normalized four-side representation. */
    pub fn margin_text(&self) -> String {
        self.margins
            .iter()
            .map(|margin| {
                format!(
                    "{}{}",
                    margin.value,
                    if margin.percent { "%" } else { "px" }
                )
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/** WebIDL member order is stable even when dictionary getters have side effects. */
pub(super) fn parse(vm: &mut Vm, value: &JsValue) -> Res<Options> {
    if !value.nullish() && value.object().is_none() {
        return Err(vm.type_error("IntersectionObserver options must be a dictionary"));
    }
    let delay = member(vm, value, "delay")?;
    if delay != JsValue::Undefined && vm.to_number(&delay)? != 0.0 {
        return Err(vm.unsupported_error("IntersectionObserver delay is unavailable"));
    }
    let root = member(vm, value, "root")?;
    if !root.nullish() {
        return Err(vm.unsupported_error("IntersectionObserver supports only a viewport root"));
    }
    let margin = member(vm, value, "rootMargin")?;
    let margins = margin_value(vm, margin)?;
    let scroll = member(vm, value, "scrollMargin")?;
    if margin_value(vm, scroll)?
        .iter()
        .any(|margin| margin.value != 0.0)
    {
        return Err(vm.unsupported_error("IntersectionObserver scrollMargin is unavailable"));
    }
    let threshold = member(vm, value, "threshold")?;
    let thresholds = thresholds(vm, threshold)?;
    let visible = member(vm, value, "trackVisibility")?;
    if visible.truthy() {
        return Err(vm.unsupported_error("IntersectionObserver visibility tracking is unavailable"));
    }
    Ok(Options {
        margins,
        thresholds,
    })
}

/** Null and omitted dictionaries expose no members. */
fn member(vm: &mut Vm, value: &JsValue, name: &str) -> Res<JsValue> {
    if value.nullish() {
        Ok(JsValue::Undefined)
    } else {
        vm.get(value, &Key::from(name))
    }
}

/** Invalid CSS margins raise SyntaxError and oversized finite margins raise RangeError. */
fn margin_value(vm: &mut Vm, value: JsValue) -> Res<[Margin; 4]> {
    let text = if value == JsValue::Undefined {
        "0px".into()
    } else {
        vm.to_text(&value)?
    };
    if text.len() > crate::constants::URL_BYTES {
        return Err(vm.range_error("IntersectionObserver margin is too large"));
    }
    let values: Option<Vec<Margin>> = text
        .to_ascii_lowercase()
        .split_ascii_whitespace()
        .map(parse_margin)
        .collect();
    let Some(values) = values.filter(|values| (1..=4).contains(&values.len())) else {
        return Err(vm.dom_exception(
            "SyntaxError",
            "IntersectionObserver rootMargin requires px or % lengths",
        ));
    };
    if values
        .iter()
        .any(|margin| margin.value.abs() > MARGIN_LIMIT)
    {
        return Err(vm.range_error("IntersectionObserver margin exceeds its supported bound"));
    }
    Ok(match values.as_slice() {
        [a] => [*a; 4],
        [a, b] => [*a, *b, *a, *b],
        [a, b, c] => [*a, *b, *c, *b],
        [a, b, c, d] => [*a, *b, *c, *d],
        _ => unreachable!("bounded CSS shorthand"),
    })
}

/** CSS numeric tokens must not contain Rust's inf/NaN spellings or internal whitespace. */
fn parse_margin(text: &str) -> Option<Margin> {
    let (number, percent) = if let Some(number) = text.strip_suffix('%') {
        (number, true)
    } else {
        (text.strip_suffix("px")?, false)
    };
    if number.is_empty()
        || number
            .bytes()
            .any(|byte| !(byte.is_ascii_digit() || b"+-.eE".contains(&byte)))
    {
        return None;
    }
    let value: f64 = number.parse().ok()?;
    value.is_finite().then_some(Margin {
        value: if value == 0.0 { 0.0 } else { value },
        percent,
    })
}

/** Consume numeric sequences incrementally so an author iterator cannot bypass the threshold cap. */
fn thresholds(vm: &mut Vm, value: JsValue) -> Res<Vec<f64>> {
    let mut values = Vec::new();
    if value.object().is_some() {
        let method = vm.get(&value, &Key::Sym(symbol::ITERATOR))?;
        if !method.nullish() {
            let iterator = vm.iterator_from_method(&value, method)?;
            vm.pin(JsValue::Object(iterator));
            while let Some(value) = vm.iter_step(iterator)? {
                if values.len() >= THRESHOLDS {
                    vm.iter_close(iterator)?;
                    return Err(vm.range_error("Too many IntersectionObserver thresholds"));
                }
                match threshold(vm, value) {
                    Ok(value) => values.push(value),
                    Err(error) => {
                        let _ = vm.iter_close(iterator);
                        return Err(error);
                    }
                }
            }
        } else {
            values.push(threshold(vm, value)?);
        }
    } else if value != JsValue::Undefined {
        values.push(threshold(vm, value)?);
    }
    if values.is_empty() {
        values.push(0.0);
    }
    values.sort_by(f64::total_cmp);
    Ok(values)
}

/** Non-finite WebIDL doubles and out-of-range ratios must never reach the comparison state. */
fn threshold(vm: &mut Vm, value: JsValue) -> Res<f64> {
    let number = vm.to_number(&value)?;
    if !number.is_finite() {
        return Err(vm.type_error("IntersectionObserver threshold must be finite"));
    }
    if !(0.0..=1.0).contains(&number) {
        return Err(vm.range_error("IntersectionObserver threshold must be between zero and one"));
    }
    Ok(number)
}
