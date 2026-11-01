/** Media elements on the coordinator: byte-range fetching through Relay, the shared byte store, and the updates that turn player status into page events. */
#[allow(dead_code)]
mod support;
use mars::runtime::media::bytes::{Bytes, CHUNK, content_range};
use mars::runtime::media::hub::{self, Status};
use mars::runtime::media::updates::{Reported, diff};
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use support::{Fixture, agent_call, html};

/// The synthetic media resource the fixture serves, longer than one range request.
const LENGTH: usize = CHUNK as usize + 4096;

/** A body whose every byte encodes its offset, so a misplaced range shows. */
fn body() -> Vec<u8> {
    (0..LENGTH).map(|at| (at % 251) as u8).collect()
}

/** Answer `Range: bytes=a-b` with 206 and Content-Range, as a media server does. */
fn ranged(request: &str) -> Vec<u8> {
    let lower = request.to_ascii_lowercase();
    let path = request.split_whitespace().nth(1).unwrap_or_default();
    if path == "/" {
        return html(
            "",
            r#"<video id="v" preload="auto" src="/clip.mp4"></video>"#,
        );
    }
    let span = lower
        .lines()
        .find_map(|line| line.strip_prefix("range: bytes="))
        .and_then(|span| span.trim().split_once('-'))
        .map(|(start, end)| {
            (
                start.parse::<usize>().unwrap(),
                end.parse::<usize>().unwrap_or(LENGTH - 1),
            )
        });
    let Some((start, end)) = span else {
        return b"HTTP/1.1 200 OK\r\nContent-Type: video/mp4\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec();
    };
    let end = end.min(LENGTH - 1);
    let mut response = format!(
        "HTTP/1.1 206 Partial Content\r\nContent-Type: video/mp4\r\nContent-Range: bytes {start}-{end}/{LENGTH}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        end - start + 1
    )
    .into_bytes();
    response.extend_from_slice(&body()[start..=end]);
    response
}

/** Evaluate an expression in the page without committing. */
fn peek(runtime: &mut mars::runtime::Runtime, expression: &str) -> Value {
    let request = json!({"request":{"kind":"evaluate","expression":expression,"options":{"return_by_value":true}}});
    agent_call(runtime, "page.devtools_peek", request).unwrap()["result"]["value"].clone()
}

/** Run the coordinator's idle turns until `done` holds or a few seconds pass. */
fn until(
    runtime: &mut mars::runtime::Runtime,
    mut done: impl FnMut(&mut mars::runtime::Runtime) -> bool,
) -> bool {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        runtime.run_page_scripts();
        if done(runtime) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(30));
    }
    false
}

/**
 * The whole coordinator path in one flow (the hub is process-wide): a page's video loads, the
 * coordinator fetches the range the native loader asks for with a Range request through Relay,
 * and the status the native player reports becomes metadata and ready-state events in the page.
 */
#[test]
fn a_video_fetches_ranges_through_relay_and_reports_metadata_to_the_page() {
    let fixture = Fixture::start(Box::new(ranged), false);
    let mut runtime = support::desktop(&fixture.url);
    agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    let key = ("page-1".to_string(), 1);
    assert!(
        until(&mut runtime, |_| hub::lock().players.contains_key(&key)),
        "the load reaches the hub"
    );
    let url = hub::lock().players[&key].url.clone();
    assert!(url.ends_with("/clip.mp4"), "{url}");
    hub::lock().players.get_mut(&key).unwrap().need = Some(0);
    assert!(until(&mut runtime, |_| hub::lock().players[&key]
        .bytes
        .length
        .is_some()));
    {
        let hub = hub::lock();
        let bytes = &hub.players[&key].bytes;
        assert_eq!(bytes.length, Some(LENGTH as u64));
        assert_eq!(bytes.read(0, 8), &body()[..8]);
        assert_eq!(bytes.mime, "video/mp4");
    }
    let request = fixture.last_request.lock().unwrap().to_ascii_lowercase();
    assert!(
        request.contains(&format!("range: bytes=0-{}", CHUNK - 1)),
        "{request}"
    );
    assert!(
        request.contains("accept-encoding: identity;q=1, *;q=0"),
        "{request}"
    );
    let status = Status {
        ready: true,
        duration: 12.5,
        width: 640,
        height: 360,
        video: true,
        likely: true,
        ..Status::default()
    };
    hub::lock().players.get_mut(&key).unwrap().status = status;
    let ready = until(&mut runtime, |runtime| {
        peek(runtime, "document.getElementById('v').readyState") == json!(4)
    });
    assert!(ready, "metadata reaches the page");
    let facts = peek(
        &mut runtime,
        "(() => { const v = document.getElementById('v'); return [v.duration, v.videoWidth, v.videoHeight, v.networkState].join('|'); })()",
    );
    assert_eq!(facts, json!("12.5|640|360|2"));
    agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    assert!(
        until(&mut runtime, |_| !hub::lock().players.contains_key(&key)),
        "a page that is gone releases its players"
    );
}

