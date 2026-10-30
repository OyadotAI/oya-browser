/**
 * URL, URLSearchParams and the location accessors over the URL Standard parser in
 * `url_parser.rs`, as Blink's DOMURL and URLSearchParams bind them: a URL owns one
 * URLSearchParams (the same object every `searchParams` read), and each side updates the
 * other, so `params.append()` rewrites `url.search` and `url.search = ...` refills the params.
 */
use super::dom::native_name;
use super::encoding::{read_slot, write_slot};
use super::url_parser::{self as parser, EncodeSet, State, UrlRecord};
use crate::v9::value::{JsValue, ObjectId};
use crate::v9::vm::data::{Class, HostData};
use crate::v9::vm::heap::{Key, Object};
use crate::v9::vm::realm::{Builder, realm, symbol};
use crate::v9::vm::{Call, Res, Vm};
use std::sync::OnceLock;

/** The URL object's private slot holding its URLSearchParams. */
const QUERY_SLOT: &str = "urlQueryObject";
/** The URLSearchParams object's private slot holding its URL. */
const URL_SLOT: &str = "paramsUrlObject";

/** The URLSearchParams Iterator prototype. */
static ITERATOR_PROTO: OnceLock<u32> = OnceLock::new();

/** Install the URLSearchParams Iterator prototype, under %IteratorPrototype%. */
pub(super) fn install_iterator(b: &mut Builder) {
    let proto = b.object(Some(b.ids.iterator_prototype));
    b.host_method(proto, "next", 0, params_next);
    let _ = ITERATOR_PROTO.set(proto);
}

/** Parse `input` against an optional base string, as the constructor and statics do. */
fn parse_with_base(vm: &mut Vm, input: &JsValue, base: &JsValue) -> Res<Option<UrlRecord>> {
    let input = vm.to_text(input)?;
    if base == &JsValue::Undefined {
        return Ok(parser::parse(&input, None));
    }
    let base = vm.to_text(base)?;
    Ok(parser::parse(&base, None).and_then(|base| parser::parse(&input, Some(&base))))
}

/** A new URL object over a record. */
fn new_url(vm: &mut Vm, record: &UrlRecord) -> Res<JsValue> {
    let id = vm.alloc(Object::new(
        Class::Host(Box::new(HostData::Url(record.href()))),
        Some(realm().ids.url_prototype),
    ))?;
    Ok(JsValue::Object(id))
}

/** `new URL(url, base)`. */
pub(in crate::v9) fn url_construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(record) = parse_with_base(vm, &call.arg(0), &call.arg(1))? else {
        return Err(vm.type_error("Failed to construct 'URL': Invalid URL"));
    };
    new_url(vm, &record)
}

/** `URL.parse(url, base)` (null on failure) and `URL.canParse(url, base)`. */
pub(in crate::v9) fn url_static(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let parsed = parse_with_base(vm, &call.arg(0), &call.arg(1))?;
    match (native_name(&call), parsed) {
        ("canParse", parsed) => Ok(JsValue::Boolean(parsed.is_some())),
        (_, Some(record)) => new_url(vm, &record),
        (_, None) => Ok(JsValue::Null),
    }
}

/** The record behind a URL object or `location`, and the URL object's id. */
fn this_url(vm: &mut Vm, this: &JsValue) -> Res<(Option<ObjectId>, UrlRecord)> {
    if *this == JsValue::Object(realm().ids.location) {
        let href = vm.page.state.href.clone();
        let record = parser::parse(&href, None)
            .or_else(|| parser::parse("about:blank", None))
            .expect("about:blank parses");
        return Ok((None, record));
    }
    if let (Some(HostData::Url(href)), JsValue::Object(id)) = (super::host_data(vm, this), this) {
        let record = parser::parse(&href, None).unwrap_or_default();
        return Ok((Some(*id), record));
    }
    Err(vm.type_error("Illegal invocation"))
}

/** URL and location component getters. */
pub(in crate::v9) fn url_get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (owner, url) = this_url(vm, &call.this)?;
    let text = match native_name(&call) {
        "href" | "toString" | "toJSON" => url.href(),
        "protocol" => format!("{}:", url.scheme),
        "host" => url.host_port(),
        "hostname" => url.host.clone().unwrap_or_default(),
        "port" => url.port.map(|p| p.to_string()).unwrap_or_default(),
        "pathname" => url.pathname(),
        "search" => prefixed('?', &url.query),
        "hash" => prefixed('#', &url.fragment),
        "origin" => url.origin(),
        "username" => url.username.clone(),
        "password" => url.password.clone(),
        "searchParams" => return search_params(vm, owner, &url),
        _ => String::new(),
    };
    JsValue::string(&text).map_err(Into::into)
}

