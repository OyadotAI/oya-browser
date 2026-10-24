/** Source-owned function metadata follows direct naming contexts without changing lexical bindings. */
use mars::v9::{event, execute, load};

/** DOM proposals expose supported metadata through ordinary script evaluation. */
fn result(source: &str, expression: &str) -> String {
    execute(&format!(
        "{source}document.getElementById('out').textContent={expression};"
    ))
    .unwrap()
    .last()
    .unwrap()
    .text
    .clone()
}

/** Declaration names and parameter counts remain observable before their source declaration runs. */
#[test]
fn ordinary_declarations_expose_their_source_name_and_parameter_count() {
    assert_eq!(
        result(
            "const early=f.name+' '+f.length;function f(a,b,){}",
            "early+' '+f.name+' '+f.length"
        ),
        "f 2 f 2"
    );
}

/** Named expressions retain their private name rather than taking a declarator or property name. */
#[test]
fn named_expression_names_preserve_private_recursive_bindings() {
    assert_eq!(
        result(
            "const external=function internal(a){return internal.name;};const box={alias:function original(){return original;}};",
            "external.name+' '+external.length+' '+external()+' '+box.alias.name+' '+(box.alias()===box.alias)"
        ),
        "internal 1 internal original true"
    );
    assert!(
        execute("const f=function privateName(){};const value=privateName;")
            .unwrap_err()
            .message
            .contains("is not defined")
    );
}

/** Only an anonymous function directly initializing the declarator receives its identifier. */
#[test]
fn direct_declarators_name_functions_and_arrows_including_grouped_literals() {
    assert_eq!(
        result(
            "const ordinary=function(a){};const arrow=(a,b)=>a;const grouped=((()=>1));const zero=()=>1;",
            "ordinary.name+' '+ordinary.length+' '+arrow.name+' '+arrow.length+' '+grouped.name+' '+zero.length"
        ),
        "ordinary 1 arrow 2 grouped 0"
    );
}

/** Identifier stores infer names for direct anonymous right sides in statements and loop headers. */
#[test]
fn identifier_assignments_infer_names_without_renaming_existing_function_values() {
    assert_eq!(
        result(
            "let stored;stored=function(a,b){};let arrow;arrow=x=>x;const alias=stored;let header;for(header=()=>1;false;){break;}let update;for(let i=0;i<1;update=function(){}){i=i+1;}",
            "stored.name+' '+arrow.name+' '+alias.name+' '+header.name+' '+update.name"
        ),
        "stored arrow stored header update"
    );
}

/** Static object keys supply their decoded string form, while explicit function names take precedence. */
#[test]
fn fixed_object_literal_keys_name_anonymous_functions_and_arrows() {
    assert_eq!(
        result(
            "const box={method:function(a){},'space key':()=>1,7:function(){},'escaped\\x20key':()=>2,explicit:function inside(){}};",
            "box.method.name+' '+box['space key'].name+' '+box[7].name+' '+box['escaped key'].name+' '+box.explicit.name"
        ),
        "method space key 7 escaped key inside"
    );
}

/** Arrays, returned functions, calls and logical expressions do not supply a direct naming context. */
#[test]
fn non_direct_expression_contexts_leave_anonymous_names_empty() {
    assert_eq!(
        result(
            "const array=[function(){},()=>1];function make(){return function(){};}const returned=make();const logical=false||function(){};let assigned;assigned=false||(()=>1);const box={method:false||function(){}};",
            "array[0].name+'|'+array[1].name+'|'+returned.name+'|'+logical.name+'|'+assigned.name+'|'+box.method.name"
        ),
        "|||||"
    );
}

/** A property assignment does not infer a semantic name from its member target. */
#[test]
fn later_member_assignments_do_not_infer_literal_property_names() {
    assert_eq!(
        result(
            "const box={};box.method=function(){};box['arrow']=()=>1;",
            "box.method.name+'|'+box.arrow.name"
        ),
        "|"
    );
}

/** Observable inference does not introduce named-expression self scopes or change lexical resolution. */
#[test]
fn inferred_display_names_do_not_create_private_recursive_bindings() {
    assert_eq!(
        result(
            "let assigned=function(){return assigned;};const previous=assigned;assigned=3;const box={method:function(){return method;}};const method=7;",
            "previous()+' '+box.method()+' '+previous.name+' '+box.method.name"
        ),
        "3 7 assigned method"
    );
}

/** Service reconstruction derives metadata from source rather than adding executable closure fields. */
#[test]
fn metadata_survives_private_state_serialization_and_event_reconstruction() {
    let source = "const f=(a,b)=>a;function run(){document.getElementById('out').textContent=f.name+' '+f.length;}";
    let state = load(source, &[]).unwrap().state;
    let state = serde_json::from_value(serde_json::to_value(state).unwrap()).unwrap();
    assert_eq!(
        event(source, "run()", state, &[]).unwrap().changes[0]
            .text()
            .unwrap(),
        "f 2"
    );
}
