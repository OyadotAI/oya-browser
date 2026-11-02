/** History, bookmarks, settings, address input, downloads and text selection: the pure rules behind Mars's everyday browser features. */
use mars::runtime::{
    bookmarks::{BOOKMARKS_LIMIT, Bookmarks},
    downloads::{self, Downloads, State},
    history::{self, HISTORY_LIMIT, History},
    settings::{SearchEngine, Settings, address_input},
    text_selection::{self, Caret, TextLine},
};

#[test]
fn history_records_once_per_address_newest_first() {
    let mut history = History::default();
    assert!(history.record("https://a.example/", "A", 1));
    assert!(history.record("https://b.example/", "B", 2));
    assert!(history.record("https://a.example/", "A again", 3));
    let urls: Vec<_> = history.visits.iter().map(|v| v.url.as_str()).collect();
    assert_eq!(urls, ["https://a.example/", "https://b.example/"]);
    assert_eq!(history.visits[0].title, "A again");
    assert_eq!(history.visits[0].time, 3);
}

#[test]
fn history_skips_addresses_mars_may_not_keep() {
    let mut history = History::default();
    for url in [
        "about:blank",
        "file:///etc/passwd",
        "https://user:pw@a.example/",
        "data:text/html,x",
    ] {
        assert!(!history.record(url, "x", 1), "{url}");
    }
    assert!(history.visits.is_empty());
}

#[test]
fn history_is_bounded_by_count_and_bytes() {
    let mut history = History::default();
    let title = "t".repeat(1000);
    for index in 0..HISTORY_LIMIT + 50 {
        history.record(
            &format!("https://example.com/{index}/{}", "p".repeat(1500)),
            &title,
            1,
        );
    }
    assert!(history.visits.len() <= HISTORY_LIMIT);
    assert!(history.encode().is_ok(), "a bounded history always encodes");
    assert!(history.visits[0].title.chars().count() <= history::TITLE_CHARS);
}

