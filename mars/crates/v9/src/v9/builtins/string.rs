/** String: the constructor, statics and prototype methods over UTF-16 code units. */
use super::*;
use crate::constants::{
    JS_STRING_UNITS, STRING_SEARCH_FLAT_UNITS, UTF16_HIGH_SURROGATE_END, UTF16_HIGH_SURROGATE_START, UTF16_LOW_SURROGATE_END,
    UTF16_LOW_SURROGATE_START,
};
use crate::protocol::EngineError;
use crate::v9::vm::data::{Class, IterState};
use crate::v9::vm::heap::Object;
use crate::v9::vm::realm::{NativeFn, realm, symbol};

mod normalization;

/** Pinned d8 cannot represent more UTF-16 units than this, independent of V9's tighter quota. */
#[cfg(feature = "conformance")]
const D8_MAX_STRING_UNITS: usize = if cfg!(target_pointer_width = "32") {
    (1 << 28) - 16
} else {
    (1 << 29) - 24
};

/** An inherently impossible d8 result throws before any allocation; smaller quota failures stay fatal. */
fn impossible_shell_string(vm: &mut Vm, length: usize) -> Res<()> {
    #[cfg(feature = "conformance")]
    if vm.page.environment.conformance_d8 && length > D8_MAX_STRING_UNITS {
        return Err(vm.range_error("Invalid string length"));
    }
    let _ = (vm, length);
    Ok(())
}

/** String and String.prototype. */
pub(super) fn install(b: &mut Builder) {
    let proto = b.object(Some(b.ids.object_prototype));
    b.ids.string_prototype = proto;
    // The intrinsic prototype is the empty string wrapper, including its immutable own length.
    b.value(proto, "length", JsValue::Number(0.0), 0);
    let ctor = b.constructor("String", 1, construct, proto, None);
    b.global("String", ctor);
    b.method(ctor, "fromCharCode", 1, from_char_code);
    b.method(ctor, "fromCodePoint", 1, from_code_point);
    b.method(ctor, "raw", 1, raw);
    let methods: [(&'static str, u32, NativeFn); 48] = [
        ("at", 1, at),
        ("charAt", 1, char_at),
        ("charCodeAt", 1, char_code_at),
        ("codePointAt", 1, code_point_at),
        ("concat", 1, concat),
        ("endsWith", 1, ends_with),
        ("includes", 1, includes),
        ("indexOf", 1, index_of),
        ("lastIndexOf", 1, last_index_of),
        ("localeCompare", 1, locale_compare),
        ("match", 1, match_method),
        ("matchAll", 1, match_all),
        ("normalize", 0, normalization::normalize),
        ("padEnd", 1, pad_end),
        ("padStart", 1, pad_start),
        ("repeat", 1, repeat),
        ("replace", 2, replace),
        ("replaceAll", 2, replace_all),
        ("search", 1, search),
        ("slice", 2, slice),
        ("split", 2, split),
        ("startsWith", 1, starts_with),
        ("substring", 2, substring),
        ("substr", 2, substr),
        ("toLowerCase", 0, to_lower),
        ("toUpperCase", 0, to_upper),
        ("toLocaleLowerCase", 0, to_locale_lower),
        ("toLocaleUpperCase", 0, to_locale_upper),
        ("toString", 0, value_of),
        ("valueOf", 0, value_of),
        ("trim", 0, trim),
        ("trimStart", 0, trim_start),
        ("trimEnd", 0, trim_end),
        ("isWellFormed", 0, is_well_formed),
        ("toWellFormed", 0, to_well_formed),
        ("anchor", 1, |vm, call| {
            create_html(vm, &call, "a", Some("name"))
        }),
        ("big", 0, |vm, call| create_html(vm, &call, "big", None)),
        ("blink", 0, |vm, call| create_html(vm, &call, "blink", None)),
        ("bold", 0, |vm, call| create_html(vm, &call, "b", None)),
        ("fixed", 0, |vm, call| create_html(vm, &call, "tt", None)),
        ("fontcolor", 1, |vm, call| {
            create_html(vm, &call, "font", Some("color"))
        }),
        ("fontsize", 1, |vm, call| {
            create_html(vm, &call, "font", Some("size"))
        }),
        ("italics", 0, |vm, call| create_html(vm, &call, "i", None)),
        ("link", 1, |vm, call| {
            create_html(vm, &call, "a", Some("href"))
        }),
        ("small", 0, |vm, call| create_html(vm, &call, "small", None)),
        ("strike", 0, |vm, call| {
            create_html(vm, &call, "strike", None)
        }),
        ("sub", 0, |vm, call| create_html(vm, &call, "sub", None)),
        ("sup", 0, |vm, call| create_html(vm, &call, "sup", None)),
    ];
    for (name, length, function) in methods {
        let method = b.method(proto, name, length, function);
        if let Some(alias) = trim_alias(name) {
            b.value(
                proto,
                alias,
                JsValue::Object(method),
                crate::v9::vm::heap::HIDDEN_ATTRS,
            );
        }
    }
    b.symbol_method(proto, symbol::ITERATOR, "[Symbol.iterator]", 0, iterator);
}

/** Annex B trimming aliases share their canonical function, including its name and identity. */
fn trim_alias(name: &str) -> Option<&'static str> {
    [("trimStart", "trimLeft"), ("trimEnd", "trimRight")]
        .into_iter()
        .find_map(|(canonical, alias)| (name == canonical).then_some(alias))
}