/** `?query` / `#fragment`, or "" when absent or empty. */
fn prefixed(mark: char, part: &Option<String>) -> String {
    match part.as_deref() {
        None | Some("") => String::new(),
        Some(text) => format!("{mark}{text}"),
    }
}

/** The URL's one URLSearchParams, made on first read. */
fn search_params(vm: &mut Vm, owner: Option<ObjectId>, url: &UrlRecord) -> Res<JsValue> {
    let Some(owner) = owner else {
        return Ok(JsValue::Undefined);
    };
    if let Some(existing) = read_slot(vm, &JsValue::Object(owner), QUERY_SLOT) {
        return Ok(existing);
    }
    let pairs = parser::form_parse(url.query.as_deref().unwrap_or(""));
    let params = new_params(vm, pairs)?;
    write_slot(vm, params, URL_SLOT, JsValue::Object(owner));
    write_slot(vm, owner, QUERY_SLOT, JsValue::Object(params));
    Ok(JsValue::Object(params))
}

/** A URLSearchParams object over pairs. */
fn new_params(vm: &mut Vm, pairs: Vec<(String, String)>) -> Res<ObjectId> {
    vm.alloc(Object::new(
        Class::Host(Box::new(HostData::SearchParams(pairs))),
        Some(realm().ids.search_params_prototype),
    ))
}

/** Store a URL object's record, refilling its URLSearchParams from the query. */
fn store_url(vm: &mut Vm, id: ObjectId, url: &UrlRecord) {
    if let Some(Class::Host(host)) = vm.heap.get_mut(id).map(|o| &mut o.class) {
        **host = HostData::Url(url.href());
    }
    if let Some(JsValue::Object(params)) = read_slot(vm, &JsValue::Object(id), QUERY_SLOT) {
        let pairs = parser::form_parse(url.query.as_deref().unwrap_or(""));
        set_pairs(vm, params, pairs);
    }
}

/** Replace a URLSearchParams object's list. */
fn set_pairs(vm: &mut Vm, id: ObjectId, pairs: Vec<(String, String)>) {
    if let Some(Class::Host(host)) = vm.heap.get_mut(id).map(|o| &mut o.class) {
        **host = HostData::SearchParams(pairs);
    }
}

/** Apply one setter to a record, as the URL Standard's setter steps do. */
fn apply_setter(url: &mut UrlRecord, name: &str, value: &str) {
    match name {
        "protocol" => {
            parser::run(&format!("{value}:"), None, url, Some(State::SchemeStart));
        }
        "username" | "password" => {
            if url.cannot_have_credentials() {
                return;
            }
            let encoded = parser::encode(value, EncodeSet::Userinfo);
            if name == "username" {
                url.username = encoded;
            } else {
                url.password = encoded;
            }
        }
        "host" | "hostname" if !url.opaque => {
            let state = if name == "host" {
                State::Host
            } else {
                State::Hostname
            };
            parser::run(value, None, url, Some(state));
        }
        "port" => {
            if url.cannot_have_credentials() {
                return;
            }
            if value.is_empty() {
                url.port = None;
            } else {
                parser::run(value, None, url, Some(State::Port));
            }
        }
        "pathname" if !url.opaque => {
            url.path.clear();
            parser::run(value, None, url, Some(State::PathStart));
        }
        "search" => set_tail(url, value, '?', State::Query),
        "hash" => set_tail(url, value, '#', State::Fragment),
        _ => {}
    }
}

/** The search and hash setters: "" clears the part, otherwise it is reparsed after its mark. */
fn set_tail(url: &mut UrlRecord, value: &str, mark: char, state: State) {
    let part = if mark == '?' {
        &mut url.query
    } else {
        &mut url.fragment
    };
    if value.is_empty() {
        *part = None;
        return;
    }
    *part = Some(String::new());
    let input = value.strip_prefix(mark).unwrap_or(value);
    parser::run(input, None, url, Some(state));
}

/** URL and location component setters; location changes become navigation requests. */
pub(in crate::v9) fn url_set(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (owner, mut url) = this_url(vm, &call.this)?;
    let value = vm.to_text(&call.arg(0))?;
    let name = native_name(&call);
    if name == "href" {
        let base = match owner {
            Some(_) => None,
            None => parser::parse(&vm.page.state.href, None),
        };
        let Some(parsed) = parser::parse(&value, base.as_ref()) else {
            return Err(vm.type_error(&format!(
                "Failed to set the 'href' property on 'URL': Invalid URL '{value}'"
            )));
        };
        url = parsed;
    } else {
        apply_setter(&mut url, name, &value);
    }
    match owner {
        Some(id) => store_url(vm, id, &url),
        None => super::window::navigate(vm, "assign", url.href(), name == "hash")?,
    }
    Ok(JsValue::Undefined)
}