#[test]
fn history_decode_rejects_untrusted_records() {
    assert!(History::decode(b"not json").is_err());
    assert!(History::decode(br#"{"version":2,"visits":[]}"#).is_err());
    assert!(
        History::decode(
            br#"{"version":1,"visits":[{"url":"javascript:alert(1)","title":"","time":1}]}"#
        )
        .is_err()
    );
    assert!(History::decode(br#"{"version":1,"visits":[],"extra":1}"#).is_err());
    let mut good = History::default();
    good.record("https://a.example/", "A", 1);
    assert_eq!(History::decode(&good.encode().unwrap()).unwrap(), good);
}

#[test]
fn history_search_matches_every_word_in_title_or_address() {
    let mut history = History::default();
    history.record("https://docs.rs/serde", "serde - Rust", 1);
    history.record("https://example.com/", "Example Domain", 2);
    assert_eq!(history.search("rust serde"), vec![1]);
    assert_eq!(history.search("EXAMPLE"), vec![0]);
    assert_eq!(history.search("").len(), 2);
    history.clear();
    assert!(history.search("").is_empty());
}

#[test]
fn suggestions_rank_site_prefix_then_address_then_title_word() {
    let mut history = History::default();
    history.record("https://news.example/rust", "Learning Rust", 1);
    history.record("https://rust-lang.org/learn", "Learn", 2);
    history.record("https://rust-lang.org/", "Rust Programming Language", 3);
    let bookmarks = Bookmarks::default();
    let found = history::suggestions(&history, &bookmarks, "rust");
    let urls: Vec<_> = found.iter().map(|s| s.url.as_str()).collect();
    assert_eq!(
        urls,
        [
            "https://rust-lang.org/",
            "https://rust-lang.org/learn",
            "https://news.example/rust"
        ]
    );
}

#[test]
fn suggestions_prefer_bookmarks_cap_at_eight_and_three_per_site() {
    let mut history = History::default();
    for index in 0..20 {
        history.record(&format!("https://site{index}.example/"), "x", index);
        history.record(&format!("https://site0.example/{index}"), "x", index);
    }
    let mut bookmarks = Bookmarks::default();
    bookmarks.toggle("https://site5.example/", "Five").unwrap();
    let found = history::suggestions(&history, &bookmarks, "site");
    assert_eq!(found.len(), history::SUGGESTION_LIMIT);
    assert!(
        found[0].bookmark,
        "a bookmark edges out history of the same weight"
    );
    let site0 = found.iter().filter(|s| s.url.contains("site0.")).count();
    assert!(site0 <= 3);
    assert!(history::suggestions(&history, &bookmarks, "   ").is_empty());
    assert!(history::suggestions(&history, &bookmarks, "zzz").is_empty());
}

#[test]
fn bookmarks_toggle_rename_remove_and_bound() {
    let mut bookmarks = Bookmarks::default();
    assert_eq!(bookmarks.toggle("https://a.example/", "A"), Ok(true));
    assert!(bookmarks.contains("https://a.example/"));
    assert!(bookmarks.rename(0, "Mine"));
    assert!(
        !bookmarks.rename(0, "  "),
        "an empty name keeps the old one"
    );
    assert_eq!(bookmarks.items[0].title, "Mine");
    assert_eq!(bookmarks.toggle("https://a.example/", "A"), Ok(false));
    assert!(bookmarks.toggle("chrome://settings", "x").is_err());
    for index in 0..BOOKMARKS_LIMIT {
        bookmarks
            .toggle(&format!("https://b.example/{index}"), "b")
            .unwrap();
    }
    assert!(
        bookmarks
            .toggle("https://one-too-many.example/", "x")
            .is_err()
    );
    assert!(bookmarks.remove(0) && !bookmarks.remove(BOOKMARKS_LIMIT + 1));
    assert_eq!(
        Bookmarks::decode(&bookmarks.encode().unwrap()).unwrap(),
        bookmarks
    );
    assert!(
        Bookmarks::decode(br#"{"version":1,"items":[{"url":"ftp://x/","title":""}]}"#).is_err()
    );
}

#[test]
fn settings_round_trip_and_reject_unsafe_values() {
    let mut settings = Settings::default();
    assert_eq!(settings.search_engine, SearchEngine::DuckDuckGo);
    assert!(settings.set_home_page("https://home.example/"));
    assert!(!settings.set_home_page("javascript:alert(1)"));
    assert_eq!(settings.home_page.as_deref(), Some("https://home.example/"));
    settings.downloads_folder = Some("relative/path".into());
    assert!(settings.encode().is_err());
    settings.downloads_folder = Some("/Users/me/Downloads".into());
    assert_eq!(
        Settings::decode(&settings.encode().unwrap()).unwrap(),
        settings
    );
    let older = br#"{"version":1,"appearance":"dark","home_page":null,"downloads_folder":null,"bookmarks_bar":true}"#;
    assert_eq!(
        Settings::decode(older).unwrap().search_engine,
        SearchEngine::DuckDuckGo
    );
}

#[test]
fn address_input_reads_typed_text_as_chrome_does() {
    let ddg = SearchEngine::DuckDuckGo;
    let cases = [
        ("example.com", "https://example.com"),
        ("example.com/path?q=1", "https://example.com/path?q=1"),
        ("docs.rs:443/serde", "https://docs.rs:443/serde"),
        ("http://plain.example/", "http://plain.example/"),
        ("localhost:3000", "http://localhost:3000"),
        ("localhost", "http://localhost"),
        ("127.0.0.1:8080/x", "http://127.0.0.1:8080/x"),
        ("[::1]:8080", "http://[::1]:8080"),
        ("rust", "https://duckduckgo.com/?q=rust"),
        (
            "rust borrow checker",
            "https://duckduckgo.com/?q=rust+borrow+checker",
        ),
        ("what is 1.5", "https://duckduckgo.com/?q=what+is+1.5"),
        ("a&b", "https://duckduckgo.com/?q=a%26b"),
    ];
    for (typed, expected) in cases {
        assert_eq!(
            address_input(typed, ddg).as_deref(),
            Some(expected),
            "{typed}"
        );
    }
    assert_eq!(address_input("   ", ddg), None);
    assert_eq!(
        address_input("rust", SearchEngine::Google).as_deref(),
        Some("https://www.google.com/search?q=rust")
    );
}

#[cfg(feature = "test-support")]
#[test]
fn record_store_keeps_kinds_independent() {
    use mars::profile::{Record, RecordStore, test_support::MemoryRecordStore};
    let store = MemoryRecordStore::default();
    store.replace(Record::History, b"h").unwrap();
    store.replace(Record::Bookmarks, b"b").unwrap();
    assert_eq!(
        store.load(Record::History).unwrap().as_deref(),
        Some(&b"h"[..])
    );
    assert_eq!(store.load(Record::Settings).unwrap(), None);
    store.fail_next_replace();
    assert!(store.replace(Record::History, b"x").is_err());
    assert_eq!(store.record(Record::History).as_deref(), Some(&b"h"[..]));
}

#[test]
fn download_names_are_safe_and_typed() {
    assert_eq!(
        downloads::file_name("https://a.example/files/report.pdf", "", None),
        "report.pdf"
    );
    assert_eq!(
        downloads::file_name("https://a.example/", "text/html", None),
        "download.html"
    );
    assert_eq!(
        downloads::file_name("https://a.example/pic", "image/png", None),
        "pic.png"
    );
    assert_eq!(
        downloads::file_name("https://a.example/a%20b.txt", "", None),
        "a b.txt"
    );
    assert_eq!(
        downloads::file_name("https://a.example/..%2F..%2Fetc", "", None),
        "etc"
    );
    assert_eq!(
        downloads::file_name(
            "https://a.example/x",
            "",
            Some("attachment; filename=\"../../evil.sh\"")
        ),
        "evil.sh"
    );
    assert_eq!(downloads::numbered("report.pdf", 2), "report (2).pdf");
    assert_eq!(downloads::numbered("README", 1), "README (1)");
}

#[test]
fn downloads_save_uniquely_without_execute_permission() {
    use std::os::unix::fs::PermissionsExt;
    let folder = std::env::temp_dir().join(format!("mars-human-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let first = downloads::save(&folder, "a.txt", b"one").unwrap();
    let second = downloads::save(&folder, "a.txt", b"two").unwrap();
    assert_eq!(first.file_name().unwrap(), "a.txt");
    assert_eq!(second.file_name().unwrap(), "a (1).txt");
    assert_eq!(std::fs::read(&first).unwrap(), b"one");
    let mode = std::fs::metadata(&second).unwrap().permissions().mode();
    assert_eq!(mode & 0o111, 0, "downloads are never executable");
    std::fs::remove_dir_all(&folder).unwrap();
}

#[test]
fn downloads_list_tracks_progress_and_is_bounded() {
    let mut list = Downloads::default();
    let id = list.start("https://a.example/x.png");
    assert!(list.active());
    list.finish(id, Ok(("/tmp/x.png".into(), 10)));
    assert!(!list.active());
    assert!(matches!(list.items[0].state, State::Done(_)));
    let failed = list.start("https://a.example/y");
    list.finish(failed, Err("no".into()));
    assert_eq!(list.items[0].state, State::Failed("no".into()));
    for _ in 0..downloads::DOWNLOADS_KEPT + 5 {
        list.start("https://a.example/z");
    }
    assert_eq!(list.items.len(), downloads::DOWNLOADS_KEPT);
}

/** Two lines of one block and a third block, measured as 10 points per character. */
fn sample() -> Vec<TextLine> {
    let line = |text: &str, y: f64, item: usize| TextLine {
        text: text.into(),
        x: 0.0,
        y,
        width: text.chars().count() as f64 * 10.0,
        height: 20.0,
        item,
    };
    vec![
        line("hello world", 0.0, 0),
        line("second line", 20.0, 0),
        line("next block", 60.0, 1),
    ]
}

/** Monospace stand-in for AppKit text measurement. */
fn mono(_item: usize, text: &str) -> f64 {
    text.chars().count() as f64 * 10.0
}

#[test]
fn selection_hits_the_nearest_character_boundary() {
    let lines = sample();
    assert_eq!(
        text_selection::hit(&lines, 34.0, 5.0, &mono),
        Some(Caret { line: 0, offset: 3 })
    );
    assert_eq!(
        text_selection::hit(&lines, 36.0, 5.0, &mono),
        Some(Caret { line: 0, offset: 4 })
    );
    assert_eq!(
        text_selection::hit(&lines, 500.0, 25.0, &mono),
        Some(Caret {
            line: 1,
            offset: 11
        })
    );
    assert_eq!(
        text_selection::hit(&lines, 0.0, 45.0, &mono).map(|c| c.line),
        Some(1)
    );
    assert_eq!(text_selection::hit(&[], 0.0, 0.0, &mono), None);
}

#[test]
fn selection_text_joins_lines_and_blocks() {
    let lines = sample();
    let from = Caret { line: 0, offset: 6 };
    let to = Caret { line: 2, offset: 4 };
    assert_eq!(
        text_selection::text(&lines, to, from),
        "world second line\nnext"
    );
    assert_eq!(text_selection::rects(&lines, from, to, &mono).len(), 3);
}

#[test]
fn double_click_selects_a_word_and_triple_click_a_line() {
    let lines = sample();
    let (a, b) = text_selection::word(&lines, Caret { line: 0, offset: 8 });
    assert_eq!(text_selection::text(&lines, a, b), "world");
    let (a, b) = text_selection::whole_line(&lines, Caret { line: 1, offset: 2 });
    assert_eq!(text_selection::text(&lines, a, b), "second line");
}
