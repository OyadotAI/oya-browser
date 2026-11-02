/** Unsupported native construction names the actual interface without evaluating author names. */
use crate::common;

/** Capture the original TypeError through public constructor calls. */
fn run(source: &str) -> String {
    let execution = common::load(source);
    assert_eq!(common::failure(&execution), None);
    common::global(&execution, "result")
}

/** Generic interfaces and the custom registry keep refusing new while naming themselves. */
#[test]
fn rejected_constructors_identify_the_native_interface() {
    assert_eq!(
        run(
            "let result='';for(const C of [Node,CSSStyleSheet,HTMLButtonElement,CustomElementRegistry]){try{new C()}catch(error){result+=(error instanceof TypeError)+'|'+error.message+';'}}"
        ),
        "true|Failed to construct 'Node': Illegal constructor;true|Failed to construct 'CSSStyleSheet': Illegal constructor;true|Failed to construct 'HTMLButtonElement': Illegal constructor;true|Failed to construct 'CustomElementRegistry': Illegal constructor;"
    );
}

/** Ordinary function calls retain a TypeError and explain the absent construction operation. */
#[test]
fn ordinary_calls_are_distinguished_from_forbidden_construction() {
    assert_eq!(
        run(
            "let result='';try{CustomElementRegistry()}catch(error){result=(error instanceof TypeError)+'|'+error.message}"
        ),
        "true|Failed to construct 'CustomElementRegistry': Please use the 'new' operator, this DOM object constructor cannot be called as a function."
    );
}

/** Renaming, bound forwarding and Reflect.construct newTarget cannot misidentify the native.
The bound copy is made before the throwing getter, since `bind` itself reads `name` by spec. */
#[test]
fn author_controlled_names_are_not_used_for_native_diagnostics() {
    assert_eq!(
        run(
            "let hits=0,result='';const bound=Node.bind(null);Object.defineProperty(Node,'name',{get(){hits++;throw Error('name getter ran')}});Object.defineProperty(CSSStyleSheet,'name',{value:'Innocent'});for(const C of [Node,CSSStyleSheet,bound]){try{Reflect.construct(C,[],function Pretend(){})}catch(error){result+=error.message+';'}}result+='getter='+hits;"
        ),
        "Failed to construct 'Node': Illegal constructor;Failed to construct 'CSSStyleSheet': Illegal constructor;Failed to construct 'Node': Illegal constructor;getter=0"
    );
}