/** The pairs a URLSearchParams init names: a sequence of pairs, a record, or a string. */
fn init_pairs(vm: &mut Vm, init: &JsValue) -> Res<Vec<(String, String)>> {
    let JsValue::Object(id) = init else {
        if init == &JsValue::Undefined {
            return Ok(Vec::new());
        }
        let text = vm.to_text(init)?;
        return Ok(parser::form_parse(text.strip_prefix('?').unwrap_or(&text)));
    };
    let iterator = vm.get(init, &Key::Sym(symbol::ITERATOR))?;
    if !iterator.nullish() {
        return sequence_pairs(vm, init);
    }
    let mut out = Vec::new();
    for key in vm.enumerable_keys(*id)? {
        let item = vm.get(init, &key)?;
        let name = crate::v9::vm::property::key_text(&key);
        out.push((name.to_string(), vm.to_text(&item)?));
    }
    Ok(out)
}

/** A sequence of two-item sequences. */
fn sequence_pairs(vm: &mut Vm, init: &JsValue) -> Res<Vec<(String, String)>> {
    let mut out = Vec::new();
    for pair in vm.iterate_to_vec(init)? {
        if !matches!(pair, JsValue::Object(_)) {
            return Err(vm.type_error("Failed to construct 'URLSearchParams': The provided value cannot be converted to a sequence."));
        }
        let items = vm.iterate_to_vec(&pair)?;
        if items.len() != 2 {
            return Err(vm.type_error("Failed to construct 'URLSearchParams': Sequence initializer must only contain pair elements"));
        }
        out.push((vm.to_text(&items[0])?, vm.to_text(&items[1])?));
    }
    Ok(out)
}

/** `new URLSearchParams(init)`. */
pub(in crate::v9) fn params_construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let pairs = init_pairs(vm, &call.arg(0))?;
    Ok(JsValue::Object(new_params(vm, pairs)?))
}

/** Compare names by UTF-16 code units, as `sort()` orders them. */
fn utf16_order(a: &str, b: &str) -> std::cmp::Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/** Whether a pair matches a name and, when one is given, a value. */
fn matches(pair: &(String, String), name: &str, value: Option<&str>) -> bool {
    pair.0 == name && value.is_none_or(|v| pair.1 == v)
}

/** URLSearchParams methods. */
pub(in crate::v9) fn params_method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (Some(HostData::SearchParams(_)), JsValue::Object(id)) =
        (super::host_data(vm, &call.this), call.this.clone())
    else {
        return Err(vm.type_error("Illegal invocation"));
    };
    let kind = native_name(&call);
    let name = match kind {
        "toString" | "size" | "sort" | "forEach" | "keys" | "values" | "entries" => String::new(),
        _ => vm.to_text(&call.arg(0))?,
    };
    let value = if matches!(kind, "set" | "append" | "delete" | "has")
        && call.args.len() > 1
        && call.arg(1) != JsValue::Undefined
    {
        Some(vm.to_text(&call.arg(1))?)
    } else {
        None
    };
    // Argument conversion may run author code that changes this live list.
    let Some(HostData::SearchParams(mut pairs)) = super::host_data(vm, &call.this) else {
        return Err(vm.type_error("Illegal invocation"));
    };
    match kind {
        "get" => {
            return match pairs.iter().find(|(key, _)| *key == name) {
                Some((_, value)) => JsValue::string(value).map_err(Into::into),
                None => Ok(JsValue::Null),
            };
        }
        "getAll" => {
            let values: Vec<JsValue> = pairs
                .iter()
                .filter(|(key, _)| *key == name)
                .filter_map(|(_, value)| JsValue::string(value).ok())
                .collect();
            return Ok(JsValue::Object(vm.new_array(values)?));
        }
        "has" => {
            let found = pairs.iter().any(|p| matches(p, &name, value.as_deref()));
            return Ok(JsValue::Boolean(found));
        }
        "toString" => return JsValue::string(&parser::form_serialize(&pairs)).map_err(Into::into),
        "size" => return Ok(JsValue::Number(pairs.len() as f64)),
        "forEach" => return for_each(vm, &call, id),
        "set" => {
            let value = value.unwrap_or_else(|| "undefined".into());
            match pairs.iter().position(|(key, _)| *key == name) {
                Some(position) => {
                    pairs[position].1 = value;
                    let mut seen = false;
                    pairs.retain(|(key, _)| *key != name || !std::mem::replace(&mut seen, true));
                }
                None => pairs.push((name, value)),
            }
        }
        "append" => pairs.push((name, value.unwrap_or_else(|| "undefined".into()))),
        "delete" => pairs.retain(|p| !matches(p, &name, value.as_deref())),
        "sort" => pairs.sort_by(|a, b| utf16_order(&a.0, &b.0)),
        kind => return params_iterator(vm, id, kind),
    }
    update(vm, id, pairs);
    Ok(JsValue::Undefined)
}

