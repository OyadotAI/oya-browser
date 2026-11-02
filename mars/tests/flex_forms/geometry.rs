/** Resize, native validation and failing candidates preserve the same usable form document. */
use super::*;

/** Agents discover the supported layout slice and its real work limits without a full-CSS claim. */
#[test]
fn desktop_capabilities_disclose_the_flex_subset_and_layout_budgets() {
    let fixture = fixture("<p>Capability fixture</p>");
    let mut runtime = runtime(&fixture.url);
    let capabilities = call(&mut runtime, "runtime.capabilities", json!({})).unwrap();
    assert_eq!(
        capabilities["subsets"]["css"],
        "Owned row/column flex layout with row wrapping, pixel gaps, bounded growth/shrinkage and alignment; explicit content-box and border-box sizing; bounded font shorthand, line-height, vh lengths, block auto margins, HTML/BODY formatting and light/dark canvas colors."
    );
    assert_eq!(
        capabilities["limits"]["layout_work_budget"],
        mars::constants::LAYOUT_WORK
    );
    assert_eq!(
        capabilities["limits"]["layout_extent"],
        mars::constants::LAYOUT_EXTENT
    );
    assert!(
        capabilities["unavailable"]
            .as_array()
            .unwrap()
            .contains(&json!("full_css"))
    );
}

/** The shipped form switches from two columns to one without changing control handles or values. */
#[test]
fn resize_preserves_identity_state_and_native_bounds() {
    let fixture = fixture(&forms());
    let mut runtime = runtime(&fixture.url);
    runtime.set_viewport(1100.0, 700.0).unwrap();
    let mut page = call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    for (id, text) in [("user", "Ada"), ("pass", "private-flex-password")] {
        let mut params = target(&page, id);
        params["text"] = json!(text);
        call(&mut runtime, "page.type", params).unwrap();
        page = call(&mut runtime, "page.inspect", json!({})).unwrap();
    }
    close(
        node(&page, "user")["bounds"]["width"].as_f64().unwrap(),
        289.0,
    );
    close(
        node(&page, "pass")["bounds"]["x"].as_f64().unwrap()
            - node(&page, "user")["bounds"]["x"].as_f64().unwrap(),
        305.0,
    );
    assert_native_bounds(&runtime, &page);
    // Chromium's 8px body margin leaves room for two 280px fields at 640; 560 forces one column.
    runtime.set_viewport(560.0, 700.0).unwrap();
    let narrow = call(&mut runtime, "page.inspect", json!({})).unwrap();
    assert_eq!(narrow["page_id"], page["page_id"]);
    assert_eq!(narrow["revision"], page["revision"]);
    assert!(
        narrow["layout_generation"].as_u64().unwrap() > page["layout_generation"].as_u64().unwrap()
    );
    for id in ["user", "pass", "region", "notes"] {
        assert_eq!(node(&narrow, id)["node_id"], node(&page, id)["node_id"]);
    }
    close(
        node(&narrow, "user")["bounds"]["width"].as_f64().unwrap(),
        518.0,
    );
    close(
        node(&narrow, "pass")["bounds"]["x"].as_f64().unwrap(),
        node(&narrow, "user")["bounds"]["x"].as_f64().unwrap(),
    );
    assert!(
        node(&narrow, "pass")["bounds"]["y"].as_f64().unwrap()
            > node(&narrow, "user")["bounds"]["y"].as_f64().unwrap()
    );
    assert_eq!(node(&narrow, "user")["value"], "Ada");
    assert!(node(&narrow, "pass").get("value").is_none());
    assert!(!narrow.to_string().contains("private-flex-password"));
    assert!(!narrow.to_string().contains("synthetic-demo-token"));
    assert_native_bounds(&runtime, &narrow);
}

/** Required errors extend the affected flex item and survive resize without overlapping the next row. */
#[test]
fn required_validation_remains_associated_and_non_overlapping_after_resize() {
    let fixture = fixture(&forms());
    let mut runtime = runtime(&fixture.url);
    runtime.set_viewport(1100.0, 700.0).unwrap();
    let page = call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    let error = call(&mut runtime, "page.click", target(&page, "submit")).unwrap_err();
    assert_eq!(error.code, "validation_error");
    let user_local = node(&page, "user")["node_id"]
        .as_str()
        .unwrap()
        .rsplit('-')
        .next()
        .unwrap()
        .parse::<usize>()
        .unwrap();
    assert_eq!(error.invalid_node.as_deref(), Some(&user_local));
    // Chromium's 8px body margin leaves room for two 280px fields at 640; 560 forces one column.
    runtime.set_viewport(560.0, 700.0).unwrap();
    let snapshot = runtime.snapshot();
    let page = snapshot.inspection;
    let display = snapshot.display.unwrap();
    let user_id = node(&page, "user")["node_id"]
        .as_str()
        .unwrap()
        .rsplit('-')
        .next()
        .unwrap()
        .parse::<usize>()
        .unwrap();
    let user = display
        .items
        .iter()
        .find(|item| item.local_id == user_id)
        .unwrap();
    assert!(!user.validation.is_empty());
    assert!(user.height > user.control_height.unwrap());
    let next_y = node(&page, "pass")["bounds"]["y"].as_f64().unwrap();
    assert!(next_y >= user.y + user.height);
    assert_eq!(
        fixture.requests.load(std::sync::atomic::Ordering::Relaxed),
        1
    );
    assert_native_bounds(&runtime, &page);
}

/** Unsupported layout degrades the candidate instead of refusing it; a percentage gap is
approximated as zero without a diagnostic, like the other approximated flex values. */
#[test]
fn unsupported_css_and_layout_commit_best_effort() {
    for body in [
        "<div style='display:flex;gap:20%'><p>candidate</p></div>",
        "<div style='display:flex'>mixed<p>candidate</p></div>",
    ] {
        let page_fixture = fixture(body);
        let mut runtime = runtime(&page_fixture.url);
        let page = call(
            &mut runtime,
            "page.navigate",
            json!({"url":page_fixture.url}),
        )
        .unwrap();
        assert!(page.to_string().contains("candidate"), "{body}");
    }
    let gap = fixture("<div style='display:flex;gap:20%'><p>candidate</p></div>");
    let mut runtime = runtime(&gap.url);
    let page = call(&mut runtime, "page.navigate", json!({"url":gap.url})).unwrap();
    assert_eq!(page["diagnostics"], json!([]));
    assert_eq!(page["degraded"], json!([]));
    let candidate = page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["text"] == "candidate")
        .unwrap();
    assert!(candidate["bounds"]["width"].as_f64().unwrap() > 0.0);
}
