/** The window `resize` event: a step that finds the viewport changed since the last step fires it
at the window before the media query report, with innerWidth already updated. A step at the same
viewport fires nothing, so a page hears one resize per change. */
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

/** Load a page script at `first` width, then run one media step at `second` width. */
fn step_at(script: &str, first: f64, second: f64) -> mars_v9::Execution {
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

/** A window resize listener hears the new size once when the viewport changed. */
#[test]
fn window_resize_listener_fires_once_after_a_viewport_change() {
    let step = step_at(
        "let log='';window.addEventListener('resize',()=>{log+=innerWidth+';'});",
        1100.0,
        640.0,
    );
    assert_eq!(common::global(&step, "log"), "640;");
}

/** Without a change in size, no resize is fired. */
#[test]
fn window_resize_listener_stays_quiet_when_the_size_holds() {
    let step = step_at(
        "let log='';window.addEventListener('resize',()=>{log+='x'});",
        640.0,
        640.0,
    );
    assert_eq!(common::global(&step, "log"), "");
}

/** The resize fires before the media query change, as the rendering steps run the resize steps
first, so a media listener that reads the page's resize state sees it already done. */
#[test]
fn window_resize_fires_before_media_query_changes() {
    let step = step_at(
        "let log='';\
        window.addEventListener('resize',()=>{log+='r'+innerWidth+';'});\
        const mql=matchMedia('(max-width: 656px)');\
        mql.addEventListener('change',()=>{log+='m;'});",
        1100.0,
        640.0,
    );
    assert_eq!(common::global(&step, "log"), "r640;m;");
}

/** A second step at the same viewport, after the resize was reported, fires nothing more. */
#[test]
fn window_resize_is_not_repeated_on_later_steps_at_the_same_size() {
    let sources = vec!["let log='';window.addEventListener('resize',()=>{log+='x'});".to_string()];
    let host = common::host();
    let before = viewport(1100.0);
    let loaded = mars_v9::load_page(common::input(&sources, &host, &before)).unwrap();
    let after = viewport(640.0);
    let first = mars_v9::report_media(
        common::input(&sources, &host, &after),
        common::round_trip(loaded.state),
    )
    .unwrap();
    assert_eq!(common::global(&first, "log"), "x");
    let again = mars_v9::report_media(
        common::input(&sources, &host, &after),
        common::round_trip(first.state),
    )
    .unwrap();
    assert_eq!(common::global(&again, "log"), "x");
}
