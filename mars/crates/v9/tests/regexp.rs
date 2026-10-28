/** RegExp semantics ported from V8's builtins-regexp and irregexp parser, through the public page
API. Each test pins one spec algorithm that V9 used to shortcut. */
mod common;
use common::eval;

/** Run `body` and read back `r`. */
fn run(body: &str) -> String {
    eval(&format!("let r;{body}"), "r")
}

/** Oversized regular expressions report the capacity reason instead of a misleading syntax error. */
#[test]
fn oversized_pattern_reports_its_engine_limit() {
    let message = run(
        "let p='a';for(let i=0;i<1000;i++)p='('+p+')';try{new RegExp(p)}catch(e){r=String(e)};",
    );
    assert!(
        message.contains("Regular expression too large"),
        "{message}"
    );
}

/** Replacement reads every capture before rejecting a callback argument list that exceeds V8's call ceiling. */
#[test]
fn replacement_rejects_oversized_callback_arguments_after_capture_getters() {
    assert_eq!(
        run(
            "const re=/./g;let calls=0,read=0;re.exec=()=>calls++?null:{length:65536,get 65535(){read++;return 'x'}};try{''.replace(re,()=>{r='called'})}catch(e){r=[e instanceof RangeError,read].join(',')};"
        ),
        "true,1"
    );
}

/** The largest supported replacement callback argument list remains callable. */
#[test]
fn replacement_accepts_callback_arguments_at_the_ceiling() {
    assert_eq!(
        run(
            "const re=/./g;let calls=0;re.exec=()=>calls++?null:{length:65523};''.replace(re,function(){r=arguments.length});"
        ),
        "65525"
    );
}

/** The flags getter is generic and reads each flag getter in spec order. */
#[test]
fn flags_getter_reads_each_flag_in_order() {
    let r = run(
        "const seen=[];const o={};for(const k of ['hasIndices','global','ignoreCase','multiline','dotAll','unicode','unicodeSets','sticky'])Object.defineProperty(o,k,{get(){seen.push(k[0]);return k==='global'||k==='sticky'}});\
        const f=Object.getOwnPropertyDescriptor(RegExp.prototype,'flags').get;r=f.call(o)+':'+seen.join('')+':'+f.name;",
    );
    assert_eq!(r, "gy:hgimduus:get flags");
}

/** Symbol.split builds its splitter through the species constructor with a y flag added. */
#[test]
fn split_uses_species_constructor() {
    let r = run(
        "let made;class R extends RegExp{static get[Symbol.species](){return function(src,fl){made=fl;return new RegExp(src,fl)}}}\
        r=new R('-','g')[Symbol.split]('a-b-c').join()+':'+made;",
    );
    assert_eq!(r, "a,b,c:gy");
}

/** Symbol.split on a plain regexp keeps captures and honors the limit. */
#[test]
fn split_keeps_captures_and_limit() {
    assert_eq!(
        run("r='a1b2c'.split(/(\\d)/).join('|')+';'+'a,b,c'.split(/,/,2).join('|');"),
        "a|1|b|2|c;a|b"
    );
}

/** Symbol.match and Symbol.replace read `flags` rather than the internal slot. */
#[test]
fn match_and_replace_read_flags_property() {
    let r = run(
        "const re=/a/g;Object.defineProperty(re,'flags',{value:''});\
        r=re[Symbol.match]('ba').index+':'+re[Symbol.replace]('aXa','b');",
    );
    // Non-global by flags, so lastIndex (2, left by the first call on the g slot) is used.
    assert_eq!(r, "1:aXb");
}

/** RegExpExec rejects a custom exec that returns a primitive. */
#[test]
fn custom_exec_must_return_object_or_null() {
    let r = run(
        "const re=/a/;re.exec=()=>1;try{re[Symbol.match]('a')}catch(e){r=e instanceof TypeError}",
    );
    assert_eq!(r, "true");
}

