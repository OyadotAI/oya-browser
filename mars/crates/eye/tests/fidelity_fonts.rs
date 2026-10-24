/**
 * Web fonts: `@font-face` parsing, the files a page's text asks for (CSS Fonts 4 matching over
 * weight, style and `unicode-range`), and shaping with a loaded file through the per-process
 * registry. The registry test loads a font macOS ships, so it runs on the platform that paints.
 */
use mars_eye::{
    DisplayItem, DisplayList, Document, FontSource, FontStyle, LoadedFont, RenderContext,
    compute_styles_with_context, content_key, font_requests, parse, render_with_context,
};

/** The viewport the pages were compared at. */
fn viewport() -> RenderContext {
    RenderContext {
        width: 1100.0,
        height: 751.0,
        ..RenderContext::default()
    }
}

/** Styles `body` with `css` in the viewport. */
fn styled(body: &str, css: &str) -> Document {
    let document = parse(body).unwrap();
    compute_styles_with_context(&document, &[css.into()], viewport()).unwrap()
}

/** Lays a styled document out. */
fn laid(document: &Document) -> DisplayList {
    render_with_context(document, viewport()).unwrap()
}

/** The first item painted for the element with `id`. */
fn find<'a>(document: &Document, display: &'a DisplayList, id: &str) -> &'a DisplayItem {
    let local = document
        .nodes
        .iter()
        .find(|node| node.html_id == id)
        .unwrap()
        .local_id;
    display
        .items
        .iter()
        .find(|item| item.local_id == local)
        .unwrap()
}

/** Two faces of one family split by weight and by script subset, as Google Fonts serves them. */
const GOOGLE_STYLE: &str = "\
@font-face { font-family: 'Google Sans'; font-style: normal; font-weight: 500; font-display: swap; \
  src: url(https://fonts.example/gs-500-latin.woff2) format('woff2'); unicode-range: U+0000-00FF, U+2000-206F; }\
@font-face { font-family: 'Google Sans'; font-style: normal; font-weight: 500; \
  src: url(https://fonts.example/gs-500-cyrillic.woff2) format('woff2'); unicode-range: U+0400-045F; }\
@font-face { font-family: \"Google Sans\"; font-weight: 700; \
  src: url(https://fonts.example/gs-700-latin.woff2) format(\"woff2\"); unicode-range: U+0000-00FF; }\
@font-face { font-family: 'Google Sans'; font-style: italic; font-weight: 100 900; \
  src: url(https://fonts.example/gs-italic.woff2) format('woff2'); }\
@media (min-width: 5000px) { @font-face { font-family: 'Wide Only'; src: url(https://fonts.example/wide.woff2); } }\
@font-face { font-family: 'No Source'; font-weight: 400; }\
h1 { font-family: 'Google Sans', Roboto, sans-serif; font-weight: 700 } \
p { font-family: 'Google Sans', sans-serif; font-weight: 400 } \
em { font-style: italic }";

/** `@font-face` rules keep family, weight range, style, display and ranges; a rule under a
false media condition or without a source is dropped. */
#[test]
fn font_face_rules_parse_into_descriptors() {
    let document = styled("<h1>Hi</h1>", GOOGLE_STYLE);
    let faces = &document.formatting.font_faces;
    assert_eq!(faces.len(), 4, "{faces:?}");
    assert_eq!(faces[0].family, "google sans");
    assert_eq!(faces[0].weight, (500, 500));
    assert_eq!(faces[0].ranges, vec![(0x0000, 0x00FF), (0x2000, 0x206F)]);
    assert_eq!(
        faces[0].sources,
        vec![FontSource::Url {
            url: "https://fonts.example/gs-500-latin.woff2".into(),
            format: Some("woff2".into())
        }]
    );
    assert_eq!(faces[3].weight, (100, 900));
    assert_eq!(faces[3].style, FontStyle::Italic);
    assert!(faces[3].ranges.is_empty());
    assert!(faces[3].covers('я') && !faces[1].covers('a'));
}

