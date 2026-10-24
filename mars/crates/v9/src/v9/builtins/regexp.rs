/** RegExp objects over V9's own engine: exec, test, flags, and the string protocol methods. */
use super::*;
use crate::protocol::EngineError;
use crate::v9::regexp::{self, Captures, MatchError};
use crate::v9::vm::data::{Class, IterState, RegExpData};
use crate::v9::vm::heap::{Object, Prop, Slot, WRITABLE};
use crate::v9::vm::realm::{realm, symbol};
use std::rc::Rc;

/** RegExp and RegExp.prototype. */
pub(super) fn install(b: &mut Builder) {
    let proto = b.object(Some(b.ids.object_prototype));
    b.ids.regexp_prototype = proto;
    let ctor = b.constructor("RegExp", 2, construct, proto, None);
    b.global("RegExp", ctor);
    b.method(proto, "exec", 1, exec);
    b.method(proto, "test", 1, test);
    b.method(proto, "toString", 0, to_string);
    b.method(proto, "compile", 2, recompile);
    b.builtin_accessor(proto, "source", Some(source), None);
    b.builtin_accessor(proto, "flags", Some(flags), None);
    for (name, getter) in [
        ("global", global as crate::v9::vm::realm::NativeFn),
        ("ignoreCase", ignore_case),
        ("multiline", multiline),
        ("dotAll", dot_all),
        ("unicode", unicode),
        ("sticky", sticky),
        ("hasIndices", has_indices),
    ] {
        b.builtin_accessor(proto, name, Some(getter), None);
    }
    b.symbol_method(
        proto,
        symbol::REPLACE,
        "[Symbol.replace]",
        2,
        symbol_replace,
    );
    b.symbol_method(proto, symbol::SPLIT, "[Symbol.split]", 2, symbol_split);
    b.symbol_method(proto, symbol::MATCH, "[Symbol.match]", 1, symbol_match);
    b.symbol_method(
        proto,
        symbol::MATCH_ALL,
        "[Symbol.matchAll]",
        1,
        symbol_match_all,
    );
    b.symbol_method(proto, symbol::SEARCH, "[Symbol.search]", 1, symbol_search);
}