/** `String(value)` and `new String(value)`. */
fn construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let text = match call.args.first() {
        None => JsString::empty(),
        Some(JsValue::Symbol(id)) if call.new_target.is_none() => {
            let description = super::symbol::symbol_description(vm, *id).unwrap_or_default();
            JsString::from_utf8(&format!("Symbol({description})"))?
        }
        Some(value) => vm.to_string(value)?,
    };
    match &call.new_target {
        None => Ok(JsValue::String(text)),
        Some(target) => {
            let proto = vm.prototype_for(target, realm().ids.string_prototype)?;
            Ok(JsValue::Object(
                vm.alloc(Object::new(Class::String(text), Some(proto)))?,
            ))
        }
    }
}

/** The receiver as a string (RequireObjectCoercible + ToString). */
fn this_string(vm: &mut Vm, call: &Call) -> Res<JsString> {
    require_receiver(vm, call)?;
    vm.to_string(&call.this)
}

/** Reject nullish receivers before looking up a pattern hook, without coercing other values. */
fn require_receiver(vm: &mut Vm, call: &Call) -> Res<()> {
    if call.this.nullish() {
        return Err(vm.type_error("String.prototype method called on null or undefined"));
    }
    Ok(())
}

/** A string from units, under the length bound. */
fn units(units: Vec<u16>) -> Res<JsValue> {
    Ok(JsValue::String(JsString::from_units(units)?))
}

/** Single Latin-1 results use the shell isolate's permanent character root. */
fn string_units(vm: &mut Vm, value: Vec<u16>) -> Res<JsValue> {
    #[cfg(feature = "conformance")]
    if vm.page.environment.conformance_d8 && value.len() == 1 && value[0] <= 255 {
        return Ok(JsValue::String(vm.character_root(value[0] as u8)?));
    }
    let _ = vm;
    units(value)
}

/** One-byte substring results use the same read-only root as character iteration. */
fn string_slice(vm: &mut Vm, text: &JsString, start: usize, end: usize) -> Res<JsString> {
    #[cfg(feature = "conformance")]
    if vm.page.environment.conformance_d8 && end == start.saturating_add(1)
        && let Some(unit) = text.unit_at(start).filter(|unit| *unit <= 255)
    {
        return vm.character_root(unit as u8);
    }
    let _ = vm;
    Ok(text.slice_range(start, end)?)
}

/** String.fromCharCode. */
fn from_char_code(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let mut out = Vec::with_capacity(call.args.len());
    for value in call.args {
        out.push(crate::v9::value::to_uint32(vm.to_number(value)?) as u16);
    }
    string_units(vm, out)
}

/** String.fromCodePoint. */
fn from_code_point(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let mut out = Vec::new();
    for value in call.args {
        let number = vm.to_number(value)?;
        if number.fract() != 0.0 || !(0.0..=1_114_111.0).contains(&number) {
            return Err(vm.range_error(&format!("Invalid code point {number}")));
        }
        push_point(&mut out, number as u32);
    }
    string_units(vm, out)
}

/** Append a code point as UTF-16. */
pub(in crate::v9) fn push_point(out: &mut Vec<u16>, point: u32) {
    if point < 0x10000 {
        out.push(point as u16);
    } else {
        let offset = point - 0x10000;
        out.push(0xd800 + (offset >> 10) as u16);
        out.push(0xdc00 + (offset & 0x3ff) as u16);
    }
}

