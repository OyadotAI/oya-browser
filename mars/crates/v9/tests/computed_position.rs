/** `getComputedStyle(el).position` answers the app's cascade, not only inline declarations. */
mod common;

/** The cascaded position the app supplies is what script reads; an inline declaration set since
wins, and an element the cascade left static reads `static` (GitHub sizes its fixed header only
when its wrapper computes to `fixed`). */
#[test]
fn computed_position_comes_from_the_cascade() {
    let mut environment = common::environment();
    environment.dom_level = mars_v9::DomLevel::Extended;
    environment.positions.insert(0, "fixed".into());
    let execution = common::load_with(
        "const main = document.getElementById('main'), first = document.getElementById('first');
         const go = document.getElementById('go'); go.style.position = 'sticky';
         const r = [getComputedStyle(main).position, getComputedStyle(main).getPropertyValue('position'),
           getComputedStyle(first).position, getComputedStyle(go).position].join('|');",
        &environment,
    );
    assert_eq!(common::failure(&execution), None);
    assert_eq!(common::global(&execution, "r"), "fixed|fixed|static|sticky");
}
