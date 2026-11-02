/** V9 reads the ES2015+ syntax real bundles use; what it cannot run fails only when reached. */
use mars::v9::{check, load};

/** Every form parses as a whole script. */
#[test]
fn modern_syntax_parses() {
    for source in [
        "var s = `a${1 + 2}b${'c'}`;",
        "var t = tag`x${y}z`;",
        "var [a, , b = 2, ...rest] = list; var {c, d: {e}, ...more} = object;",
        "function f(a = 1, {b} = {}, ...c) { return a; }",
        "var g = (a = 1, [b], ...c) => a; var h = async x => await x;",
        "class A extends B { static #n = 1; #x; constructor() { super(); } get v() { return this.#x; } set v(x) { this.#x = x; } static { A.n = 2; } *gen() { yield 1; } async m() {} }",
        "var o = { get a() { return 1; }, set a(v) {}, [k]: 1, m() {}, async *n() {}, ...p, q };",
        "f(...args, 1); var arr = [...a, ...b]; var obj = {...a, b: 1};",
        "var v = a?.b?.[c]?.(d) ?? e; x ??= 1; y ||= 2; z &&= 3;",
        "var p = 2 ** 3 ** 2; p **= 2;",
        "for (const x of list) {} for (var k in object) {} for (let [k, v] of map) {}",
        "outer: for (;;) { inner: for (;;) { break outer; continue inner; } }",
        "switch (x) { case 1: y(); break; case 2: default: z(); }",
        "try { a(); } catch (e) { b(); } finally { c(); } try {} catch {}",
        "function t() { throw new Error('x'); }",
        "var n = new Foo(1).bar, m = new Foo, q = new a.b.C(); function F() { return new.target; }",
        "typeof a; void 0; delete o.p; ~x; !y; -z; +w; x >>> 1; x << 2; x >> 3; x & y | z ^ w;",
        "var r = /a[/]b\\/c/gi.test(s); var d = x / y / z; if (/x/.test(s)) {}",
        "var c = a ? b : c ? d : e;",
        "do x++; while (x < 3) var after = 1;",
        "var big = 10n, legacy = 010, sep = 1_000, hex = 0xFFn;",
        "label: { break label; }",
        "var j = import('./x.js');",
        "async function* gen() { for await (const x of y) yield* x; }",
        "#!/usr/bin/env node\nvar shebang = 1;",
        "with (o) { x; }",
        "var \\u0061bc = 1, ünï = 2;",
    ] {
        assert!(check(source).is_ok(), "{source}: {:?}", check(source).err());
    }
}

/** Invalid JavaScript is still a syntax error, never a silent deferral. */
#[test]
fn invalid_javascript_is_a_syntax_error() {
    for source in [
        "var = 1;",
        "a b;",
        "if (x) { ;",
        "return 1;",
        "break;",
        "for (;;) { continue missing; }",
        "f(a,,b);",
        "`abc ${x",
        "try {}",
        "x?.;",
        "if (x) let y = 1;",
        "const k;",
        "import x from 'y';",
        "export var z = 1;",
    ] {
        let error = check(source).unwrap_err();
        assert_eq!(error.code, "script_syntax_error", "{source}");
    }
}

/** Templates evaluate as string concatenation of cooked chunks and substitutions. */
#[test]
fn templates_evaluate_to_strings() {
    let execution = load("let s = `a${1 + 2}b\\n${'c'}`; let plain = `x`;", &[]).unwrap();
    let globals = execution.state.globals();
    let value = |name: &str| globals.get(name).and_then(|cell| cell.value.clone());
    assert_eq!(value("s").unwrap().to_dom().unwrap(), "a3b\nc");
    assert_eq!(value("plain").unwrap().to_dom().unwrap(), "x");
}

/** A regular expression literal is an opaque object value. */
#[test]
fn regex_literals_are_opaque_objects() {
    let execution = load("let t = typeof /x/g;", &[]).unwrap();
    let t = execution
        .state
        .globals()
        .get("t")
        .unwrap()
        .value
        .clone()
        .unwrap();
    assert_eq!(t.to_dom().unwrap(), "object");
}

/** `import.meta` is only valid in module code: a classic script holding it is an early error. */
#[test]
fn import_meta_outside_a_module_is_a_syntax_error() {
    let error = check("var i = import.meta;").unwrap_err();
    assert_eq!(error.code, "script_syntax_error");
}

/** `with` runs in sloppy code, both at the top level and inside a function. */
#[test]
fn with_statements_run() {
    assert!(load("var o = {x: 1}; var y; with (o) { y = x; }", &[]).is_ok());
    assert!(
        load(
            "var ok = 1; function later() { with ({}) {} } later();",
            &[]
        )
        .is_ok()
    );
}
