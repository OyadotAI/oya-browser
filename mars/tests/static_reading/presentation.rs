/** Presentation-only changes reuse retained CSS and VM state without fetching or replaying page code. */
use super::*;

/** This is a synthetic styled fixture; the original example source is separately tested unchanged at document stages. */
const STYLED: &str = "<style>html{color-scheme:light dark;background:light-dark(#eee,#222)}body{font:16px/1.6 system-ui,sans-serif;max-width:26em;margin:auto;padding:25vh 2em 2em;text-align:center}</style><p id=out>Initial counter</p><input id=name><button id=step type=button onclick='step()'>Step</button><button id=probe type=button onclick='probe()'>Probe</button><script>var count=1;window.localStorage.setItem('count',count);function step(){count++;window.localStorage.setItem('count',count);document.getElementById('out').textContent=count;}function probe(){document.getElementById('out').textContent=count+' '+window.localStorage.getItem('count');}</script>";

/** The UA's 1em paragraph top margin; body padding keeps it from collapsing, as in Chromium. */
const P_MARGIN: f64 = 16.0;

/** Context reflow preserves edited inputs, page/revision identity and both script and Storage state. */
#[test]
fn resize_and_appearance_recompute_retained_css_without_replaying_scripts_or_fetching() {
    let fixture = fixture(STYLED);
    let mut runtime = runtime(&[&fixture.url]);
    let page = call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    let mut params = target(&page, "name");
    params["text"] = json!("Ada");
    call(&mut runtime, "page.type", params).unwrap();
    let page = call(&mut runtime, "page.inspect", json!({})).unwrap();
    let mut page = call(&mut runtime, "page.click", target(&page, "step")).unwrap();
    assert_eq!(node(&page, "out")["text"], "2");
    for (width, height, appearance, background, x) in [
        (640.0, 400.0, Appearance::Dark, "#222", 112.0),
        (1100.0, 600.0, Appearance::Light, "#eee", 342.0),
    ] {
        runtime
            .set_render_context(RenderContext {
                width,
                height,
                appearance,
            })
            .unwrap();
        let inspected = call(&mut runtime, "page.inspect", json!({})).unwrap();
        assert_eq!(inspected["page_id"], page["page_id"]);
        assert_eq!(inspected["revision"], page["revision"]);
        assert!(
            inspected["layout_generation"].as_u64().unwrap()
                > page["layout_generation"].as_u64().unwrap()
        );
        assert_eq!(node(&inspected, "name")["value"], "Ada");
        assert_eq!(
            node(&inspected, "name")["node_id"],
            node(&page, "name")["node_id"]
        );
        let snapshot = runtime.snapshot();
        assert_eq!(
            snapshot.render_context,
            RenderContext {
                width,
                height,
                appearance
            }
        );
        let display = snapshot.display.unwrap();
        assert_eq!(display.canvas_background.as_deref(), Some(background));
        assert_eq!(
            (
                display.items[0].x,
                display.items[0].width,
                display.items[0].y
            ),
            (x, 416.0, height / 4.0 + P_MARGIN)
        );
        page = call(&mut runtime, "page.click", target(&inspected, "probe")).unwrap();
        assert_eq!(node(&page, "out")["text"], "2 2");
    }
    assert_eq!(fixture.requests.load(Ordering::Relaxed), 1);
    human_tab(&mut runtime, "tab.open", json!({}));
    runtime
        .set_render_context(RenderContext {
            width: 640.0,
            height: 480.0,
            appearance: Appearance::Dark,
        })
        .unwrap();
    human_tab(&mut runtime, "tab.select", json!({"tab_id":"tab-1"}));
    let activated = call(&mut runtime, "page.inspect", json!({})).unwrap();
    assert_eq!(activated["page_id"], page["page_id"]);
    assert_eq!(activated["revision"], page["revision"]);
    assert_eq!(node(&activated, "name")["value"], "Ada");
    assert_eq!(node(&activated, "out")["text"], "2 2");
    assert_eq!(
        runtime
            .snapshot()
            .display
            .unwrap()
            .canvas_background
            .as_deref(),
        Some("#222")
    );
    assert_eq!(fixture.requests.load(Ordering::Relaxed), 1);
}