impl Vm<'_> {
    /** A new RegExp object; a bad pattern throws SyntaxError. */
    pub fn new_regexp(&mut self, source: JsString, flags: &str) -> Res<ObjectId> {
        self.regexp_compile(&source, flags)?;
        let id = self.alloc(Object::new(
            Class::RegExp(Box::new(RegExpData {
                source,
                flags: flags.to_string(),
            })),
            Some(realm().ids.regexp_prototype),
        ))?;
        self.heap.put(
            id,
            key("lastIndex"),
            Prop {
                slot: Slot::Data(JsValue::Number(0.0)),
                attrs: WRITABLE,
            },
        );
        Ok(id)
    }
    /** The compiled program for a source and flags, cached for the request. */
    pub fn regexp_compile(&mut self, source: &JsString, flags: &str) -> Res<Rc<regexp::Program>> {
        let cache_key = (source.clone(), flags.to_string());
        if let Some(program) = self.regexps.get(&cache_key) {
            return Ok(program.clone());
        }
        let program = regexp::compile(source.as_units(), flags).map_err(|error| {
            let message = format!(
                "Invalid regular expression: /{}/{flags}: {}",
                source.lossy(),
                error.0
            );
            self.syntax_error(&message)
        })?;
        let program = Rc::new(program);
        self.regexps.insert(cache_key, program.clone());
        Ok(program)
    }
    /** The data of a RegExp receiver. */
    fn regexp_data(&mut self, value: &JsValue) -> Res<(ObjectId, RegExpData)> {
        if let JsValue::Object(id) = value
            && let Some(Class::RegExp(data)) = self.heap.get(*id).map(|o| &o.class)
        {
            return Ok((*id, (**data).clone()));
        }
        Err(self.type_error("RegExp method called on incompatible receiver"))
    }
    /** Run the engine; a step overrun is an engine limit. */
    fn run_regexp(
        &mut self,
        program: &regexp::Program,
        text: &JsString,
        start: usize,
    ) -> Res<Option<Captures>> {
        self.charge(text.len().saturating_sub(start) / crate::constants::JS_STRING_SCAN_UNITS + 1)?;
        regexp::exec(program, text.as_units(), start).map_err(|MatchError::Limit| {
            EngineError::limit("regular expression steps", crate::constants::REGEXP_STEPS).into()
        })
    }
    /** RegExpBuiltinExec: honors lastIndex for global and sticky patterns. */
    pub fn regexp_exec_raw(&mut self, id: ObjectId, text: &JsString) -> Res<Option<Captures>> {
        let (_, data) = self.regexp_data(&JsValue::Object(id))?;
        let program = self.regexp_compile(&data.source, &data.flags)?;
        let uses_last = program.global || program.sticky;
        let last = if uses_last {
            let value = self.get(&JsValue::Object(id), &key("lastIndex"))?;
            self.to_integer(&value)?.max(0.0) as usize
        } else {
            0
        };
        if last > text.len() {
            self.set(
                &JsValue::Object(id),
                key("lastIndex"),
                JsValue::Number(0.0),
                true,
            )?;
            return Ok(None);
        }
        let result = self.run_regexp(&program, text, last)?;
        if uses_last {
            let next = match &result {
                Some(captures) => captures[0].map_or(0, |(_, end)| end),
                None => 0,
            };
            self.set(
                &JsValue::Object(id),
                key("lastIndex"),
                JsValue::Number(next as f64),
                true,
            )?;
        }
        Ok(result)
    }
    /** The match array exec returns. */
    pub fn match_array(
        &mut self,
        id: ObjectId,
        text: &JsString,
        captures: &Captures,
    ) -> Res<JsValue> {
        let (_, data) = self.regexp_data(&JsValue::Object(id))?;
        let program = self.regexp_compile(&data.source, &data.flags)?;
        let mut elements = Vec::with_capacity(captures.len());
        for capture in captures {
            elements.push(match capture {
                Some((start, end)) => {
                    JsValue::String(JsString::from_slice(&text.as_units()[*start..*end])?)
                }
                None => JsValue::Undefined,
            });
        }
        let array = self.new_array(elements.clone())?;
        let index = captures[0].map_or(0, |(start, _)| start);
        self.put(array, "index", JsValue::Number(index as f64))?;
        self.put(array, "input", JsValue::String(text.clone()))?;
        let groups = if program.names.is_empty() {
            JsValue::Undefined
        } else {
            let groups = self.alloc(Object::new(Class::Plain, None))?;
            for (name, index) in &program.names {
                self.put(
                    groups,
                    name,
                    elements.get(*index).cloned().unwrap_or(JsValue::Undefined),
                )?;
            }
            JsValue::Object(groups)
        };
        self.put(array, "groups", groups)?;
        Ok(JsValue::Object(array))
    }
    /** RegExpExec: a user `exec` if overridden, else the builtin. */
    pub fn regexp_exec(&mut self, regexp: &JsValue, text: &JsString) -> Res<JsValue> {
        let exec = self.get(regexp, &key("exec"))?;
        let builtin =
            matches!(&exec, JsValue::Object(id) if *id >= crate::v9::vm::heap::NATIVE_BASE);
        if !builtin && self.is_callable(&exec) {
            return self.call(exec, regexp.clone(), vec![JsValue::String(text.clone())]);
        }
        let (id, _) = self.regexp_data(regexp)?;
        match self.regexp_exec_raw(id, text)? {
            Some(captures) => self.match_array(id, text, &captures),
            None => Ok(JsValue::Null),
        }
    }
    /** Step a matchAll iterator. */
    pub fn matches_step(&mut self, iterator: ObjectId) -> Res<Option<JsValue>> {
        let Some(Class::Iter(state)) = self.heap.get(iterator).map(|o| &o.class) else {
            return Ok(None);
        };
        let IterState::Matches {
            regexp,
            text,
            global,
            unicode,
            done,
        } = (**state).clone()
        else {
            return Ok(None);
        };
        if done {
            return Ok(None);
        }
        let result = self.regexp_exec(&JsValue::Object(regexp), &text)?;
        let finish = |vm: &mut Vm| {
            if let Some(Class::Iter(state)) = vm.heap.get_mut(iterator).map(|o| &mut o.class)
                && let IterState::Matches { done, .. } = state.as_mut()
            {
                *done = true;
            }
        };
        if result == JsValue::Null {
            finish(self);
            return Ok(None);
        }
        if !global {
            finish(self);
            return Ok(Some(result));
        }
        let matched = self.get(&result, &key("0"))?;
        let matched = self.to_string(&matched)?;
        if matched.is_empty() {
            let last = self.get(&JsValue::Object(regexp), &key("lastIndex"))?;
            let last = self.to_integer(&last)? as usize;
            let next = advance(&text, last, unicode);
            self.set(
                &JsValue::Object(regexp),
                key("lastIndex"),
                JsValue::Number(next as f64),
                true,
            )?;
        }
        Ok(Some(result))
    }
}

