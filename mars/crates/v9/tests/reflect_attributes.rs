/** IDL [Reflect] attributes in V9: each reflection type's getting and setting rules. */
use crate::common;
use common::{environment, global, host, page};
use mars_v9::{DomLevel, PageInput, StorageState};

/** Run a script against the test page and return a global, failing on any script failure. */
fn eval(source: &str, name: &str) -> String {
    let host = host();
    let mut environment = environment();
    environment.dom_level = DomLevel::Extended;
    let execution = mars_v9::load_page(PageInput {
        sources: &[source.to_string()],
        source_keys: &[],
        document: page(),
        storage: StorageState::default(),
        host: &host,
        environment: &environment,
        modules: mars_v9::ModuleInput::NONE,
    })
    .expect("page loads");
    assert!(execution.failed.is_empty(), "{:?}", execution.failed);
    global(&execution, name)
}

#[test]
fn strings_reflect_with_mapped_content_names_and_legacy_null() {
    let source = "const m=document.createElement('meta');m.httpEquiv='refresh';\
        const t=document.createElement('table');t.bgColor=null;const p=document.createElement('p');p.align=null;\
        const r=[m.getAttribute('http-equiv'),JSON.stringify(t.getAttribute('bgcolor')),p.getAttribute('align'),\
        document.createElement('hr').width===''].join('|');";
    assert_eq!(eval(source, "r"), "refresh|\"\"|null|true");
}

#[test]
fn enumerated_attributes_use_missing_and_invalid_defaults_ascii_case_insensitively() {
    let source = "const i=document.createElement('input');const a=[i.type];i.setAttribute('type','CHECKBOX');a.push(i.type);\
        i.setAttribute('type','bogus');a.push(i.type);i.setAttribute('type','\\u212Aeygen');a.push(i.type);\
        const t=document.createElement('track');a.push(t.kind);t.setAttribute('kind','x');a.push(t.kind);\
        const d=document.createElement('div');a.push(JSON.stringify(d.dir));d.setAttribute('dir','RTL');a.push(d.dir);\
        d.setAttribute('dir','up');a.push(JSON.stringify(d.dir));\
        const f=document.createElement('form');a.push(f.method,f.enctype);f.method='POST';a.push(f.method,f.getAttribute('method'));\
        const r=a.join('|');";
    assert_eq!(
        eval(source, "r"),
        "text|checkbox|text|text|subtitles|metadata|\"\"|rtl|\"\"|get|application/x-www-form-urlencoded|post|POST"
    );
}

#[test]
fn nullable_enumerations_read_null_when_missing_and_remove_on_null() {
    let source = "const g=document.createElement('img');const a=[g.crossOrigin];g.setAttribute('crossorigin','');\
        a.push(g.crossOrigin);g.setAttribute('crossorigin','USE-credentials');a.push(g.crossOrigin);\
        g.setAttribute('crossorigin','x');a.push(g.crossOrigin);g.crossOrigin=null;a.push(g.hasAttribute('crossorigin'));\
        const r=a.join('|');";
    assert_eq!(
        eval(source, "r"),
        "|anonymous|use-credentials|anonymous|false"
    );
}

#[test]
fn booleans_reflect_presence() {
    let source = "const i=document.createElement('input');i.defaultChecked=true;const a=[i.getAttribute('checked')];\
        i.setAttribute('formnovalidate','no');a.push(i.formNoValidate);i.defaultChecked=0;a.push(i.hasAttribute('checked'));\
        const r=JSON.stringify(a);";
    assert_eq!(eval(source, "r"), "[\"\",true,false]");
}

#[test]
fn urls_resolve_against_the_document_and_form_action_defaults_to_it() {
    let source = "const q=document.createElement('q');const a=[q.cite];q.setAttribute('cite','http://site.example/a/../b');\
        a.push(q.cite);const f=document.createElement('form');a.push(f.action===document.URL);\
        const r=a.join('|');";
    assert_eq!(eval(source, "r"), "|http://site.example/b|true");
}

#[test]
fn longs_parse_html_integers_and_wrap_on_setting() {
    let source = "const o=document.createElement('ol');const a=[o.start];o.setAttribute('start','  -12abc');a.push(o.start);\
        o.setAttribute('start','\\u00a07');a.push(o.start);o.setAttribute('start','2147483648');a.push(o.start);\
        o.start=4294967297;a.push(o.getAttribute('start'));o.start=-1.9;a.push(o.getAttribute('start'));\
        const r=a.join('|');";
    assert_eq!(eval(source, "r"), "1|-12|1|1|1|-1");
}

#[test]
fn limited_longs_default_to_minus_one_and_throw_on_negative() {
    let source = "const i=document.createElement('input');const a=[i.maxLength];i.setAttribute('maxlength','-5');\
        a.push(i.maxLength);i.setAttribute('maxlength','+9');a.push(i.maxLength);\
        try{i.maxLength=-1;a.push('no throw')}catch(e){a.push(e.name)}i.maxLength=0;a.push(i.getAttribute('maxlength'));\
        const r=a.join('|');";
    assert_eq!(eval(source, "r"), "-1|-1|9|IndexSizeError|0");
}

#[test]
fn unsigned_longs_keep_to_the_signed_range_and_fall_back() {
    let source = "const c=document.createElement('canvas');const a=[c.width,c.height];c.setAttribute('width','-0');\
        a.push(c.width);c.width=2147483648;a.push(c.getAttribute('width'));\
        const i=document.createElement('input');a.push(i.size);try{i.size=0}catch(e){a.push(e.name)}\
        i.setAttribute('size','0');a.push(i.size);\
        const t=document.createElement('textarea');t.cols=0;a.push(t.getAttribute('cols'),t.rows);\
        const r=a.join('|');";
    assert_eq!(eval(source, "r"), "300|150|0|300|20|IndexSizeError|20|20|2");
}

#[test]
fn clamped_unsigned_longs_clamp_parsed_values() {
    let source = "const d=document.createElement('td');const a=[d.colSpan,d.rowSpan];d.setAttribute('colspan','0');\
        a.push(d.colSpan);d.setAttribute('colspan','5000');a.push(d.colSpan);d.setAttribute('rowspan','0');\
        a.push(d.rowSpan);d.setAttribute('rowspan','x');a.push(d.rowSpan);\
        const r=a.join('|');";
    assert_eq!(eval(source, "r"), "1|1|1|1000|0|1");
}

#[test]
fn limited_doubles_ignore_non_positive_sets_and_reject_non_finite() {
    let source = "const p=document.createElement('progress');const a=[p.max];p.setAttribute('max','1e2');a.push(p.max);\
        p.setAttribute('max','-0');a.push(Object.is(p.max,1));p.max=-1;a.push(p.getAttribute('max'));\
        p.max=1e25;a.push(p.getAttribute('max'));p.setAttribute('max','1.8e308');a.push(p.max);\
        try{p.max=NaN;a.push('no throw')}catch(e){a.push(e.name)}\
        const r=a.join('|');";
    assert_eq!(eval(source, "r"), "1|100|true|-0|1e+25|1|TypeError");
}

#[test]
fn obsolete_and_table_interfaces_get_their_own_prototypes() {
    let source = "const r=[document.createElement('h3') instanceof HTMLHeadingElement,\
        'align' in document.createElement('p'),'align' in document.createElement('section'),\
        document.createElement('marquee').scrollAmount,document.createElement('col').span].join('|');";
    assert_eq!(eval(source, "r"), "true|true|false|6|1");
}