/** String.raw. */
fn raw(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let strings = call.arg(0);
    let raw = vm.get(&strings, &key("raw"))?;
    let length = vm.length_of(&raw)?;
    let mut out = JsString::empty();
    let mut overflow = None;
    for index in 0..length {
        let chunk = vm.get(&raw, &key(&index.to_string()))?;
        let piece = vm.to_string(&chunk)?;
        raw_append(vm, &mut out, &mut overflow, piece);
        if index + 1 < length && index + 1 < call.args.len() {
            let piece = vm.to_string(&call.args[index + 1])?;
            raw_append(vm, &mut out, &mut overflow, piece);
        }
    }
    if let Some(error) = overflow {
        return Err(error);
    }
    Ok(JsValue::String(out))
}

/** Preserve later getter and coercion errors after a raw result exceeds its storage limit. */
fn raw_append(vm: &mut Vm, out: &mut JsString, overflow: &mut Option<crate::v9::vm::Thrown>, piece: JsString) {
    if overflow.is_some() {
        return;
    }
    match vm.concat_strings(out, &piece) {
        Ok(value) => *out = value,
        Err(error) => *overflow = Some(error),
    }
}

/** String.prototype.at. */
fn at(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let text = this_string(vm, &call)?;
    let relative = vm.to_integer(&call.arg(0))?;
    let index = if relative < 0.0 {
        text.len() as f64 + relative
    } else {
        relative
    };
    if index < 0.0 || index >= text.len() as f64 {
        return Ok(JsValue::Undefined);
    }
    string_units(vm, vec![
        text.unit_at(index as usize).expect("checked string index"),
    ])
}

/** String.prototype.charAt. */
fn char_at(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let text = this_string(vm, &call)?;
    let index = vm.to_integer(&call.arg(0))?;
    if index < 0.0 || index >= text.len() as f64 {
        return Ok(JsValue::String(JsString::empty()));
    }
    string_units(vm, vec![
        text.unit_at(index as usize).expect("checked string index"),
    ])
}

/** String.prototype.charCodeAt. */
fn char_code_at(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let text = this_string(vm, &call)?;
    let index = vm.to_integer(&call.arg(0))?;
    if index < 0.0 || index >= text.len() as f64 {
        return Ok(JsValue::Number(f64::NAN));
    }
    Ok(JsValue::Number(f64::from(
        text.unit_at(index as usize).expect("checked string index"),
    )))
}

/** String.prototype.codePointAt. */
fn code_point_at(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let text = this_string(vm, &call)?;
    let index = vm.to_integer(&call.arg(0))?;
    if index < 0.0 || index >= text.len() as f64 {
        return Ok(JsValue::Undefined);
    }
    let high = text.unit_at(index as usize).expect("checked string index");
    let low = text.unit_at(index as usize + 1);
    let value = match low {
        Some(low) if (0xd800..=0xdbff).contains(&high) && (0xdc00..=0xdfff).contains(&low) => {
            0x10000 + ((u32::from(high) - 0xd800) << 10) + (u32::from(low) - 0xdc00)
        }
        _ => u32::from(high),
    };
    Ok(JsValue::Number(f64::from(value)))
}

/** String.prototype.concat. */
fn concat(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let mut text = this_string(vm, &call)?;
    for value in call.args {
        let part = vm.to_string(value)?;
        text = vm.concat_strings(&text, &part)?;
    }
    Ok(JsValue::String(text))
}

/** Reject RegExp arguments where the spec requires a string. */
fn not_regexp(vm: &mut Vm, value: &JsValue, method: &str) -> Res<JsString> {
    if super::regexp::is_regexp(vm, value)? {
        return Err(vm.type_error(&format!(
            "First argument to String.prototype.{method} must not be a regular expression"
        )));
    }
    vm.to_string(value)
}

/** Find `needle` at or after `from`. */
pub(in crate::v9) fn find(haystack: &[u16], needle: &[u16], from: usize) -> Option<usize> {
    if needle.is_empty() {
        return (from <= haystack.len()).then_some(from);
    }
    if needle.len() > haystack.len() {
        return None;
    }
    (from..=haystack.len() - needle.len())
        .find(|start| haystack[*start..*start + needle.len()] == *needle)
}

/** String.prototype.endsWith. */
fn ends_with(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let text = this_string(vm, &call)?;
    let needle = not_regexp(vm, &call.arg(0), "endsWith")?;
    let end = if call.arg(1) == JsValue::Undefined {
        text.len()
    } else {
        vm.to_integer(&call.arg(1))?.clamp(0.0, text.len() as f64) as usize
    };
    if needle.len() > end {
        return Ok(JsValue::Boolean(false));
    }
    Ok(JsValue::Boolean(text_matches_at(
        &text,
        &needle,
        end - needle.len(),
    )))
}

