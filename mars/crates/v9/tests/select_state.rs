/** Current dropdown state remains distinct from default markup and exact duplicate values. */
mod common;

#[test]
fn value_index_and_option_selection_agree_without_rewriting_defaults() {
    let source = "const s=document.createElement('select');s.innerHTML='<option value=a selected>A</option><optgroup><option value=b disabled>B</option><option value=b>C</option></optgroup>';s.value='b';const out=[s.value,s.selectedIndex,s.options[0].hasAttribute('selected'),s.options[1].selected];s.selectedIndex=2;out.push(s.selectedIndex,s.value);s.value='missing';out.push(s.value,s.selectedIndex);s.options[0].selected=true;out.push(s.value,s.selectedIndex);const r=out.join('|');";
    assert_eq!(common::eval(source, "r"), "b|1|true|true|2|b||-1|a|0");
}

#[test]
fn multiple_selection_preserves_current_state() {
    let source = "const s=document.createElement('select');s.multiple=true;s.innerHTML='<option value=a>A</option><option value=b>B</option>';s.options[0].selected=true;s.options[1].selected=true;const out=[s.options[0].selected,s.options[1].selected];s.value='b';out.push(s.options[0].selected,s.options[1].selected);s.selectedIndex=-1;out.push(s.value,s.selectedIndex);const r=out.join('|');";
    assert_eq!(common::eval(source, "r"), "true|true|false|true||-1");
}

#[test]
fn clean_defaults_follow_attributes_but_dirty_values_do_not() {
    let source = "const s=document.createElement('select');s.innerHTML='<option value=a>A</option><option value=b>B</option>';const out=[s.value,s.selectedIndex];s.options[1].setAttribute('selected','');out.push(s.value);s.value='a';s.options[0].removeAttribute('selected');out.push(s.value,s.options[0].hasAttribute('selected'));const r=out.join('|');";
    assert_eq!(common::eval(source, "r"), "a|0|b|a|false");
}

#[test]
fn detached_selection_survives_insertion_and_removal() {
    let source = "const s=document.createElement('select');s.innerHTML='<option value=a selected>A</option><option value=b>B</option>';s.value='b';document.body.appendChild(s);const out=[s.value,s.selectedIndex];s.remove();out.push(s.value);s.value='missing';document.body.appendChild(s);out.push(s.value,s.selectedIndex);const r=out.join('|');";
    assert_eq!(common::eval(source, "r"), "b|1|b||-1");
}

/** Structural and OPTION changes reset a dropdown; explicit empty SELECT assignments survive. */
#[test]
fn option_mutations_restore_the_first_enabled_choice_without_changing_defaults() {
    let source = "const s=document.createElement('select');s.innerHTML='<option value=x disabled>X</option><optgroup><option value=a>A</option></optgroup><option value=b>B</option>';s.value='b';s.options[2].remove();const out=[s.value,s.selectedIndex];s.options[1].selected=false;out.push(s.value,s.selectedIndex);s.value='missing';out.push(s.value,s.selectedIndex);s.selectedIndex=-1;out.push(s.value,s.selectedIndex);s.setAttribute('size','2');s.options[1].selected=false;out.push(s.value,s.selectedIndex);const r=out.join('|');";
    assert_eq!(common::eval(source, "r"), "a|1|a|1||-1||-1||-1");
}

/** Reinsertion and cross-SELECT moves never leave two selected options in a single-select. */
#[test]
fn moving_selected_options_normalizes_both_owners() {
    let source = "const s=document.createElement('select');s.innerHTML='<option value=a>A</option><option value=b>B</option>';s.value='b';const b=s.options[1];b.remove();s.appendChild(b);const out=[s.options[0].selected,s.options[1].selected,s.value];const t=document.createElement('select');t.innerHTML='<option value=c>C</option>';t.appendChild(b);out.push(s.value,t.value,s.options[0].selected,t.options[0].selected,t.options[1].selected);const r=out.join('|');";
    assert_eq!(
        common::eval(source, "r"),
        "false|true|b|a|b|true|false|true"
    );
}