/** The constructor honors IsRegExp, new.target's prototype and returns a same-constructor input. */
#[test]
fn constructor_follows_is_regexp() {
    let r = run(
        "const like={source:'x+',flags:'g',[Symbol.match]:true,constructor:RegExp};\
        class S extends RegExp{} const s=new S('a');\
        r=[RegExp(like)===like, new RegExp(like).source, new RegExp(like).global, Object.getPrototypeOf(s)===S.prototype, typeof RegExp[Symbol.species]].join();",
    );
    assert_eq!(r, "true,x+,true,true,function");
}

/** The d flag produces `indices` with named groups. */
#[test]
fn has_indices_builds_indices_array() {
    let r = run(
        "const m=/(?<x>b)(c)?/d.exec('abd');r=JSON.stringify(m.indices)+':'+m.indices.groups.x.join('-')+':'+m.indices[2];",
    );
    assert_eq!(r, "[[1,2],[1,2],null]:1-2:undefined");
}

/** source escapes `/` and line terminators; the empty pattern is `(?:)`. */
#[test]
fn source_is_escaped() {
    let r = run(
        "r=[new RegExp('/').source, new RegExp('\\n').source, new RegExp('[/]').source, new RegExp('').source].join(' ');",
    );
    assert_eq!(r, "\\/ \\n [/] (?:)");
}

/** Search restores lastIndex, and exec reads lastIndex even for non-global patterns. */
#[test]
fn search_and_exec_handle_last_index() {
    let r = run(
        "const re=/b/g;re.lastIndex=2;const i=re[Symbol.search]('abc');let reads=0;const n=/a/;\
        n.lastIndex={valueOf(){reads++;return 0}};n.exec('a');r=i+':'+re.lastIndex+':'+reads;",
    );
    assert_eq!(r, "1:2:1");
}

/** compile (Annex B) refuses flags with a RegExp pattern and resets lastIndex. */
#[test]
fn compile_reinitializes() {
    let r = run(
        "const re=/a/g;re.lastIndex=3;re.compile('b','i');let t;try{re.compile(/x/,'g')}catch(e){t=e instanceof TypeError}\
        r=re.source+re.flags+re.lastIndex+t;",
    );
    assert_eq!(r, "bi0true");
}

/** Captures inside a quantified group reset on each iteration (RepeatMatcher step 4). */
#[test]
fn quantified_captures_reset_each_iteration() {
    let r = run("r=JSON.stringify(/(z)((a+)?(b+)?(c))*/.exec('zaacbbbcac'));");
    assert_eq!(r, "[\"zaacbbbcac\",\"z\",\"ac\",\"a\",null,\"c\"]");
}

/** With the u flag, identity escapes, lone brackets, quantified lookaheads and class ranges
over class escapes are SyntaxErrors. */
#[test]
fn unicode_mode_is_strict() {
    let r = run(
        "r=['\\\\A','\\\\c','\\\\x1',']','{','(?=.)*','[\\\\d-a]','\\\\u{}','[\\\\1]'].map(p=>{try{new RegExp(p,'u');return 0}catch(e){return e instanceof SyntaxError?1:2}}).join('');",
    );
    assert_eq!(r, "111111111");
}

/** Group names decode surrogate pairs and escapes; without named groups `\k` is literal. */
#[test]
fn group_names_and_annex_b_k() {
    let r = run(
        "const m=/(?<\u{1d453}>a)(?<\\u{1d4d3}>b)/.exec('ab');r=[m.groups['\u{1d453}'],m.groups['\u{1d4d3}'],/\\k<a/.test('k<a')].join();\
        try{new RegExp('(?<a-b>x)')}catch(e){r+=e instanceof SyntaxError}",
    );
    assert_eq!(r, "a,b,truetrue");
}

/** With a named group declared, `\k` inside a class is no identity escape and is a SyntaxError; without one Annex B keeps it a literal `k`. */
#[test]
fn class_k_escape_is_an_error_only_with_named_groups() {
    let r = run(
        "let out=[];try{new RegExp('[\\\\k](?<a>)');out.push(false)}catch(e){out.push(e instanceof SyntaxError)}out.push(new RegExp('[\\\\k]').test('k'));r=out.join();",
    );
    assert_eq!(r, "true,true");
}