/** Compare UTF-16 units without flattening either potentially compact string. */
fn text_matches_at(text: &JsString, needle: &JsString, start: usize) -> bool {
    if needle.len() > text.len().saturating_sub(start) {
        return false;
    }
    if let (Some(haystack), Some(pattern)) = (text.flat_units(), needle.flat_units()) {
        return haystack[start..start + pattern.len()] == *pattern;
    }
    (0..needle.len()).all(|offset| text.unit_at(start + offset) == needle.unit_at(offset))
}

/** Search a compact string without allocating a flattened copy. */
fn find_string(vm: &mut Vm, text: &JsString, needle: &JsString, from: usize) -> Res<Option<usize>> {
    if let (Some(haystack), Some(pattern)) = (text.flat_units(), needle.flat_units()) {
        return Ok(find(haystack, pattern, from));
    }
    if needle.len() > text.len() {
        return Ok(None);
    }
    if text.len() <= STRING_SEARCH_FLAT_UNITS && needle.len() <= STRING_SEARCH_FLAT_UNITS {
        let haystack = vm.materialize_string(text)?;
        let pattern = vm.materialize_string(needle)?;
        return Ok(find(haystack, pattern, from));
    }
    Ok((from..=text.len() - needle.len()).find(|start| text_matches_at(text, needle, *start)))
}

/** String.prototype.includes. */
fn includes(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let text = this_string(vm, &call)?;
    let needle = not_regexp(vm, &call.arg(0), "includes")?;
    let from = vm.to_integer(&call.arg(1))?.clamp(0.0, text.len() as f64) as usize;
    Ok(JsValue::Boolean(
        find_string(vm, &text, &needle, from)?.is_some(),
    ))
}

/** String.prototype.indexOf. */
fn index_of(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let text = this_string(vm, &call)?;
    let needle = arg_string(vm, &call, 0)?;
    let from = vm.to_integer(&call.arg(1))?.clamp(0.0, text.len() as f64) as usize;
    Ok(JsValue::Number(
        find_string(vm, &text, &needle, from)?.map_or(-1.0, |i| i as f64),
    ))
}

/** String.prototype.lastIndexOf. */
fn last_index_of(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let text = this_string(vm, &call)?;
    let needle = arg_string(vm, &call, 0)?;
    let position = vm.to_number(&call.arg(1))?;
    let from = if position.is_nan() {
        text.len()
    } else {
        position.max(0.0).min(text.len() as f64) as usize
    };
    if needle.len() > text.len() {
        return Ok(JsValue::Number(-1.0));
    }
    let start = from.min(text.len() - needle.len());
    for index in (0..=start).rev() {
        if text_matches_at(&text, &needle, index) {
            return Ok(JsValue::Number(index as f64));
        }
    }
    Ok(JsValue::Number(-1.0))
}

/** String.prototype.localeCompare: code-unit order (no Intl). */
fn locale_compare(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let text = this_string(vm, &call)?;
    let other = arg_string(vm, &call, 0)?;
    Ok(JsValue::Number(
        match lossy_points(&text).cmp(lossy_points(&other)) {
            std::cmp::Ordering::Less => -1.0,
            std::cmp::Ordering::Equal => 0.0,
            std::cmp::Ordering::Greater => 1.0,
        },
    ))
}

/** Preserve lossy scalar ordering without creating contiguous UTF-16 or UTF-8 copies. */
fn lossy_points(text: &JsString) -> impl Iterator<Item = char> + '_ {
    char::decode_utf16((0..text.len()).filter_map(|index| text.unit_at(index)))
        .map(|point| point.unwrap_or(char::REPLACEMENT_CHARACTER))
}

/** Delegate to a pattern's symbol method, else build a RegExp from it. */
fn delegate(vm: &mut Vm, call: &Call, symbol: u32, flags: &str) -> Res<Option<JsValue>> {
    require_receiver(vm, call)?;
    let pattern = call.arg(0);
    if matches!(pattern, JsValue::Object(_)) {
        if let Some(method) = vm.get_method(&pattern, &Key::Sym(symbol))? {
            let mut args = vec![call.this.clone()];
            if symbol == symbol::REPLACE || symbol == symbol::SPLIT {
                args.push(call.arg(1));
            }
            return vm.call(method, pattern, args).map(Some);
        }
    }
    if flags.is_empty() {
        return Ok(None);
    }
    let text = this_string(vm, call)?;
    let flags = if flags == "-" {
        JsValue::Undefined
    } else {
        JsValue::String(JsString::from_utf8(flags)?)
    };
    let regexp = super::regexp::create(vm, pattern, flags)?;
    let method = vm.get(&regexp, &Key::Sym(symbol))?;
    vm.call(method, regexp, vec![JsValue::String(text)])
        .map(Some)
}

