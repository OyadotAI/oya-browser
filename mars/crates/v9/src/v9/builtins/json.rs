/** JSON.parse and JSON.stringify, bounded by the heap and string limits. */
use super::*;
use crate::constants::{JS_STRING_UNITS, SYNTAX_DEPTH};
use crate::protocol::EngineError;
use crate::v9::vm::data::Class;
use crate::v9::vm::heap::{CONFIGURABLE, FastMap};
use crate::v9::vm::property::Descriptor;
use crate::v9::vm::realm::{Static, string, symbol};

mod diagnostics;
mod input;
mod raw;
use diagnostics::Reason;
use input::Input;
pub(in crate::v9) use raw::branded as is_raw_json;
mod reviver;
use reviver::{Parsed, Reviver, Snapshot};

/** The JSON namespace. */
pub(super) fn install(b: &mut Builder) {
    let json = b.object(Some(b.ids.object_prototype));
    b.global("JSON", json);
    raw::install(b, json);
    b.method(json, "parse", 2, parse);
    b.method(json, "stringify", 3, stringify);
    b.put(
        json,
        Key::Sym(symbol::TO_STRING_TAG),
        Static::Value(string("JSON")),
        CONFIGURABLE,
    );
}

/** A JSON text parser building heap values directly. */
struct Reader<'a> {
    units: Input<'a>,
    position: usize,
    depth: usize,
    /** Retain source spans only when a callable reviver can observe them. */
    capture: bool,
    snapshot: Snapshot,
}

