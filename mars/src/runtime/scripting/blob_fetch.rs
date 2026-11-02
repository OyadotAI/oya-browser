/** Context-captured Blob responses never acquire a network capability or consult another registry. */
use super::{FAILED_TO_FETCH, HostRequest, HostResponse, VM_RESPONSE_BYTES, failed};
use crate::runtime::{csp, worker_loading::Policy};
use url::Url;

#[cfg(test)]
mod tests;

/** Every Blob URL enters this branch, including missing snapshots and unsupported request kinds. */
pub(super) fn is_blob(address: &str) -> bool {
    Url::parse(address).is_ok_and(|url| url.scheme() == "blob")
}

/** Admission rechecks the captured creator origin, policy and byte limit without resolving URL text. */
pub(super) fn response(request: &HostRequest, policy: &Policy) -> HostResponse {
    accepted(request, policy).unwrap_or_else(|| failed(request.id, FAILED_TO_FETCH))
}

/** Only GET can consume this immutable snapshot; Range is one bounded, satisfiable byte interval. */
fn accepted(request: &HostRequest, policy: &Policy) -> Option<HostResponse> {
    let source = request.blob_source.as_ref()?;
    let mut url = Url::parse(&request.url).ok()?;
    if request.kind != "fetch"
        || request.method != "GET"
        || request.body.is_some()
        || !request.body_bytes.is_empty()
        || url.scheme() != "blob"
        || source.validate(&request.url).is_err()
        || source.origin != policy.base.origin().ascii_serialization()
        || source.bytes.len() > VM_RESPONSE_BYTES
        || !csp::allows_connect(&policy.csp, &policy.base, &url)
    {
        return None;
    }
    let range = range_header(request)?;
    let (start, end) = match range {
        Some(value) => interval(value, source.bytes.len())?,
        None => (0, source.bytes.len()),
    };
    let mut headers = vec![
        ("content-length".into(), (end - start).to_string()),
        ("content-type".into(), source.mime_type.clone()),
    ];
    if range.is_some() {
        headers.push((
            "content-range".into(),
            format!("bytes {start}-{}/{}", end - 1, source.bytes.len()),
        ));
    }
    url.set_fragment(None);
    Some(HostResponse {
        id: request.id,
        status: if range.is_some() { 206 } else { 200 },
        status_text: if range.is_some() {
            "Partial Content"
        } else {
            "OK"
        }
        .into(),
        url: url.into(),
        headers,
        body_bytes: source.bytes[start..end].to_vec(),
        ..HostResponse::default()
    })
}

/** Repeated fields are a multi-range request and fail instead of silently choosing one value. */
fn range_header(request: &HostRequest) -> Option<Option<&str>> {
    let mut values = request
        .headers
        .iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case("range"));
    let first = values.next().map(|(_, value)| value.as_str());
    values.next().is_none().then_some(first)
}

/** Return an exclusive end only for a single HTTP byte range that overlaps the actual body. */
fn interval(value: &str, length: usize) -> Option<(usize, usize)> {
    let (start, end) = value.strip_prefix("bytes=")?.split_once('-')?;
    if length == 0 {
        return None;
    }
    if start.is_empty() {
        let suffix = decimal(end)?;
        return (suffix > 0).then_some((length.saturating_sub(suffix), length));
    }
    let start = decimal(start)?;
    if start >= length {
        return None;
    }
    let last = if end.is_empty() {
        length - 1
    } else {
        decimal(end)?
    };
    (last >= start).then_some((start, last.min(length - 1) + 1))
}

/** Signs, separators, non-ASCII digits and integer overflow never become a different interval. */
fn decimal(value: &str) -> Option<usize> {
    (!value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| value.parse().ok())
        .flatten()
}
