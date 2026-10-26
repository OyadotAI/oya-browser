/** Public error behavior preserves observable descriptor reads and safely reports rejected JSON and Date inputs. */
mod common;

/** Ambiguous descriptors are rejected only after their fields have been read in the specified order. */
#[test]
fn descriptor_fields_are_read_before_ambiguity_is_reported() {
    let source = "const log=[];const descriptor=new Proxy({},{has(target,key){log.push('has:'+key);return true},get(target,key){log.push('get:'+key);return key==='get'||key==='set'?undefined:0}});let invalid=false;try{Object.create(null,{x:descriptor})}catch(error){invalid=error instanceof TypeError && error.message.startsWith('Invalid property descriptor')}const r=[invalid,log.join('|')].join(';');";
    assert_eq!(
        common::eval(source, "r"),
        "true;has:enumerable|get:enumerable|has:configurable|get:configurable|has:value|get:value|has:writable|get:writable|has:get|get:get|has:set|get:set"
    );
}

/** A value getter's abrupt completion prevents the later writable and accessor property reads. */
#[test]
fn descriptor_value_failure_stops_before_writable() {
    let source = "const marker={};let log='';const descriptor={get enumerable(){log+='enumerable;';return true},get configurable(){log+='configurable;';return true},get value(){log+='value;';throw marker},get writable(){log+='writable;';return true}};let same=false;try{Object.defineProperty({},'x',descriptor)}catch(error){same=error===marker}const r=[same,log].join('|');";
    assert_eq!(
        common::eval(source, "r"),
        "true|enumerable;configurable;value;"
    );
}

/** Invalid getter and setter messages identify the rejected accessor without coercing its object value. */
#[test]
fn invalid_accessor_diagnostics_do_not_run_conversion_hooks() {
    let source = "let calls=0;const invalid={get toString(){calls++;throw 7},get [Symbol.toPrimitive](){calls++;throw 8}};const out=[];for(const descriptor of [{get:0},{set:0},{set:invalid}]){try{Object.create(null,{x:descriptor})}catch(error){out.push(error instanceof TypeError,error.message)}}out.push(calls);const r=out.join('|');";
    assert_eq!(
        common::eval(source, "r"),
        "true|Getter must be a function: 0|true|Setter must be a function: 0|true|Setter must be a function: #<Object>|0"
    );
}

/** Unterminated JSON strings report the original UTF16 end position instead of losing the failed string production. */
#[test]
fn json_unterminated_strings_keep_utf16_positions() {
    let source = "const out=[];for(const input of ['{\\\"ab ','{\\\"aΩ ','{\\\"a💩 ']){try{JSON.parse(input)}catch(error){out.push(error instanceof SyntaxError,error.message)}}const r=out.join('|');";
    assert_eq!(
        common::eval(source, "r"),
        "true|Unterminated string in JSON at position 5 (line 1 column 6)|true|Unterminated string in JSON at position 5 (line 1 column 6)|true|Unterminated string in JSON at position 6 (line 1 column 7)"
    );
}

/** CRLF is one diagnostic line break while CRCR, LFLF and LFCR each contribute two. */
#[test]
fn json_source_locations_count_crlf_once() {
    let source = "const out=[];for(const newline of ['\\n','\\r','\\r\\n','\\r\\r','\\n\\n','\\n\\r']){try{JSON.parse('{'+newline+'7:1}')}catch(error){const match=/position (\\d+) \\(line (\\d+) column (\\d+)\\)/.exec(error.message);out.push(error.message.startsWith(\"Expected property name or '}' in JSON\"),match.slice(1).join(':'))}}const r=out.join('|');";
    assert_eq!(
        common::eval(source, "r"),
        "true|2:2:1|true|2:2:1|true|3:2:1|true|3:3:1|true|3:3:1|true|3:3:1"
    );
}

/** JSON distinguishes an illegal unescaped control from a missing property name after a comma. */
#[test]
fn json_diagnostics_retain_the_rejected_grammar_production() {
    let source = "const out=[];for(const input of ['{\\\"a\\nb\\\":\\\"b\\\"}','\\n  { \\n\\n\\\"12345\\\": 5,,}']){try{JSON.parse(input)}catch(error){out.push(error.message)}}const r=out.join('|');";
    assert_eq!(
        common::eval(source, "r"),
        "Bad control character in string literal in JSON at position 3 (line 1 column 4)|Expected double-quoted property name in JSON at position 18 (line 4 column 12)"
    );
}

