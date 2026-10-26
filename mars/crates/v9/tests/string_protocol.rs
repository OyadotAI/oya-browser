/** String methods preserve observable Symbol protocol lookup, coercion order and receivers. */
mod common;

/** An explicit false @@match makes an actual RegExp acceptable as a search string. */
#[test]
fn string_search_methods_honor_disabled_match() {
    assert_eq!(
        common::eval(
            "const re=/a/;re[Symbol.match]=false;const r=['includes','startsWith','endsWith'].map(k=>'/a/'[k](re)).join();",
            "r"
        ),
        "true,true,true"
    );
}

/** IsRegExp reads the Symbol getter after the receiver and before search-string coercion. */
#[test]
fn search_brand_checks_follow_observable_order() {
    assert_eq!(
        common::eval(
            "let log='';const text={toString(){log+='t';return 'abc'}};const needle={get[Symbol.match](){log+='m';return false},toString(){log+='s';return 'a'}};const pos={valueOf(){log+='p';return 0}};String.prototype.startsWith.call(text,needle,pos);const r=log;",
            "r"
        ),
        "tmsp"
    );
}

/** Ordinary objects claiming @@match are rejected without coercing them to strings. */
#[test]
fn string_search_methods_reject_match_claiming_objects() {
    assert_eq!(
        common::eval(
            "let converted=false;const pattern={[Symbol.match]:true,toString(){converted=true;return 'a'}};const r=['includes','startsWith','endsWith'].map(k=>{try{'abc'[k](pattern);return false}catch(e){return e instanceof TypeError&&!converted}}).join();",
            "r"
        ),
        "true,true,true"
    );
}

/** Protocol methods pass the unconverted receiver and keep each trailing argument intact. */
#[test]
fn symbol_protocol_receives_original_values() {
    assert_eq!(
        common::eval(
            "const receiver={toString(){throw Error('coerced')}};const extra={};const r=['match','matchAll','search','replace','replaceAll','split'].map(k=>{const pattern={};pattern[Symbol[k==='replaceAll'?'replace':k]]=function(value,arg){return this===pattern&&value===receiver&&(k==='replace'||k==='replaceAll'||k==='split'?arg===extra:arguments.length===1)};return String.prototype[k].call(receiver,pattern,extra)}).join();",
            "r"
        ),
        "true,true,true,true,true,true"
    );
}

/** Non-callable Symbol hooks throw rather than falling through to ordinary string matching. */
#[test]
fn symbol_protocol_rejects_non_callable_hooks() {
    assert_eq!(
        common::eval(
            "const r=['match','matchAll','search','replace','replaceAll','split'].map(k=>{const p={[Symbol[k==='replaceAll'?'replace':k]]:1};try{'abc'[k](p);return false}catch(e){return e instanceof TypeError}}).join();",
            "r"
        ),
        "true,true,true,true,true,true"
    );
}

/** Nullish receiver checks precede any author code reached through a pattern's hook getter. */
#[test]
fn nullish_receivers_fail_before_protocol_lookup() {
    assert_eq!(
        common::eval(
            "let reads=0;const r=['match','matchAll','search','replace','replaceAll','split'].map(k=>{const p={get[Symbol[k==='replaceAll'?'replace':k]](){reads++;return ()=>true}};try{String.prototype[k].call(null,p);return false}catch(e){return e instanceof TypeError&&reads===0}}).join();",
            "r"
        ),
        "true,true,true,true,true,true"
    );
}

/** Global-only methods must read observable flags, including on RegExp-like objects. */
#[test]
fn global_protocol_methods_use_observable_flags() {
    assert_eq!(
        common::eval(
            "let log='';const p={get[Symbol.match](){log+='m';return true},get flags(){log+='f';return 'g'},[Symbol.matchAll](){log+='a';return 1},[Symbol.replace](){log+='r';return 2}};'x'.matchAll(p);'x'.replaceAll(p,'y');const r=log;",
            "r"
        ),
        "mfamfr"
    );
}

/** A negative end position clamps to zero instead of indexing relative to the string's end. */
#[test]
fn ends_with_clamps_negative_positions() {
    assert_eq!(
        common::eval(
            "const r=['abc'.endsWith('b',-1),'abc'.endsWith('',-1),'abc'.endsWith('c',Infinity)].join();",
            "r"
        ),
        "false,true,true"
    );
}

/** A missing hook coerces its input during RegExpInitialize without calling the global binding. */
#[test]
fn missing_match_hook_coerces_the_pattern() {
    assert_eq!(
        common::eval(
            "const p=/a/;p[Symbol.match]=null;globalThis.RegExp=()=>{throw Error('replaced')};const r='x/a/y'.match(p)[0];",
            "r"
        ),
        "/a/"
    );
}

/** The fallback creates a RegExp directly, so a missing hook getter runs only once. */
#[test]
fn missing_hook_does_not_repeat_the_brand_check() {
    assert_eq!(
        common::eval(
            "const calls=[];const p={get[Symbol.match](){calls.push('match');return undefined},[Symbol.toPrimitive](){calls.push('primitive');return ''}};''.match(p);const r=calls.join();",
            "r"
        ),
        "match,primitive"
    );
}

/** The pinned V8 protocol consults object patterns, leaving primitive prototypes untouched. */
#[test]
fn primitive_patterns_do_not_use_object_protocol_hooks() {
    assert_eq!(
        common::eval(
            "let count=0;Object.prototype[Symbol.replace]=()=>{count++;return 'hook'};''.replace(0);''.replace(0.1);const boxed=''.replace(new Number(0));const r=[count,boxed].join();",
            "r"
        ),
        "1,hook"
    );
}

/** Generic String methods must run an overridden wrapper conversion just like other objects. */
#[test]
fn boxed_receivers_use_their_conversion_methods() {
    assert_eq!(
        common::eval(
            "const s=new String('old');s.toString=()=> 'new';const r=String.prototype.includes.call(s,'new');",
            "r"
        ),
        "true"
    );
}

/** Annex B trim aliases share their canonical function names and identities. */
#[test]
fn trim_aliases_share_canonical_functions() {
    assert_eq!(
        common::eval(
            "const p=String.prototype;const r=[p.trimLeft===p.trimStart,p.trimRight===p.trimEnd,p.trimLeft.name,p.trimRight.name].join();",
            "r"
        ),
        "true,true,trimStart,trimEnd"
    );
}
