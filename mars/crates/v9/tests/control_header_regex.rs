/** Statement headers permit regex expressions without changing division after calls. */
use crate::common;

#[test]
fn regex_expression_statement_follows_for_of_and_nested_if() {
    let source = r#"let count=0;for(const [key,value] of Object.entries({x:'4px'}))/^x$/.test(key)&&/^-?\d*\.?\d+px$/.test(value)?count++:0;if(((true))) /yes/.test('yes')&&count++;const r=count;"#;
    assert_eq!(common::eval(source, "r"), "2");
}

#[test]
fn grouping_calls_and_keyword_members_still_use_division() {
    let source = "function f(){return 8}const obj={if:f,while:f};const x=6;const r=[f()/2,(x)/2,obj.if()/2,obj.while()/4].join();";
    assert_eq!(common::eval(source, "r"), "4,3,4,2");
}

#[test]
fn comments_and_template_substitutions_keep_header_context() {
    let source = "let out='';for(let n=0;n<1;n++)/*gap*/ /x/.test('x')&&(out+=`a${(()=>{if(true)/x/.test('x');return 3})()}`);const r=out;";
    assert_eq!(common::eval(source, "r"), "a3");
}
