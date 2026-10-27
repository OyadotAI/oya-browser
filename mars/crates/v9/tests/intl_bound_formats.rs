/** ECMA-402 bound formats: format and compare are accessors that return one cached bound function per formatter, and their receivers and lengths follow the spec. */
mod common;

/** The Collator compare getter returns a function that works unbound, so `const cmp = collator.compare` keeps the collator's options. */
#[test]
fn collator_compare_works_unbound_through_its_accessor() {
    assert_eq!(
        common::eval(
            "const c=new Intl.Collator('en',{sensitivity:'base'});const cmp=c.compare;const r=String(cmp('a','A')===0);",
            "r"
        ),
        "true"
    );
}

/** The bound format and compare functions are cached on the formatter, so reading them twice gives the same function. */
#[test]
fn bound_functions_are_identical_on_repeated_reads() {
    assert_eq!(
        common::eval(
            "const c=new Intl.Collator();const d=new Intl.DateTimeFormat('en-US',{timeZone:'UTC'});const n=new Intl.NumberFormat();const r=String([c.compare===c.compare,d.format===d.format,n.format===n.format].join());",
            "r"
        ),
        "true,true,true"
    );
}

/** Each formatter caches its own bound function, so two formatters never share one. */
#[test]
fn bound_functions_are_not_shared_between_formatters() {
    assert_eq!(
        common::eval(
            "const a=new Intl.Collator();const b=new Intl.Collator();const r=String(a.compare!==b.compare);",
            "r"
        ),
        "true"
    );
}

/** The bound functions are not own properties of the formatter, so nothing extra is visible to scripts. */
#[test]
fn bound_function_cache_is_not_an_own_property() {
    assert_eq!(
        common::eval(
            "const c=new Intl.Collator();c.compare;const d=new Intl.DateTimeFormat();d.format;const r=String([Object.getOwnPropertyNames(c).length,Object.getOwnPropertyNames(d).length].join());",
            "r"
        ),
        "0,0"
    );
}

/** The bound DateTimeFormat format formats its argument with the formatter's options, unbound. */
#[test]
fn date_format_works_unbound_through_its_accessor() {
    assert_eq!(
        common::eval(
            "const f=new Intl.DateTimeFormat('en-US',{timeZone:'UTC',year:'numeric'}).format;const r=f(0);",
            "r"
        ),
        "1970"
    );
}

/** A Collator's compare getter refuses any other receiver, and a DateTimeFormat format getter refuses a Collator. */
#[test]
fn accessor_getters_reject_incompatible_receivers() {
    assert_eq!(
        common::eval(
            "const cg=Object.getOwnPropertyDescriptor(Intl.Collator.prototype,'compare').get;const dg=Object.getOwnPropertyDescriptor(Intl.DateTimeFormat.prototype,'format').get;let n=0;for(const [g,recv] of [[cg,{}],[cg,new Intl.NumberFormat()],[dg,new Intl.Collator()],[dg,{}]]){try{g.call(recv)}catch(e){if(e instanceof TypeError)n++}}const r=String(n);",
            "r"
        ),
        "4"
    );
}

/** A bound compare ignores its receiver and keeps comparing with the collator it was read from. */
#[test]
fn bound_compare_ignores_the_call_receiver() {
    assert_eq!(
        common::eval(
            "const c=new Intl.Collator('en',{sensitivity:'base'});const cmp=c.compare;const r=String([cmp.call(null,'a','A'),cmp.call({},'a','A')].join());",
            "r"
        ),
        "0,0"
    );
}

/** The bound compare function has length 2 and the bound format functions have length 1, as the spec gives them. */
#[test]
fn bound_functions_have_spec_lengths() {
    assert_eq!(
        common::eval(
            "const r=String([new Intl.Collator().compare.length,new Intl.DateTimeFormat().format.length,new Intl.NumberFormat().format.length].join());",
            "r"
        ),
        "2,1,1"
    );
}

/** Collator resolvedOptions takes no arguments, so its length is zero rather than one. */
#[test]
fn resolved_options_length_is_zero() {
    assert_eq!(
        common::eval(
            "const r=String(Intl.Collator.prototype.resolvedOptions.length);",
            "r"
        ),
        "0"
    );
}

/** RelativeTimeFormat keeps format as a method (not an accessor) with its two-argument length. */
#[test]
fn relative_time_format_keeps_format_as_a_method() {
    assert_eq!(
        common::eval(
            "const f=Intl.RelativeTimeFormat.prototype.format;const r=String([typeof f,f.length,typeof Object.getOwnPropertyDescriptor(Intl.RelativeTimeFormat.prototype,'format').value].join());",
            "r"
        ),
        "function,2,function"
    );
}
