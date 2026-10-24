// Semantics regression probe: every failed check logs FAIL; a passing run logs only its last line.
function check(name, got, want) { if (got !== want) console.error("FAIL " + name + ": got " + JSON.stringify(got) + " want " + JSON.stringify(want)); }
function outer() { const f = () => arguments.length; return f(); } check("arrow arguments", outer(1, 2), 2);
var keys = []; for (var k in "ab") keys.push(k); for (var k2 in [5, , 7]) keys.push(k2); check("for-in", keys.join(), "0,1,0,2");
var fs = []; for (var i = 0; i < 3; i++) fs.push(function () { return i; }); check("var loop closure", fs[0](), 3);
check("sloppy this", (function () { return this; })(), window);
blk: { if (true) break blk; check("label block", 1, 2); }
try { [3, 1, 2].sort(function () { throw new Error("cmp"); }); } catch (e) { check("sort throw", e.message, "cmp"); }
var finished = false; function* g() { try { yield 1; yield 2; } finally { finished = true; } } for (var x of g()) { break; } check("for-of return", finished, true);
class P { #secret = 4; static count = 2; static { P.count++; } #m() { return this.#secret; } get v() { return this.#m(); } has(o) { return #secret in o; } } check("private", new P().v, 4); check("static block", P.count, 3); check("brand", new P().has({}), false);
var caught; (async function () { throw new Error("async boom"); })().catch(function (e) { caught = e.message; check("async reject", caught, "async boom"); });
check("spread map", [...new Map([[1, 2]])][0].join(), "1,2"); check("spread string", [..."héllo"].length, 5); check("spread args", (function () { return [...arguments]; })(1, 2).length, 2);
var cyc = {}; cyc.self = cyc; try { JSON.stringify(cyc); } catch (e) { check("cyclic json", e instanceof TypeError, true); }
check("float", String(0.1 + 0.2), "0.30000000000000004"); check("big", String(1e21), "1e+21"); check("neg zero", String(-0), "0"); check("toFixed", (-1.005).toFixed(2), "-1.00"); check("toFixed2", (2.5).toFixed(0), "3"); check("exp", (123456).toExponential(2), "1.23e+5"); check("prec", (0.000123).toPrecision(2), "0.00012");
check("date parse", Date.parse("Tue, 01 Jan 2019 10:00:00 GMT"), 1546336800000);
check("regex unicode", /\u{1F600}/u.test("😀"), true); check("regex sticky g", "aaa".replace(/a/gy, "b"), "bbb"); check("regex backref", /(a)\1/.test("aa"), true);
check("string compare", "b" > "a" && "B" < "a", true); check("instanceof bound", new (function F() {}.bind(null))() instanceof Object, true);
var o2 = { a: 1 }; Object.defineProperty(o2, "hidden", { value: 2, enumerable: false }); check("json enumerable", JSON.stringify(o2), '{"a":1}');
check("array holes", [, 1].length + ":" + (0 in [, 1]), "2:false"); check("join nested", [1, [2, [3]]].join(), "1,2,3");
check("getter inherit", Object.create({ get g() { return this.v; } }, { v: { value: 9 } }).g, 9);
check("labeled continue do", (function(){ var n=0; a: do { n++; if (n < 3) continue a; } while (n < 5); return n; })(), 5);
check("switch default middle", (function(v){ switch(v){ case 1: return 'one'; default: return 'd'; case 2: return 'two'; } })(2), "two");
check("comma in for", (function(){ for (var i=0, j=10; i<j; i++, j--); return i; })(), 5);
check("getter on class static", (class { static get x() { return 7; } }).x, 7);
check("new.target", (function F() { return new.target === F; }).call({}), false);
check("Symbol.toPrimitive", +{ [Symbol.toPrimitive]() { return 42; } }, 42);
check("valueOf add", ({ valueOf() { return 1; } }) + 1, 2);
check("array sort default", [10, 9, 1].sort().join(), "1,10,9");
check("string raw", String.raw`a\nb`, "a\\nb");
check("Number parse", Number("0x10") + Number(" 12 ") + Number(""), 28);
check("parseFloat", parseFloat("3.14abc"), 3.14);
console.log("probe2 done");
