/** Actual GIF/WebP sources pass through Canvas image response, immutable pixels, and origin-clean enforcement. */
use super::*;
use crate::v9::{DocumentHost, ImageResponse, PageEnvironment, PageInput, StorageState};
const GIF: &[u8] =
    include_bytes!("../../../../crates/eye/tests/fixtures/canvas-images/first-frame.gif");
const WEBP: &[u8] =
    include_bytes!("../../../../crates/eye/tests/fixtures/canvas-images/alpha.webp");

/** Image assets keep encoded ownership and origin provenance separate from their decoded bitmap. */
fn asset(mime: &str, bytes: &[u8], clean: bool) -> ImageAsset {
    ImageAsset {
        mime: mime.into(),
        bytes: std::sync::Arc::new(bytes.to_vec()),
        decoded: 16,
        image_source: super::super::ImageSelection::new(
            &url::Url::parse("https://site.test/").unwrap(),
            "/image",
            1.0,
            crate::v9::ImageCors::NoCors,
        ),
        origin_clean: clean,
    }
}

#[test]
fn real_decoded_image_sources_retain_origin_provenance_and_exact_page_admission() {
    for (mime, bytes) in [("image/gif", GIF), ("image/webp", WEBP)] {
        let asset = asset(mime, bytes, false);
        let mut used = IMAGE_SOURCE_BYTES - 16;
        let (bitmap, clean) = source(Some(&asset), &mut used);
        assert!(!clean);
        assert!(bitmap.is_some());
        assert_eq!(used, IMAGE_SOURCE_BYTES);
        assert_eq!(asset.bytes.as_slice(), bytes);
        let mut unavailable = IMAGE_SOURCE_BYTES - 15;
        assert!(source(Some(&asset), &mut unavailable).0.is_none());
        assert_eq!(unavailable, IMAGE_SOURCE_BYTES - 15);
    }
}

#[test]
fn dishonest_decoded_sizes_and_header_only_sources_never_consume_page_admission() {
    let mut invalid = asset("image/gif", GIF, true);
    invalid.decoded = 4;
    let mut used = 0;
    assert!(source(Some(&invalid), &mut used).0.is_none());
    assert_eq!(used, 0);
    invalid = asset("image/gif", &GIF[..13], false);
    assert_eq!(source(Some(&invalid), &mut used), (None, false));
    assert_eq!(used, 0);
}

/** Complete a genuine native decode through V9's same ImageResponse seam used by image loading. */
fn draw(mime: &str, bytes: &[u8], clean: bool) -> Vec<String> {
    let asset = asset(mime, bytes, clean);
    let (bitmap, origin_clean) = source(Some(&asset), &mut 0);
    assert!(bitmap.is_some());
    let sources = vec!["const c=document.createElement('canvas');c.width=2;c.height=2;const x=c.getContext('2d');const i=new Image();i.onload=()=>{x.drawImage(i,0,0);try{console.log(Array.from(x.getImageData(0,0,2,2).data).join())}catch(e){console.log(e.name)}};i.src='/image'".into()];
    let host = DocumentHost {
        imports: None,
        canvas: &crate::platform::NativeCanvas,
        languages: &[],
        fragments: &crate::v9::SimpleFragmentParser,
    };
    let environment = PageEnvironment {
        dom_level: crate::v9::DomLevel::Extended,
        ..PageEnvironment::default()
    };
    let input = || PageInput {
        sources: &sources,
        source_keys: &[],
        document: crate::document::ScriptDocument {
            identity: "native-canvas-source".into(),
            charset: "UTF-8".into(),
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
    let result = crate::v9::resume_images(
        input(),
        loaded.state,
        vec![ImageResponse {
            request,
            dimensions: Some((2, 2)),
            bitmap,
            origin_clean,
            current_src: String::new(),
        }],
    )
    .unwrap();
    assert!(result.failed.is_empty(), "{:?}", result.failed);
    result
        .effects
        .console
        .into_iter()
        .map(|entry| entry.text)
        .collect()
}

#[test]
fn gif_and_webp_pixels_draw_through_the_actual_canvas_image_response() {
    assert_eq!(
        draw("image/gif", GIF, true),
        ["255,0,0,255,0,255,0,255,0,0,0,0,0,0,255,255"]
    );
    assert_eq!(
        draw("image/webp", WEBP, true),
        ["255,0,0,128,0,255,0,255,0,0,0,0,0,0,255,64"]
    );
}

#[test]
fn cross_origin_gif_and_webp_sources_taint_canvas_readback() {
    assert_eq!(draw("image/gif", GIF, false), ["SecurityError"]);
    assert_eq!(draw("image/webp", WEBP, false), ["SecurityError"]);
}