/** Global-only string methods inspect public flags after IsRegExp and before their hooks. */
fn require_global_pattern(vm: &mut Vm, call: &Call, method: &str) -> Res<()> {
    require_receiver(vm, call)?;
    let pattern = call.arg(0);
    if !super::regexp::is_regexp(vm, &pattern)? {
        return Ok(());
    }
    let flags = vm.get(&pattern, &key("flags"))?;
    if flags.nullish()
        || !vm
            .to_string(&flags)?
            .code_units()
            .any(|unit| unit == u16::from(b'g'))
    {
        return Err(vm.type_error(&format!("{method} requires a global RegExp")));
    }
    Ok(())
}

/** String.prototype.match. */
fn match_method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    Ok(delegate(vm, &call, symbol::MATCH, "-")?.unwrap_or(JsValue::Null))
}

/** String.prototype.matchAll. */
fn match_all(vm: &mut Vm, call: Call) -> Res<JsValue> {
    require_global_pattern(vm, &call, "String.prototype.matchAll")?;
    Ok(delegate(vm, &call, symbol::MATCH_ALL, "g")?.unwrap_or(JsValue::Null))
}

/** String.prototype.search. */
fn search(vm: &mut Vm, call: Call) -> Res<JsValue> {
    Ok(delegate(vm, &call, symbol::SEARCH, "-")?.unwrap_or(JsValue::Number(-1.0)))
}

/** Pad to a length with a filler. */
fn pad(vm: &mut Vm, call: &Call, start: bool) -> Res<JsValue> {
    let text = this_string(vm, call)?;
    let target = vm.to_integer(&call.arg(0))?.max(0.0) as usize;
    if target <= text.len() {
        return Ok(JsValue::String(text));
    }
    let filler = match call.arg(1) {
        JsValue::Undefined => JsString::from_utf8(" ")?,
        value => vm.to_string(&value)?,
    };
    if filler.is_empty() {
        return Ok(JsValue::String(text));
    }
    impossible_shell_string(vm, target)?;
    if target > JS_STRING_UNITS {
        return Err(EngineError::limit("JavaScript string UTF-16 units", JS_STRING_UNITS).into());
    }
    let fill_length = target - text.len();
    let repeated = vm.repeat_string(&filler, fill_length / filler.len())?;
    let tail = vm.copy_string_range(&filler, 0, fill_length % filler.len())?;
    let fill = vm.concat_strings(&repeated, &tail)?;
    let result = if start {
        vm.concat_strings(&fill, &text)?
    } else {
        vm.concat_strings(&text, &fill)?
    };
    Ok(JsValue::String(result))
}

/** String.prototype.padEnd. */
fn pad_end(vm: &mut Vm, call: Call) -> Res<JsValue> {
    pad(vm, &call, false)
}
/** String.prototype.padStart. */
fn pad_start(vm: &mut Vm, call: Call) -> Res<JsValue> {
    pad(vm, &call, true)
}

/** String.prototype.repeat. */
fn repeat(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let text = this_string(vm, &call)?;
    let count = vm.to_integer(&call.arg(0))?;
    if count < 0.0 || count.is_infinite() {
        return Err(vm.range_error(&format!("Invalid count value: {count}")));
    }
    let total = text.len().saturating_mul(count as usize);
    impossible_shell_string(vm, total)?;
    if total > JS_STRING_UNITS {
        return Err(EngineError::limit("JavaScript string UTF-16 units", JS_STRING_UNITS).into());
    }
    Ok(JsValue::String(vm.repeat_string(&text, count as usize)?))
}