/** The index after `index`, stepping over a surrogate pair in unicode mode. */
pub(in crate::v9) fn advance(text: &JsString, index: usize, unicode: bool) -> usize {
    let units = text.as_units();
    if unicode
        && index + 1 < units.len()
        && (0xd800..=0xdbff).contains(&units[index])
        && (0xdc00..=0xdfff).contains(&units[index + 1])
    {
        return index + 2;
    }
    index + 1
}

/** `RegExp(pattern, flags)` and `new RegExp(...)`. */
fn construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let pattern = call.arg(0);
    let flags_arg = call.arg(1);
    if let JsValue::Object(id) = &pattern
        && let Some(Class::RegExp(data)) = vm.heap.get(*id).map(|o| &o.class)
    {
        let data = (**data).clone();
        if call.new_target.is_none() && flags_arg == JsValue::Undefined {
            return Ok(pattern);
        }
        let flags = if flags_arg == JsValue::Undefined {
            data.flags.clone()
        } else {
            vm.to_text(&flags_arg)?
        };
        return vm.new_regexp(data.source, &flags).map(JsValue::Object);
    }
    let source = if pattern == JsValue::Undefined {
        JsString::empty()
    } else {
        vm.to_string(&pattern)?
    };
    let flags = if flags_arg == JsValue::Undefined {
        String::new()
    } else {
        vm.to_text(&flags_arg)?
    };
    let source = if source.is_empty() {
        JsString::from_utf8("(?:)")?
    } else {
        source
    };
    vm.new_regexp(source, &flags).map(JsValue::Object)
}

/** RegExp.prototype.exec. */
fn exec(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (id, _) = vm.regexp_data(&call.this)?;
    let text = arg_string(vm, &call, 0)?;
    match vm.regexp_exec_raw(id, &text)? {
        Some(captures) => vm.match_array(id, &text, &captures),
        None => Ok(JsValue::Null),
    }
}

/** RegExp.prototype.test. */
fn test(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let text = arg_string(vm, &call, 0)?;
    if let Ok((id, _)) = vm.regexp_data(&call.this) {
        let exec = vm.get(&call.this, &key("exec"))?;
        if matches!(&exec, JsValue::Object(f) if *f >= crate::v9::vm::heap::NATIVE_BASE) {
            return Ok(JsValue::Boolean(vm.regexp_exec_raw(id, &text)?.is_some()));
        }
    }
    let result = vm.regexp_exec(&call.this, &text)?;
    Ok(JsValue::Boolean(result != JsValue::Null))
}

/** RegExp.prototype.toString. */
fn to_string(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let source = vm.get(&call.this, &key("source"))?;
    let source = vm.to_text(&source)?;
    let flags = vm.get(&call.this, &key("flags"))?;
    let flags = vm.to_text(&flags)?;
    text(vm, &format!("/{source}/{flags}"))
}

/** RegExp.prototype.compile (legacy): replace pattern and flags in place. */
fn recompile(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (id, _) = vm.regexp_data(&call.this)?;
    let source = arg_string(vm, &call, 0)?;
    let flags = if call.arg(1) == JsValue::Undefined {
        String::new()
    } else {
        arg_string(vm, &call, 1)?.lossy()
    };
    vm.regexp_compile(&source, &flags)?;
    if let Some(Class::RegExp(data)) = vm.heap.get_mut(id).map(|o| &mut o.class) {
        data.source = source;
        data.flags = flags;
    }
    Ok(call.this)
}

/** RegExp.prototype.source. */
fn source(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if call.this == JsValue::Object(realm().ids.regexp_prototype) {
        return text(vm, "(?:)");
    }
    let (_, data) = vm.regexp_data(&call.this)?;
    Ok(JsValue::String(data.source))
}

