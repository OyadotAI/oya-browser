/* Shell work-budget overrides are explicit, bounded and separate from production defaults. */
#![cfg(feature = "conformance")]
use crate::common;

/** The shell rejects overrides outside its explicitly bounded range. */
#[test]
fn shell_operation_budget_rejects_out_of_range_values() {
    let sources = ["let r=1".to_string()];
    let host = common::host();
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    for limit in [1, usize::MAX] {
        environment.conformance_operation_limit = Some(limit);
        assert!(mars_v9::load_page(common::input(&sources, &host, &environment)).is_err());
    }
    for limit in [400_000_000, 4_000_000_000] {
        environment.conformance_operation_limit = Some(limit);
        assert!(mars_v9::load_page(common::input(&sources, &host, &environment)).is_ok());
    }
}
