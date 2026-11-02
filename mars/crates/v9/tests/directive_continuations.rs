/** Directive prologues contain entire literal statements, not string tokens inside continued expressions. */
use crate::common;

/** A newline before in continues the expression and leaves legacy reflection on the function available. */
#[test]
fn newline_in_expression_is_not_a_strict_directive() {
    let source = "function f(){'use strict'\nin Number;return this}const r=[f.arguments===null,f()===globalThis].join();";
    assert_eq!(common::eval(source, "r"), "true,true");
}

/** Both keyword binary operators keep sloppy duplicate formals legal after a continued string expression. */
#[test]
fn keyword_operator_continuations_preserve_sloppy_parameter_rules() {
    let source = "const out=[];for(const operator of ['in','instanceof']){const f=Function('a','a',\"'use strict'\\n\"+operator+' Number;return [this===globalThis,a].join()');out.push(f(1,2),f.arguments===null)}const r=out.join(':');";
    assert_eq!(common::eval(source, "r"), "true,2:true:true,2:true");
}

/** Non-simple formals are legal when the initial string is an operand rather than a Use Strict Directive. */
#[test]
fn keyword_continuations_do_not_reject_non_simple_formals() {
    let source = "function f(a=7){'use strict'\nin Number;return [a,this===globalThis]}function g(a=8){'use strict'\ninstanceof Number;return [a,this===globalThis]}const r=[f().join(),g().join()].join(':');";
    assert_eq!(common::eval(source, "r"), "7,true:8,true");
}

/** A line break before unary not or bitwise not inserts a semicolon, leaving the real directive effective. */
#[test]
fn unary_not_statements_preserve_automatic_strict_directives() {
    let source = "const out=[];for(const expression of ['!false','!!false','~0','~~0']){const f=Function(\"'use strict'\\n\"+expression+';return this');out.push(f()===undefined);try{f.arguments;out.push(false)}catch(error){out.push(error instanceof TypeError)}}const r=out.join();";
    assert_eq!(
        common::eval(source, "r"),
        "true,true,true,true,true,true,true,true"
    );
}

/** The mode established before a unary expression controls subsequent references and is not just reflection metadata. */
#[test]
fn strict_directives_before_unary_statements_reject_unresolvable_writes() {
    let source = "function f(){'use strict'\n!false;directiveMissing=1}function g(){'use strict'/*line\n*/~0;directiveMissing=1}let count=0;for(const run of [f,g]){try{run()}catch(error){if(error instanceof ReferenceError)count++}}const r=[count,typeof directiveMissing].join();";
    assert_eq!(common::eval(source, "r"), "2,undefined");
}

/** Existing member, binary, conditional, comma and division continuations remain part of the initial expression. */
#[test]
fn punctuator_continuations_still_end_the_directive_prologue() {
    let source = "const out=[];for(const tail of ['+\"\"','[0]','.length','?1:0',',0','&&1','||1','??1','/1']){const f=Function(\"'use strict'\\n\"+tail+';return this');out.push(f.arguments===null,f()===globalThis)}const r=out.every(Boolean);";
    assert_eq!(common::eval(source, "r"), "true");
}

/** Calls and tagged templates also continue the literal despite producing a runtime error when invoked. */
#[test]
fn call_and_tagged_template_continuations_do_not_make_functions_strict() {
    let source = "const out=[];for(const tail of ['(0)','`text`']){const f=Function(\"'use strict'\\n\"+tail);out.push(f.arguments===null);try{f();out.push(false)}catch(error){out.push(error instanceof TypeError)}}const r=out.join();";
    assert_eq!(common::eval(source, "r"), "true,true,true,true");
}

/** A preceding ordinary directive can use ASI, while escaped spellings never acquire Use Strict status. */
#[test]
fn raw_directive_spelling_and_ordinary_string_prologues_are_preserved() {
    let source = "function strict(){'ordinary'\n'use strict'\n!false;return this}function escaped(){'use\\x20strict'\n!false;return this}const r=[strict()===undefined,escaped()===globalThis].join();";
    assert_eq!(common::eval(source, "r"), "true,true");
}

/** Direct eval inherits a strict caller even when the evaluated first expression is not itself a strict directive. */
#[test]
fn continued_strings_do_not_clear_inherited_eval_strictness() {
    let source = "function strict(){'use strict';return eval(\"'use strict'\\nin Number;this\")}function sloppy(){return eval(\"'use strict'\\nin Number;this\")}const r=[strict()===undefined,sloppy()===globalThis].join();";
    assert_eq!(common::eval(source, "r"), "true,true");
}
