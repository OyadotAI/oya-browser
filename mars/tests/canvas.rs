/* Actual native glyphs, PNG bytes and Eye publication are checked together without network access. */
#![cfg(target_os = "macos")]
use mars::{document, v9};
use mars_core::canvas::CanvasHost;

/** Load a real canvas through the injected host and the actual document projection. */
fn draw(source: &str) -> (document::Document, v9::Execution) {
    let document = document::parse("<canvas id='paint' width='100' height='50'></canvas>").unwrap();
    let host = v9::DocumentHost {
        imports: None,
        canvas: &mars_eye::platform::NativeCanvas,
        languages: &[],
        fragments: &document::DomFragments,
    };
    let environment = v9::PageEnvironment {
        dom_level: v9::DomLevel::Extended,
        ..Default::default()
    };
    let result = v9::load_page(v9::PageInput {
        sources: &[source.into()],
        source_keys: &[],
        document: document::script_document(&document, "canvas-test").unwrap(),
        storage: Default::default(),
        host: &host,
        environment: &environment,
        modules: v9::ModuleInput::NONE,
    })
    .unwrap();
    assert!(result.failed.is_empty(), "{:?}", result.failed);
    (document, result)
}
#[test]
fn native_text_png_and_published_pixels_are_the_same_bitmap() {
    let (document, result) = draw(
        "const c=document.getElementById('paint'),a=c.getContext('2d');a.fillStyle='red';a.fillRect(0,0,10,10);a.font='16px sans-serif';a.fillText('Canvas',12,28);console.log(a.measureText('Canvas').width);console.log(c.toDataURL().slice(0,22));",
    );
    assert!(result.effects.console[0].text.parse::<f64>().unwrap() > 0.0);
    assert_eq!(result.effects.console[1].text, "data:image/png;base64,");
    let applied = document::apply_script_changes(&document, &result.script_changes).unwrap();
    let bitmap = applied
        .nodes
        .iter()
        .find_map(|node| node.canvas.as_ref())
        .unwrap();
    assert_eq!(&bitmap.pixels[..4], &[255, 0, 0, 255]);
    assert!(
        bitmap
            .pixels
            .chunks_exact(4)
            .skip(100 * 12)
            .any(|p| p[3] > 0),
        "actual glyph ink"
    );
    let png = mars_eye::platform::NativeCanvas.png(bitmap).unwrap();
    let mut decoder = png::Decoder::new(png.as_slice()).read_info().unwrap();
    let mut decoded = vec![0; decoder.output_buffer_size()];
    decoder.next_frame(&mut decoded).unwrap();
    assert_eq!(decoded.as_slice(), &*bitmap.pixels);
    let display = document::render(&applied, 600.0).unwrap();
    assert!(
        display
            .items
            .iter()
            .any(|item| item.canvas.as_ref() == Some(bitmap))
    );
}
#[test]
fn publication_rejects_non_canvas_targets_without_mutating_the_document() {
    let document = document::parse("<div id='target'>preserved</div>").unwrap();
    let node = document.nodes[0].local_id;
    let changes = [mars_core::dom::ScriptChange::Canvas {
        node,
        bitmap: mars_core::canvas::Bitmap::new(1, 1).unwrap(),
    }];
    assert!(document::apply_script_changes(&document, &changes).is_err());
    assert!(document.nodes.iter().all(|n| n.canvas.is_none()));
}
#[test]
fn canvas_candidate_writes_do_not_change_committed_pixels() {
    let mut original = mars_core::canvas::Bitmap::new(1, 1).unwrap();
    original
        .pixels
        .make_mut()
        .unwrap()
        .copy_from_slice(&[255, 0, 0, 255]);
    let mut candidate = original.clone();
    candidate.pixels.make_mut().unwrap().fill(0);
    assert_eq!(&*original.pixels, &[255, 0, 0, 255]);
}

#[test]
fn content_dimensions_match_reflection_backing_and_layout() {
    for (value, expected) in [
        ("4junk", 4),
        (" +7tail", 7),
        ("invalid", 300),
        ("-1", 300),
        ("2147483648", 300),
    ] {
        let source = format!(
            "const c=document.getElementById('paint');c.setAttribute('width','{value}');console.log(c.width);c.getContext('2d').fillRect(0,0,1,1);"
        );
        let (document, result) = draw(&source);
        assert_eq!(result.effects.console[0].text, expected.to_string());
        let applied = document::apply_script_changes(&document, &result.script_changes).unwrap();
        let display = document::render(&applied, 600.0).unwrap();
        let item = display
            .items
            .iter()
            .find(|item| item.canvas.is_some())
            .unwrap();
        assert_eq!(item.canvas.as_ref().unwrap().width, expected);
        assert_eq!(item.width, f64::from(expected));
    }
}

#[test]
fn head_canvas_publishes_only_after_body_adoption() {
    let source = "const c=document.createElement('canvas');document.head.appendChild(c);c.getContext('2d').fillRect(0,0,1,1);window.onload=()=>document.getElementById('paint').parentNode.appendChild(c);";
    let (document, loaded) = draw(source);
    assert!(
        !loaded
            .script_changes
            .iter()
            .any(|change| matches!(change, mars_core::dom::ScriptChange::Canvas { .. }))
    );
    let document = document::apply_script_changes(&document, &loaded.script_changes).unwrap();
    let host = v9::DocumentHost {
        imports: None,
        canvas: &mars_eye::platform::NativeCanvas,
        languages: &[],
        fragments: &document::DomFragments,
    };
    let environment = v9::PageEnvironment {
        dom_level: v9::DomLevel::Extended,
        ..Default::default()
    };
    let resumed = v9::page_event(
        v9::PageInput {
            sources: &[source.into()],
            source_keys: &[],
            document: document::script_document(&document, "canvas-test").unwrap(),
            storage: Default::default(),
            host: &host,
            environment: &environment,
            modules: v9::ModuleInput::NONE,
        },
        loaded.state,
        v9::PageEvent::Load,
    )
    .unwrap();
    assert!(resumed.failed.is_empty(), "{:?}", resumed.failed);
    let applied = document::apply_script_changes(&document, &resumed.script_changes).unwrap();
    let bitmap = applied
        .nodes
        .iter()
        .find_map(|n| n.canvas.as_ref())
        .unwrap();
    assert_eq!(&bitmap.pixels[..4], &[0, 0, 0, 255]);
}