/** Planned ranges start at the reader, stop at held chunks and at the end, and skip held bytes. */
#[test]
fn byte_store_plans_missing_ranges_and_reads_contiguous_bytes() {
    let mut bytes = Bytes::default();
    assert_eq!(bytes.plan(0), Some((0, CHUNK - 1)));
    bytes.length = Some(CHUNK * 3);
    bytes.insert(CHUNK, vec![7; 100], 0);
    assert_eq!(bytes.plan(0), Some((0, CHUNK - 1)));
    assert_eq!(bytes.plan(CHUNK), None);
    assert_eq!(bytes.plan(CHUNK + 100), Some((CHUNK + 100, 2 * CHUNK + 99)));
    assert_eq!(bytes.read(CHUNK + 90, 50), &[7; 10]);
    assert!(bytes.has(CHUNK * 3), "past the end counts as held");
}

/** Overlapping inserts keep one copy of each byte; a whole Blob reads end to end. */
#[test]
fn byte_store_trims_overlap_and_holds_whole_blobs() {
    let mut bytes = Bytes::default();
    bytes.insert(0, vec![1; 10], 0);
    bytes.insert(5, vec![2; 10], 0);
    assert_eq!(bytes.read(0, 20), &[1; 10]);
    assert_eq!(bytes.read(10, 20), &[2; 5]);
    assert_eq!(bytes.spans(), vec![(0, 15)]);
    let blob = Bytes::whole(vec![9; 32], "audio/mpeg");
    assert_eq!(
        (blob.length, blob.mime.as_str(), blob.read(0, 64).len()),
        (Some(32), "audio/mpeg", 32)
    );
}

/** Content-Range headers give the first byte and the total, which may be unknown. */
#[test]
fn content_range_parses_start_and_total() {
    assert_eq!(content_range("bytes 0-99/1000"), Some((0, Some(1000))));
    assert_eq!(content_range("bytes 500-999/*"), Some((500, None)));
    assert_eq!(content_range("items 0-1/2"), None);
}

/** A ready player first sends metadata and progress; while playing, time every quarter second, then the end. */
#[test]
fn status_changes_become_updates_in_order() {
    let mut sent = Reported::default();
    let start = Instant::now();
    let mut status = Status {
        ready: true,
        duration: 4.0,
        width: 2,
        height: 2,
        video: true,
        buffered: vec![[0.0, 4.0]],
        ..Status::default()
    };
    let kinds = |updates: Vec<Value>| {
        updates
            .iter()
            .map(|u| u["kind"].as_str().unwrap().to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        kinds(diff(1, &status, &mut sent, start)),
        ["metadata", "progress"]
    );
    status.playing = true;
    status.time = 1.0;
    assert_eq!(kinds(diff(1, &status, &mut sent, start)), ["time"]);
    assert!(diff(1, &status, &mut sent, start + Duration::from_millis(100)).is_empty());
    assert_eq!(
        kinds(diff(
            1,
            &status,
            &mut sent,
            start + Duration::from_millis(300)
        )),
        ["time"]
    );
    status.playing = false;
    status.ended = true;
    status.time = 4.0;
    assert_eq!(
        kinds(diff(
            1,
            &status,
            &mut sent,
            start + Duration::from_millis(400)
        )),
        ["time", "ended"]
    );
    let failed = Status {
        error: Some((4, "MEDIA_ELEMENT_ERROR: Format error".into())),
        ..Status::default()
    };
    let updates = diff(2, &failed, &mut Reported::default(), start);
    assert_eq!((updates.len(), updates[0]["code"].as_u64()), (1, Some(4)));
}

/** The host answers canPlayType from AVFoundation: MP4 with H.264 and AAC plays on macOS. */
#[cfg(target_os = "macos")]
#[test]
fn decoder_support_includes_h264_and_aac_in_mp4() {
    let support = mars::runtime::media::support();
    assert!(
        support.containers.iter().any(|c| c == "video/mp4"),
        "{support:?}"
    );
    assert!(
        support.codecs.iter().any(|c| c == "video/mp4|avc1"),
        "{support:?}"
    );
    assert!(
        support.codecs.iter().any(|c| c == "video/mp4|mp4a"),
        "{support:?}"
    );
}
