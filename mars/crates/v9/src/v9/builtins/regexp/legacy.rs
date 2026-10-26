/** Annex B RegExp statics derive from a retained input and exact UTF-16 capture positions. */
use super::*;
use crate::v9::vm::data::Class;
use crate::v9::vm::heap::CONFIGURABLE;

/** Record keys cannot be named or overwritten by page JavaScript. */
const INPUT: &str = "regexpLegacyInput";
/** Capture pairs retain offsets rather than copies of every matched substring. */
const CAPTURES: &str = "regexpLegacyCaptures";
/** Assigning input does not rewrite captures or the preceding match's context. */
const INPUT_OVERRIDE: &str = "regexpLegacyInputOverride";
/** Every successful group has a start and an exclusive end offset. */
const PAIR_WIDTH: usize = 2;
/** An unmatched group has no source interval. */
const UNMATCHED: f64 = -1.0;

/** Legacy aliases expose distinct accessors while sharing the same retained match facts. */
pub(super) fn install(b: &mut Builder, ctor: ObjectId) {
    let names = [
        ("input", "get input", "set input"),
        ("$_", "get $_", "set $_"),
        ("lastMatch", "get lastMatch", "set lastMatch"),
        ("$&", "get $&", "set $&"),
        ("lastParen", "get lastParen", "set lastParen"),
        ("$+", "get $+", "set $+"),
        ("leftContext", "get leftContext", "set leftContext"),
        ("$`", "get $`", "set $`"),
        ("rightContext", "get rightContext", "set rightContext"),
        ("$'", "get $'", "set $'"),
        ("$1", "get $1", "set $1"),
        ("$2", "get $2", "set $2"),
        ("$3", "get $3", "set $3"),
        ("$4", "get $4", "set $4"),
        ("$5", "get $5", "set $5"),
        ("$6", "get $6", "set $6"),
        ("$7", "get $7", "set $7"),
        ("$8", "get $8", "set $8"),
        ("$9", "get $9", "set $9"),
    ];
    for (name, get, set) in names {
        let get = b.function(get, 0, get_stat);
        let set = b.function(set, 1, set_stat);
        b.put(
            ctor,
            key(name),
            Static::Accessor(Some(get), Some(set)),
            CONFIGURABLE,
        );
    }
}

impl Vm<'_> {
    /** All native exec paths update the same realm-local record, including global string methods. */
    pub(super) fn record_legacy_match(&mut self, input: &JsString, captures: &Captures) -> Res<()> {
        let values = captures
            .iter()
            .flat_map(|span| match span {
                Some((start, end)) => {
                    [JsValue::Number(*start as f64), JsValue::Number(*end as f64)]
                }
                None => [JsValue::Number(UNMATCHED), JsValue::Number(UNMATCHED)],
            })
            .collect();
        let positions = self.new_array(values)?;
        let owner = realm().ids.regexp_prototype;
        self.materialize(owner)?;
        self.heap.remove(owner, &Key::private(INPUT_OVERRIDE));
        self.put_key(owner, Key::private(INPUT), JsValue::String(input.clone()))?;
        self.put_key(owner, Key::private(CAPTURES), JsValue::Object(positions))?;
        Ok(())
    }
}

/** The getter's immutable definition names its field; public function-name writes do not route it. */
fn field(call: &Call) -> &'static str {
    realm()
        .native(call.callee)
        .map(|native| native.name.get("get ".len()..).unwrap_or(""))
        .unwrap_or("")
}

/** Read a private data record without invoking script accessors on a forged or changed value. */
fn data(vm: &Vm, owner: ObjectId, name: &str) -> Option<JsValue> {
    match &vm.heap.get(owner)?.props.get(&Key::private(name))?.slot {
        Slot::Data(value) => Some(value.clone()),
        _ => None,
    }
}

/** The unmodified subject is shared until a later successful match replaces it. */
fn input(vm: &Vm, owner: ObjectId) -> JsString {
    match data(vm, owner, INPUT) {
        Some(JsValue::String(value)) => value,
        _ => JsString::empty(),
    }
}

/** Captured positions are inert numeric array entries produced by the RegExp engine. */
fn positions<'a>(vm: &'a Vm<'_>, owner: ObjectId) -> &'a [JsValue] {
    let Some(JsValue::Object(id)) = data(vm, owner, CAPTURES) else {
        return &[];
    };
    match vm.heap.get(id).map(|object| &object.class) {
        Some(Class::Array(values)) => values.dense_slice().unwrap_or(&[]),
        _ => &[],
    }
}

/** Match contexts and every numbered group are slices of the original UTF-16 subject. */
fn get_stat(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let owner = realm().ids.regexp_prototype;
    let name = field(&call);
    if matches!(name, "input" | "$_") {
        return Ok(
            data(vm, owner, INPUT_OVERRIDE).unwrap_or_else(|| JsValue::String(input(vm, owner)))
        );
    }
    let text = input(vm, owner);
    let spans = positions(vm, owner);
    let whole = span(spans, 0).unwrap_or((0, 0));
    let range = match name {
        "leftContext" | "$`" => Some((0, whole.0)),
        "rightContext" | "$'" if !spans.is_empty() => Some((whole.1, text.len())),
        "rightContext" | "$'" => None,
        "lastMatch" | "$&" => Some(whole),
        "lastParen" | "$+" => (spans.len() > PAIR_WIDTH)
            .then(|| span(spans, spans.len() / PAIR_WIDTH - 1))
            .flatten(),
        _ => name
            .strip_prefix('$')
            .and_then(|index| index.parse::<usize>().ok())
            .and_then(|index| span(spans, index)),
    };
    let units = range
        .and_then(|(start, end)| text.as_units().get(start..end))
        .unwrap_or(&[]);
    Ok(JsValue::String(JsString::from_slice(units)?))
}

/** Validate offsets when restoring a record, so private state never permits an unchecked slice. */
fn span(values: &[JsValue], index: usize) -> Option<(usize, usize)> {
    let pair = values
        .get(index.checked_mul(PAIR_WIDTH)?..)?
        .get(..PAIR_WIDTH)?;
    match (&pair[0], &pair[1]) {
        (JsValue::Number(start), JsValue::Number(end))
            if start.is_finite()
                && end.is_finite()
                && *start >= 0.0
                && *end >= *start
                && start.fract() == 0.0
                && end.fract() == 0.0 =>
        {
            Some((*start as usize, *end as usize))
        }
        _ => None,
    }
}

/** Only input has a meaningful setter; readonly compatibility setters leave match facts intact. */
fn set_stat(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let owner = realm().ids.regexp_prototype;
    if matches!(field(&call), "input" | "$_") {
        let value = vm.to_string(&call.arg(0))?;
        vm.materialize(owner)?;
        vm.put_key(owner, Key::private(INPUT_OVERRIDE), JsValue::String(value))?;
    }
    Ok(JsValue::Undefined)
}
