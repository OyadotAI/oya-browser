// Language regression probe: every failed check logs FAIL; a passing run logs only its last line.
function check(name, got, want) { if (got !== want) console.error("FAIL " + name + ": got " + got + " want " + want); }
class A { constructor(x) { this.x = x; } get double() { return this.x * 2; } static make() { return new A(3); } m() { return 'A'; } }
class B extends A { constructor() { super(5); this.y = 1; } m() { return super.m() + 'B'; } }
check("class", new B().double, 10); check("super", new B().m(), "AB"); check("static", A.make().x, 3);
check("instanceof", new B() instanceof A, true);
let o = {a: 1, ...{b: 2}, ['c' + 1]: 3, f() { return this.a; }};
check("spread", o.b, 2); check("computed", o.c1, 3); check("method", o.f(), 1);
const {a, b: bb = 9, ...rest} = {a: 1, c: 4, d: 5}; check("destructure", a + bb + rest.c + rest.d, 19);
const [p, , q = 7, ...r] = [1, 2, undefined, 4, 5]; check("array destructure", p + q + r.length, 10);
function* gen() { const x = yield 1; yield x * 2; return 9; }
const g = gen(); check("gen1", g.next().value, 1); check("gen2", g.next(21).value, 42); const last = g.next(); check("gen3", last.value + "" + last.done, "9true");
check("spread gen", [...(function*(){ yield* [1,2]; yield 3; })()].join(), "1,2,3");
let log = []; try { throw new TypeError("boom"); } catch (e) { log.push(e.message, e instanceof TypeError, e.name); } finally { log.push("f"); }
check("try", log.join(), "boom,true,TypeError,f");
function fin() { try { return 1; } finally { log.push("fin"); } } check("finally return", fin(), 1);
outer: for (let i = 0; i < 3; i++) { for (let j = 0; j < 3; j++) { if (j == 1) continue outer; if (i == 2) break outer; log.push(i + "" + j); } }
check("labels", log.slice(-2).join(), "00,10");
const fns = []; for (let i = 0; i < 3; i++) fns.push(() => i); check("per-iteration let", fns.map(f => f()).join(), "0,1,2");
switch (3) { case 1: log.push("x"); case 3: log.push("three"); case 4: log.push("four"); break; default: log.push("d"); }
check("switch", log.slice(-2).join(), "three,four");
check("typeof undeclared", typeof notDefinedAnywhere, "undefined");
check("template", `a${1+1}b${"c"}`, "a2bc");
check("regex", /(\d+)-(\w+)/.exec("x 12-ab y")[2], "ab");
check("replace", "a-b-c".replace(/-/g, "+"), "a+b+c");
check("replace fn", "abc".replace(/b/, m => m.toUpperCase()), "aBc");
check("split", "a, b,c".split(/,\s*/).length, 3);
check("match g", "a1b22c333".match(/\d+/g).join("|"), "1|22|333");
check("named groups", "2024-10".match(/(?<y>\d{4})-(?<m>\d\d)/).groups.m, "10");
const m = new Map([[1, 'a']]); m.set('k', 2); check("map", m.get(1) + m.size, "a2");
const s = new Set([1,2,2,3]); check("set", [...s].join(), "1,2,3");
check("json", JSON.parse('{"a":[1,2,{"b":null}]}').a[2].b, null);
check("date", new Date(Date.UTC(2020, 0, 15, 10)).toISOString(), "2020-01-15T10:00:00.000Z");
check("number", (1234.5678).toFixed(2), "1234.57"); check("radix", (255).toString(16), "ff");
check("parseInt", parseInt("0x1f"), 31); check("math", Math.max(1, 5, 3), 5);
check("optional", o?.zz?.yy, undefined); check("nullish", null ?? "d", "d");
let cnt = 0; o.n ??= 5; o.n ||= 6; check("logical assign", o.n, 5);
check("arguments", (function(){ return arguments.length; })(1,2,3), 3);
check("bind", (function(a, b){ return this.x + a + b; }).bind({x: 1}, 2)(3), 6);
check("getter def", Object.defineProperty({}, 'q', {get(){ return 4; }}).q, 4);
check("keys order", Object.keys({b:1, 2:1, a:1, 1:1}).join(), "1,2,b,a");
check("array methods", [3,1,2].sort().concat([4]).reverse().slice(1, 3).join(), "3,2");
check("includes", [NaN].includes(NaN), true); check("flat", [1,[2,[3]]].flat(Infinity).join(), "1,2,3");
check("string methods", " Abc ".trim().toLowerCase().padStart(5, "*"), "**abc");
check("at", [1,2,3].at(-1), 3); check("from", Array.from({length: 3}, (_, i) => i * 2).join(), "0,2,4");
check("symbol", typeof Symbol.iterator, "symbol");
check("proto", Object.getPrototypeOf([]) === Array.prototype, true);
check("toString tag", Object.prototype.toString.call([]), "[object Array]");
var hoisted = 1; check("window var", window.hoisted, 1);
check("eval", eval("1 + 2"), 3); check("Function", new Function("a", "b", "return a * b")(3, 4), 12);
let order = [];
Promise.resolve(1).then(v => order.push("then" + v)); queueMicrotask(() => order.push("micro")); order.push("sync");
(async function() { const v = await Promise.resolve(5); order.push("await" + v); check("async order", order.join(), "sync,then1,micro,await5"); console.log("async done"); })();
console.log("lang done");
