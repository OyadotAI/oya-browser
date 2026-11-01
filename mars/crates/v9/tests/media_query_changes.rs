/** Media query lists are live: `matches` follows the viewport and a `change` listener hears a
resize that flips the query, as Chrome's MediaQueryList does. Libraries such as iron-media-query
(YouTube's masthead narrow mode) only learn about a resize from that event. */
use crate::common;
use mars_v9::PageEnvironment;

/** A page environment at the given viewport width. */
fn viewport(width: f64) -> PageEnvironment {
    PageEnvironment {
        viewport_width: width,
        viewport_height: 800.0,
        ..common::environment()
    }
}

/** Load a page script at `first` width, then let a step run at `second` width. */
fn resize(script: &str, first: f64, second: f64) -> mars_v9::Execution {
    let sources = vec![script.to_string()];
    let host = common::host();
    let before = viewport(first);
    let loaded = mars_v9::load_page(common::input(&sources, &host, &before)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let after = viewport(second);
    let step = mars_v9::report_media(
        common::input(&sources, &host, &after),
        common::round_trip(loaded.state),
    )
    .unwrap();
    assert!(step.failed.is_empty(), "{:?}", common::failure(&step));
    step
}

/** `matches` is read from the viewport at the time of the read, not frozen at creation: a
listener that reads `matches` after the resize sees the new result. */
#[test]
fn media_query_matches_follows_the_viewport_it_is_read_in() {
    let step = resize(
        "let before=matchMedia('(max-width: 656px)').matches;let log='';\
        const mql=matchMedia('(max-width: 656px)');\
        mql.addEventListener('change',()=>{log+=mql.matches+';'});",
        1100.0,
        640.0,
    );
    assert_eq!(common::global(&step, "before"), "false");
    assert_eq!(common::global(&step, "log"), "true;");
}

/** A change listener hears the flip a resize makes, with the new `matches` and `media`. */
#[test]
fn media_query_change_listener_fires_when_a_resize_flips_the_query() {
    let step = resize(
        "let log='';const mql=matchMedia('(max-width: 656px)');\
        mql.addEventListener('change',e=>{log+=e.matches+' '+e.media+';'});",
        1100.0,
        640.0,
    );
    assert_eq!(common::global(&step, "log"), "true (max-width: 656px);");
}

/** `addListener`, the legacy spelling iron-media-query uses, is the same registration. */
#[test]
fn media_query_legacy_add_listener_fires_on_change() {
    let step = resize(
        "let log='';const mql=matchMedia('(max-width: 656px)');\
        mql.addListener(e=>{log+=e.matches+';'});",
        1100.0,
        640.0,
    );
    assert_eq!(common::global(&step, "log"), "true;");
}

/** A resize that leaves the query's result unchanged fires nothing. */
#[test]
fn media_query_change_listener_stays_quiet_when_the_result_holds() {
    let step = resize(
        "let log='';const mql=matchMedia('(max-width: 656px)');\
        mql.addListener(()=>{log+='x'});",
        1100.0,
        1000.0,
    );
    assert_eq!(common::global(&step, "log"), "");
}

/** A removed listener is not called on a later flip. */
#[test]
fn media_query_removed_listener_is_not_called() {
    let step = resize(
        "let log='';const mql=matchMedia('(max-width: 656px)');\
        const f=()=>{log+='x'};mql.addListener(f);mql.removeListener(f);",
        1100.0,
        640.0,
    );
    assert_eq!(common::global(&step, "log"), "");
}