impl Reader<'_> {
    /** A property's original value and source span, captured before its parent is complete. */
    fn remember(&mut self, holder: ObjectId, name: Key) {
        if let Some(parsed) = &self.snapshot.root {
            self.snapshot
                .properties
                .insert((holder, name), parsed.clone());
        }
    }
    /** Skip JSON whitespace. */
    fn skip(&mut self) {
        while matches!(
            self.units.get(self.position),
            Some(0x20 | 0x09 | 0x0a | 0x0d)
        ) {
            self.position += 1;
        }
    }
    /** The SyntaxError for the current position. */
    fn error(&self, vm: &mut Vm) -> crate::v9::vm::Thrown {
        self.failure(vm, Reason::Unexpected)
    }
    /** The caller retains which grammar production rejected the current token. */
    fn failure(&self, vm: &mut Vm, reason: Reason) -> crate::v9::vm::Thrown {
        diagnostics::error(vm, self.units, self.position, reason)
    }
    /** One value. */
    fn value(&mut self, vm: &mut Vm) -> Res<JsValue> {
        self.skip();
        let start = self.position;
        self.depth += 1;
        if self.depth > SYNTAX_DEPTH {
            return Err(EngineError::limit("JSON nesting", SYNTAX_DEPTH).into());
        }
        vm.charge(1)?;
        let result = match self.units.get(self.position) {
            Some(0x7b) => self.object(vm),
            Some(0x5b) => self.array(vm),
            Some(0x22) => self.string(vm).map(JsValue::String),
            Some(0x74) => self.word(vm, "true", JsValue::Boolean(true)),
            Some(0x66) => self.word(vm, "false", JsValue::Boolean(false)),
            Some(0x6e) => self.word(vm, "null", JsValue::Null),
            Some(unit) if unit == 0x2d || (0x30..=0x39).contains(&unit) => self.number(vm),
            _ => Err(self.error(vm)),
        };
        self.depth -= 1;
        if self.capture
            && let Ok(value) = &result
        {
            self.snapshot.root = Some(Parsed {
                value: value.clone(),
                start,
                end: self.position,
            });
        }
        result
    }
    /** A literal word. */
    fn word(&mut self, vm: &mut Vm, word: &str, value: JsValue) -> Res<JsValue> {
        for character in word.bytes() {
            if self.units.get(self.position) != Some(u16::from(character)) {
                return Err(self.error(vm));
            }
            self.position += 1;
        }
        Ok(value)
    }
    /** A number in JSON's stricter grammar. */
    fn number(&mut self, vm: &mut Vm) -> Res<JsValue> {
        let start = self.position;
        if self.units.get(self.position) == Some(0x2d) {
            self.position += 1;
        }
        let digits = |reader: &mut Self| {
            let begin = reader.position;
            while reader
                .units
                .get(reader.position)
                .is_some_and(|u| (0x30..=0x39).contains(&u))
            {
                reader.position += 1;
            }
            reader.position > begin
        };
        if self.units.get(self.position) == Some(0x30) {
            self.position += 1;
            if self
                .units
                .get(self.position)
                .is_some_and(|unit| (0x30..=0x39).contains(&unit))
            {
                return Err(self.failure(vm, Reason::UnexpectedNumber));
            }
        } else if !digits(self) {
            return Err(self.failure(vm, Reason::NumberAfterMinus));
        }
        if self.units.get(self.position) == Some(0x2e) {
            self.position += 1;
            if !digits(self) {
                return Err(self.failure(vm, Reason::Fraction));
            }
        }
        if matches!(self.units.get(self.position), Some(0x65 | 0x45)) {
            self.position += 1;
            if matches!(self.units.get(self.position), Some(0x2b | 0x2d)) {
                self.position += 1;
            }
            if !digits(self) {
                return Err(self.failure(vm, Reason::Exponent));
            }
        }
        if self.position - start > JS_STRING_UNITS {
            return Err(
                EngineError::limit("JSON numeric token UTF-16 units", JS_STRING_UNITS).into(),
            );
        }
        let text = String::from_utf16_lossy(&self.units.range(start, self.position));
        Ok(JsValue::Number(text.parse().unwrap_or(f64::NAN)))
    }
    /** A quoted string. */
    fn string(&mut self, vm: &mut Vm) -> Res<JsString> {
        self.position += 1;
        let mut out = Vec::new();
        loop {
            let Some(unit) = self.units.get(self.position) else {
                return Err(self.failure(vm, Reason::UnterminatedString));
            };
            self.position += 1;
            match unit {
                0x22 => break,
                0x5c => {
                    let Some(escape) = self.units.get(self.position) else {
                        return Err(self.error(vm));
                    };
                    self.position += 1;
                    out.push(match escape {
                        0x22 => 0x22,
                        0x5c => 0x5c,
                        0x2f => 0x2f,
                        0x62 => 0x08,
                        0x66 => 0x0c,
                        0x6e => 0x0a,
                        0x72 => 0x0d,
                        0x74 => 0x09,
                        0x75 => self.unicode_escape(vm)?,
                        _ => {
                            self.position -= 1;
                            return Err(self.failure(vm, Reason::EscapedCharacter));
                        }
                    });
                }
                unit if unit < 0x20 => {
                    self.position -= 1;
                    return Err(self.failure(vm, Reason::ControlCharacter));
                }
                unit => out.push(unit),
            }
            if out.len() > JS_STRING_UNITS {
                return Err(
                    EngineError::limit("JavaScript string UTF-16 units", JS_STRING_UNITS).into(),
                );
            }
        }
        Ok(JsString::from_units(out)?)
    }
    /** A Unicode escape stops at the first nonhexadecimal source unit, preserving its exact diagnostic position. */
    fn unicode_escape(&mut self, vm: &mut Vm) -> Res<u16> {
        let mut value = 0;
        for _ in 0..4 {
            let digit = self
                .units
                .get(self.position)
                .and_then(|unit| char::from_u32(u32::from(unit)))
                .and_then(|character| character.to_digit(16));
            let Some(digit) = digit else {
                return Err(self.failure(vm, Reason::UnicodeEscape));
            };
            value = value * 16 + digit as u16;
            self.position += 1;
        }
        Ok(value)
    }
    /** `[...]` */
    fn array(&mut self, vm: &mut Vm) -> Res<JsValue> {
        self.position += 1;
        let array = vm.new_array(Vec::new())?;
        self.skip();
        if self.units.get(self.position) == Some(0x5d) {
            self.position += 1;
            return Ok(JsValue::Object(array));
        }
        let mut index = 0;
        loop {
            let value = self.value(vm)?;
            self.remember(array, key(&index.to_string()));
            vm.array_append(&JsValue::Object(array), value)?;
            index += 1;
            self.skip();
            match self.units.get(self.position) {
                Some(0x2c) => self.position += 1,
                Some(0x5d) => {
                    self.position += 1;
                    return Ok(JsValue::Object(array));
                }
                _ => return Err(self.failure(vm, Reason::ArraySeparator)),
            }
        }
    }
    /** `{...}` */
    fn object(&mut self, vm: &mut Vm) -> Res<JsValue> {
        self.position += 1;
        let object = vm.new_object()?;
        self.skip();
        if self.units.get(self.position) == Some(0x7d) {
            self.position += 1;
            return Ok(JsValue::Object(object));
        }
        let mut first = true;
        loop {
            self.skip();
            if self.units.get(self.position) != Some(0x22) {
                return Err(self.failure(
                    vm,
                    if first {
                        Reason::PropertyNameOrBrace
                    } else {
                        Reason::PropertyName
                    },
                ));
            }
            let name = self.string(vm)?;
            self.skip();
            if self.units.get(self.position) != Some(0x3a) {
                return Err(self.failure(vm, Reason::Colon));
            }
            self.position += 1;
            let value = self.value(vm)?;
            self.remember(object, Key::Str(name.clone()));
            vm.define(object, Key::Str(name), Descriptor::data(value))?;
            self.skip();
            match self.units.get(self.position) {
                Some(0x2c) => {
                    self.position += 1;
                    first = false;
                }
                Some(0x7d) => {
                    self.position += 1;
                    return Ok(JsValue::Object(object));
                }
                _ => return Err(self.failure(vm, Reason::ObjectSeparator)),
            }
        }
    }
}

