/** Actual screenshot pixels establish that validation text survives the native repaint pass. */
use super::*;

/** Anti-aliased error text must contain several strongly red pixels inside its own reserved region. */
const MIN_INK_PIXELS: usize = 4;
/** Native system red retains a high red channel in both light and dark appearances. */
const MIN_RED_CHANNEL: u8 = 120;
/** Neutral backgrounds and borders must not be mistaken for validation ink. */
const RED_DOMINANCE: u8 = 40;

/** The crop uses current root-view geometry and PNG scaling, never a hard-coded toolbar offset. */
pub(super) fn assert_ink(window: &mut Window, capture: &Value) {
    let state = window.call("window.inspect", json!({}));
    let file = std::fs::File::open(capture["path"].as_str().unwrap()).unwrap();
    let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().unwrap();
    let mut bytes = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut bytes).unwrap();
    let scale = (
        info.width as f64 / state["width"].as_f64().unwrap(),
        info.height as f64 / state["height"].as_f64().unwrap(),
    );
    let regions = state["native_validation"].as_array().unwrap();
    assert!(!regions.is_empty());
    for region in regions {
        assert!(
            ink_count(&bytes, &info, region, scale) >= MIN_INK_PIXELS,
            "validation ink missing in {region}"
        );
    }
}

/** Only pixels within the validation line itself can satisfy the regression assertion. */
fn ink_count(bytes: &[u8], info: &png::OutputInfo, region: &Value, scale: (f64, f64)) -> usize {
    let channels = info.color_type.samples();
    assert!(channels >= 3);
    let left = (region["x"].as_f64().unwrap() * scale.0).floor().max(0.0) as usize;
    let top = (region["y"].as_f64().unwrap() * scale.1).floor().max(0.0) as usize;
    let right = ((region["x"].as_f64().unwrap() + region["width"].as_f64().unwrap()) * scale.0)
        .ceil()
        .clamp(0.0, info.width as f64) as usize;
    let bottom = ((region["y"].as_f64().unwrap() + region["height"].as_f64().unwrap()) * scale.1)
        .ceil()
        .clamp(0.0, info.height as f64) as usize;
    (top..bottom)
        .flat_map(|y| (left..right).map(move |x| (y * info.width as usize + x) * channels))
        .filter(|offset| red(&bytes[*offset..*offset + channels]))
        .count()
}

/** Saturated red excludes gray widgets, black text and the surrounding panel border. */
fn red(pixel: &[u8]) -> bool {
    pixel[0] >= MIN_RED_CHANNEL
        && pixel[0] > pixel[1].saturating_add(RED_DOMINANCE)
        && pixel[0] > pixel[2].saturating_add(RED_DOMINANCE)
}
