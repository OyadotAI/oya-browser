/** DateTimeFormat method brands belong to native methods and cannot be replaced by receiver metadata or prototype spoofing. */
mod common;

/** Other native Intl instances cannot redirect a borrowed DateTimeFormat method into their formatter implementation. */
#[test]
fn date_parts_rejects_other_formatter_kinds() {
    assert_eq!(
        common::eval(
            "const f=Intl.DateTimeFormat.prototype.formatToParts;let n=0;for(const receiver of [new Intl.NumberFormat(),new Intl.Collator(),new Intl.RelativeTimeFormat(),new Intl.ListFormat(),new Intl.Segmenter()]){try{f.call(receiver,0)}catch(e){if(e instanceof TypeError)n++}}const r=n;",
            "r"
        ),
        "5"
    );
}

/** Prototype chains and matching tags do not supply native formatter state, and proxy traps are never consulted for branding. */
#[test]
fn date_method_brand_checks_do_not_consult_author_properties() {
    assert_eq!(
        common::eval(
            "const real=new Intl.DateTimeFormat();const f=Intl.DateTimeFormat.prototype.formatToParts;let n=0,touched=0;const proxy=new Proxy(real,{get(){touched++;throw 1}});const fake={[Symbol.toStringTag]:'Intl.DateTimeFormat',get constructor(){touched++;throw 2}};for(const receiver of [null,undefined,{},fake,Intl.DateTimeFormat.prototype,Object.create(real),proxy]){try{f.call(receiver,0)}catch(e){if(e instanceof TypeError)n++}}const r=[n,touched].join();",
            "r"
        ),
        "7,0"
    );
}

/** An incompatible receiver throws before date arguments can run valueOf or toString. */
#[test]
fn invalid_date_receiver_is_rejected_before_argument_conversion() {
    assert_eq!(
        common::eval(
            "const bad=new Intl.NumberFormat();let touched=0,n=0;const value={valueOf(){touched++;throw 1},toString(){touched++;throw 2}};for(const name of ['format','formatToParts','formatRange','resolvedOptions']){try{Intl.DateTimeFormat.prototype[name].call(bad,value,value)}catch(e){if(e instanceof TypeError)n++}}const r=[n,touched].join();",
            "r"
        ),
        "4,0"
    );
}

/** Genuine state remains valid even when public constructor, tag and method-name properties are changed. */
#[test]
fn genuine_date_formatters_do_not_depend_on_mutable_public_metadata() {
    assert_eq!(
        common::eval(
            "const formatter=new Intl.DateTimeFormat('en-US',{timeZone:'UTC'});const f=Intl.DateTimeFormat.prototype.formatToParts;Object.defineProperty(f,'name',{value:'NumberFormat'});Object.defineProperty(formatter,'constructor',{get(){throw 1}});Object.defineProperty(formatter,Symbol.toStringTag,{value:'Intl.NumberFormat'});const parts=f.call(formatter,0);const r=[Array.isArray(parts),parts.map(p=>p.value).join('')===formatter.format(0),formatter.resolvedOptions().locale].join();",
            "r"
        ),
        "true,true,en-US"
    );
}

/** Real foreign creation-context instances share the brand while results belong to the borrowed method's realm. */
#[cfg(feature = "conformance")]
#[test]
fn borrowed_date_methods_accept_real_foreign_instances() {
    let environment = mars_v9::PageEnvironment {
        conformance_d8: true,
        ..common::environment()
    };
    let execution = common::load_with(
        "const child=Realm.create();const foreign=Realm.eval(child,\"new Intl.DateTimeFormat('en-US',{timeZone:'UTC'})\");const method=Realm.eval(child,'Intl.DateTimeFormat.prototype.formatToParts');const foreignArray=Realm.eval(child,'Array.prototype');const own=new Intl.DateTimeFormat('en-US',{timeZone:'UTC'});const a=Intl.DateTimeFormat.prototype.formatToParts.call(foreign,0);const b=method.call(own,0);const r=[Array.isArray(a),Object.getPrototypeOf(a)===Array.prototype,Object.getPrototypeOf(b)===foreignArray,a.map(p=>p.value).join('')===b.map(p=>p.value).join('')].join();",
        &environment,
    );
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "true,true,true,true");
}
