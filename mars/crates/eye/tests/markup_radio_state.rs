/** Initial radio state is parser state, never an agent's click: disabled or hidden checked radios parse and keep their group's last-checked rule. */
use mars_eye::parse;

/** A checked radio inside a disabled fieldset, and a hidden one, no longer fail the whole document. */
#[test]
fn disabled_and_hidden_checked_radios_parse() {
    let html = "<form><fieldset disabled><input type=radio name=a checked id=x></fieldset>\
        <input type=radio name=b checked hidden id=y><input type=checkbox checked disabled id=z></form>";
    let document = parse(html).unwrap();
    let by_id = |id: &str| {
        document
            .nodes
            .iter()
            .find(|n| n.attributes.get("id").map(String::as_str) == Some(id))
            .unwrap()
    };
    assert!(by_id("x").checked && by_id("x").disabled);
    assert!(by_id("y").checked);
    assert!(by_id("z").checked);
}

/** Within one group the last checked radio wins; groups are split by name and by form owner. */
#[test]
fn last_checked_radio_wins_per_group() {
    let html = "<form id=f1><input type=radio name=g checked id=a><input type=radio name=g checked id=b>\
        <input type=radio name=h checked id=c></form><form id=f2><input type=radio name=g checked id=d></form>\
        <input type=radio checked id=e><input type=radio checked id=e2>";
    let document = parse(html).unwrap();
    let checked: Vec<&str> = document
        .nodes
        .iter()
        .filter(|n| n.checked)
        .filter_map(|n| n.attributes.get("id").map(String::as_str))
        .collect();
    assert_eq!(checked, ["b", "c", "d", "e", "e2"]);
}
