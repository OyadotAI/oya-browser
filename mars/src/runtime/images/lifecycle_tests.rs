/** A real bounded PNG decode drives native V9 completion without sockets, filesystem pages, or a browser. */
use super::*;
use crate::v9::{DocumentHost, ImageResponse, PageEnvironment, PageInput, StorageState};

/** Encode a valid image in memory so the test exercises actual PNG validation and pixel decoding. */
fn image_data() -> String {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, 2, 1);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&[0, 0, 0, 0, 0, 0, 0, 0])
        .unwrap();
    format!("data:image/png;base64,{}", base64(&bytes))
}

/** Run the actual native image decoder and deliver its result through V9's public host boundary. */
fn decoded_events(source: &str) -> Vec<String> {
    let sources = vec![format!(
        "const i=new Image();i.onload=()=>console.log('load:'+i.naturalWidth+'x'+i.naturalHeight);i.onerror=()=>console.log('error:'+i.complete);i.src={};console.log('pending:'+i.complete);",
        serde_json::to_string(source).unwrap()
    )];
    let host = DocumentHost {
        languages: &[],
        fragments: &crate::v9::SimpleFragmentParser,
    };
    let environment = PageEnvironment::default();
    let input = || PageInput {
        sources: &sources,
        document: crate::document::ScriptDocument {
            identity: "decoded-image".into(),
            nodes: Vec::new(),
        },
        storage: StorageState::default(),
        host: &host,
        environment: &environment,
        modules: crate::v9::ModuleInput::NONE,
    };
    let loaded = crate::v9::load_page(input()).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", loaded.failed);
    let request = loaded.effects.image_requests[0].clone();
    let dimensions = data_url(&request.url).ok().map(|decoded| match decoded {
        Fetched::Raster(_, size) | Fetched::Vector(_, size) => size,
    });
    let mut events: Vec<_> = loaded
        .effects
        .console
        .into_iter()
        .map(|entry| entry.text)
        .collect();
    let finished = crate::v9::resume_images(
        input(),
        loaded.state,
        vec![ImageResponse {
            request,
            dimensions,
        }],
    )
    .unwrap();
    events.extend(finished.effects.console.into_iter().map(|entry| entry.text));
    events
}

#[test]
fn actual_png_decode_completes_a_pending_image() {
    assert_eq!(decoded_events(&image_data()), ["pending:false", "load:2x1"]);
}

#[test]
fn invalid_png_bytes_produce_error_instead_of_a_successful_load() {
    assert_eq!(
        decoded_events("data:image/png;base64,AAAA"),
        ["pending:false", "error:true"]
    );
}

#[test]
fn lifecycle_source_selection_ignores_paint_only_lazy_attributes() {
    let document =
        crate::document::parse("<img src='data:image/png;base64,AAAA' data-src='lazy.png'>")
            .unwrap();
    let image = document
        .nodes
        .iter()
        .find(|node| node.tag == "img")
        .unwrap();
    assert_eq!(
        selected_source(&document, image, 800.0),
        Some(("data:image/png;base64,AAAA".into(), 1.0))
    );
    assert_eq!(sources(&document, image, 800.0), [("lazy.png".into(), 1.0)]);
}

#[test]
fn lifecycle_picture_source_uses_only_actual_srcset() {
    let document = crate::document::parse("<picture><source data-srcset='lazy.png 2x'><source srcset='chosen.png 2x'><img src='fallback.png'></picture>").unwrap();
    let image = document
        .nodes
        .iter()
        .find(|node| node.tag == "img")
        .unwrap();
    assert_eq!(
        selected_source(&document, image, 800.0),
        Some(("chosen.png".into(), 2.0))
    );
}
