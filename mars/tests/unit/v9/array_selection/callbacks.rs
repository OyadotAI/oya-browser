/** First-class predicates reuse existing receiver conversion, adaptation and atomic event rules. */
use super::*;

/** Both methods pass the current element, numeric index, source array and optional callback receiver. */
#[test]
fn callbacks_receive_value_index_source_and_explicit_receiver() {
    let source = "const a=[2,4];let trail='';const s=a.some(function(v,i,original){trail=trail+v+':'+i+':'+(original===a)+':'+this.n+';';return false;},{n:7});const f=a.find(function(v,i,original){return v===4&&i===1&&original===a&&this.n===8;},{n:8});";
    assert_eq!(
        result(source, "s+' '+f+' '+trail"),
        "false 4 2:0:true:7;4:1:true:7;"
    );
}

/** Strict callbacks keep their receiver as given; sloppy callbacks without one get the global object. */
#[test]
fn ordinary_strict_and_sloppy_callback_receiver_rules_are_preserved() {
    let source = "const s=[1].some(function(){'use strict';return this===undefined;});const f=[2].find(function(){return this===globalThis;});";
    assert_eq!(result(source, "s+' '+f"), "true 2");
    for method in ["some", "find"] {
        assert_eq!(
            result(
                "",
                &format!("[7].{method}(function(){{'use strict';return this===4;}},4)")
            ),
            if method == "some" { "true" } else { "7" }
        );
    }
}

/** A sloppy callback given a primitive thisArg receives it boxed, as OrdinaryCallBindThis requires. */
#[test]
fn sloppy_callbacks_box_a_primitive_receiver() {
    for method in ["some", "find"] {
        assert_eq!(
            result(
                "",
                &format!("[7].{method}(function(){{return typeof this==='object'&&this==4;}},4)")
            ),
            if method == "some" { "true" } else { "7" }
        );
    }
}

/** Arrow lexical receivers and bound receivers prevail over the array method's callback thisArg. */
#[test]
fn arrow_and_bound_predicates_keep_their_own_receivers() {
    let source = "const box={n:4,run:function(){return [4].find(v=>v===this.n,{n:99});}};const f=box.run();const s=[7].some(function(v){return v===this.n;}.bind({n:7}),{n:99});";
    assert_eq!(result(source, "f+' '+s"), "4 true");
}

/** Extracted methods use the ordinary call/apply/bind paths, work on array-likes and refuse nullish receivers. */
#[test]
fn methods_support_call_apply_bind_generic_receivers_and_refuse_nullish_ones() {
    let source = "const some=[].some;const find=[].find;const yes=v=>v>1;";
    assert_eq!(
        result(
            source,
            "some.call([1,2],yes)+' '+find.apply([1,2],[yes])+' '+some.bind([2],yes)()+' '+find.bind([3],yes)()"
        ),
        "true 2 true 3"
    );
    assert_eq!(
        result(
            "",
            "[].some.call({},()=>true)+' '+[].find.call({length:0},()=>true)+' '+[].find.call('abc',v=>v==='b')"
        ),
        "false undefined b"
    );
    for method in ["some", "find"] {
        for receiver in ["null", "undefined"] {
            let error = execute(&format!("[].{method}.call({receiver},()=>true);")).unwrap_err();
            assert_eq!(error.code, "script_execution_error");
            assert!(error.message.contains("TypeError"), "{}", error.message);
        }
    }
}

/** Function and host values are truthy just like objects; predicates do not demand Boolean returns. */
#[test]
fn callback_truthiness_accepts_functions_and_projected_host_values() {
    let page = mars::document::parse("<p id=out></p>").unwrap();
    let document = mars::document::script_document(&page, "predicate-page").unwrap();
    let source = "const a=[1].some(()=>document);const b=[2].find(()=>()=>false);document.getElementById('out').textContent=a+' '+b;";
    let execution =
        mars::v9::load_document_scripts(&[source.into()], document, Default::default()).unwrap();
    assert_eq!(execution.changes[0].text().unwrap(), "true 2");
}

/** Arguments beyond thisArg still evaluate once, in ordinary source order, before selection. */
#[test]
fn extra_arguments_evaluate_normally_and_are_ignored_by_the_method() {
    let source = "let trail='';function arg(n){trail=trail+n;return n;}const s=[1].some(()=>true,undefined,arg('a'),arg('b'));const f=[2].find(()=>true,undefined,arg('c'));";
    assert_eq!(result(source, "s+' '+f+' '+trail"), "true 2 abc");
}

/** A failing callback cannot change the saved page state, and a later event uses the prior captured values. */
#[test]
fn callback_failure_and_budget_exhaustion_preserve_the_prior_state() {
    let source = "let n=0;const a=[7];function bad(){n++;a[0]=99;document.getElementById('out').textContent='partial';a.some(()=>missing());}function exhausted(){n++;a[0]=99;a.find(()=>{while(true){}});}function run(){n++;document.getElementById('out').textContent=n+' '+a.find(()=>true);}";
    let mut state = load(source, &[]).unwrap().state;
    for expected in ["1 7", "2 7"] {
        assert_eq!(
            event(source, "bad()", state.clone(), &[]).unwrap_err().code,
            "script_execution_error"
        );
        assert_eq!(
            event(source, "exhausted()", state.clone(), &[])
                .unwrap_err()
                .code,
            "limit_exceeded"
        );
        let execution = next(source, "run()", state);
        assert_eq!(execution.changes[0].text().unwrap(), expected);
        state = execution.state;
    }
}