/** Parse JSON text from units into heap values. */
pub(in crate::v9) fn json_parse_units(vm: &mut Vm, units: &[u16]) -> Res<JsValue> {
    read_json(vm, Input::Units(units), false).map(|(value, _)| value)
}

/** Parse once, optionally preserving source records for JSON.parse's reviver context. */
fn read_json(vm: &mut Vm, units: Input<'_>, capture: bool) -> Res<(JsValue, Snapshot)> {
    let mut reader = Reader {
        units,
        position: 0,
        depth: 0,
        capture,
        snapshot: Snapshot::default(),
    };
    let value = reader.value(vm)?;
    reader.skip();
    if reader.position < units.len() {
        return Err(reader.failure(vm, Reason::TrailingCharacter));
    }
    Ok((value, reader.snapshot))
}

/** Parse JSON text held outside the VM (a response body), which may exceed one string's bound. */
pub(in crate::v9) fn json_parse_text(vm: &mut Vm, text: &str) -> Res<JsValue> {
    let units: Vec<u16> = text.encode_utf16().collect();
    json_parse_units(vm, &units)
}

/** JSON.parse. */
fn parse(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let text = arg_string(vm, &call, 0)?;
    let reviver = call.arg(1);
    let callable = vm.is_callable(&reviver);
    let input = Input::from_string(&text);
    let (value, snapshot) = read_json(vm, input, callable)?;
    if !callable {
        return Ok(value);
    }
    let root = vm.new_object()?;
    vm.put(root, "", value)?;
    let visitor = Reviver {
        function: reviver,
        units: input,
        snapshot,
    };
    visitor.visit(
        vm,
        JsValue::Object(root),
        Key::from(""),
        visitor.snapshot.root.as_ref(),
    )
}

/** State for one stringify call. */
struct Writer {
    replacer: JsValue,
    allow: Option<Vec<Key>>,
    gap: Vec<u16>,
    stack: Vec<ObjectId>,
    out: Vec<u16>,
}

impl Writer {
    /** Append ASCII. */
    fn ascii(&mut self, text: &str) {
        self.out.extend(text.bytes().map(u16::from));
    }
    /** Append a quoted JSON string. */
    fn quote(&mut self, text: &JsString) {
        self.out.push(0x22);
        let mut index = 0;
        let flat = text.flat_units();
        let unit_at = |index| match flat {
            Some(units) => units.get(index).copied(),
            None => text.unit_at(index),
        };
        while index < text.len() {
            let unit = unit_at(index).expect("valid string index");
            match unit {
                0x22 => self.ascii("\\\""),
                0x5c => self.ascii("\\\\"),
                0x08 => self.ascii("\\b"),
                0x0c => self.ascii("\\f"),
                0x0a => self.ascii("\\n"),
                0x0d => self.ascii("\\r"),
                0x09 => self.ascii("\\t"),
                unit if unit < 0x20 => self.ascii(&format!("\\u{unit:04x}")),
                0xd800..=0xdbff
                    if unit_at(index + 1).is_some_and(|low| (0xdc00..=0xdfff).contains(&low)) =>
                {
                    self.out.push(unit);
                    self.out
                        .push(unit_at(index + 1).expect("checked low surrogate"));
                    index += 1;
                }
                0xd800..=0xdfff => self.ascii(&format!("\\u{unit:04x}")),
                unit => self.out.push(unit),
            }
            index += 1;
        }
        self.out.push(0x22);
    }
    /** A newline plus indentation for nesting depth. */
    fn indent(&mut self) {
        if self.gap.is_empty() {
            return;
        }
        self.out.push(0x0a);
        for _ in 0..self.stack.len() {
            let gap = self.gap.clone();
            self.out.extend(gap);
        }
    }
}

