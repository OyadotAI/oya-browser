/** Reviver source context reflects exact JSON tokens and the graph before callbacks run. */
mod common;

/** Context preserves token spelling, including escapes, exponent notation and negative zero. */
#[test]
fn primitive_context_preserves_exact_source_tokens() {
    let source = r#"let seen=[];JSON.parse('[ 1e2, -0, "\\u0041",true,null ]',
        function(key,value,context){if(key!=='')seen.push(context.source);return value});
        let r=seen.join('|');"#;
    assert_eq!(common::eval(source, "r"), "1e2|-0|\"\\u0041\"|true|null");
}

/** Replacing a sibling object suppresses source for every child of the replacement. */
#[test]
fn replaced_subtrees_do_not_inherit_original_source() {
    let source = "let seen=[];JSON.parse('[1,{\"x\":2}]',function(key,value,context){\
        if(key==='0')this[1]={x:2};seen.push(key+':'+String(context.source));return value});\
        let r=seen.join('|');";
    assert_eq!(
        common::eval(source, "r"),
        "0:1|x:undefined|1:undefined|:undefined"
    );
}

/** Appending to an original parsed array visits the new value with an empty context. */
#[test]
fn values_added_to_parsed_objects_have_no_source_record() {
    let source = "let seen=[];JSON.parse('[1,[]]',function(key,value,context){\
        if(value===1)this[1].push('added');seen.push(key+':'+String(context.source));return value});\
        let r=seen.join('|');";
    assert_eq!(
        common::eval(source, "r"),
        "0:1|0:undefined|1:undefined|:undefined"
    );
}

/** Array traversal snapshots length and still invokes the reviver for a removed sibling. */
#[test]
fn revivers_visit_deleted_array_indices() {
    let source = "let seen=[];JSON.parse('[1,2]',function(key,value,context){\
        if(key==='0')delete this[1];seen.push(key+':'+String(value)+':'+String(context.source));\
        return value});let r=seen.join('|');";
    assert_eq!(
        common::eval(source, "r"),
        "0:1:1|1:undefined:undefined|:1,:undefined"
    );
}

/** A duplicate object's last occurrence supplies its final primitive's source spelling. */
#[test]
fn duplicate_property_source_comes_from_the_last_value() {
    let source = "let r='';JSON.parse('{\"x\":1,\"x\":2e0}',function(key,value,context){\
        if(key==='x')r=context.source;return value});";
    assert_eq!(common::eval(source, "r"), "2e0");
}
