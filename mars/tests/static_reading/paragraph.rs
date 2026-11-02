/** The retained long paragraph stays one node through full runtime publication and native reflow. */
use super::*;

/** The complete frozen HTTP response body commits; inspection and painted lines preserve its complete text. */
#[test]
fn unchanged_httpbin_commits_complete_paragraph_and_scrolls_without_new_node_ids() {
    let fixture = fixture(include_str!("../../fixtures/reading/httpbin.html"));
    let mut runtime = runtime(&[&fixture.url]);
    runtime
        .set_render_context(RenderContext {
            width: 640.0,
            height: 240.0,
            appearance: Appearance::Light,
        })
        .unwrap();
    let page = call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    let paragraphs = page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|node| node["tag"] == "p")
        .collect::<Vec<_>>();
    assert_eq!(paragraphs.len(), 1);
    let paragraph = paragraphs[0];
    let text = paragraph["text"].as_str().unwrap();
    assert_eq!(text.len(), 3588);
    let parsed =
        mars::document::parse(include_str!("../../fixtures/reading/httpbin.html")).unwrap();
    assert_eq!(
        text,
        parsed
            .nodes
            .iter()
            .find(|node| node.tag == "p")
            .unwrap()
            .text
    );
    assert!(
        text.trim()
            .starts_with("Availing himself of the mild, summer-cool weather")
    );
    assert!(text.contains("It was the Bottle Conjuror!"));
    assert!(
        text.trim()
            .ends_with("and all of them a care-killing competency.")
    );
    let initial_layout = call(&mut runtime, "page.layout", json!({})).unwrap();
    let painted = initial_layout["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["node_id"] == paragraph["node_id"])
        .unwrap();
    // Lines join with a space, except after a hyphen, where CSS may break inside a word.
    let painted_text = painted["lines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|line| line["text"].as_str().unwrap())
        .fold(String::new(), |mut text, line| {
            if !text.is_empty() && !text.ends_with('-') {
                text.push(' ');
            }
            text.push_str(line);
            text
        });
    assert_eq!(
        painted_text,
        text.split_whitespace().collect::<Vec<_>>().join(" ")
    );
    assert!(painted["lines"].as_array().unwrap().len() > 10);
    let extent = initial_layout["height"].as_f64().unwrap();
    for y in [extent / 2.0, extent] {
        let inspected = call(
            &mut runtime,
            "page.scroll",
            json!({"page_id":page["page_id"],"expected_revision":page["revision"],"y":y}),
        )
        .unwrap();
        assert_eq!(inspected["page_id"], page["page_id"]);
        assert_eq!(inspected["revision"], page["revision"]);
        assert!(
            inspected["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|node| node["node_id"] == paragraph["node_id"] && node["text"] == text)
        );
    }
    runtime.set_viewport(320.0, 240.0).unwrap();
    let narrow = call(&mut runtime, "page.inspect", json!({})).unwrap();
    let narrow_paragraph = narrow["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["tag"] == "p")
        .unwrap();
    assert_eq!(narrow_paragraph["node_id"], paragraph["node_id"]);
    assert_eq!(narrow_paragraph["text"], text);
    assert_eq!(fixture.requests.load(Ordering::Relaxed), 1);
}
