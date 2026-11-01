/** The CacheStorage and Cache operations a page's `caches` object sends. */
use super::super::{Ctx, Handler, Reply, fail, ok};
use super::{CachedRequest, CachedResponse, Entry, Query, answer, key};
use crate::protocol::EngineResult;
use crate::runtime::supervisor::Runtime;
use serde_json::{Value, json};

/** Cache Storage operations. */
pub(in crate::runtime::platform) const HANDLERS: &[(&str, Handler)] = &[
    ("caches.open", open),
    ("caches.has", has),
    ("caches.delete", delete_cache),
    ("caches.keys", keys),
    ("caches.match", storage_match),
    ("cache.match", cache_match),
    ("cache.put", put),
    ("cache.delete", delete_entries),
    ("cache.keys", cache_keys),
];

/** The request a page described, or None when absent. */
fn request(args: &Value) -> Option<CachedRequest> {
    serde_json::from_value(args["request"].clone()).ok()
}

/** caches.open(name). */
fn open(runtime: &mut Runtime, ctx: &Ctx, args: &Value, _: &[u8]) -> EngineResult<Reply> {
    let name = args["name"].as_str().unwrap_or_default();
    let mut next = runtime.platform.caches.clone();
    let id = next.open(&key(ctx), name);
    match runtime.commit_caches(next)? {
        Reply::Answer(body, _) if body.get("error").is_some() => {
            Ok(Reply::Answer(body, Vec::new()))
        }
        _ => ok(json!(id)),
    }
}

/** caches.has(name). */
fn has(runtime: &mut Runtime, ctx: &Ctx, args: &Value, _: &[u8]) -> EngineResult<Reply> {
    let name = args["name"].as_str().unwrap_or_default();
    ok(json!(
        runtime
            .platform
            .caches
            .caches(&key(ctx))
            .iter()
            .any(|cache| cache.name == name)
    ))
}

/** caches.delete(name). */
fn delete_cache(runtime: &mut Runtime, ctx: &Ctx, args: &Value, _: &[u8]) -> EngineResult<Reply> {
    let name = args["name"].as_str().unwrap_or_default();
    let mut next = runtime.platform.caches.clone();
    let caches = next.caches(&key(ctx));
    let before = caches.len();
    caches.retain(|cache| cache.name != name);
    if caches.len() == before {
        return ok(json!(false));
    }
    runtime.commit_caches(next)?;
    ok(json!(true))
}

/** caches.keys(): names in creation order. */
fn keys(runtime: &mut Runtime, ctx: &Ctx, _: &Value, _: &[u8]) -> EngineResult<Reply> {
    let names: Vec<String> = runtime
        .platform
        .caches
        .caches(&key(ctx))
        .iter()
        .map(|cache| cache.name.clone())
        .collect();
    ok(json!(names))
}

/** caches.match(request, {cacheName}): the first match across caches in creation order. */
fn storage_match(runtime: &mut Runtime, ctx: &Ctx, args: &Value, _: &[u8]) -> EngineResult<Reply> {
    let query = request(args);
    let options = Query::from(&args["options"]);
    let name = args["name"].as_str();
    let caches = runtime.platform.caches.caches(&key(ctx));
    let found = caches
        .iter()
        .filter(|cache| name.is_none_or(|name| cache.name == name))
        .find_map(|cache| cache.lookup(query.as_ref(), options).into_iter().next());
    answer(&found.into_iter().collect::<Vec<_>>())
}

/** cache.match / matchAll: every match in this cache. */
fn cache_match(runtime: &mut Runtime, ctx: &Ctx, args: &Value, _: &[u8]) -> EngineResult<Reply> {
    let query = request(args);
    let options = Query::from(&args["options"]);
    let id = args["cache"].as_u64().unwrap_or_default();
    match runtime.platform.caches.cache(&key(ctx), id) {
        Some(cache) => answer(&cache.lookup(query.as_ref(), options)),
        None => ok(json!([])),
    }
}

/** The entries a put batch describes, with bodies sliced from the concatenated bytes. */
fn entries(args: &Value, bytes: &[u8]) -> Option<Vec<Entry>> {
    let mut offset = 0usize;
    let mut out = Vec::new();
    for item in args["entries"].as_array()? {
        let request: CachedRequest = serde_json::from_value(item["request"].clone()).ok()?;
        let mut response: CachedResponse = serde_json::from_value(item["response"].clone()).ok()?;
        let length = usize::try_from(item["response"]["length"].as_u64()?).ok()?;
        let end = offset
            .checked_add(length)
            .filter(|end| *end <= bytes.len())?;
        response.body = crate::runtime::base64(&bytes[offset..end]);
        offset = end;
        out.push(Entry { request, response });
    }
    (offset == bytes.len()).then_some(out)
}

/** cache.put / add / addAll: one batch, all or nothing. */
fn put(runtime: &mut Runtime, ctx: &Ctx, args: &Value, bytes: &[u8]) -> EngineResult<Reply> {
    let Some(batch) = entries(args, bytes) else {
        return fail("TypeError", "Invalid cache entries.");
    };
    let id = args["cache"].as_u64().unwrap_or_default();
    let mut next = runtime.platform.caches.clone();
    let Some(cache) = next.cache(&key(ctx), id) else {
        return fail("InvalidStateError", "The cache was deleted.");
    };
    for entry in batch {
        cache.put(entry);
    }
    runtime.commit_caches(next)
}

/** cache.delete(request, options). */
fn delete_entries(runtime: &mut Runtime, ctx: &Ctx, args: &Value, _: &[u8]) -> EngineResult<Reply> {
    let Some(query) = request(args) else {
        return fail("TypeError", "A request is required.");
    };
    let options = Query::from(&args["options"]);
    let id = args["cache"].as_u64().unwrap_or_default();
    let mut next = runtime.platform.caches.clone();
    let deleted = next
        .cache(&key(ctx), id)
        .is_some_and(|cache| cache.delete(&query, options));
    if deleted {
        runtime.commit_caches(next)?;
    }
    ok(json!(deleted))
}

/** cache.keys(request?, options): the matching requests in order. */
fn cache_keys(runtime: &mut Runtime, ctx: &Ctx, args: &Value, _: &[u8]) -> EngineResult<Reply> {
    let query = request(args);
    let options = Query::from(&args["options"]);
    let id = args["cache"].as_u64().unwrap_or_default();
    let list: Vec<Value> = runtime
        .platform
        .caches
        .cache(&key(ctx), id)
        .map(|cache| {
            cache
                .lookup(query.as_ref(), options)
                .iter()
                .map(|entry| json!(entry.request))
                .collect()
        })
        .unwrap_or_default();
    ok(Value::Array(list))
}
