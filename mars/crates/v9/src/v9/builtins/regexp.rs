/** RegExp objects over V9's own engine: exec, test, flags, and the string protocol methods. */
use super::*;
use crate::protocol::EngineError;
use crate::v9::regexp::{self, Captures, MatchError};
use crate::v9::vm::data::{Class, IterState, RegExpData};
use crate::v9::vm::heap::{Object, Prop, Slot, WRITABLE};
use crate::v9::vm::realm::Static;
use crate::v9::vm::realm::{realm, symbol};
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

mod escape;
mod legacy;

/** The builtin exec dispatch index is shared by immutable realm templates. */
static BUILTIN_EXEC: AtomicU32 = AtomicU32::new(0);

/** RegExp and RegExp.prototype. */
pub(super) fn install(b: &mut Builder) {
    let proto = b.object(Some(b.ids.object_prototype));
    b.ids.regexp_prototype = proto;
    let ctor = b.constructor("RegExp", 2, construct, proto, None);
    b.global("RegExp", ctor);
    b.method(ctor, "escape", 1, escape::escape);
    legacy::install(b, ctor);
    let species = b.function("get [Symbol.species]", 0, species);
    b.put(
        ctor,
        Key::Sym(symbol::SPECIES),
        Static::Accessor(Some(species), None),
        crate::v9::vm::heap::CONFIGURABLE,
    );
    let exec_id = b.method(proto, "exec", 1, exec);
    BUILTIN_EXEC.store(exec_id, Ordering::Relaxed);
    b.method(proto, "test", 1, test);
    b.method(proto, "toString", 0, to_string);
    b.method(proto, "compile", 2, recompile);
    for (name, getter_name, getter) in [
        (
            "source",
            "get source",
            source as crate::v9::vm::realm::NativeFn,
        ),
        ("flags", "get flags", flags),
        ("global", "get global", global),
        ("ignoreCase", "get ignoreCase", ignore_case),
        ("multiline", "get multiline", multiline),
        ("dotAll", "get dotAll", dot_all),
        ("unicode", "get unicode", unicode),
        ("unicodeSets", "get unicodeSets", unicode_sets),
        ("sticky", "get sticky", sticky),
        ("hasIndices", "get hasIndices", has_indices),
    ] {
        let get = b.function(getter_name, 0, getter);
        b.put(
            proto,
            Key::from(name),
            Static::Accessor(Some(get), None),
            crate::v9::vm::heap::CONFIGURABLE,
        );
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

/** The largest ToLength result, 2^53 - 1. */
const MAX_LENGTH: f64 = 9_007_199_254_740_991.0;

/** The %RegExp% intrinsic as a value. */
fn regexp_ctor() -> JsValue {
    let definition = realm()
        .object(realm().ids.regexp_prototype)
        .expect("RegExp prototype exists");
    match &definition
        .get(&key("constructor"))
        .expect("RegExp constructor exists")
        .1
    {
        Static::Value(value) => value.clone(),
        _ => unreachable!("intrinsic constructor is a data property"),
    }
}

/** The builtin exec has a distinct identity in each realm but the same dispatcher. */
fn builtin_exec() -> JsValue {
    let index = crate::v9::vm::realm::native_index(BUILTIN_EXEC.load(Ordering::Relaxed));
    JsValue::Object(realm().native_identity(index))
}

/** RegExpCreate initializes directly, without the constructor's observable IsRegExp lookup. */
pub(super) fn create(vm: &mut Vm, pattern: JsValue, flags: JsValue) -> Res<JsValue> {
    initialize(vm, &pattern, &flags, realm().ids.regexp_prototype).map(JsValue::Object)
}

/** ToLength, which lastIndex and every index read back from a match go through. */
fn to_length(vm: &mut Vm, value: &JsValue) -> Res<f64> {
    Ok(vm.to_integer(value)?.clamp(0.0, MAX_LENGTH))
}

/** AdvanceStringIndex over a ToLength index, which can sit past the string's end. */
fn advance_index(text: &JsString, index: f64, unicode: bool) -> f64 {
    if index + 1.0 >= text.len() as f64 {
        return index + 1.0;
    }
    advance(text, index as usize, unicode) as f64
}

impl Vm<'_> {
    /** A new RegExp object; a bad pattern throws SyntaxError. */
    pub fn new_regexp(&mut self, source: JsString, flags: &str) -> Res<ObjectId> {
        self.alloc_regexp(source, flags, realm().ids.regexp_prototype)
    }
    /** RegExpAlloc plus RegExpInitialize: compile first, so a bad pattern allocates nothing. */
    fn alloc_regexp(&mut self, source: JsString, flags: &str, proto: ObjectId) -> Res<ObjectId> {
        self.regexp_compile(&source, flags)?;
        let id = self.alloc(Object::new(
            Class::RegExp(Box::new(RegExpData {
                source,
                flags: flags.to_string(),
            })),
            Some(proto),
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
        let program = regexp::compile(
            source.as_units(),
            flags,
            self.page.environment.regexp_buffer_boundaries,
        )
        .map_err(|error| {
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
    /** RegExpBuiltinExec: lastIndex is always read through ToLength (it is observable), but only
    global and sticky patterns start from it or write it back. */
    pub fn regexp_exec_raw(&mut self, id: ObjectId, text: &JsString) -> Res<Option<Captures>> {
        let (_, data) = self.regexp_data(&JsValue::Object(id))?;
        let program = self.regexp_compile(&data.source, &data.flags)?;
        let this = JsValue::Object(id);
        let value = self.get(&this, &key("lastIndex"))?;
        let last = to_length(self, &value)?;
        let uses_last = program.global || program.sticky;
        let start = if uses_last { last } else { 0.0 };
        if start > text.len() as f64 {
            if uses_last {
                self.set(&this, key("lastIndex"), JsValue::Number(0.0), true)?;
            }
            return Ok(None);
        }
        let result = self.run_regexp(&program, text, start as usize)?;
        if let Some(captures) = &result {
            self.record_legacy_match(text, captures)?;
        }
        if uses_last {
            let next = match &result {
                Some(captures) => captures[0].map_or(0, |(_, end)| end),
                None => 0,
            };
            self.set(&this, key("lastIndex"), JsValue::Number(next as f64), true)?;
        }
        Ok(result)
    }
    /** The match array exec returns, with `indices` when the pattern has the d flag. */
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
        self.pin(JsValue::Object(array));
        let index = captures[0].map_or(0, |(start, _)| start);
        self.put(array, "index", JsValue::Number(index as f64))?;
        self.put(array, "input", JsValue::String(text.clone()))?;
        let groups = self.groups_object(&program, &elements)?;
        self.put(array, "groups", groups)?;
        if program.indices {
            let indices = self.match_indices(&program, captures)?;
            self.put(array, "indices", indices)?;
        }
        Ok(JsValue::Object(array))
    }
    /** The null-prototype `groups` object mapping each name to its entry, or undefined. */
    fn groups_object(&mut self, program: &regexp::Program, values: &[JsValue]) -> Res<JsValue> {
        if program.names.is_empty() {
            return Ok(JsValue::Undefined);
        }
        let groups = self.alloc(Object::new(Class::Plain, None))?;
        for (name, index) in &program.names {
            let value = values.get(*index).cloned().unwrap_or(JsValue::Undefined);
            self.put(groups, name, value)?;
        }
        Ok(JsValue::Object(groups))
    }
    /** MakeMatchIndicesIndexPairArray: a [start, end] pair (or undefined) per capture, plus
    `groups` of the same pairs by name. */
    fn match_indices(&mut self, program: &regexp::Program, captures: &Captures) -> Res<JsValue> {
        let mut pairs = Vec::with_capacity(captures.len());
        for capture in captures {
            pairs.push(match capture {
                Some((start, end)) => JsValue::Object(self.new_array(vec![
                    JsValue::Number(*start as f64),
                    JsValue::Number(*end as f64),
                ])?),
                None => JsValue::Undefined,
            });
            if let Some(JsValue::Object(pair)) = pairs.last() {
                self.pin(JsValue::Object(*pair));
            }
        }
        let array = self.new_array(pairs.clone())?;
        self.pin(JsValue::Object(array));
        let groups = self.groups_object(program, &pairs)?;
        self.put(array, "groups", groups)?;
        Ok(JsValue::Object(array))
    }
    /** RegExpExec: a script's `exec` must return an object or null; without one the receiver
    must be a real RegExp. */
    pub fn regexp_exec(&mut self, regexp: &JsValue, text: &JsString) -> Res<JsValue> {
        let exec = self.get(regexp, &key("exec"))?;
        let builtin = exec == builtin_exec();
        if !builtin && self.is_callable(&exec) {
            let result = self.call(exec, regexp.clone(), vec![JsValue::String(text.clone())])?;
            if !matches!(result, JsValue::Object(_) | JsValue::Null) {
                return Err(self.type_error(
                    "RegExp exec method returned something other than an Object or null",
                ));
            }
            return Ok(result);
        }
        let (id, _) = self.regexp_data(regexp)?;
        match self.regexp_exec_raw(id, text)? {
            Some(captures) => self.match_array(id, text, &captures),
            None => Ok(JsValue::Null),
        }
    }
    /** %RegExpStringIteratorPrototype%.next: one RegExpExec per step. */
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
        if self.to_string(&matched)?.is_empty() {
            advance_last_index(self, &JsValue::Object(regexp), &text, unicode)?;
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

/** After an empty match: lastIndex = AdvanceStringIndex(ToLength(lastIndex)), so global loops
make progress. */
fn advance_last_index(vm: &mut Vm, rx: &JsValue, text: &JsString, unicode: bool) -> Res<()> {
    let last = vm.get(rx, &key("lastIndex"))?;
    let last = to_length(vm, &last)?;
    let next = advance_index(text, last, unicode);
    vm.set(rx, key("lastIndex"), JsValue::Number(next), true)
}

/** IsRegExp: an object whose @@match is truthy, or, with @@match undefined, a real RegExp. */
pub(in crate::v9) fn is_regexp(vm: &mut Vm, value: &JsValue) -> Res<bool> {
    let JsValue::Object(id) = value else {
        return Ok(false);
    };
    let matcher = vm.get(value, &Key::Sym(symbol::MATCH))?;
    if matcher != JsValue::Undefined {
        return Ok(matcher.truthy());
    }
    Ok(matches!(
        vm.heap.get(*id).map(|o| &o.class),
        Some(Class::RegExp(_))
    ))
}

/** `this` for the generic protocol methods: any object, else a TypeError. */
fn object_this(vm: &mut Vm, call: &Call, method: &str) -> Res<JsValue> {
    match &call.this {
        JsValue::Object(_) => Ok(call.this.clone()),
        _ => {
            let receiver = match &call.this {
                JsValue::Symbol(id) => format!("Symbol({})", super::symbol_text(vm, *id)),
                other => vm.to_string(other)?.lossy(),
            };
            let method = method.replace("[Symbol.", ".@@");
            let method = method.trim_end_matches(']');
            Err(vm.type_error(&format!(
                "Method {method} called on incompatible receiver {receiver}"
            )))
        }
    }
}

/** ToString(Get(rx, "flags")): the protocol methods read flags this way, never the slot. */
fn flags_of(vm: &mut Vm, rx: &JsValue) -> Res<String> {
    let flags = vm.get(rx, &key("flags"))?;
    vm.to_text(&flags)
}

/** SpeciesConstructor(rx, %RegExp%). */
fn species_constructor(vm: &mut Vm, rx: &JsValue) -> Res<JsValue> {
    let ctor = vm.get(rx, &key("constructor"))?;
    if ctor == JsValue::Undefined {
        return Ok(regexp_ctor());
    }
    if !matches!(ctor, JsValue::Object(_)) {
        return Err(vm.type_error("object.constructor is not an object"));
    }
    let species = vm.get(&ctor, &Key::Sym(symbol::SPECIES))?;
    if species.nullish() {
        return Ok(regexp_ctor());
    }
    if vm.is_constructor(&species) {
        return Ok(species);
    }
    Err(vm.type_error("object.constructor[Symbol.species] is not a constructor"))
}

/** get RegExp[Symbol.species]: the receiver, so subclasses build their own kind. */
fn species(_vm: &mut Vm, call: Call) -> Res<JsValue> {
    Ok(call.this)
}

/** `RegExp(pattern, flags)` and `new RegExp(...)`, per the spec's RegExp constructor. */
fn construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let pattern = call.arg(0);
    let flags_arg = call.arg(1);
    let pattern_is_regexp = is_regexp(vm, &pattern)?;
    let new_target = match &call.new_target {
        Some(target) => target.clone(),
        None => {
            let active = JsValue::Object(call.callee);
            if pattern_is_regexp && flags_arg == JsValue::Undefined {
                let pattern_ctor = vm.get(&pattern, &key("constructor"))?;
                if pattern_ctor.same_value(&active) {
                    return Ok(pattern);
                }
            }
            active
        }
    };
    let slot = match &pattern {
        JsValue::Object(id) => match vm.heap.get(*id).map(|o| &o.class) {
            Some(Class::RegExp(data)) => Some((**data).clone()),
            _ => None,
        },
        _ => None,
    };
    let (source, flags) = if let Some(data) = slot {
        let flags = if flags_arg == JsValue::Undefined {
            JsValue::String(JsString::from_utf8(&data.flags)?)
        } else {
            flags_arg
        };
        (JsValue::String(data.source), flags)
    } else if pattern_is_regexp {
        let source = vm.get(&pattern, &key("source"))?;
        let flags = if flags_arg == JsValue::Undefined {
            vm.get(&pattern, &key("flags"))?
        } else {
            flags_arg
        };
        (source, flags)
    } else {
        (pattern, flags_arg)
    };
    let proto = vm.prototype_for(&new_target, realm().ids.regexp_prototype)?;
    initialize(vm, &source, &flags, proto).map(JsValue::Object)
}

/** RegExpInitialize's coercions: undefined pattern and flags are empty. */
fn initialize(vm: &mut Vm, source: &JsValue, flags: &JsValue, proto: ObjectId) -> Res<ObjectId> {
    let (source, flags) = coerce_source(vm, source, flags)?;
    vm.alloc_regexp(source, &flags, proto)
}

/** ToString of a pattern and its flags, with undefined meaning empty. */
fn coerce_source(vm: &mut Vm, source: &JsValue, flags: &JsValue) -> Res<(JsString, String)> {
    let source = if *source == JsValue::Undefined {
        JsString::empty()
    } else {
        vm.to_string(source)?
    };
    let flags = if *flags == JsValue::Undefined {
        String::new()
    } else {
        vm.to_text(flags)?
    };
    Ok((source, flags))
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

/** RegExp.prototype.test: skips building the match array when exec is the builtin. */
fn test(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let rx = object_this(vm, &call, "RegExp.prototype.test")?;
    let text = arg_string(vm, &call, 0)?;
    if let Ok((id, _)) = vm.regexp_data(&rx) {
        let exec = vm.get(&rx, &key("exec"))?;
        if exec == builtin_exec() {
            return Ok(JsValue::Boolean(vm.regexp_exec_raw(id, &text)?.is_some()));
        }
    }
    let result = vm.regexp_exec(&rx, &text)?;
    Ok(JsValue::Boolean(result != JsValue::Null))
}

/** RegExp.prototype.toString: generic over any object's source and flags. */
fn to_string(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let rx = object_this(vm, &call, "RegExp.prototype.toString")?;
    let source = vm.get(&rx, &key("source"))?;
    let source = vm.to_string(&source)?;
    let flags = vm.get(&rx, &key("flags"))?;
    let flags = vm.to_string(&flags)?;
    let mut out = vec![u16::from(b'/')];
    out.extend_from_slice(source.as_units());
    out.push(u16::from(b'/'));
    out.extend_from_slice(flags.as_units());
    Ok(JsValue::String(JsString::from_units(out)?))
}

/** RegExp.prototype.compile (Annex B): reinitialize in place; a RegExp pattern lends its slots
and refuses separate flags, and lastIndex resets through a throwing Set. */
fn recompile(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (id, _) = vm.regexp_data(&call.this)?;
    let pattern = call.arg(0);
    let flags = call.arg(1);
    let slot = match &pattern {
        JsValue::Object(p) => match vm.heap.get(*p).map(|o| &o.class) {
            Some(Class::RegExp(data)) => Some((**data).clone()),
            _ => None,
        },
        _ => None,
    };
    let (source, flags) = match slot {
        Some(_) if flags != JsValue::Undefined => {
            return Err(
                vm.type_error("Cannot supply flags when constructing one RegExp from another")
            );
        }
        Some(data) => (data.source, data.flags),
        None => coerce_source(vm, &pattern, &flags)?,
    };
    vm.regexp_compile(&source, &flags)?;
    if let Some(Class::RegExp(data)) = vm.heap.get_mut(id).map(|o| &mut o.class) {
        data.source = source;
        data.flags = flags;
    }
    vm.set(&call.this, key("lastIndex"), JsValue::Number(0.0), true)?;
    Ok(call.this)
}

/** EscapeRegExpPattern: `/` outside a class and line terminators are escaped so the source
reparses as a literal; the empty pattern reads `(?:)`. */
fn escape_source(source: &JsString) -> Res<JsString> {
    let units = source.as_units();
    if units.is_empty() {
        return Ok(JsString::from_utf8("(?:)")?);
    }
    let mut out = Vec::with_capacity(units.len());
    let (mut escaped, mut class) = (false, false);
    for &unit in units {
        let terminator = match unit {
            0x0a => Some("n"),
            0x0d => Some("r"),
            0x2028 => Some("u2028"),
            0x2029 => Some("u2029"),
            _ => None,
        };
        if let Some(spelling) = terminator {
            if !escaped {
                out.push(u16::from(b'\\'));
            }
            out.extend(spelling.encode_utf16());
            escaped = false;
            continue;
        }
        if unit == u16::from(b'/') && !escaped && !class {
            out.push(u16::from(b'\\'));
        }
        out.push(unit);
        if escaped {
            escaped = false;
        } else if unit == u16::from(b'\\') {
            escaped = true;
        } else if unit == u16::from(b'[') {
            class = true;
        } else if unit == u16::from(b']') {
            class = false;
        }
    }
    Ok(JsString::from_units(out)?)
}

/** RegExp.prototype.source. */
fn source(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if call.this == JsValue::Object(realm().ids.regexp_prototype) {
        return text(vm, "(?:)");
    }
    let (_, data) = vm.regexp_data(&call.this)?;
    Ok(JsValue::String(escape_source(&data.source)?))
}

/** RegExp.prototype.flags: generic, reading each flag getter in the spec's order. */
fn flags(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let rx = object_this(vm, &call, "RegExp.prototype.flags getter")?;
    let mut out = String::new();
    for (name, flag) in [
        ("hasIndices", 'd'),
        ("global", 'g'),
        ("ignoreCase", 'i'),
        ("multiline", 'm'),
        ("dotAll", 's'),
        ("unicode", 'u'),
        ("unicodeSets", 'v'),
        ("sticky", 'y'),
    ] {
        if vm.get(&rx, &key(name))?.truthy() {
            out.push(flag);
        }
    }
    text(vm, &out)
}

/** A boolean flag getter: undefined on the prototype itself, a TypeError on other objects. */
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
/** RegExp.prototype.unicodeSets. */
fn unicode_sets(vm: &mut Vm, call: Call) -> Res<JsValue> {
    flag(vm, &call, 'v')
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

/** The string-length guard every replacement builder checks. */
fn check_length(out: &[u16]) -> Res<()> {
    if out.len() > crate::constants::JS_STRING_UNITS {
        return Err(EngineError::limit(
            "JavaScript string UTF-16 units",
            crate::constants::JS_STRING_UNITS,
        )
        .into());
    }
    Ok(())
}

/** RegExp.prototype[Symbol.replace]: collect every RegExpExec result, then substitute. */
fn symbol_replace(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let rx = object_this(vm, &call, "RegExp.prototype[Symbol.replace]")?;
    let text = arg_string(vm, &call, 0)?;
    let replacement = call.arg(1);
    let functional = vm.is_callable(&replacement);
    let template = if functional {
        JsString::empty()
    } else {
        vm.to_string(&replacement)?
    };
    let flags = flags_of(vm, &rx)?;
    let global = flags.contains('g');
    let unicode = flags.contains('u') || flags.contains('v');
    if global {
        vm.set(&rx, key("lastIndex"), JsValue::Number(0.0), true)?;
    }
    let mut results = Vec::new();
    loop {
        let result = vm.regexp_exec(&rx, &text)?;
        if result == JsValue::Null {
            break;
        }
        vm.pin(result.clone());
        results.push(result.clone());
        if !global {
            break;
        }
        let matched = vm.get(&result, &key("0"))?;
        if vm.to_string(&matched)?.is_empty() {
            advance_last_index(vm, &rx, &text, unicode)?;
        }
    }
    let source = text.as_units();
    let mut out: Vec<u16> = Vec::new();
    let mut next = 0;
    for result in results {
        let length = vm.length_of(&result)?;
        let matched = vm.get(&result, &key("0"))?;
        let matched = vm.to_string(&matched)?;
        let position = vm.get(&result, &key("index"))?;
        let position = vm.to_integer(&position)?.clamp(0.0, source.len() as f64) as usize;
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
            let groups = if groups == JsValue::Undefined {
                groups
            } else {
                JsValue::Object(vm.to_object(&groups)?)
            };
            expand(vm, &template, &text, position, &matched, &captures, &groups)?
        };
        if position >= next {
            out.extend_from_slice(&source[next..position]);
            out.extend_from_slice(&replaced);
            next = (position + matched.len()).min(source.len());
        }
        check_length(&out)?;
    }
    if next < source.len() {
        out.extend_from_slice(&source[next..]);
    }
    Ok(JsValue::String(JsString::from_units(out)?))
}

/** The splitter's own program when nothing a script can observe differs from the spec loop:
the intrinsic constructor built it and exec is the builtin. Then the split runs on the engine
directly instead of one RegExpExec per position. */
fn fast_splitter(
    vm: &mut Vm,
    ctor: &JsValue,
    splitter: &JsValue,
) -> Res<Option<Rc<regexp::Program>>> {
    if *ctor != regexp_ctor() {
        return Ok(None);
    }
    let exec = vm.get(splitter, &key("exec"))?;
    if exec != builtin_exec() {
        return Ok(None);
    }
    let (_, data) = vm.regexp_data(splitter)?;
    vm.regexp_compile(&data.source, &data.flags).map(Some)
}

/** RegExp.prototype[Symbol.split]: a sticky splitter from the species constructor, tried at
every position. */
fn symbol_split(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let rx = object_this(vm, &call, "RegExp.prototype[Symbol.split]")?;
    let text = arg_string(vm, &call, 0)?;
    let ctor = species_constructor(vm, &rx)?;
    let flags = flags_of(vm, &rx)?;
    let unicode = flags.contains('u') || flags.contains('v');
    let new_flags = if flags.contains('y') {
        flags
    } else {
        format!("{flags}y")
    };
    let splitter = vm.construct(
        ctor.clone(),
        vec![rx, JsValue::String(JsString::from_utf8(&new_flags)?)],
        None,
    )?;
    vm.pin(splitter.clone());
    let result = JsValue::Object(vm.new_array(Vec::new())?);
    vm.pin(result.clone());
    let limit = if call.arg(1) == JsValue::Undefined {
        u32::MAX
    } else {
        crate::v9::value::to_uint32(vm.to_number(&call.arg(1))?)
    };
    if limit == 0 {
        return Ok(result);
    }
    let size = text.len();
    if size == 0 {
        if vm.regexp_exec(&splitter, &text)? == JsValue::Null {
            vm.array_append(&result, JsValue::String(text))?;
        }
        return Ok(result);
    }
    let fast = fast_splitter(vm, &ctor, &splitter)?;
    let units = text.as_units();
    let (mut p, mut q, mut count) = (0usize, 0usize, 0u32);
    while q < size {
        let (end, captures) = match &fast {
            Some(program) => match vm.run_regexp(program, &text, q)? {
                Some(found) => {
                    let end = found[0].map_or(q, |(_, end)| end);
                    let mut values = Vec::with_capacity(found.len());
                    for capture in found.iter().skip(1) {
                        values.push(match capture {
                            Some((a, b)) => JsValue::String(JsString::from_slice(&units[*a..*b])?),
                            None => JsValue::Undefined,
                        });
                    }
                    (Some(end), values)
                }
                None => (None, Vec::new()),
            },
            None => split_step(vm, &splitter, &text, q)?,
        };
        let Some(end) = end.map(|end| end.min(size)) else {
            q = advance(&text, q, unicode);
            continue;
        };
        if end == p {
            q = advance(&text, q, unicode);
            continue;
        }
        let piece = JsString::from_slice(&units[p.min(q)..q])?;
        vm.array_append(&result, JsValue::String(piece))?;
        count += 1;
        if count == limit {
            return Ok(result);
        }
        p = end;
        for value in captures {
            vm.array_append(&result, value)?;
            count += 1;
            if count == limit {
                return Ok(result);
            }
        }
        q = p;
    }
    let tail = JsString::from_slice(&units[p.min(size)..])?;
    vm.array_append(&result, JsValue::String(tail))?;
    Ok(result)
}

/** One spec split step: lastIndex = q, RegExpExec, then the end from lastIndex and the
captures from the result. */
fn split_step(
    vm: &mut Vm,
    splitter: &JsValue,
    text: &JsString,
    q: usize,
) -> Res<(Option<usize>, Vec<JsValue>)> {
    vm.set(splitter, key("lastIndex"), JsValue::Number(q as f64), true)?;
    let found = vm.regexp_exec(splitter, text)?;
    if found == JsValue::Null {
        return Ok((None, Vec::new()));
    }
    vm.pin(found.clone());
    let end = vm.get(splitter, &key("lastIndex"))?;
    let end = to_length(vm, &end)?.min(text.len() as f64) as usize;
    let count = vm.length_of(&found)?;
    let mut captures = Vec::new();
    for index in 1..count {
        captures.push(vm.get(&found, &key(&index.to_string()))?);
    }
    Ok((Some(end), captures))
}

/** RegExp.prototype[Symbol.match]. */
fn symbol_match(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let rx = object_this(vm, &call, "RegExp.prototype[Symbol.match]")?;
    let text = arg_string(vm, &call, 0)?;
    let flags = flags_of(vm, &rx)?;
    if !flags.contains('g') {
        return vm.regexp_exec(&rx, &text);
    }
    let unicode = flags.contains('u') || flags.contains('v');
    vm.set(&rx, key("lastIndex"), JsValue::Number(0.0), true)?;
    let result = JsValue::Object(vm.new_array(Vec::new())?);
    vm.pin(result.clone());
    let mut count = 0usize;
    loop {
        let found = vm.regexp_exec(&rx, &text)?;
        if found == JsValue::Null {
            return Ok(if count == 0 { JsValue::Null } else { result });
        }
        let matched = vm.get(&found, &key("0"))?;
        let matched = vm.to_string(&matched)?;
        if matched.is_empty() {
            advance_last_index(vm, &rx, &text, unicode)?;
        }
        vm.array_append(&result, JsValue::String(matched))?;
        count += 1;
    }
}

/** RegExp.prototype[Symbol.matchAll]: a matcher from the species constructor, carrying over
lastIndex. */
fn symbol_match_all(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let rx = object_this(vm, &call, "RegExp.prototype[Symbol.matchAll]")?;
    let text = arg_string(vm, &call, 0)?;
    let ctor = species_constructor(vm, &rx)?;
    let flags = flags_of(vm, &rx)?;
    let matcher = vm.construct(
        ctor,
        vec![rx.clone(), JsValue::String(JsString::from_utf8(&flags)?)],
        None,
    )?;
    vm.pin(matcher.clone());
    let last = vm.get(&rx, &key("lastIndex"))?;
    let last = to_length(vm, &last)?;
    vm.set(&matcher, key("lastIndex"), JsValue::Number(last), true)?;
    let JsValue::Object(matcher) = matcher else {
        return Err(vm.type_error("RegExp species constructor returned a non-object"));
    };
    vm.new_iterator(IterState::Matches {
        regexp: matcher,
        text,
        global: flags.contains('g'),
        unicode: flags.contains('u') || flags.contains('v'),
        done: false,
    })
    .map(JsValue::Object)
}

/** RegExp.prototype[Symbol.search]: lastIndex is written only when it is not already the
value being restored. */
fn symbol_search(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let rx = object_this(vm, &call, "RegExp.prototype[Symbol.search]")?;
    let text = arg_string(vm, &call, 0)?;
    let previous = vm.get(&rx, &key("lastIndex"))?;
    let zero = JsValue::Number(0.0);
    if !previous.same_value(&zero) {
        vm.set(&rx, key("lastIndex"), zero, true)?;
    }
    let result = vm.regexp_exec(&rx, &text)?;
    let current = vm.get(&rx, &key("lastIndex"))?;
    if !current.same_value(&previous) {
        vm.set(&rx, key("lastIndex"), previous, true)?;
    }
    if result == JsValue::Null {
        return Ok(JsValue::Number(-1.0));
    }
    vm.get(&result, &key("index"))
}
