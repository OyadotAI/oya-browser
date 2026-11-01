/** Media elements through page scripts: the load algorithm, host updates and the events they fire, text tracks and Media Source. */
use crate::common;
use mars_v9::{
    DomLevel, Execution, MediaCommand, MediaSupport, PageEnvironment, PageEvent, VmState,
};

/** A host that plays MP4 (H.264 and AAC) and MP3, from files and from Media Source. */
fn environment() -> PageEnvironment {
    PageEnvironment {
        dom_level: DomLevel::Extended,
        media: MediaSupport {
            containers: vec!["video/mp4".into(), "audio/mp4".into(), "audio/mpeg".into()],
            codecs: vec![
                "video/mp4|avc1".into(),
                "video/mp4|mp4a".into(),
                "audio/mp4|mp4a".into(),
                "audio/mpeg|mp3".into(),
            ],
            streams: vec!["video/mp4".into(), "audio/mp4".into()],
        },
        ..common::environment()
    }
}

/** One page under test: its sources and the last committed execution. */
struct Page {
    sources: Vec<String>,
    environment: PageEnvironment,
    last: Execution,
    clock: f64,
    /** Every media command the page sent, in order. */
    sent: Vec<MediaCommand>,
}

impl Page {
    /** Load a script against the common page, then run the tasks it queued. */
    fn load(source: &str) -> Self {
        let environment = environment();
        let sources = vec![source.to_string()];
        let host = common::host();
        let last =
            mars_v9::load_page(common::input(&sources, &host, &environment)).expect("page loads");
        assert!(last.failed.is_empty(), "{:?}", common::failure(&last));
        let sent = last.effects.media.clone();
        let mut page = Self {
            sources,
            environment,
            last,
            clock: 0.0,
            sent,
        };
        page.timers();
        page
    }
    /** The committed state, round-tripped as the app stores it. */
    fn state(&self) -> VmState {
        common::round_trip(self.last.state.clone())
    }
    /** Record a step's outcome. */
    fn commit(&mut self, execution: Execution) {
        assert!(
            execution.failed.is_empty(),
            "{:?}",
            common::failure(&execution)
        );
        self.sent.extend(execution.effects.media.iter().cloned());
        self.last = execution;
    }
    /** Run due timers (the media task queue) until none are due. */
    fn timers(&mut self) {
        for _ in 0..16 {
            let Some(due) = self.last.effects.next_timer_ms else {
                return;
            };
            if due > self.clock + 50.0 {
                return;
            }
            self.clock = self.clock.max(due);
            let host = common::host();
            let input = common::input(&self.sources, &host, &self.environment);
            let next =
                mars_v9::run_timers(input, self.state(), self.clock, 64).expect("timers run");
            self.commit(next);
        }
    }
    /** Deliver host updates, then run the tasks they queued. */
    fn update(&mut self, updates: &str) {
        let host = common::host();
        let input = common::input(&self.sources, &host, &self.environment);
        let event = PageEvent::Media {
            updates: updates.into(),
        };
        let next = mars_v9::page_event(input, self.state(), event).expect("update delivers");
        self.commit(next);
        self.timers();
    }
    /** A top-level binding the page keeps, as text. */
    fn global(&self, name: &str) -> String {
        common::global(&self.last, name)
    }
}

/** canPlayType answers from the host's decoder list: probably with codecs, maybe without, empty otherwise. */
#[test]
fn can_play_type_answers_from_host_support() {
    let page = Page::load(
        "const v = document.createElement('video');\
         const r = [v.canPlayType('video/mp4; codecs=\"avc1.42E01E, mp4a.40.2\"'), v.canPlayType('video/mp4'),\
           v.canPlayType('video/webm; codecs=\"vp9\"'), v.canPlayType('video/mp4; codecs=\"hvc1\"'),\
           MediaSource.isTypeSupported('video/mp4; codecs=\"avc1.4d401f\"'), MediaSource.isTypeSupported('video/webm')].join('|');",
    );
    assert_eq!(page.global("r"), "probably|maybe|||true|false");
}