/** RegExp.prototype.flags in canonical order. */
fn flags(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if call.this == JsValue::Object(realm().ids.regexp_prototype) {
        return text(vm, "");
    }
    let (_, data) = vm.regexp_data(&call.this)?;
    let canonical: String = "dgimsuvy"
        .chars()
        .filter(|flag| data.flags.contains(*flag))
        .collect();
    text(vm, &canonical)
}

/** A boolean flag getter. */
fn flag(vm: &mut Vm, call: &Call, flag: char) -> Res<JsValue> {
    if call.this == JsValue::Object(realm().ids.regexp_prototype) {
        return Ok(JsValue::Undefined);
    }
    let (_, data) = vm.regexp_data(&call.this)?;
    Ok(JsValue::Boolean(data.flags.contains(flag)))
}

/** RegExp.prototype.global. */
fn global(vm: &mut Vm, call: Call) -> Res<JsValue> {
    flag(vm, &call, 'g')
}
/** RegExp.prototype.ignoreCase. */
fn ignore_case(vm: &mut Vm, call: Call) -> Res<JsValue> {
    flag(vm, &call, 'i')
}
/** RegExp.prototype.multiline. */
fn multiline(vm: &mut Vm, call: Call) -> Res<JsValue> {
    flag(vm, &call, 'm')
}
/** RegExp.prototype.dotAll. */
fn dot_all(vm: &mut Vm, call: Call) -> Res<JsValue> {
    flag(vm, &call, 's')
}
/** RegExp.prototype.unicode. */
fn unicode(vm: &mut Vm, call: Call) -> Res<JsValue> {
    flag(vm, &call, 'u')
}
/** RegExp.prototype.sticky. */
fn sticky(vm: &mut Vm, call: Call) -> Res<JsValue> {
    flag(vm, &call, 'y')
}
/** RegExp.prototype.hasIndices. */
fn has_indices(vm: &mut Vm, call: Call) -> Res<JsValue> {
    flag(vm, &call, 'd')
}

/** Expand `$&`, `$1`, `$<name>` and friends in a replacement template. */
pub(in crate::v9) fn expand(
    vm: &mut Vm,
    template: &JsString,
    text: &JsString,
    position: usize,
    matched: &JsString,
    captures: &[JsValue],
    groups: &JsValue,
) -> Res<Vec<u16>> {
    let units = template.as_units();
    let source = text.as_units();
    let mut out = Vec::with_capacity(units.len());
    let mut index = 0;
    while index < units.len() {
        let unit = units[index];
        if unit != u16::from(b'$') || index + 1 >= units.len() {
            out.push(unit);
            index += 1;
            continue;
        }
        let next = units[index + 1];
        match next {
            0x24 => {
                out.push(0x24);
                index += 2;
            }
            0x26 => {
                out.extend_from_slice(matched.as_units());
                index += 2;
            }
            0x60 => {
                out.extend_from_slice(&source[..position.min(source.len())]);
                index += 2;
            }
            0x27 => {
                let end = (position + matched.len()).min(source.len());
                out.extend_from_slice(&source[end..]);
                index += 2;
            }
            0x30..=0x39 => {
                let first = usize::from(next - 0x30);
                let two = units
                    .get(index + 2)
                    .filter(|d| (0x30..=0x39).contains(*d))
                    .map(|d| first * 10 + usize::from(d - 0x30));
                let (group, width) = match two {
                    Some(group) if group >= 1 && group <= captures.len() => (group, 3),
                    _ => (first, 2),
                };
                if group >= 1 && group <= captures.len() {
                    if let JsValue::String(value) = &captures[group - 1] {
                        out.extend_from_slice(value.as_units());
                    }
                    index += width;
                } else {
                    out.push(unit);
                    index += 1;
                }
            }
            0x3c if *groups != JsValue::Undefined => {
                let close = units[index + 2..]
                    .iter()
                    .position(|u| *u == u16::from(b'>'));
                match close {
                    Some(close) => {
                        let name = String::from_utf16_lossy(&units[index + 2..index + 2 + close]);
                        let value = vm.get(groups, &key(&name))?;
                        if value != JsValue::Undefined {
                            out.extend_from_slice(vm.to_string(&value)?.as_units());
                        }
                        index += close + 3;
                    }
                    None => {
                        out.push(unit);
                        index += 1;
                    }
                }
            }
            _ => {
                out.push(unit);
                index += 1;
            }
        }
        if out.len() > crate::constants::JS_STRING_UNITS {
            return Err(EngineError::limit(
                "JavaScript string UTF-16 units",
                crate::constants::JS_STRING_UNITS,
            )
            .into());
        }
    }
    Ok(out)
}