/** SerializeJSONProperty: false when the value is skipped (undefined, functions, symbols). */
fn serialize(vm: &mut Vm, writer: &mut Writer, holder: &JsValue, name: &Key) -> Res<bool> {
    vm.deeper(|vm| serialize_property(vm, writer, holder, name))
}

/** Property callbacks can create new nesting, so serialization shares the native call guard. */
fn serialize_property(vm: &mut Vm, writer: &mut Writer, holder: &JsValue, name: &Key) -> Res<bool> {
    vm.charge(1)?;
    let mut value = vm.get(holder, name)?;
    if matches!(
        value,
        JsValue::Object(_) | JsValue::BigInt(_) | JsValue::BigIntWide(_)
    ) {
        let to_json = vm.get(&value, &key("toJSON"))?;
        if vm.is_callable(&to_json) {
            value = vm.call(to_json, value.clone(), vec![name.value()])?;
        }
    }
    if vm.is_callable(&writer.replacer) {
        value = vm.call(
            writer.replacer.clone(),
            holder.clone(),
            vec![name.value(), value],
        )?;
    }
    if let JsValue::Object(id) = value {
        match vm.heap.get(id).map(|o| &o.class) {
            Some(Class::Number(_)) => value = JsValue::Number(vm.to_number(&value)?),
            Some(Class::String(_)) => value = JsValue::String(vm.to_string(&value)?),
            Some(Class::Boolean(flag)) => value = JsValue::Boolean(*flag),
            Some(Class::BigInt(big)) => value = JsValue::BigInt(*big),
            Some(Class::BigIntWide(big)) => value = JsValue::BigIntWide(big.clone()),
            _ => {}
        }
    }
    match &value {
        JsValue::BigInt(_) | JsValue::BigIntWide(_) => {
            return Err(vm.type_error("Do not know how to serialize a BigInt"));
        }
        JsValue::Null => writer.ascii("null"),
        JsValue::Boolean(flag) => writer.ascii(if *flag { "true" } else { "false" }),
        JsValue::String(text) => writer.quote(text),
        #[cfg(feature = "conformance")]
        JsValue::Smi(number) => writer.ascii(&crate::v9::value::number_to_string(*number)),
        JsValue::Number(number) => {
            if number.is_finite() {
                writer.ascii(&crate::v9::value::number_to_string(*number));
            } else {
                writer.ascii("null");
            }
        }
        #[cfg(feature = "conformance")]
        JsValue::SharedNumber(number) | JsValue::LocalNumber(number) => {
            if number.is_finite() {
                writer.ascii(&crate::v9::value::number_to_string(number.value));
            } else {
                writer.ascii("null");
            }
        }
        JsValue::Object(id) if !vm.is_callable(&value) => {
            if let Some(source) = raw::text(vm, &value)? {
                if writer.out.len().saturating_add(source.len()) > JS_STRING_UNITS {
                    return Err(vm.range_error("Invalid string length"));
                }
                writer
                    .out
                    .extend((0..source.len()).filter_map(|index| source.unit_at(index)));
                return Ok(true);
            }
            if writer.stack.contains(id) {
                return Err(vm.type_error("Converting circular structure to JSON"));
            }
            if writer.stack.len() > SYNTAX_DEPTH {
                return Err(EngineError::limit("JSON nesting", SYNTAX_DEPTH).into());
            }
            writer.stack.push(*id);
            let result = if super::array::is_array_checked(vm, &value)? {
                serialize_array(vm, writer, &value)
            } else {
                serialize_object(vm, writer, &value)
            };
            writer.stack.pop();
            result?;
        }
        _ => return Ok(false),
    }
    if writer.out.len() > JS_STRING_UNITS {
        return Err(vm.range_error("Invalid string length"));
    }
    Ok(true)
}

/** An array body. */
fn serialize_array(vm: &mut Vm, writer: &mut Writer, value: &JsValue) -> Res<()> {
    let length = vm.length_of(value)?;
    writer.out.push(0x5b);
    for index in 0..length {
        if index > 0 {
            writer.out.push(0x2c);
        }
        writer.indent();
        if !serialize(vm, writer, value, &key(&index.to_string()))? {
            writer.ascii("null");
        }
    }
    if length > 0 {
        writer.stack.pop();
        writer.indent();
        if let JsValue::Object(id) = value {
            writer.stack.push(*id);
        }
    }
    writer.out.push(0x5d);
    Ok(())
}

