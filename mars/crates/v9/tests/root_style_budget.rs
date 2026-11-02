/** Growing root styles count their final wire state rather than every intermediate string. */
use crate::common;

#[test]
fn repeated_root_css_variables_coalesce_within_the_existing_budget() {
    let source = "const root=document.documentElement;for(let i=0;i<400;i++)root.style.setProperty('--theme-'+i,'x'.repeat(100));const r=root.style.getPropertyValue('--theme-399').length;";
    let mut environment = common::environment();
    environment.dom_level = mars_v9::DomLevel::Extended;
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "100");
    let writes = execution.script_changes.iter().filter(|change| matches!(change, mars_v9::document::ScriptChange::RootAttribute { name, .. } if name == "style")).count();
    assert_eq!(writes, 1);
}
