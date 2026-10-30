/** Shared parser regressions separate grammar recovery from bounded numeric and resource refusal. */
use mars_core::canvas::Path;
use mars_core::svg_path::{self, Segment, parse, parse_prefix};

/** Every absolute and relative command reaches the shared normalized representation. */
#[test]
fn all_commands_and_implicit_move_repetitions_normalize() {
    let source = "m1 1 2 0 h2 v2 l-2 0 c0 1 1 1 1 0 s1-1 1 0 q1 1 2 0 t2 0 a1 1 0 011 1 z";
    let parsed = parse(source).unwrap();
    assert_eq!(parsed[0], Segment::Move(1.0, 1.0));
    assert_eq!(parsed[1], Segment::Line(3.0, 1.0));
    assert_eq!(parsed[2], Segment::Line(5.0, 1.0));
    assert_eq!(parsed.last(), Some(&Segment::Close));
    assert!(
        parsed[5..parsed.len() - 1]
            .iter()
            .all(|segment| matches!(segment, Segment::Cubic(_)))
    );
    assert!(parse(&source.to_ascii_uppercase()).is_ok());
}

/** Compact signs, decimals and exponents are token boundaries rather than required whitespace. */
#[test]
fn compact_numbers_and_arc_flags_parse_without_separators() {
    assert_eq!(
        parse("M.6.3L1e1-2").unwrap(),
        vec![Segment::Move(0.6, 0.3), Segment::Line(10.0, -2.0)]
    );
    let arc = parse("M0 0A1 1 0 011 1").unwrap();
    assert_eq!(arc.len(), 2);
    assert!(matches!(arc[1], Segment::Cubic(_)));
}

/** Canvas retains complete preceding groups while Eye rejects malformed data as a whole. */
#[test]
fn malformed_data_has_explicit_strict_and_prefix_policies() {
    for suffix in ["C1 2 3", "L3", "X1 2", ",,3 4", "L,3 4"] {
        let source = format!("M1 2L3 4{suffix}");
        assert!(parse(&source).is_err(), "{suffix}");
        assert_eq!(
            parse_prefix(&source).unwrap(),
            vec![Segment::Move(1.0, 2.0), Segment::Line(3.0, 4.0)],
            "{suffix}"
        );
    }
    assert!(parse_prefix("L1 2").unwrap().is_empty());
    assert!(parse_prefix("").unwrap().is_empty());
}

/** Numeric tokens after close cannot repeat a command that consumes no arguments. */
#[test]
fn numbers_after_close_stop_at_the_completed_close() {
    assert_eq!(
        parse_prefix("M1 2Z3 4").unwrap(),
        vec![Segment::Move(1.0, 2.0), Segment::Close]
    );
    assert!(parse("M1 2Z3 4").is_err());
}

/** Recovery cannot turn valid but unrepresentable geometry into a silently shortened successful path. */
#[test]
fn numeric_failures_propagate_in_recovery_mode() {
    for source in [
        "M0 0L1e999 0",
        "M1e308 0l1e308 0",
        "M0 0A1e200 1e200 0 0 1 1 1",
    ] {
        assert_eq!(parse_prefix(source).unwrap_err().code, "canvas_geometry");
    }
}

/** Independent scan, command and emitted-segment bounds apply even when arcs produce no segments. */
#[test]
fn source_commands_and_segments_each_have_a_bound() {
    let excessive_source = " ".repeat(svg_path::SOURCE_BYTES + 1);
    let excessive_commands = format!("M0 0{}", "A1 1 0 0 0 0 0".repeat(4096));
    let excessive_segments = format!(
        "M0 0{}",
        "Z".repeat(mars_core::constants::SVG_PATH_SEGMENTS)
    );
    for source in [&excessive_source, &excessive_commands, &excessive_segments] {
        assert!(
            parse_prefix(source)
                .unwrap_err()
                .message
                .starts_with("QuotaExceededError:")
        );
    }
}

/** SVG construction charges its trailing current-point subpath against the ordinary Canvas point budget. */
#[test]
fn canvas_svg_bridge_adds_the_final_subpath_and_enforces_canvas_bounds() {
    let path = Path::from_svg("M1 2L3 4").unwrap();
    assert_eq!(path.point_count(), 3);
    let full = format!("M0 0{}", "L1 1".repeat(1023));
    assert!(
        Path::from_svg(&full)
            .unwrap_err()
            .message
            .starts_with("QuotaExceededError:")
    );
    assert_eq!(
        Path::from_svg("M1e13 0").unwrap_err().code,
        "canvas_geometry"
    );
}
