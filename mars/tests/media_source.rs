/**
 * Media Source playback end to end, headless: a page creates a MediaSource, adds a video and an audio SourceBuffer,
 * appends the H.264 and AAC fixtures it fetched through Relay, and plays. The native players are ticked from the
 * test thread in place of the window, and the page must reach HAVE_ENOUGH_DATA, see its time advance, and show a
 * decoded picture in the frame the painter would draw.
 */
#[allow(dead_code)]
mod support;
use serde_json::{Value, json};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use support::{Fixture, agent_call, html};

/// The players' hub is process-wide and every test's page is `page-1`, so the tests take turns.
static SERIAL: Mutex<()> = Mutex::new(());

/// The fixtures, made with ffmpeg (the commands are in the decoder's and the audio renderer's tests).
const VIDEO: &[u8] = include_bytes!("fixtures/tiny-h264.mp4");
const AUDIO: &[u8] = include_bytes!("fixtures/tiny-aac.mp4");
/// The video fixture's picture size.
const SIZE: (usize, usize) = (64, 48);
/// How long the whole flow may take before the test gives up.
const DEADLINE: Duration = Duration::from_secs(20);
/// The runtime's idle turn spacing.
const TURN: Duration = Duration::from_millis(30);
/// RGBA bytes per pixel of a frame.
const PIXEL: usize = 4;

/// The page: a muted video (so play needs no activation) fed from both fixtures, logging its times and state.
const PAGE: &str = r#"<video id="v" muted></video><script>
const v = document.getElementById('v'); window.times = []; window.state = 'start';
v.addEventListener('timeupdate', () => times.push(v.currentTime));
v.addEventListener('error', () => { state = 'error:' + (v.error && v.error.message); });
const ms = new MediaSource(); v.src = URL.createObjectURL(ms);
ms.addEventListener('sourceopen', async () => {
  try {
    const load = async (type, url) => {
      const bytes = await (await fetch(url)).arrayBuffer();
      const sb = ms.addSourceBuffer(type);
      await new Promise(done => { sb.addEventListener('updateend', done, { once: true }); sb.appendBuffer(bytes); });
    };
    await load('video/mp4; codecs="avc1.42C00A"', '/video.mp4');
    await load('audio/mp4; codecs="mp4a.40.2"', '/audio.mp4');
    state = 'appended';
    await v.play();
    state = 'playing';
  } catch (e) { state = 'failed:' + e; }
});
</script>"#;

/// Where the video fixture's second fragment (its second moof) starts, so it can be appended in two halves.
const SECOND_FRAGMENT: usize = 2663;
/// Where the first half of the fixture ends in media time: the first fragment holds five frames at ten per second.
const FIRST_HALF_END: f64 = 0.5;
/// How far a reported time may sit from the expected one.
const TIME_EPSILON: f64 = 0.05;

/// The page for the stall test: half the video fixture, play, the rest on `waiting`, then endOfStream.
fn halves_page() -> String {
    format!(
        r#"<video id="v" muted></video><script>
const v = document.getElementById('v'); window.events = []; window.state = 'start'; let rest = null;
for (const name of ['waiting', 'playing', 'ended']) v.addEventListener(name, () => {{
  events.push(name + '@' + v.readyState + '@' + v.currentTime.toFixed(2));
  if (name === 'waiting' && rest) {{ const run = rest; rest = null; run(); }}
}});
const ms = new MediaSource(); v.src = URL.createObjectURL(ms);
ms.addEventListener('sourceopen', async () => {{
  try {{
    const bytes = await (await fetch('/video.mp4')).arrayBuffer();
    const sb = ms.addSourceBuffer('video/mp4; codecs="avc1.42C00A"');
    const append = chunk => new Promise(done => {{ sb.addEventListener('updateend', done, {{ once: true }}); sb.appendBuffer(chunk); }});
    await append(bytes.slice(0, {SECOND_FRAGMENT}));
    rest = () => append(bytes.slice({SECOND_FRAGMENT})).then(() => {{ ms.endOfStream(); state = 'ended-stream'; }});
    state = 'half';
    await v.play();
    state = 'playing';
  }} catch (e) {{ state = 'failed:' + e; }}
}});
</script>"#
    )
}

/** A 200 response carrying one fixture. */
fn media(mime: &str, body: &[u8]) -> Vec<u8> {
    let mut response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    response.extend_from_slice(body);
    response
}

/** Evaluate an expression in the page without committing. */
fn peek(runtime: &mut mars::runtime::Runtime, expression: &str) -> Value {
    let request = json!({"request":{"kind":"evaluate","expression":expression,"options":{"return_by_value":true}}});
    agent_call(runtime, "page.devtools_peek", request).unwrap()["result"]["value"].clone()
}