/** replace and replaceAll with a string pattern. */
fn replace_string(vm: &mut Vm, call: &Call, all: bool) -> Res<JsValue> {
    let text = this_string(vm, call)?;
    let needle = arg_string(vm, call, 0)?;
    let replacement = call.arg(1);
    let functional = vm.is_callable(&replacement);
    let template = if functional {
        JsString::empty()
    } else {
        vm.to_string(&replacement)?
    };
    let mut positions = Vec::new();
    let mut from = 0;
    while let Some(found) = find_string(vm, &text, &needle, from)? {
        positions.push(found);
        if !all {
            break;
        }
        from = found + needle.len().max(1);
        if from > text.len() {
            break;
        }
    }
    if positions.is_empty() {
        return Ok(JsValue::String(text));
    }
    let mut out = Vec::new();
    let mut next = 0;
    for position in positions {
        out.extend((next..position).filter_map(|index| text.unit_at(index)));
        let replaced = if functional {
            let value = vm.call(
                replacement.clone(),
                JsValue::Undefined,
                vec![
                    JsValue::String(needle.clone()),
                    JsValue::Number(position as f64),
                    JsValue::String(text.clone()),
                ],
            )?;
            let value = vm.to_string(&value)?;
            (0..value.len())
                .filter_map(|index| value.unit_at(index))
                .collect()
        } else {
            super::regexp::expand(
                vm,
                &template,
                &text,
                position,
                &needle,
                &[],
                &JsValue::Undefined,
            )?
        };
        out.extend_from_slice(&replaced);
        next = position + needle.len();
        if out.len() > JS_STRING_UNITS {
            return Err(
                EngineError::limit("JavaScript string UTF-16 units", JS_STRING_UNITS).into(),
            );
        }
    }
    out.extend((next..text.len()).filter_map(|index| text.unit_at(index)));
    units(out)
}

/** String.prototype.replace. */
fn replace(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if let Some(result) = delegate(vm, &call, symbol::REPLACE, "")? {
        return Ok(result);
    }
    replace_string(vm, &call, false)
}

/** String.prototype.replaceAll. */
fn replace_all(vm: &mut Vm, call: Call) -> Res<JsValue> {
    require_global_pattern(vm, &call, "String.prototype.replaceAll")?;
    if let Some(result) = delegate(vm, &call, symbol::REPLACE, "")? {
        return Ok(result);
    }
    replace_string(vm, &call, true)
}

/** String.prototype.slice. */
fn slice(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let text = this_string(vm, &call)?;
    let start = vm.relative_index(&call.arg(0), text.len(), 0)?;
    let end = vm.relative_index(&call.arg(1), text.len(), text.len())?;
    Ok(JsValue::String(string_slice(vm, &text, start, end.max(start))?))
}

/** String.prototype.split. */
fn split(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if let Some(result) = delegate(vm, &call, symbol::SPLIT, "")? {
        return Ok(result);
    }
    let text = this_string(vm, &call)?;
    let limit = if call.arg(1) == JsValue::Undefined {
        u32::MAX
    } else {
        crate::v9::value::to_uint32(vm.to_number(&call.arg(1))?)
    } as usize;
    let result = vm.new_array(Vec::new())?;
    let separator = arg_string(vm, &call, 0)?;
    if limit == 0 {
        return Ok(JsValue::Object(result));
    }
    if call.arg(0) == JsValue::Undefined {
        vm.array_append(&JsValue::Object(result), JsValue::String(text))?;
        return Ok(JsValue::Object(result));
    }
    let mut parts = Vec::new();
    if separator.is_empty() {
        for index in 0..text.len().min(limit) {
            parts.push(JsValue::String(string_slice(vm, &text, index, index + 1)?));
        }
    } else {
        let mut start = 0;
        while let Some(found) = find_string(vm, &text, &separator, start)? {
            parts.push(JsValue::String(string_slice(vm, &text, start, found)?));
            start = found + separator.len();
            if parts.len() >= limit {
                break;
            }
        }
        if parts.len() < limit {
            parts.push(JsValue::String(string_slice(vm, &text, start, text.len())?));
        }
    }
    for part in parts {
        vm.array_append(&JsValue::Object(result), part)?;
    }
    Ok(JsValue::Object(result))
}

/** String.prototype.startsWith. */
fn starts_with(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let text = this_string(vm, &call)?;
    let needle = not_regexp(vm, &call.arg(0), "startsWith")?;
    let start = vm.to_integer(&call.arg(1))?.clamp(0.0, text.len() as f64) as usize;
    Ok(JsValue::Boolean(text_matches_at(&text, &needle, start)))
}

/** String.prototype.substring. */
fn substring(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let text = this_string(vm, &call)?;
    let length = text.len() as f64;
    let start = vm.to_integer(&call.arg(0))?.clamp(0.0, length) as usize;
    let end = if call.arg(1) == JsValue::Undefined {
        text.len()
    } else {
        vm.to_integer(&call.arg(1))?.clamp(0.0, length) as usize
    };
    let (low, high) = (start.min(end), start.max(end));
    Ok(JsValue::String(string_slice(vm, &text, low, high)?))
}