/** Setting src runs the load algorithm at a stable state: one load command and loadstart. */
#[test]
fn src_starts_a_load_and_fires_loadstart() {
    let page = Page::load(
        "let log = []; let r = '';const v = document.createElement('video');\
         for (const t of ['loadstart','emptied','abort']) v.addEventListener(t, () => { log.push(t); r = [log.join(','), v.networkState, v.currentSrc, v.readyState].join('|'); });\
         document.getElementById('main').appendChild(v); v.src = 'clip.mp4';",
    );
    assert_eq!(
        page.global("r"),
        "loadstart|2|https://example.com/app/clip.mp4|0"
    );
    let loads: Vec<_> = page
        .sent
        .iter()
        .filter(|c| matches!(c, MediaCommand::Load { .. }))
        .collect();
    assert_eq!(loads.len(), 1, "{:?}", page.sent);
    let MediaCommand::Load { url, settings, .. } = loads[0] else {
        unreachable!()
    };
    assert_eq!(url, "https://example.com/app/clip.mp4");
    assert!(settings.paused && settings.video);
}

/** The page script that records events and a summary of the element after each. */
const RECORDER: &str = "let log = []; let r = '';\
    const v = document.createElement('video'); document.getElementById('main').appendChild(v);\
    const summary = () => [log.join(','), v.currentTime, v.duration, v.paused, v.ended, v.readyState, v.videoWidth].join('|');\
    for (const t of ['loadstart','durationchange','resize','loadedmetadata','loadeddata','canplay','canplaythrough','play','playing','pause','waiting','seeking','seeked','timeupdate','ended','volumechange','ratechange','error','emptied','abort']) v.addEventListener(t, () => { log.push(t); r = summary(); });";