/** forEach over a live view: entries added during the walk are visited too. */
fn for_each(vm: &mut Vm, call: &Call, id: ObjectId) -> Res<JsValue> {
    if !vm.is_callable(&call.arg(0)) {
        return Err(vm.type_error("URLSearchParams.forEach requires a callable callback"));
    }
    let mut index = 0;
    loop {
        let Some(HostData::SearchParams(pairs)) = super::host_data(vm, &JsValue::Object(id)) else {
            return Ok(JsValue::Undefined);
        };
        let Some((key, value)) = pairs.get(index).cloned() else {
            return Ok(JsValue::Undefined);
        };
        let args = vec![
            JsValue::string(&value)?,
            JsValue::string(&key)?,
            call.this.clone(),
        ];
        vm.call(call.arg(0), call.arg(1), args)?;
        index += 1;
    }
}

/** Store the list and write it back to the owning URL's query ("" becomes no query). */
fn update(vm: &mut Vm, id: ObjectId, pairs: Vec<(String, String)>) {
    let query = parser::form_serialize(&pairs);
    set_pairs(vm, id, pairs);
    let Some(JsValue::Object(owner)) = read_slot(vm, &JsValue::Object(id), URL_SLOT) else {
        return;
    };
    let Some(HostData::Url(href)) = super::host_data(vm, &JsValue::Object(owner)) else {
        return;
    };
    let Some(mut url) = parser::parse(&href, None) else {
        return;
    };
    url.query = (!query.is_empty()).then_some(query);
    if let Some(Class::Host(host)) = vm.heap.get_mut(owner).map(|o| &mut o.class) {
        **host = HostData::Url(url.href());
    }
}

/** A live iterator over a URLSearchParams list: each `next()` reads the list as it is now,
so entries deleted or added during a for-of are seen as the standard's iterator sees them. */
fn params_iterator(vm: &mut Vm, target: ObjectId, kind: &str) -> Res<JsValue> {
    let proto = *ITERATOR_PROTO.get().expect("installed");
    let id = vm.alloc(Object::new(Class::Plain, Some(proto)))?;
    write_slot(vm, id, "iterTarget", JsValue::Object(target));
    write_slot(vm, id, "iterIndex", JsValue::Number(0.0));
    write_slot(vm, id, "iterKind", JsValue::string(kind)?);
    write_slot(vm, id, "iterDone", JsValue::Boolean(false));
    Ok(JsValue::Object(id))
}

/** The iterator's `next()`. */
fn params_next(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (Some(target), Some(JsValue::Number(index)), Some(kind)) = (
        read_slot(vm, &call.this, "iterTarget"),
        read_slot(vm, &call.this, "iterIndex"),
        read_slot(vm, &call.this, "iterKind"),
    ) else {
        return Err(vm.type_error("Illegal invocation"));
    };
    if read_slot(vm, &call.this, "iterDone").is_some_and(|value| value.truthy()) {
        return vm.iter_result(JsValue::Undefined, true);
    }
    let Some(HostData::SearchParams(pairs)) = super::host_data(vm, &target) else {
        return Err(vm.type_error("Illegal invocation"));
    };
    let Some((name, value)) = pairs.get(index as usize) else {
        let id = call.this.object().expect("slots imply an object");
        write_slot(vm, id, "iterDone", JsValue::Boolean(true));
        return vm.iter_result(JsValue::Undefined, true);
    };
    let id = call.this.object().expect("slots imply an object");
    write_slot(vm, id, "iterIndex", JsValue::Number(index + 1.0));
    let item = match vm.to_text(&kind)?.as_str() {
        "keys" => JsValue::string(name)?,
        "values" => JsValue::string(value)?,
        _ => {
            let pair = vec![JsValue::string(name)?, JsValue::string(value)?];
            JsValue::Object(vm.new_array(pair)?)
        }
    };
    vm.iter_result(item, false)
}
