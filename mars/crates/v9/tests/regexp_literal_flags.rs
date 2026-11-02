/** RegExp literal flag validation happens during parsing without changing constructor coercions or slash boundaries. */
use crate::common;

/** Unicode escapes are prohibited in literal flags even when they decode to a supported flag. */
#[test]
fn escaped_regexp_flags_are_parse_errors() {
    for source in [
        r"/foo/\u0069",
        r"/foo/\u{69}",
        r"/foo/g\u0069",
        r"/foo/\u0076",
        r"/foo/\u00",
    ] {
        assert_eq!(
            mars_v9::check(source).unwrap_err().code,
            "script_syntax_error",
            "{source}"
        );
    }
}

/** Identifier parts after the closing slash belong to the flags and cannot become separate source tokens. */
#[test]
fn unsupported_regexp_flag_characters_are_parse_errors() {
    for flags in ["a", "0", "_", "$", "π", "\u{301}", "\u{200c}", "\u{200d}"] {
        let source = format!("/foo/{flags}");
        assert_eq!(
            mars_v9::check(&source).unwrap_err().code,
            "script_syntax_error",
            "{flags}"
        );
    }
}

/** Repeated flags and mutually exclusive Unicode modes are rejected even in uncalled functions. */
#[test]
fn duplicate_and_conflicting_flags_are_early_errors() {
    for flags in ["dd", "gg", "ii", "mm", "ss", "uu", "vv", "yy", "uv", "vu"] {
        let source = format!("function never(){{return /foo/{flags}}}");
        assert_eq!(
            mars_v9::check(&source).unwrap_err().code,
            "script_syntax_error",
            "{flags}"
        );
    }
}

/** Direct and indirect eval and dynamic constructors report the literal grammar diagnostic. */
#[test]
fn dynamic_parsing_reports_invalid_regexp_flags() {
    let source = r#"const out=[];const code=String.raw`/foo/\u0069`;for(const parse of [source=>eval(source),source=>(0,eval)(source),source=>Function(source)]){try{parse(code);out.push(false)}catch(error){out.push(error instanceof SyntaxError,error.toString()==='SyntaxError: Invalid regular expression flags')}}const r=out.join();"#;
    assert_eq!(common::eval(source, "r"), "true,true,true,true,true,true");
}

/** Invalid flag spelling prevents all source execution, including statements before an unused literal. */
#[test]
fn invalid_flags_never_allow_prior_eval_side_effects() {
    let source = "let calls=0;const out=[];for(const flags of ['gg','a','uv']){try{eval('calls++;function never(){return /foo/'+flags+'}');out.push(false)}catch(error){out.push(error instanceof SyntaxError)}}out.push(calls);const r=out.join();";
    assert_eq!(common::eval(source, "r"), "true,true,true,0");
}

/** Every supported flag remains admitted in each valid Unicode combination and in arbitrary order. */
#[test]
fn valid_regexp_flags_keep_their_runtime_values() {
    let source = "const out=[/foo/dgimsuy.flags,/foo/vymsgid.flags,/foo/.flags,/foo/gi.test('FOO'),/foo/v.test('foo')];const r=out.join(':');";
    assert_eq!(common::eval(source, "r"), "dgimsuy:dgimsvy::true:true");
}

/** Flags stop at punctuation and trivia, leaving division, comments and ordinary following identifiers intact. */
#[test]
fn regexp_flag_scanning_preserves_slash_boundaries() {
    let source = "let value=12;value/=3;const a=/[/]/g.test('/');const b=/foo/ /*gap*/.test('foo');const c=/foo/g;const ratio=18/3/2;const d=/foo/g\nconst named=5;const r=[value,a,b,c.flags,ratio,d.flags,named].join();";
    assert_eq!(common::eval(source, "r"), "4,true,true,g,3,g,5");
}

/** Constructor flags remain string values produced through the usual ordered user coercions. */
#[test]
fn regexp_constructor_flag_coercions_remain_runtime_operations() {
    let source = r#"let log='';const pattern={toString(){log+='p';return 'foo'}};const flags={toString(){log+='f';return '\\u0069'}};let invalid=false;try{new RegExp(pattern,flags)}catch(error){invalid=error instanceof SyntaxError}const valid=new RegExp('foo','\u0069');const r=[log,invalid,valid.ignoreCase,valid.test('FOO')].join();"#;
    assert_eq!(common::eval(source, "r"), "pf,true,true,true");
}
