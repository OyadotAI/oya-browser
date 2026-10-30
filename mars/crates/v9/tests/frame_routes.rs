/** Native route hints must stay bounded and name projected iframe owners, without ambient authority. */
mod common;
use mars_v9::PageEnvironment;

#[test]
fn old_environments_default_to_no_scripted_frame_owners() {
    let mut value = serde_json::to_value(PageEnvironment::default()).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .remove("scripted_frame_owners");
    assert!(
        serde_json::from_value::<PageEnvironment>(value)
            .unwrap()
            .scripted_frame_owners
            .is_empty()
    );
}

#[test]
fn non_frame_route_owners_are_rejected() {
    let mut environment = common::environment();
    environment.scripted_frame_owners.insert(0);
    let host = common::host();
    let result = mars_v9::load_page(common::input(&[], &host, &environment));
    assert_eq!(
        result.err().unwrap().message,
        "Invalid scripted frame owner."
    );
}

#[test]
fn oversized_route_sets_are_rejected_before_execution() {
    let mut environment = common::environment();
    environment.scripted_frame_owners = (0..=mars_v9::constants::NODES).collect();
    let host = common::host();
    let error = mars_v9::load_page(common::input(&[], &host, &environment))
        .err()
        .unwrap();
    assert_eq!(error.code, "limit_exceeded");
}