/** The files requested are the first listed family's faces nearest each run's weight and style,
one per script subset the run's characters use; nothing is asked twice. */
#[test]
fn text_requests_the_faces_it_needs() {
    let document = styled(
        "<h1>Hello</h1><p>Привет <em>world</em></p><p>Again</p>",
        GOOGLE_STYLE,
    );
    let requests: Vec<(usize, String)> = font_requests(&document)
        .into_iter()
        .map(|request| (request.face, request.url))
        .collect();
    assert_eq!(
        requests,
        vec![
            (2, "https://fonts.example/gs-700-latin.woff2".to_owned()),
            (1, "https://fonts.example/gs-500-cyrillic.woff2".to_owned()),
            (3, "https://fonts.example/gs-italic.woff2".to_owned()),
            (0, "https://fonts.example/gs-500-latin.woff2".to_owned()),
        ]
    );
}

/** A face whose `local()` font is installed needs no file, and a face already loaded is not
asked for again. */
#[cfg(target_os = "macos")]
#[test]
fn installed_and_loaded_faces_are_not_requested() {
    let mut document = styled(
        "<p id=p>Local text</p>",
        "@font-face { font-family: Site; src: local('Helvetica'), url(https://fonts.example/site.woff2) } \
         @font-face { font-family: Later; src: url(https://fonts.example/later.woff2) } \
         p { font-family: Site, Later, sans-serif }",
    );
    assert!(font_requests(&document).is_empty());
    document.nodes[0].style.font_families = vec!["later".into()];
    assert_eq!(font_requests(&document).len(), 1);
    document.formatting.fonts.push(LoadedFont {
        face: 1,
        key: 7,
        data: String::new(),
    });
    assert!(font_requests(&document).is_empty());
}

/** Standard base64 with padding, as the app encodes font bytes. */
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let value = chunk
            .iter()
            .enumerate()
            .fold(0u32, |acc, (i, b)| acc | (u32::from(*b) << (16 - 8 * i)));
        for i in 0..4 {
            out.push(if i <= chunk.len() {
                ALPHABET[((value >> (18 - 6 * i)) & 63) as usize] as char
            } else {
                '='
            });
        }
    }
    out
}

/** A loaded file shapes the runs that name its family: the item carries the file's key, the
weight is clamped to the face's declaration, and the measured width is the file's, not the
fallback's. Fonts never reach the system registry: the family stays unknown by name. */
#[cfg(target_os = "macos")]
#[test]
fn loaded_file_shapes_text_through_the_registry() {
    let path = "/System/Library/Fonts/Supplemental/Times New Roman.ttf";
    let bytes = std::fs::read(path).unwrap();
    let css = "@font-face { font-family: 'Site Serif'; font-weight: 700; src: url(https://fonts.example/site.ttf) } \
               p { font-family: 'Site Serif', sans-serif; font-weight: 400; font-size: 32px }";
    let markup = "<p id=web>Measured in the page's font</p>";
    let fallback = styled(markup, css);
    let mut document = fallback.clone();
    document.formatting.fonts.push(LoadedFont {
        face: 0,
        key: content_key(&bytes),
        data: base64(&bytes),
    });
    let (plain, loaded) = (laid(&fallback), laid(&document));
    let (before, after) = (
        find(&fallback, &plain, "web"),
        find(&document, &loaded, "web"),
    );
    assert_eq!(before.font_family, mars_eye::platform::Family::SansSerif);
    assert_eq!(
        after.font_family,
        mars_eye::platform::Family::Web(content_key(&bytes))
    );
    assert_eq!(
        after.font_weight, 700,
        "clamped to the face's declared weight"
    );
    assert_ne!(before.lines[0].width, after.lines[0].width);
    assert!(!mars_eye::platform::local_font_exists("Site Serif"));
}

/** Bytes CoreText rejects leave the family on its fallback instead of failing the page. */
#[cfg(target_os = "macos")]
#[test]
fn rejected_font_bytes_fall_back() {
    let mut document = styled(
        "<p id=p>Text</p>",
        "@font-face { font-family: Broken; src: url(https://fonts.example/x.ttf) } p { font-family: Broken, serif }",
    );
    document.formatting.fonts.push(LoadedFont {
        face: 0,
        key: 42,
        data: base64(b"not a font at all"),
    });
    let display = laid(&document);
    assert_eq!(
        find(&document, &display, "p").font_family,
        mars_eye::platform::Family::Serif
    );
}