/** RegExp.prototype[Symbol.replace]. */
fn symbol_replace(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let regexp = call.this.clone();
    let (id, data) = vm.regexp_data(&regexp)?;
    let text = arg_string(vm, &call, 0)?;
    let replacement = call.arg(1);
    let functional = vm.is_callable(&replacement);
    let template = if functional {
        JsString::empty()
    } else {
        vm.to_string(&replacement)?
    };
    let global = data.flags.contains('g');
    let unicode = data.flags.contains('u');
    if global {
        vm.set(&regexp, key("lastIndex"), JsValue::Number(0.0), true)?;
    }
    let mut results = Vec::new();
    loop {
        let result = vm.regexp_exec(&regexp, &text)?;
        if result == JsValue::Null {
            break;
        }
        vm.pin(result.clone());
        results.push(result.clone());
        if !global {
            break;
        }
        let matched = vm.get(&result, &key("0"))?;
        let matched = vm.to_string(&matched)?;
        if matched.is_empty() {
            let last = vm.get(&regexp, &key("lastIndex"))?;
            let last = vm.to_integer(&last)? as usize;
            vm.set(
                &regexp,
                key("lastIndex"),
                JsValue::Number(advance(&text, last, unicode) as f64),
                true,
            )?;
        }
        if results.len() > text.len() + 1 {
            break;
        }
    }
    let _ = id;
    let source = text.as_units();
    let mut out: Vec<u16> = Vec::new();
    let mut next = 0;
    for result in results {
        let length = vm.length_of(&result)?;
        let matched = vm.get(&result, &key("0"))?;
        let matched = vm.to_string(&matched)?;
        let position = vm.get(&result, &key("index"))?;
        let position = (vm.to_integer(&position)?.max(0.0) as usize).min(source.len());
        let mut captures = Vec::new();
        for index in 1..length {
            let value = vm.get(&result, &key(&index.to_string()))?;
            captures.push(if value == JsValue::Undefined {
                value
            } else {
                JsValue::String(vm.to_string(&value)?)
            });
        }
        let groups = vm.get(&result, &key("groups"))?;
        let replaced: Vec<u16> = if functional {
            let mut args = vec![JsValue::String(matched.clone())];
            args.extend(captures.iter().cloned());
            args.push(JsValue::Number(position as f64));
            args.push(JsValue::String(text.clone()));
            if groups != JsValue::Undefined {
                args.push(groups.clone());
            }
            let value = vm.call(replacement.clone(), JsValue::Undefined, args)?;
            vm.to_string(&value)?.as_units().to_vec()
        } else {
            expand(vm, &template, &text, position, &matched, &captures, &groups)?
        };
        if position >= next {
            out.extend_from_slice(&source[next..position]);
            out.extend_from_slice(&replaced);
            next = (position + matched.len()).min(source.len());
        }
        if out.len() > crate::constants::JS_STRING_UNITS {
            return Err(EngineError::limit(
                "JavaScript string UTF-16 units",
                crate::constants::JS_STRING_UNITS,
            )
            .into());
        }
    }
    out.extend_from_slice(&source[next..]);
    Ok(JsValue::String(JsString::from_units(out)?))
}