/** String.prototype.substr. */
fn substr(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let text = this_string(vm, &call)?;
    let start = vm.relative_index(&call.arg(0), text.len(), 0)?;
    let count = if call.arg(1) == JsValue::Undefined {
        text.len() - start
    } else {
        (vm.to_integer(&call.arg(1))?.max(0.0) as usize).min(text.len() - start)
    };
    Ok(JsValue::String(string_slice(vm, &text, start, start + count)?))
}

/** Case mapping over code points; unpaired surrogates stay. Locale variants also validate their locales argument. */
fn map_case(vm: &mut Vm, call: &Call, upper: bool, locale: bool) -> Res<JsValue> {
    let text = this_string(vm, call)?;
    if locale {
        // ECMA-402 requires the locales list to canonicalize (and throw on bad tags) even though casing is not localized.
        crate::v9::builtins::intl::canonical_list(vm, &call.arg(0))?;
    }
    if text.len() > JS_STRING_UNITS {
        return Err(EngineError::limit("JavaScript string UTF-16 units", JS_STRING_UNITS).into());
    }
    let flat = text.flat_units();
    let code_units = || {
        (0..text.len()).map(|index| {
            flat.map_or_else(
                || text.unit_at(index).expect("valid string index"),
                |units| units[index],
            )
        })
    };
    if code_units().all(|unit| unit < 0x80) {
        let out: Vec<u16> = code_units()
            .map(|unit| {
                let byte = unit as u8;
                u16::from(if upper {
                    byte.to_ascii_uppercase()
                } else {
                    byte.to_ascii_lowercase()
                })
            })
            .collect();
        return units(out);
    }
    let mut out = Vec::with_capacity(text.len());
    let mut offset = 0;
    let mut preceded_by_cased = false;
    while let Some(point) = case_point(&text, &mut offset) {
        match point {
            Ok(c) => {
                if !upper && c == 'Σ' && preceded_by_cased && !following_cased(&text, offset) {
                    out.push('ς' as u16);
                } else {
                    let mapped: String = if upper {
                        c.to_uppercase().collect()
                    } else {
                        c.to_lowercase().collect()
                    };
                    out.extend(mapped.encode_utf16());
                }
                if !case_ignorable(c) {
                    preceded_by_cased = cased(c);
                }
            }
            Err(unit) => {
                out.push(unit);
                preceded_by_cased = false;
            }
        }
    }
    units(out)
}

/** Decode one UTF-16 code point while retaining lone surrogates. */
fn case_point(text: &JsString, offset: &mut usize) -> Option<Result<char, u16>> {
    let first = text.unit_at(*offset)?;
    *offset += 1;
    if (UTF16_HIGH_SURROGATE_START..=UTF16_HIGH_SURROGATE_END).contains(&first) {
        if let Some(second) = text
            .unit_at(*offset)
            .filter(|unit| (UTF16_LOW_SURROGATE_START..=UTF16_LOW_SURROGATE_END).contains(unit))
        {
            *offset += 1;
            let scalar = 0x10000 + ((u32::from(first) - 0xd800) << 10) + u32::from(second) - 0xdc00;
            return Some(Ok(char::from_u32(scalar).expect("valid surrogate pair")));
        }
    }
    Some(char::from_u32(u32::from(first)).ok_or(first))
}

/** Final sigma looks through case-ignorable code points for the next cased character. */
fn following_cased(text: &JsString, mut offset: usize) -> bool {
    while let Some(point) = case_point(text, &mut offset) {
        match point {
            Ok(character) if case_ignorable(character) => continue,
            Ok(character) => return cased(character),
            Err(_) => return false,
        }
    }
    false
}

/** Unicode's derived Cased property drives contextual lowercase mappings. */
fn cased(character: char) -> bool {
    icu_properties::CodePointSetData::new::<icu_properties::props::Cased>().contains(character)
}

/** Unicode's Case_Ignorable property does not break final-sigma context. */
fn case_ignorable(character: char) -> bool {
    icu_properties::CodePointSetData::new::<icu_properties::props::CaseIgnorable>()
        .contains(character)
}

/** String.prototype.toLowerCase. */
fn to_lower(vm: &mut Vm, call: Call) -> Res<JsValue> {
    map_case(vm, &call, false, false)
}
/** String.prototype.toUpperCase. */
fn to_upper(vm: &mut Vm, call: Call) -> Res<JsValue> {
    map_case(vm, &call, true, false)
}
/** String.prototype.toLocaleLowerCase, which canonicalizes its locales argument first. */
fn to_locale_lower(vm: &mut Vm, call: Call) -> Res<JsValue> {
    map_case(vm, &call, false, true)
}
/** String.prototype.toLocaleUpperCase, which canonicalizes its locales argument first. */
fn to_locale_upper(vm: &mut Vm, call: Call) -> Res<JsValue> {
    map_case(vm, &call, true, true)
}

