/** Custom property names keep their case through the CSSOM, since CSS compares them case-sensitively. */
use crate::common;

#[test]
fn custom_property_names_keep_their_case_in_the_style_attribute() {
    let source = "const b=document.body;b.style.setProperty('--AirImage-height','100%');b.style.setProperty('COLOR','red');const v=b.style.getPropertyValue('--AirImage-height');const c=b.style.getPropertyValue('color');const a=b.getAttribute('style');";
    let mut environment = common::environment();
    environment.dom_level = mars_v9::DomLevel::Extended;
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "v"), "100%");
    assert_eq!(common::global(&execution, "c"), "red");
    assert!(
        common::global(&execution, "a").contains("--AirImage-height: 100%"),
        "{}",
        common::global(&execution, "a")
    );
}