/** Metadata from the host fires the metadata and ready-state events in the specification's order. */
#[test]
fn metadata_fires_the_ready_state_events_in_order() {
    let mut page = Page::load(&format!("{RECORDER} v.src = 'clip.mp4';"));
    page.update(r#"[{"player":1,"kind":"metadata","duration":10.5,"width":640,"height":360,"audio":true,"video":true,"enough":true}]"#);
    assert_eq!(
        page.global("r"),
        "loadstart,durationchange,resize,loadedmetadata,loadeddata,canplay,canplaythrough|0|10.5|true|false|4|640"
    );
}

/** Unmuted play() without a person's interaction rejects with NotAllowedError; muted play() starts. */
#[test]
fn autoplay_policy_allows_only_muted_playback_before_interaction() {
    let page = Page::load(
        "let r = ''; const a = document.createElement('video'); a.src = 'a.mp4';\
         a.play().then(() => r += 'played', e => r += e.name);\
         const b = document.createElement('video'); b.muted = true; b.src = 'b.mp4'; b.play(); r += '|' + b.paused;",
    );
    assert_eq!(page.global("r"), "|falseNotAllowedError");
}

/** Playing, time updates and the end: play, playing, timeupdate, then timeupdate, pause and ended. */
#[test]
fn playback_runs_to_the_end_with_ended_and_pause() {
    let mut page = Page::load(&format!(
        "{RECORDER} v.muted = true; v.src = 'clip.mp4'; v.play();"
    ));
    page.update(r#"[{"player":1,"kind":"metadata","duration":2,"width":320,"height":240,"audio":false,"video":true,"enough":true}]"#);
    assert!(page.global("r").contains("play,"), "{}", page.global("r"));
    assert!(page.global("r").contains("playing"), "{}", page.global("r"));
    page.update(r#"[{"player":1,"kind":"time","time":1.25,"rate":1,"playing":true}]"#);
    assert!(
        page.global("r")
            .ends_with("timeupdate|1.25|2|false|false|4|320"),
        "{}",
        page.global("r")
    );
    page.update(r#"[{"player":1,"kind":"ended","time":2}]"#);
    assert!(
        page.global("r")
            .ends_with("timeupdate,pause,ended|2|2|true|true|4|320"),
        "{}",
        page.global("r")
    );
    let paused: Vec<bool> = page
        .sent
        .iter()
        .filter_map(|c| match c {
            MediaCommand::Load { settings, .. } | MediaCommand::Settings { settings, .. } => {
                Some(settings.paused)
            }
            _ => None,
        })
        .collect();
    assert_eq!(paused, [false, true], "{:?}", page.sent);
}

/** Setting currentTime seeks: seeking, the host's seek, then timeupdate and seeked at the new time. */
#[test]
fn seeking_fires_seeking_then_seeked() {
    let mut page = Page::load(&format!("{RECORDER} v.src = 'clip.mp4';"));
    page.update(r#"[{"player":1,"kind":"metadata","duration":30,"width":320,"height":240,"video":true,"enough":true}]"#);
    let before = page.sent.len();
    page.sources.push(String::new());
    page.update(r#"[{"player":1,"kind":"control","action":"seek","time":12}]"#);
    assert!(
        page.sent[before..]
            .iter()
            .any(|c| matches!(c, MediaCommand::Seek { time, .. } if *time == 12.0)),
        "{:?}",
        page.sent
    );
    assert!(
        page.global("r").contains("seeking|12"),
        "{}",
        page.global("r")
    );
    page.update(r#"[{"player":1,"kind":"seeked","time":12}]"#);
    assert!(
        page.global("r")
            .ends_with("seeking,timeupdate,seeked|12|30|true|false|4|320"),
        "{}",
        page.global("r")
    );
}

/** A host error before metadata fails the element with MEDIA_ERR_SRC_NOT_SUPPORTED and an error event. */
#[test]
fn a_failed_resource_sets_media_error_four() {
    let mut page = Page::load(
        "let r = ''; const v = document.createElement('video'); v.muted = true; v.src = 'missing.mp4';\
         const p = v.play().catch(e => r += 'rejected:' + e.name + '|');\
         v.addEventListener('error', () => { r += [v.error.code, v.error instanceof MediaError, v.networkState, MediaError.MEDIA_ERR_SRC_NOT_SUPPORTED].join(','); });",
    );
    page.update(
        r#"[{"player":1,"kind":"error","code":4,"message":"MEDIA_ELEMENT_ERROR: Format error"}]"#,
    );
    assert_eq!(page.global("r"), "4,true,3,4rejected:NotSupportedError|");
}

/** Source children are tried in order after loadstart; a type the decoders cannot play is skipped with an error event. */
#[test]
fn source_selection_skips_unplayable_types() {
    let page = Page::load(
        "let r = ''; const v = document.createElement('video');\
         const a = document.createElement('source'); a.type = 'video/webm'; a.src = 'a.webm'; a.onerror = () => r += 'skipped-webm|';\
         const b = document.createElement('source'); b.type = 'video/mp4'; b.src = 'b.mp4';\
         v.append(a, b); document.getElementById('main').appendChild(v);\
         v.addEventListener('loadstart', () => r += v.currentSrc);",
    );
    assert_eq!(
        page.global("r"),
        "https://example.com/app/b.mp4skipped-webm|"
    );
}

/** VTTCue is constructible with every attribute; TextTrack cues fire enter, exit and cuechange as time passes. */
#[test]
fn vtt_cues_enter_and_exit_with_playback_time() {
    let mut page = Page::load(&format!(
        "{RECORDER} v.src = 'clip.mp4';\
         const cue = new VTTCue(1, 2, 'Hello <b>world</b>'); cue.id = 'c1'; cue.line = 3; cue.align = 'start';\
         const track = v.addTextTrack('subtitles', 'English', 'en'); track.mode = 'showing'; track.addCue(cue);\
         let cues = [cue instanceof TextTrackCue, cue.startTime, cue.endTime, cue.text, cue.line, cue.align, cue.size, cue.position, track.cues.length, track.cues.getCueById('c1') === cue, cue.track === track, v.textTracks.length, cue.getCueAsHTML().childNodes.length].join(',');\
         let seen = ''; cue.onenter = () => seen += 'enter;'; cue.onexit = () => seen += 'exit;'; track.oncuechange = () => seen += 'cuechange;';"
    ));
    assert_eq!(
        page.global("cues"),
        "true,1,2,Hello <b>world</b>,3,start,100,auto,1,true,true,1,2"
    );
    page.update(r#"[{"player":1,"kind":"metadata","duration":5,"width":320,"height":240,"video":true,"enough":true}]"#);
    page.update(r#"[{"player":1,"kind":"time","time":1.5,"rate":1,"playing":false}]"#);
    assert_eq!(page.global("seen"), "enter;cuechange;");
    let captions: Vec<_> = page
        .sent
        .iter()
        .filter_map(|c| match c {
            MediaCommand::Captions { lines, .. } => Some(lines.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        captions.last(),
        Some(&vec!["Hello world".to_string()]),
        "{:?}",
        page.sent
    );
    page.update(r#"[{"player":1,"kind":"time","time":2.5,"rate":1,"playing":false}]"#);
    assert_eq!(page.global("seen"), "enter;cuechange;exit;cuechange;");
    assert!(
        page.global("r").contains("timeupdate"),
        "{}",
        page.global("r")
    );
}

/** Media interfaces that only the platform creates refuse construction, and their methods print as native. */
#[test]
fn platform_interfaces_are_not_constructible_and_print_native() {
    let page = Page::load(
        "const r = ['TimeRanges','MediaError','TextTrack','TextTrackCue','TextTrackList','SourceBuffer'].map(n => { try { new globalThis[n](); return 'built'; } catch (e) { return e.name; } }).join(',') + '|' +\
         String(HTMLMediaElement.prototype.play).includes('[native code]') + '|' + Object.prototype.toString.call(document.createElement('video').buffered);",
    );
    assert_eq!(
        page.global("r"),
        "TypeError,TypeError,TypeError,TypeError,TypeError,TypeError|true|[object TimeRanges]"
    );
}

/** volume outside [0, 1] throws IndexSizeError; changes fire volumechange and reach the host settings. */
#[test]
fn volume_is_range_checked_and_reaches_the_host() {
    let page = Page::load(
        "let r = ''; const v = document.createElement('audio'); v.src = 'a.mp3';\
         try { v.volume = 2; } catch (e) { r += e.name; }\
         v.addEventListener('volumechange', () => r += '|' + v.volume);\
         setTimeout(() => { v.volume = 0.5; }, 0);",
    );
    assert_eq!(page.global("r"), "IndexSizeError|0.5");
    let last = page.sent.iter().rev().find_map(|c| match c {
        MediaCommand::Settings { settings, .. } => Some(settings.clone()),
        _ => None,
    });
    let settings = last.expect("settings sent");
    assert_eq!(settings.volume, 0.5);
    assert!(!settings.video);
}

/** new Audio(src) is an audio element that loads its source. */
#[test]
fn audio_constructor_loads_its_source() {
    let page = Page::load(
        "let r = ''; const a = new Audio('song.mp3'); a.addEventListener('loadstart', () => r = a.currentSrc + '|' + (a instanceof HTMLAudioElement));",
    );
    assert_eq!(page.global("r"), "https://example.com/app/song.mp3|true");
}

/** Media Source: an object URL attaches, sourceopen fires, and appended fMP4 fragments become buffered ranges. */
#[test]
fn media_source_buffers_appended_fragments() {
    let mut page = Page::load(&format!(
        "{RECORDER} let m = ''; const ms = new MediaSource(); v.src = URL.createObjectURL(ms);\
         const segment = {SEGMENT};\
         ms.addEventListener('sourceopen', () => {{\
           m += 'open:' + ms.readyState;\
           const sb = ms.addSourceBuffer('video/mp4; codecs=\"avc1.42E01E\"');\
           sb.addEventListener('updateend', () => {{ m += '|updateend:' + sb.buffered.length + ':' + sb.buffered.start(0) + '-' + sb.buffered.end(0) + ':' + v.buffered.length; }});\
           try {{ sb.appendBuffer(new Uint8Array(segment)); }} catch (e) {{ m += '|' + e; }}\
         }});"
    ));
    let _ = &mut page;
    assert_eq!(page.global("m"), "open:open|updateend:1:0-2:1");
    assert!(
        page.sent
            .iter()
            .any(|c| matches!(c, MediaCommand::Load { stream: true, .. })),
        "{:?}",
        page.sent
    );
    assert!(
        page.sent
            .iter()
            .any(|c| matches!(c, MediaCommand::Append { bytes, .. } if !bytes.is_empty())),
        "{:?}",
        page.sent
    );
}

/** A tiny fragmented MP4 built in the page: moov with one 320x240 video track at timescale 1000,
then one moof whose trun holds two samples of 1000 ticks from tfdt 0, so it buffers [0, 2). */
const SEGMENT: &str = "(() => {\
  const u32 = n => [n >>> 24 & 255, n >>> 16 & 255, n >>> 8 & 255, n & 255];\
  const box = (type, ...parts) => { const body = [].concat(...parts); return [...u32(body.length + 8), ...[...type].map(c => c.charCodeAt(0)), ...body]; };\
  const zeros = n => new Array(n).fill(0);\
  const tkhd = box('tkhd', u32(0), u32(0), u32(0), u32(1), u32(0), u32(0), zeros(8), zeros(8), zeros(36), u32(320 << 16), u32(240 << 16));\
  const mdhd = box('mdhd', u32(0), u32(0), u32(0), u32(1000), u32(0), zeros(4));\
  const hdlr = box('hdlr', u32(0), u32(0), [...'vide'].map(c => c.charCodeAt(0)), zeros(12), [0]);\
  const avc1 = box('avc1', zeros(78), box('avcC', [1, 0x42, 0xE0, 0x1E, 0xFF, 0xE1, 0, 0]));
  const minf = box('minf', box('stbl', box('stsd', u32(0), u32(1), avc1)));
  const moov = box('moov', box('trak', tkhd, box('mdia', mdhd, hdlr, minf)));\
  const traf = box('traf', box('tfhd', u32(0), u32(1)), box('tfdt', u32(0), u32(0)), box('trun', u32(0x100), u32(2), u32(1000), u32(1000)));\
  return [...moov, ...box('moof', traf)];\
})()";

/** The page for one Media Source test: the recorder, a source attached to a video, and `body` run once it opens. */
fn open(body: &str) -> String {
    format!(
        "{RECORDER} let m = ''; const ms = new MediaSource(); v.src = URL.createObjectURL(ms);\
         const seg = {SEGMENT}; const moovSize = seg[0] << 24 | seg[1] << 16 | seg[2] << 8 | seg[3];\
         const media = seg.slice(moovSize);\
         ms.addEventListener('sourceopen', () => {{ {body} }});"
    )
}

/** isTypeSupported answers for the containers and codecs the host streams, and for nothing else. */
#[test]
fn is_type_supported_answers_for_streamable_mp4_codecs() {
    let page = Page::load(
        "let r = [MediaSource.isTypeSupported('video/mp4; codecs=\"avc1.4d401f\"'),\
           MediaSource.isTypeSupported('video/mp4; codecs=\"avc1.42E01E, mp4a.40.2\"'),\
           MediaSource.isTypeSupported('audio/mp4; codecs=\"mp4a.40.2\"'),\
           MediaSource.isTypeSupported('video/mp4; codecs=\"vp09.00.10.08\"'),\
           MediaSource.isTypeSupported('video/webm'),\
           MediaSource.isTypeSupported('video/mp4')].join('|');",
    );
    assert_eq!(page.global("r"), "true|true|true|false|false|true");
}

/** addSourceBuffer refuses a type the host cannot stream with NotSupportedError. */
#[test]
fn add_source_buffer_refuses_an_unstreamable_type() {
    let page = Page::load(&open(
        "try { ms.addSourceBuffer('video/webm; codecs=\"vp9\"'); } catch (e) { m = e.name; }",
    ));
    assert_eq!(page.global("m"), "NotSupportedError");
}

/** An append is asynchronous: updating is set at once, updatestart, update and updateend follow in order. */
#[test]
fn append_brackets_the_update_events_around_the_async_work() {
    let page = Page::load(&open(
        "const sb = ms.addSourceBuffer('video/mp4; codecs=\"avc1.42E01E\"');\
         sb.addEventListener('updatestart', () => m += 'start;');\
         sb.addEventListener('update', () => m += 'update;');\
         sb.addEventListener('updateend', () => m += 'end:' + sb.buffered.end(0) + ';');\
         sb.appendBuffer(new Uint8Array(seg)); m += 'sync:' + sb.updating + ';';",
    ));
    assert_eq!(page.global("m"), "sync:true;start;update;end:2;");
}

/** timestampOffset moves a segment's samples on the timeline: the range becomes [10, 12). */
#[test]
fn timestamp_offset_moves_the_buffered_range() {
    let page = Page::load(&open(
        "const sb = ms.addSourceBuffer('video/mp4; codecs=\"avc1.42E01E\"');\
         sb.timestampOffset = 10;\
         sb.addEventListener('updateend', () => m += sb.buffered.start(0) + '-' + sb.buffered.end(0));\
         sb.appendBuffer(new Uint8Array(seg));",
    ));
    assert_eq!(page.global("m"), "10-12");
}

/** The append window drops samples that end before its start and clips the one that straddles it. */
#[test]
fn append_window_start_drops_samples_before_it() {
    let page = Page::load(&open(
        "const sb = ms.addSourceBuffer('video/mp4; codecs=\"avc1.42E01E\"');\
         sb.appendWindowStart = 1;\
         sb.addEventListener('updateend', () => m += sb.buffered.start(0) + '-' + sb.buffered.end(0));\
         sb.appendBuffer(new Uint8Array(seg));",
    ));
    assert_eq!(page.global("m"), "1-2");
}

/** The append window's end clips the samples that run past it. */
#[test]
fn append_window_end_clips_samples_after_it() {
    let page = Page::load(&open(
        "const sb = ms.addSourceBuffer('video/mp4; codecs=\"avc1.42E01E\"');\
         sb.appendWindowEnd = 1.5;\
         sb.addEventListener('updateend', () => m += sb.buffered.start(0) + '-' + sb.buffered.end(0));\
         sb.appendBuffer(new Uint8Array(seg));",
    ));
    assert_eq!(page.global("m"), "0-1.5");
}

/** A media segment before any init segment is an append error: error, then updateend, and nothing buffers. */
#[test]
fn media_segment_before_its_init_is_an_append_error() {
    let page = Page::load(&open(
        "const sb = ms.addSourceBuffer('video/mp4; codecs=\"avc1.42E01E\"');\
         sb.addEventListener('error', () => m += 'error;');\
         sb.addEventListener('updateend', () => m += 'end:' + sb.buffered.length + ';');\
         sb.appendBuffer(new Uint8Array(media));",
    ));
    assert_eq!(page.global("m"), "error;end:0;");
}

/** A box cut by one append waits in the buffer for the next append to complete it. */
#[test]
fn a_box_split_across_appends_is_completed_by_the_next() {
    let page = Page::load(&open(
        "const sb = ms.addSourceBuffer('video/mp4; codecs=\"avc1.42E01E\"');\
         let n = 0;\
         sb.addEventListener('updateend', () => {{\
           n++;\
           if (n === 1) {{ m += 'first:' + sb.buffered.length + ';'; sb.appendBuffer(new Uint8Array(seg).subarray(40)); }}\
           else m += 'second:' + sb.buffered.length + ':' + sb.buffered.end(0);\
         }});\
         sb.appendBuffer(new Uint8Array(seg).subarray(0, 40));",
    ));
    assert_eq!(page.global("m"), "first:0;second:1:2");
}

/** remove drops the buffered time between its bounds and leaves the rest of the range. */
#[test]
fn remove_trims_the_buffered_ranges() {
    let page = Page::load(&open(
        "const sb = ms.addSourceBuffer('video/mp4; codecs=\"avc1.42E01E\"');\
         let n = 0;\
         sb.addEventListener('updateend', () => {{\
           n++;\
           if (n === 1) {{ m += sb.buffered.start(0) + '-' + sb.buffered.end(0) + ';'; sb.remove(0, 1); }}\
           else m += sb.buffered.start(0) + '-' + sb.buffered.end(0);\
         }});\
         sb.appendBuffer(new Uint8Array(seg));",
    ));
    assert_eq!(page.global("m"), "0-2;1-2");
}

/** A later media segment joins the init segment already kept for the buffer; its ranges merge. */
#[test]
fn a_media_segment_reuses_the_init_segment_kept_for_the_buffer() {
    let page = Page::load(&open(
        "const sb = ms.addSourceBuffer('video/mp4; codecs=\"avc1.42E01E\"');\
         let n = 0;\
         sb.addEventListener('error', () => m += 'error;');\
         sb.addEventListener('updateend', () => {{\
           n++;\
           if (n === 1) sb.appendBuffer(new Uint8Array(media));\
           else m += sb.buffered.length + ':' + sb.buffered.end(0);\
         }});\
         sb.appendBuffer(new Uint8Array(seg));",
    ));
    assert_eq!(page.global("m"), "1:2");
}

/** An init segment whose codec the type does not name is refused, so mismatched tracks never buffer. */
#[test]
fn an_init_segment_with_another_codec_is_an_append_error() {
    let page = Page::load(&open(
        "const sb = ms.addSourceBuffer('audio/mp4; codecs=\"mp4a.40.2\"');\
         sb.addEventListener('error', () => m += 'error;');\
         sb.addEventListener('updateend', () => m += 'end:' + sb.buffered.length + ';');\
         sb.appendBuffer(new Uint8Array(seg));",
    ));
    assert_eq!(page.global("m"), "error;end:0;");
}

/** Bytes that are not ISO BMFF (a WebM header here) are refused as an append error. */
#[test]
fn bytes_that_are_not_mp4_are_an_append_error() {
    let page = Page::load(&open(
        "const sb = ms.addSourceBuffer('video/mp4; codecs=\"avc1.42E01E\"');\
         sb.addEventListener('error', () => m += 'error;');\
         sb.addEventListener('updateend', () => m += 'end:' + sb.buffered.length + ';');\
         sb.appendBuffer(new Uint8Array([0x1a, 0x45, 0xdf, 0xa3, 0, 0, 0, 0x7f, 1, 2, 3, 4]));",
    ));
    assert_eq!(page.global("m"), "error;end:0;");
}

/** abort during an append cancels it: abort then updateend, and the append never buffers. */
#[test]
fn abort_during_an_append_cancels_it() {
    let page = Page::load(&open(
        "const sb = ms.addSourceBuffer('video/mp4; codecs=\"avc1.42E01E\"');\
         sb.addEventListener('abort', () => m += 'abort;');\
         sb.addEventListener('updateend', () => m += 'end:' + sb.buffered.length + ';');\
         sb.appendBuffer(new Uint8Array(seg)); sb.abort();",
    ));
    assert_eq!(page.global("m"), "abort;end:0;");
}

/** endOfStream moves the source to ended and fires sourceended. */
#[test]
fn end_of_stream_ends_the_source() {
    let page = Page::load(&open(
        "ms.addEventListener('sourceended', () => m += 'ended:' + ms.readyState);\
         ms.endOfStream();",
    ));
    assert_eq!(page.global("m"), "ended:ended");
}