/** RegExp.prototype[Symbol.split]. */
fn symbol_split(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (_, data) = vm.regexp_data(&call.this)?;
    let text = arg_string(vm, &call, 0)?;
    let limit = if call.arg(1) == JsValue::Undefined {
        u32::MAX
    } else {
        crate::v9::value::to_uint32(vm.to_number(&call.arg(1))?)
    };
    let flags: String = data
        .flags
        .chars()
        .filter(|flag| *flag != 'g' && *flag != 'y')
        .collect();
    let program = vm.regexp_compile(&data.source, &format!("{flags}y"))?;
    let result = vm.new_array(Vec::new())?;
    if limit == 0 {
        return Ok(JsValue::Object(result));
    }
    let units = text.as_units();
    if units.is_empty() {
        if vm.run_regexp(&program, &text, 0)?.is_none() {
            vm.array_append(&JsValue::Object(result), JsValue::String(text))?;
        }
        return Ok(JsValue::Object(result));
    }
    let mut start = 0;
    let mut position = 0;
    let mut count = 0u32;
    while position < units.len() {
        let Some(captures) = vm.run_regexp(&program, &text, position)? else {
            position = advance(&text, position, program.unicode);
            continue;
        };
        let end = captures[0]
            .map_or(position, |(_, end)| end)
            .min(units.len());
        if end == start {
            position = advance(&text, position, program.unicode);
            continue;
        }
        let piece = JsString::from_slice(&units[start..position])?;
        vm.array_append(&JsValue::Object(result), JsValue::String(piece))?;
        count += 1;
        if count == limit {
            return Ok(JsValue::Object(result));
        }
        for capture in captures.iter().skip(1) {
            let value = match capture {
                Some((a, b)) => JsValue::String(JsString::from_slice(&units[*a..*b])?),
                None => JsValue::Undefined,
            };
            vm.array_append(&JsValue::Object(result), value)?;
            count += 1;
            if count == limit {
                return Ok(JsValue::Object(result));
            }
        }
        start = end;
        position = end;
    }
    let tail = JsString::from_slice(&units[start..])?;
    vm.array_append(&JsValue::Object(result), JsValue::String(tail))?;
    Ok(JsValue::Object(result))
}

/** RegExp.prototype[Symbol.match]. */
fn symbol_match(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let regexp = call.this.clone();
    let (_, data) = vm.regexp_data(&regexp)?;
    let text = arg_string(vm, &call, 0)?;
    if !data.flags.contains('g') {
        return vm.regexp_exec(&regexp, &text);
    }
    vm.set(&regexp, key("lastIndex"), JsValue::Number(0.0), true)?;
    let result = vm.new_array(Vec::new())?;
    loop {
        let found = vm.regexp_exec(&regexp, &text)?;
        if found == JsValue::Null {
            break;
        }
        let matched = vm.get(&found, &key("0"))?;
        let matched = vm.to_string(&matched)?;
        if matched.is_empty() {
            let last = vm.get(&regexp, &key("lastIndex"))?;
            let last = vm.to_integer(&last)? as usize;
            vm.set(
                &regexp,
                key("lastIndex"),
                JsValue::Number(advance(&text, last, data.flags.contains('u')) as f64),
                true,
            )?;
        }
        vm.array_append(&JsValue::Object(result), JsValue::String(matched))?;
    }
    let empty = matches!(vm.heap.get(result).map(|o| &o.class), Some(Class::Array(elements)) if elements.is_empty());
    Ok(if empty {
        JsValue::Null
    } else {
        JsValue::Object(result)
    })
}

/** RegExp.prototype[Symbol.matchAll]. */
fn symbol_match_all(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (_, data) = vm.regexp_data(&call.this)?;
    let text = arg_string(vm, &call, 0)?;
    let clone = vm.new_regexp(data.source.clone(), &data.flags)?;
    let last = vm.get(&call.this, &key("lastIndex"))?;
    vm.set(&JsValue::Object(clone), key("lastIndex"), last, true)?;
    vm.new_iterator(IterState::Matches {
        regexp: clone,
        text,
        global: data.flags.contains('g'),
        unicode: data.flags.contains('u'),
        done: false,
    })
    .map(JsValue::Object)
}

/** RegExp.prototype[Symbol.search]. */
fn symbol_search(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let regexp = call.this.clone();
    let text = arg_string(vm, &call, 0)?;
    let previous = vm.get(&regexp, &key("lastIndex"))?;
    vm.set(&regexp, key("lastIndex"), JsValue::Number(0.0), true)?;
    let result = vm.regexp_exec(&regexp, &text)?;
    vm.set(&regexp, key("lastIndex"), previous, true)?;
    if result == JsValue::Null {
        return Ok(JsValue::Number(-1.0));
    }
    vm.get(&result, &key("index"))
}
