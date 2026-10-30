/** Actual PNG decode, creator policy and V9 drawImage consume one captured image without network work. */
use super::*;
use crate::v9::{DocumentHost, ImageResponse, PageEnvironment, PageInput, StorageState};

/** A complete two-pixel PNG makes exact raster output observable through Canvas readback. */
fn png_bytes() -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, 2, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&[255, 0, 0, 255, 0, 255, 0, 255])
            .unwrap();
    }
    bytes
}

/** Direct source fixture retains the canonical URL/creator binding used by real captures. */
fn captured(bytes: Vec<u8>) -> (String, BlobUrlSource) {
    (
        "blob:https://example.test/12345678-1234-4123-8123-123456789abc".into(),
        BlobUrlSource {
            bytes: std::sync::Arc::new(bytes),
            mime_type: "application/octet-stream".into(),
            origin: "https://example.test".into(),
        },
    )
}

#[test]
fn png_capture_checks_origin_and_all_image_policies_before_returning_clean_pixels() {
    let base = Url::parse("https://example.test/page").unwrap();
    let (address, source) = captured(png_bytes());
    let (fetched, bitmap) = decode(&base, &["img-src blob:".into()], &address, &source).unwrap();
    assert_eq!(&*bitmap.pixels, &[255, 0, 0, 255, 0, 255, 0, 255]);
    assert!(matches!(fetched, Fetched::Raster(asset, (2,1)) if asset.origin_clean));
    assert!(
        decode(
            &base,
            &["img-src blob:".into(), "default-src 'none'".into()],
            &address,
            &source
        )
        .is_none()
    );
    assert!(
        decode(
            &Url::parse("https://other.test/").unwrap(),
            &[],
            &address,
            &source
        )
        .is_none()
    );
    let (address, header_only) = captured(b"GIF89a\x02\0\x01\0".to_vec());
    assert!(decode(&base, &[], &address, &header_only).is_none());
    let (address, svg) =
        captured(b"<svg xmlns='http://www.w3.org/2000/svg' width='2' height='1'/>".to_vec());
    assert!(decode(&base, &[], &address, &svg).is_none());
}

#[test]
fn admitted_then_revoked_blob_pixels_reach_draw_image_and_clean_readback() {
    let bytes = serde_json::to_string(&png_bytes()).unwrap();
    let sources = vec![format!(
        "const url=URL.createObjectURL(new Blob([new Uint8Array({bytes})]));const img=new Image();img.onload=()=>{{const c=document.createElement('canvas');c.width=2;c.height=1;const x=c.getContext('2d');x.drawImage(img,0,0);console.log(Array.from(x.getImageData(0,0,2,1).data).join(','));}};img.src=url;setTimeout(()=>URL.revokeObjectURL(url),1);"
    )];
    let host = DocumentHost {
        imports: None,
        canvas: &mars_eye::platform::NativeCanvas,
        languages: &[],
        fragments: &crate::v9::SimpleFragmentParser,
    };
    let environment = PageEnvironment {
        url: "https://example.test/page".into(),
        ..PageEnvironment::default()
    };
    let input = || PageInput {
        sources: &sources,
        source_keys: &[],
        document: crate::document::ScriptDocument {
            identity: "blob-image".into(),
            charset: "UTF-8".into(),
            nodes: Vec::new(),
        },
        storage: StorageState::default(),
        host: &host,
        environment: &environment,
        modules: crate::v9::ModuleInput::NONE,
    };
    let loaded = crate::v9::load_page(input()).unwrap();
    assert!(loaded.failed.is_empty());
    let mut request = loaded.effects.image_requests[0].clone();
    let revoked = crate::v9::run_timers(input(), loaded.state, 1.0, 32).unwrap();
    let (_, bitmap) = decode(
        &Url::parse(&environment.url).unwrap(),
        &[],
        &request.url,
        request.blob_source.as_ref().unwrap(),
    )
    .unwrap();
    request.blob_source = None;
    let completed = crate::v9::resume_images(
        input(),
        revoked.state,
        vec![ImageResponse {
            request,
            dimensions: Some((2, 1)),
            bitmap: Some(bitmap),
            origin_clean: true,
        }],
    )
    .unwrap();
    assert!(completed.failed.is_empty(), "{:?}", completed.failed);
    assert_eq!(
        completed
            .effects
            .console
            .iter()
            .map(|entry| entry.text.as_str())
            .collect::<Vec<_>>(),
        ["255,0,0,255,0,255,0,255"]
    );
}