/** Tick the players and run the coordinator's idle turns until `done` holds or the deadline passes. */
fn until(
    runtime: &mut mars::runtime::Runtime,
    mut done: impl FnMut(&mut mars::runtime::Runtime) -> bool,
) -> bool {
    let deadline = Instant::now() + DEADLINE;
    while Instant::now() < deadline {
        mars::desktop::media_testing::tick();
        runtime.run_page_scripts();
        if done(runtime) {
            return true;
        }
        std::thread::sleep(TURN);
    }
    false
}

/** The page's state and times, for a failure message. */
fn report(runtime: &mut mars::runtime::Runtime) -> Value {
    peek(
        runtime,
        "state + ' ' + document.getElementById('v').readyState + ' ' + JSON.stringify(times)",
    )
}

/** Both fixtures append, the element reaches HAVE_ENOUGH_DATA, its time advances while it plays, and a picture is decoded. */
#[cfg(target_os = "macos")]
#[test]
fn a_media_source_with_video_and_audio_plays_through_the_native_player() {
    let _turn = SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let fixture = Fixture::routes(vec![
        ("/", html("", PAGE)),
        ("/video.mp4", media("video/mp4", VIDEO)),
        ("/audio.mp4", media("audio/mp4", AUDIO)),
    ]);
    let mut runtime = support::desktop(&fixture.url);
    agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    let ready = until(&mut runtime, |runtime| {
        peek(runtime, "document.getElementById('v').readyState") == json!(4)
    });
    assert!(ready, "HAVE_ENOUGH_DATA: {}", report(&mut runtime));
    let advanced = until(&mut runtime, |runtime| {
        peek(
            runtime,
            "times.length >= 3 && times[times.length - 1] > times[0]",
        ) == json!(true)
    });
    assert!(advanced, "timeupdate advances: {}", report(&mut runtime));
    assert_eq!(peek(&mut runtime, "state"), json!("playing"));
    let (width, height, pixels) =
        mars::desktop::media_testing::frame("page-1").expect("a picture was decoded");
    assert_eq!((width, height), SIZE);
    let first = &pixels[..PIXEL];
    assert!(
        pixels.chunks(PIXEL).any(|pixel| pixel != first),
        "the picture is not one flat color"
    );
}

/**
 * Play runs into the end of what is buffered: the element fires `waiting` at HAVE_CURRENT_DATA with its time held at
 * the buffered end, resumes with `playing` once the rest arrives, and ends only at the duration endOfStream set.
 */
#[cfg(target_os = "macos")]
#[test]
fn playback_stalls_at_the_buffered_end_and_resumes_when_more_arrives() {
    let _turn = SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let page: &'static str = Box::leak(halves_page().into_boxed_str());
    let fixture = Fixture::routes(vec![
        ("/", html("", page)),
        ("/video.mp4", media("video/mp4", VIDEO)),
    ]);
    let mut runtime = support::desktop(&fixture.url);
    agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    let ended = until(&mut runtime, |runtime| {
        peek(runtime, "events.some(e => e.startsWith('ended@'))") == json!(true)
    });
    let events = peek(&mut runtime, "events.join(' ')");
    assert!(
        ended,
        "the stream ends: {} {}",
        report(&mut runtime),
        events
    );
    let events = events.as_str().unwrap().to_string();
    let names: Vec<&str> = events.split(' ').collect();
    let waiting = names
        .iter()
        .find(|event| event.starts_with("waiting@"))
        .unwrap_or_else(|| panic!("a waiting event: {events}"));
    let mut parts = waiting.split('@').skip(1);
    assert_eq!(parts.next(), Some("2"), "HAVE_CURRENT_DATA: {events}");
    let held: f64 = parts.next().unwrap().parse().unwrap();
    assert!((held - FIRST_HALF_END).abs() < TIME_EPSILON, "{events}");
    let stalled = names
        .iter()
        .position(|event| event.starts_with("waiting@"))
        .unwrap();
    let resumed = names
        .iter()
        .skip(stalled + 1)
        .any(|event| event.starts_with("playing@4"));
    assert!(resumed, "playing follows waiting: {events}");
    assert_eq!(
        peek(&mut runtime, "[v.ended, v.paused, state].join('|')"),
        json!("true|true|ended-stream")
    );
    let end: f64 = peek(&mut runtime, "v.currentTime").as_f64().unwrap();
    assert!(
        (end - 1.0).abs() < TIME_EPSILON,
        "ended at the duration: {end} {events}"
    );
}
