/** String: the constructor, statics and prototype methods over UTF-16 code units. */
use super::*;
use crate::constants::JS_STRING_UNITS;
use crate::protocol::EngineError;
use crate::v9::vm::data::{Class, IterState};
use crate::v9::vm::heap::Object;
use crate::v9::vm::realm::{NativeFn, realm, symbol};

mod normalization;

/** String and String.prototype. */
pub(super) fn install(b: &mut Builder) {
    let proto = b.object(Some(b.ids.object_prototype));
    b.ids.string_prototype = proto;
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
        ("toLocaleLowerCase", 0, to_lower),
        ("toLocaleUpperCase", 0, to_upper),
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

/** String.fromCharCode. */
fn from_char_code(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let mut out = Vec::with_capacity(call.args.len());
    for value in call.args {
        out.push(crate::v9::value::to_uint32(vm.to_number(value)?) as u16);
    }
    units(out)
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
    units(out)
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
    let mut out = Vec::new();
    for index in 0..length {
        let chunk = vm.get(&raw, &key(&index.to_string()))?;
        out.extend_from_slice(vm.to_string(&chunk)?.as_units());
        if index + 1 < length && index + 1 < call.args.len() {
            out.extend_from_slice(vm.to_string(&call.args[index + 1])?.as_units());
        }
    }
    units(out)
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
    units(vec![text.as_units()[index as usize]])
}

/** String.prototype.charAt. */
fn char_at(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let text = this_string(vm, &call)?;
    let index = vm.to_integer(&call.arg(0))?;
    if index < 0.0 || index >= text.len() as f64 {
        return Ok(JsValue::String(JsString::empty()));
    }
    units(vec![text.as_units()[index as usize]])
}

/** String.prototype.charCodeAt. */
fn char_code_at(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let text = this_string(vm, &call)?;
    let index = vm.to_integer(&call.arg(0))?;
    if index < 0.0 || index >= text.len() as f64 {
        return Ok(JsValue::Number(f64::NAN));
    }
    Ok(JsValue::Number(f64::from(text.as_units()[index as usize])))
}

/** String.prototype.codePointAt. */
fn code_point_at(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let text = this_string(vm, &call)?;
    let index = vm.to_integer(&call.arg(0))?;
    if index < 0.0 || index >= text.len() as f64 {
        return Ok(JsValue::Undefined);
    }
    let point = text.code_points_from(index as usize);
    let value = match point {
        [high, low] => 0x10000 + ((u32::from(*high) - 0xd800) << 10) + (u32::from(*low) - 0xdc00),
        [unit] => u32::from(*unit),
        _ => 0,
    };
    Ok(JsValue::Number(f64::from(value)))
}

/** String.prototype.concat. */
fn concat(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let mut text = this_string(vm, &call)?;
    for value in call.args {
        let part = vm.to_string(value)?;
        text = text.concat(&part)?;
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
    let units = &text.as_units()[..end];
    Ok(JsValue::Boolean(units.ends_with(needle.as_units())))
}

/** String.prototype.includes. */
fn includes(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let text = this_string(vm, &call)?;
    let needle = not_regexp(vm, &call.arg(0), "includes")?;
    let from = vm.to_integer(&call.arg(1))?.clamp(0.0, text.len() as f64) as usize;
    Ok(JsValue::Boolean(
        find(text.as_units(), needle.as_units(), from).is_some(),
    ))
}

/** String.prototype.indexOf. */
fn index_of(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let text = this_string(vm, &call)?;
    let needle = arg_string(vm, &call, 0)?;
    let from = vm.to_integer(&call.arg(1))?.clamp(0.0, text.len() as f64) as usize;
    Ok(JsValue::Number(
        find(text.as_units(), needle.as_units(), from).map_or(-1.0, |i| i as f64),
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
    let units = text.as_units();
    let needle = needle.as_units();
    if needle.len() > units.len() {
        return Ok(JsValue::Number(-1.0));
    }
    let start = from.min(units.len() - needle.len());
    for index in (0..=start).rev() {
        if units[index..index + needle.len()] == *needle {
            return Ok(JsValue::Number(index as f64));
        }
    }
    Ok(JsValue::Number(-1.0))
}

/** String.prototype.localeCompare: code-unit order (no Intl). */
fn locale_compare(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let text = this_string(vm, &call)?;
    let other = arg_string(vm, &call, 0)?;
    Ok(JsValue::Number(match text.lossy().cmp(&other.lossy()) {
        std::cmp::Ordering::Less => -1.0,
        std::cmp::Ordering::Equal => 0.0,
        std::cmp::Ordering::Greater => 1.0,
    }))
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
    if flags.nullish() || !vm.to_string(&flags)?.as_units().contains(&u16::from(b'g')) {
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
    let filler = match call.arg(1) {
        JsValue::Undefined => JsString::from_utf8(" ")?,
        value => vm.to_string(&value)?,
    };
    if target <= text.len() || filler.is_empty() {
        return Ok(JsValue::String(text));
    }
    if target > JS_STRING_UNITS {
        return Err(EngineError::limit("JavaScript string UTF-16 units", JS_STRING_UNITS).into());
    }
    let fill: Vec<u16> = filler
        .as_units()
        .iter()
        .copied()
        .cycle()
        .take(target - text.len())
        .collect();
    let mut out = Vec::with_capacity(target);
    if start {
        out.extend_from_slice(&fill);
        out.extend_from_slice(text.as_units());
    } else {
        out.extend_from_slice(text.as_units());
        out.extend_from_slice(&fill);
    }
    units(out)
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
    if total > JS_STRING_UNITS {
        return Err(EngineError::limit("JavaScript string UTF-16 units", JS_STRING_UNITS).into());
    }
    units(text.as_units().repeat(count as usize))
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
    let source = text.as_units();
    let mut positions = Vec::new();
    let mut from = 0;
    while let Some(found) = find(source, needle.as_units(), from) {
        positions.push(found);
        if !all {
            break;
        }
        from = found + needle.len().max(1);
        if from > source.len() {
            break;
        }
    }
    let mut out = Vec::new();
    let mut next = 0;
    for position in positions {
        out.extend_from_slice(&source[next..position]);
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
            vm.to_string(&value)?.as_units().to_vec()
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
    out.extend_from_slice(&source[next..]);
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
    units(text.as_units()[start..end.max(start)].to_vec())
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
    if limit == 0 {
        return Ok(JsValue::Object(result));
    }
    if call.arg(0) == JsValue::Undefined {
        vm.array_append(&JsValue::Object(result), JsValue::String(text))?;
        return Ok(JsValue::Object(result));
    }
    let separator = arg_string(vm, &call, 0)?;
    let source = text.as_units();
    let mut parts = Vec::new();
    if separator.is_empty() {
        for unit in source.iter().take(limit) {
            parts.push(JsValue::String(JsString::from_slice(&[*unit])?));
        }
    } else {
        let mut start = 0;
        while let Some(found) = find(source, separator.as_units(), start) {
            parts.push(JsValue::String(JsString::from_slice(
                &source[start..found],
            )?));
            start = found + separator.len();
            if parts.len() >= limit {
                break;
            }
        }
        if parts.len() < limit {
            parts.push(JsValue::String(JsString::from_slice(&source[start..])?));
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
    Ok(JsValue::Boolean(
        text.as_units()[start..].starts_with(needle.as_units()),
    ))
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
    units(text.as_units()[low..high].to_vec())
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
    units(text.as_units()[start..start + count].to_vec())
}

/** Case mapping over code points; unpaired surrogates stay. */
fn map_case(vm: &mut Vm, call: &Call, upper: bool) -> Res<JsValue> {
    let text = this_string(vm, call)?;
    if text.as_units().iter().all(|unit| *unit < 0x80) {
        let out: Vec<u16> = text
            .as_units()
            .iter()
            .map(|unit| {
                let byte = *unit as u8;
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
    for point in char::decode_utf16(text.as_units().iter().copied()) {
        match point {
            Ok(c) => {
                let mapped: String = if upper {
                    c.to_uppercase().collect()
                } else {
                    c.to_lowercase().collect()
                };
                out.extend(mapped.encode_utf16());
            }
            Err(error) => out.push(error.unpaired_surrogate()),
        }
    }
    units(out)
}

/** String.prototype.toLowerCase. */
fn to_lower(vm: &mut Vm, call: Call) -> Res<JsValue> {
    map_case(vm, &call, false)
}
/** String.prototype.toUpperCase. */
fn to_upper(vm: &mut Vm, call: Call) -> Res<JsValue> {
    map_case(vm, &call, true)
}

/** String.prototype.toString and valueOf. */
fn value_of(vm: &mut Vm, call: Call) -> Res<JsValue> {
    match &call.this {
        JsValue::String(text) => Ok(JsValue::String(text.clone())),
        JsValue::Object(id) => match vm.heap.get(*id).map(|o| &o.class) {
            Some(Class::String(text)) => Ok(JsValue::String(text.clone())),
            _ => Err(vm.type_error("String.prototype.valueOf requires that 'this' be a String")),
        },
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
    let source = text.as_units();
    let mut low = 0;
    let mut high = source.len();
    if start {
        while low < high && is_space(source[low]) {
            low += 1;
        }
    }
    if end {
        while high > low && is_space(source[high - 1]) {
            high -= 1;
        }
    }
    units(source[low..high].to_vec())
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
    Ok(JsValue::Boolean(
        String::from_utf16(text.as_units()).is_ok(),
    ))
}

/** String.prototype.toWellFormed. */
fn to_well_formed(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let text = this_string(vm, &call)?;
    units(
        String::from_utf16_lossy(text.as_units())
            .encode_utf16()
            .collect(),
    )
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
        for &unit in value.as_units() {
            if unit == u16::from(b'"') {
                out.extend("&quot;".encode_utf16());
            } else {
                out.push(unit);
            }
        }
        out.push(u16::from(b'"'));
    }
    out.push(u16::from(b'>'));
    out.extend_from_slice(text.as_units());
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
