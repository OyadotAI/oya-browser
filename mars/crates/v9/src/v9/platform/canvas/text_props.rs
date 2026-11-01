/** Canvas text and smoothing properties: letter and word spacing reach the font, and the keyword properties are validated stores. */
use super::*;

/** Keywords each store accepts; anything else is ignored and the previous keyword stays in effect. */
const KERNING: &[&str] = &["auto", "normal", "none"];
const STRETCH: &[&str] = &[
    "ultra-condensed",
    "extra-condensed",
    "condensed",
    "semi-condensed",
    "normal",
    "semi-expanded",
    "expanded",
    "extra-expanded",
    "ultra-expanded",
];
const VARIANT_CAPS: &[&str] = &[
    "normal",
    "small-caps",
    "all-small-caps",
    "petite-caps",
    "all-petite-caps",
    "unicase",
    "titling-caps",
];
const RENDERING: &[&str] = &[
    "auto",
    "optimizeSpeed",
    "optimizeLegibility",
    "geometricPrecision",
];
const SMOOTHING: &[&str] = &["low", "medium", "high"];

/** Each property is a getter and setter pair that validates before it stores anything. */
pub(super) fn install(b: &mut Builder, proto: u32) {
    for name in [
        "letterSpacing",
        "wordSpacing",
        "fontKerning",
        "fontStretch",
        "fontVariantCaps",
        "textRendering",
        "imageSmoothingQuality",
        "lang",
    ] {
        b.accessor(proto, name, Some(get), Some(set));
    }
}

/** Getters report the canonical stored value, with lengths always spelled in pixels. */
fn get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let node = context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    let drawing = &vm.page.state.canvases[&node].drawing;
    let text = match dom::native_name(&call) {
        "letterSpacing" => format!("{}px", drawing.font.letter_spacing),
        "wordSpacing" => format!("{}px", drawing.font.word_spacing),
        "fontKerning" => drawing.kerning.clone(),
        "fontStretch" => drawing.stretch.clone(),
        "fontVariantCaps" => drawing.variant_caps.clone(),
        "textRendering" => drawing.rendering.clone(),
        "imageSmoothingQuality" => drawing.smoothing_quality.clone(),
        _ => drawing.lang.clone(),
    };
    Ok(JsValue::string(&text)?)
}

/** Conversion can run script, so the target context is reacquired before any value is stored. */
fn set(vm: &mut Vm, call: Call) -> Res<JsValue> {
    context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    let name = dom::native_name(&call);
    let text = vm.to_text(&call.arg(0))?;
    let node = context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    let drawing = &mut vm
        .page
        .state
        .canvases
        .get_mut(&node)
        .expect("canvas")
        .drawing;
    match name {
        "letterSpacing" => store_length(&text, &mut drawing.font.letter_spacing, drawing.font.size),
        "wordSpacing" => store_length(&text, &mut drawing.font.word_spacing, drawing.font.size),
        "fontKerning" => keyword(&text, KERNING, &mut drawing.kerning),
        "fontStretch" => keyword(&text, STRETCH, &mut drawing.stretch),
        "fontVariantCaps" => keyword(&text, VARIANT_CAPS, &mut drawing.variant_caps),
        "textRendering" => keyword(&text, RENDERING, &mut drawing.rendering),
        "imageSmoothingQuality" => keyword(&text, SMOOTHING, &mut drawing.smoothing_quality),
        _ => drawing.lang = text,
    }
    Ok(JsValue::Undefined)
}

/** Keep the previous keyword unless the new one is one of the listed spellings exactly. */
fn keyword(text: &str, allowed: &[&str], slot: &mut String) {
    if let Some(found) = allowed.iter().find(|candidate| **candidate == text) {
        *slot = (*found).to_string();
    }
}

/** A spacing length is px, em (against the font size) or a bare zero; anything else leaves the old value. */
fn store_length(text: &str, slot: &mut f64, font_size: f64) {
    if let Some(px) = length(text, font_size) {
        *slot = px;
    }
}

/** Parse one CSS length into pixels, rejecting nonfinite numbers and unknown units. */
fn length(text: &str, font_size: f64) -> Option<f64> {
    let text = text.trim();
    if text == "0" {
        return Some(0.0);
    }
    let (number, scale) = match (text.strip_suffix("px"), text.strip_suffix("em")) {
        (Some(number), _) => (number, 1.0),
        (None, Some(number)) => (number, font_size),
        (None, None) => return None,
    };
    let value: f64 = number.trim().parse().ok()?;
    value.is_finite().then_some(value * scale)
}
