/** Canvas font setter: the CSS font shorthand parses fully, invalid values keep the old font, and the getter reports canonical text. */
use crate::common;
use mars_core::canvas::CanvasHost;
use mars_v9::{DocumentHost, PageInput, StorageState};

/** Pixel-only host: font parsing is independent of glyph rasterization. */
struct Pixels;
impl CanvasHost for Pixels {
    fn available(&self) -> bool {
        true
    }
}

/** Execute a script with a fresh canvas context bound to `a`; the result is the global `r`. */
fn run(source: &str) -> String {
    let host = DocumentHost {
        imports: None,
        canvas: &Pixels,
        ..common::host()
    };
    let environment = common::environment();
    let setup = "const c=document.createElement('canvas');const a=c.getContext('2d');";
    let execution = mars_v9::load_page(PageInput {
        sources: &[format!("{setup}{source}")],
        source_keys: &[],
        document: common::page(),
        storage: StorageState::default(),
        host: &host,
        environment: &environment,
        modules: mars_v9::ModuleInput::NONE,
    })
    .unwrap();
    assert_eq!(common::failure(&execution), None);
    common::global(&execution, "r")
}

/** The initial font is the canvas default, reported in canonical form. */
#[test]
fn default_font_reads_back_as_ten_pixel_sans_serif() {
    assert_eq!(run("const r=a.font;"), "10px sans-serif");
}

/** A size and family round-trip exactly as written. */
#[test]
fn size_and_family_round_trip() {
    assert_eq!(run("a.font='11px Arial';const r=a.font;"), "11px Arial");
}

/** Weight and style keywords are reported in the shorthand's order, with bold spelled as the keyword. */
#[test]
fn bold_weight_round_trips_as_its_keyword() {
    assert_eq!(
        run("a.font='bold 11px Arial';const r=a.font;"),
        "bold 11px Arial"
    );
}

/** Numeric weights other than normal and bold stay numeric. */
#[test]
fn numeric_weight_is_reported_as_a_number() {
    assert_eq!(
        run("a.font='600 11px Arial';const r=a.font;"),
        "600 11px Arial"
    );
}

/** Every prefix part is accepted and reported in canonical order, with the family list quoted where needed. */
#[test]
fn full_shorthand_with_quoted_family_and_generic_canonicalizes() {
    assert_eq!(
        run(
            "a.font='italic small-caps 700 condensed 11px \"Times New Roman\", serif';const r=a.font;"
        ),
        "italic small-caps bold condensed 11px \"Times New Roman\", serif"
    );
}

/** A point size converts to CSS pixels, reading back with at most four decimals. */
#[test]
fn point_size_converts_to_pixels() {
    assert_eq!(
        run("a.font='11pt Arial';const r=a.font;"),
        "14.6667px Arial"
    );
}

/** Em and percentage sizes resolve against the canvas element's font size, which defaults to 16px. */
#[test]
fn em_and_percentage_sizes_resolve_against_the_element_font() {
    assert_eq!(
        run(
            "a.font='1em serif';const first=a.font;a.font='50% serif';const r=[first,a.font].join('|');"
        ),
        "16px serif|8px serif"
    );
}

/** The line height after a slash is validated and dropped from the reported value. */
#[test]
fn line_height_is_accepted_and_not_reported() {
    assert_eq!(run("a.font='11px/1.5 Arial';const r=a.font;"), "11px Arial");
}

/** Quoted single-word names are reported quoted only when they are not identifiers. */
#[test]
fn single_quoted_family_with_a_space_is_double_quoted_on_read() {
    assert_eq!(
        run("a.font=\"11px 'Courier New'\";const r=a.font;"),
        "11px \"Courier New\""
    );
}

/** Relative weight keywords step from the normal weight of the canvas. */
#[test]
fn relative_weights_step_from_normal() {
    assert_eq!(
        run(
            "a.font='bolder 11px Arial';const first=a.font;a.font='lighter 11px Arial';const r=[first,a.font].join('|');"
        ),
        "bold 11px Arial|100 11px Arial"
    );
}

/** Every malformed value is ignored and the previously set font survives it. */
#[test]
fn invalid_values_are_ignored_and_keep_the_previous_font() {
    assert_eq!(
        run(concat!(
            "a.font='12px serif';const seen=[];",
            "for(const v of ['bogus','11 Arial','bold 11px','11px \"Arial','0px Arial','inherit 11px Arial','11px Arial,','1e3px Arial','12px']){a.font=v;seen.push(a.font)}",
            "const r=seen.every(f=>f==='12px serif');",
        )),
        "true"
    );
}

/** A value that becomes invalid after a valid one is still ignored, not partially applied. */
#[test]
fn a_partly_valid_value_applies_nothing() {
    assert_eq!(
        run("a.font='11px Arial';a.font='bold 11px';const r=a.font;"),
        "11px Arial"
    );
}

/** Keywords and units are ASCII case-insensitive, and the canonical text is lowercase. */
#[test]
fn keywords_and_units_are_ascii_case_insensitive() {
    assert_eq!(
        run("a.font='ITALIC Bold 11PX Arial';const r=a.font;"),
        "italic bold 11px Arial"
    );
}