/** Invalid JSON is converted once, and its diagnostic is built from the captured text without invoking a reviver. */
#[test]
fn json_diagnostic_context_uses_the_original_input_once() {
    let source = "let conversions=0;let revivals=0;const input={toString(){conversions++;return '\\u2028{}'}};let message;try{JSON.parse(input,()=>revivals++)}catch(error){message=error.message}const r=[conversions,revivals,message===\"Unexpected token '\\u2028', \\\"\\u2028{}\\\" is not valid JSON\"].join();";
    assert_eq!(common::eval(source, "r"), "1,0,true");
}

/** Escape diagnostics identify the first invalid source unit rather than the string's opening or decoded output. */
#[test]
fn json_escape_errors_preserve_the_failure_position() {
    let source = "const out=[];for(const input of ['\\\"a\\\\q\\\"','\\\"a\\\\u12x4\\\"','1e+']){try{JSON.parse(input)}catch(error){out.push(error.message)}}const r=out.join('|');";
    assert_eq!(
        common::eval(source, "r"),
        "Bad escaped character in JSON at position 3 (line 1 column 4)|Bad Unicode escape in JSON at position 6 (line 1 column 7)|Exponent part is missing a number in JSON at position 3 (line 1 column 4)"
    );
}

/** Receiver formatting bypasses overridden function source and name hooks and leaves explicit Error messages intact. */
#[test]
fn date_incompatible_receiver_abbreviates_source_without_author_code() {
    let source = "let calls=0;const long='x'.repeat(500);const receiver=Function('return '+JSON.stringify(long));Object.defineProperty(receiver,'name',{get(){calls++;throw 1}});Object.defineProperty(receiver,'toString',{get(){calls++;throw 2}});Object.defineProperty(receiver,Symbol.toPrimitive,{get(){calls++;throw 3}});const method=Date.prototype.setDate;Object.defineProperty(method,'name',{get(){calls++;throw 4}});let message;let typed=false;try{method.call(receiver)}catch(error){typed=error instanceof TypeError;message=error.message}const r=[typed,message.startsWith('Method Date.prototype.setDate called on incompatible receiver function anonymous('),message.includes('...<omitted>...'),message.length<256,calls,Error(long).message===long].join();";
    assert_eq!(common::eval(source, "r"), "true,true,true,true,0,true");
}

/** The source threshold counts UTF16 units, so multibyte letters do not trigger premature omission. */
#[test]
fn date_receiver_preview_uses_function_source_units() {
    let source = "const short=Function('/*'+'Ω'.repeat(80)+'*/');const full=short.toString();let message;try{Date.prototype.setDate.call(short)}catch(error){message=error.message}const r=[full.length<=128,message.endsWith(full),message.includes('...<omitted>...')].join();";
    assert_eq!(common::eval(source, "r"), "true,true,false");
}

/** Date brand failure precedes argument conversion, and generated field getters retain their own brand diagnostic. */
#[test]
fn date_brand_failures_do_not_convert_arguments() {
    let source = "let conversions=0;const argument={valueOf(){conversions++;throw 1}};const out=[];try{Date.prototype.setDate.call(null,argument)}catch(error){out.push(error.message)}try{Date.prototype.getTime.call({})}catch(error){out.push(error.message)}out.push(conversions);const r=out.join('|');";
    assert_eq!(
        common::eval(source, "r"),
        "Method Date.prototype.setDate called on incompatible receiver null|this is not a Date object.|0"
    );
}

/** An incompatible proxy receiver is described from its internal target without invoking any handler trap. */
#[test]
fn date_receiver_diagnostics_do_not_invoke_proxy_traps() {
    let source = "let traps=0;function target(){}const receiver=new Proxy(target,{get(){traps++;throw 1},getPrototypeOf(){traps++;throw 2},apply(){traps++;throw 3}});let message;try{Date.prototype.setDate.call(receiver)}catch(error){message=error.message}const r=[message.endsWith(Function.prototype.toString.call(target)),traps].join();";
    assert_eq!(common::eval(source, "r"), "true,0");
}
