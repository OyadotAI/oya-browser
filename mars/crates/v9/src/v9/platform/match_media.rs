use super::Target;
/**
 * MediaQueryList objects, per CSSOM View. `matches` is read from the viewport at each read, and a
 * `change` listener is told when a viewport change flips the query: the "evaluate media queries
 * and report changes" step runs before the next task. A list that answered once at creation
 * would leave a page that asked at load (iron-media-query) unaware that its window was resized.
 */
use super::encoding::{read_slot, write_slot};
use super::events::fire_with_fields;
use super::window::media_matches;
use crate::v9::value::{JsValue, ObjectId};
use crate::v9::vm::data::Class;
use crate::v9::vm::heap::Object;
use crate::v9::vm::realm::realm;
use crate::v9::vm::{Call, Res, Vm};

/** The hidden slot that keeps a list's query text out of script's reach. */
const QUERY_SLOT: &str = "matchMedia.query";
/** The most lists that may hear changes at once. Past it a list still answers `matches` live, but its
listeners are not reported: a ceiling on the page's retained lists, not a spec behaviour. */
pub(in crate::v9) const LISTENED_LIMIT: usize = 256;

/** A new MediaQueryList for `query`, which keeps no result of its own: every read asks the viewport. */
pub(in crate::v9) fn create(vm: &mut Vm, query: &str) -> Res<ObjectId> {
    let id = vm.alloc(Object::new(
        Class::Plain,
        Some(realm().ids.media_query_list_prototype),
    ))?;
    write_slot(vm, id, QUERY_SLOT, JsValue::string(query)?);
    Ok(id)
}

/** The `matches` getter: the query against the viewport as it is now. */
pub(in crate::v9) fn matches(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let query = query_of(vm, &call.this)?;
    Ok(JsValue::Boolean(media_matches(vm, &query)))
}

/** The `media` getter: the query text the list was created with. */
pub(in crate::v9) fn media(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let query = query_of(vm, &call.this)?;
    Ok(JsValue::string(&query)?)
}

/** The query text of a MediaQueryList receiver; any other receiver is an illegal invocation. */
fn query_of(vm: &mut Vm, this: &JsValue) -> Res<String> {
    match read_slot(vm, this, QUERY_SLOT) {
        Some(query) => vm.to_text(&query),
        None => Err(vm.type_error("Illegal invocation")),
    }
}

/**
 * Registers a list for reports when a `change` listener is added to it. The list is remembered with
 * the result it answers now, so the first report after a resize compares against what the listener
 * was built for. Other targets and event types are not lists and pass straight through.
 */
pub(in crate::v9) fn watch(vm: &mut Vm, target: Target, kind: &str) -> Res<()> {
    let Target::Object(id) = target else {
        return Ok(());
    };
    if kind != "change" || vm.page.state.media_lists.contains_key(&id) {
        return Ok(());
    }
    let Some(query) = read_slot(vm, &JsValue::Object(id), QUERY_SLOT) else {
        return Ok(());
    };
    if vm.page.state.media_lists.len() >= LISTENED_LIMIT {
        return Ok(());
    }
    let query = vm.to_text(&query)?;
    let matches = media_matches(vm, &query);
    vm.page.state.media_lists.insert(id, matches);
    Ok(())
}

/** Evaluates every listened-to list against the viewport and reports the ones whose result flipped. */
pub(in crate::v9) fn report(vm: &mut Vm) -> Res<()> {
    let ids: Vec<ObjectId> = vm.page.state.media_lists.keys().copied().collect();
    for id in ids {
        report_one(vm, id)?;
    }
    Ok(())
}

/** One list: when its result differs from the one its listeners last heard, record it and fire `change`. */
fn report_one(vm: &mut Vm, id: ObjectId) -> Res<()> {
    let Some(query) = read_slot(vm, &JsValue::Object(id), QUERY_SLOT) else {
        return Ok(());
    };
    let query = vm.to_text(&query)?;
    let now = media_matches(vm, &query);
    if vm.page.state.media_lists.get(&id) == Some(&now) {
        return Ok(());
    }
    vm.page.state.media_lists.insert(id, now);
    let fields = [
        ("matches", JsValue::Boolean(now)),
        ("media", JsValue::string(&query)?),
    ];
    fire_with_fields(vm, Target::Object(id), "change", &fields)?;
    Ok(())
}
