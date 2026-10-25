/**
 * Viewport scrolling as page scripts see it: `scroll` fires at the document and reaches window
 * capture listeners, and `scrollY` reports the offset. LinkedIn's lazy loader listens on the
 * window with capture and loads `data-delayed-url` images once `getBoundingClientRect` puts them
 * in view, so without these its images below the first screen never loaded.
 */
mod common;

use common::{failure, host, input, load_with, round_trip};
use mars_v9::PageEvent;

/** A scroll event reaches window capture listeners and document listeners, with scrollY set. */
#[test]
fn scroll_fires_at_the_document_and_reaches_the_window() {
    let mut environment = common::environment();
    let sources = vec![
        "let seen = '';
         window.addEventListener('scroll', e => { seen += 'window:' + scrollY + ':' + pageYOffset }, true);
         document.addEventListener('scroll', e => { seen += '|document:' + e.bubbles });"
            .to_string(),
    ];
    let execution = load_with(&sources[0], &environment);
    assert_eq!(failure(&execution), None);
    environment.scroll_y = 700.0;
    let host = host();
    let after = mars_v9::page_event(
        input(&sources, &host, &environment),
        round_trip(execution.state),
        PageEvent::Scroll,
    )
    .expect("scroll event");
    assert_eq!(
        common::global(&after, "seen"),
        "window:700:700|document:true"
    );
}