/** String.prototype.toString and valueOf. */
fn value_of(vm: &mut Vm, call: Call) -> Res<JsValue> {
    match &call.this {
        JsValue::String(text) => Ok(JsValue::String(text.clone())),
        JsValue::Object(id) => vm.string_data(*id).map(JsValue::String).ok_or_else(|| {
            vm.type_error("String.prototype.valueOf requires that 'this' be a String")
        }),
        _ => Err(vm.type_error("String.prototype.valueOf requires that 'this' be a String")),
    }
}

/** ECMAScript whitespace units. */
pub(in crate::v9) fn is_space(unit: u16) -> bool {
    char::from_u32(u32::from(unit)).is_some_and(crate::v9::source::whitespace)
}

/** Trim either or both ends. */
fn trim_with(vm: &mut Vm, call: &Call, start: bool, end: bool) -> Res<JsValue> {
    let text = this_string(vm, call)?;
    let mut low = 0;
    let mut high = text.len();
    if start {
        while low < high && text.unit_at(low).is_some_and(is_space) {
            low += 1;
        }
    }
    if end {
        while high > low && text.unit_at(high - 1).is_some_and(is_space) {
            high -= 1;
        }
    }
    if low == 0 && high == text.len() {
        return Ok(JsValue::String(text));
    }
    Ok(JsValue::String(string_slice(vm, &text, low, high)?))
}

/** String.prototype.trim. */
fn trim(vm: &mut Vm, call: Call) -> Res<JsValue> {
    trim_with(vm, &call, true, true)
}
/** String.prototype.trimStart. */
fn trim_start(vm: &mut Vm, call: Call) -> Res<JsValue> {
    trim_with(vm, &call, true, false)
}
/** String.prototype.trimEnd. */
fn trim_end(vm: &mut Vm, call: Call) -> Res<JsValue> {
    trim_with(vm, &call, false, true)
}

/** String.prototype.isWellFormed. */
fn is_well_formed(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let text = this_string(vm, &call)?;
    Ok(JsValue::Boolean(well_formed(&text)))
}

/** Check surrogate pairing directly across rope node boundaries. */
fn well_formed(text: &JsString) -> bool {
    char::decode_utf16((0..text.len()).filter_map(|index| text.unit_at(index)))
        .all(|unit| unit.is_ok())
}

/** String.prototype.toWellFormed. */
fn to_well_formed(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let text = this_string(vm, &call)?;
    if well_formed(&text) {
        return Ok(JsValue::String(text));
    }
    if text.len() > JS_STRING_UNITS {
        return Err(EngineError::limit("JavaScript string UTF-16 units", JS_STRING_UNITS).into());
    }
    let mut out = Vec::with_capacity(text.len());
    for point in char::decode_utf16((0..text.len()).filter_map(|index| text.unit_at(index))) {
        push_point(
            &mut out,
            point.unwrap_or(char::REPLACEMENT_CHARACTER) as u32,
        );
    }
    units(out)
}

/** CreateHTML (spec B.2.2.2.1), behind the legacy String.prototype HTML methods: the string
wrapped in `<tag>`, with the argument as a quoted attribute whose `"` become `&quot;`. Works
on UTF-16 units so lone surrogates survive. */
fn create_html(vm: &mut Vm, call: &Call, tag: &str, attribute: Option<&str>) -> Res<JsValue> {
    let text = this_string(vm, call)?;
    let mut out: Vec<u16> = format!("<{tag}").encode_utf16().collect();
    if let Some(attribute) = attribute {
        let value = arg_string(vm, call, 0)?;
        out.extend(format!(" {attribute}=\"").encode_utf16());
        for unit in (0..value.len()).filter_map(|index| value.unit_at(index)) {
            if unit == u16::from(b'"') {
                out.extend("&quot;".encode_utf16());
            } else {
                out.push(unit);
            }
        }
        out.push(u16::from(b'"'));
    }
    out.push(u16::from(b'>'));
    out.extend((0..text.len()).filter_map(|index| text.unit_at(index)));
    out.extend(format!("</{tag}>").encode_utf16());
    Ok(JsValue::String(JsString::from_slice(&out)?))
}

/** String.prototype[Symbol.iterator]. */
fn iterator(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let text = this_string(vm, &call)?;
    vm.new_iterator(IterState::String {
        text,
        index: 0,
        done: false,
    })
    .map(JsValue::Object)
}