/** An object body. */
fn serialize_object(vm: &mut Vm, writer: &mut Writer, value: &JsValue) -> Res<()> {
    let JsValue::Object(id) = value else {
        return Ok(());
    };
    let keys = match &writer.allow {
        Some(allow) => allow.clone(),
        None => vm.enumerable_keys(*id)?,
    };
    writer.out.push(0x7b);
    let mut first = true;
    for name in keys {
        let mark = writer.out.len();
        if !first {
            writer.out.push(0x2c);
        }
        writer.indent();
        if let Key::Str(text) = &name {
            writer.quote(text);
        }
        writer.out.push(0x3a);
        if !writer.gap.is_empty() {
            writer.out.push(0x20);
        }
        if serialize(vm, writer, value, &name)? {
            first = false;
        } else {
            writer.out.truncate(mark);
        }
    }
    if !first {
        writer.stack.pop();
        writer.indent();
        writer.stack.push(*id);
    }
    writer.out.push(0x7d);
    Ok(())
}

/** JSON.stringify. */
pub(in crate::v9) fn stringify(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let replacer = call.arg(1);
    let allow = if !vm.is_callable(&replacer) && super::array::is_array_checked(vm, &replacer)? {
        let mut names = Vec::new();
        let length = vm.length_of(&replacer)?;
        for index in 0..length {
            let item = vm.get(&replacer, &key(&index.to_string()))?;
            if replacer_name(vm, &item) {
                let name = Key::Str(vm.to_string(&item)?);
                if !names.contains(&name) {
                    names.push(name);
                }
            }
        }
        Some(names)
    } else {
        None
    };
    let gap: Vec<u16> = match spacing_value(vm, call.arg(2))? {
        #[cfg(feature = "conformance")]
        JsValue::Smi(count) => vec![0x20; count.clamp(0.0, MAX_GAP as f64) as usize],
        JsValue::Number(count) => vec![0x20; count.clamp(0.0, MAX_GAP as f64) as usize],
        #[cfg(feature = "conformance")]
        JsValue::SharedNumber(count) | JsValue::LocalNumber(count) => {
            vec![0x20; count.value.clamp(0.0, MAX_GAP as f64) as usize]
        }
        JsValue::String(text) => (0..text.len().min(MAX_GAP))
            .filter_map(|index| text.unit_at(index))
            .collect(),
        _ => Vec::new(),
    };
    let mut writer = Writer {
        replacer: if vm.is_callable(&replacer) {
            replacer
        } else {
            JsValue::Undefined
        },
        allow,
        gap,
        stack: Vec::new(),
        out: Vec::new(),
    };
    let holder = vm.new_object()?;
    vm.put(holder, "", call.arg(0))?;
    if !serialize(vm, &mut writer, &JsValue::Object(holder), &key(""))? {
        return Ok(JsValue::Undefined);
    }
    if writer.out.len() > JS_STRING_UNITS {
        return Err(vm.range_error("Invalid string length"));
    }
    Ok(JsValue::String(JsString::from_units(writer.out)?))
}

/** Only strings and numbers, including their wrappers, become replacer property names. */
fn replacer_name(vm: &Vm, value: &JsValue) -> bool {
    match value {
        JsValue::String(_) | JsValue::Number(_) => true,
        #[cfg(feature = "conformance")]
        JsValue::SharedNumber(_) | JsValue::LocalNumber(_) | JsValue::Smi(_) => true,
        JsValue::Object(id) => matches!(
            vm.heap.get(*id).map(|object| &object.class),
            Some(Class::String(_) | Class::Number(_))
        ),
        _ => false,
    }
}

/** Boxed spacing values use the observable numeric or string conversion before indentation. */
fn spacing_value(vm: &mut Vm, value: JsValue) -> Res<JsValue> {
    let class = value
        .object()
        .and_then(|id| vm.heap.get(id))
        .map(|o| &o.class);
    if matches!(class, Some(Class::Number(_))) {
        return Ok(JsValue::Number(vm.to_number(&value)?));
    }
    if matches!(class, Some(Class::String(_))) {
        return Ok(JsValue::String(vm.to_string(&value)?));
    }
    Ok(value)
}

/** JSON indentation is capped at ten characters. */
const MAX_GAP: usize = 10;
