/**
 * Regression tests for the builtins ported from the specification and V8's src/builtins:
 * Promise capabilities and combinators, the Proxy-aware integrity internal methods,
 * ObjectDefineProperties, collection receiver checks, Reflect.construct's constructor checks,
 * the Number and Boolean prototype wrappers, Date setters on invalid dates and
 * Date.prototype[Symbol.toPrimitive]. Each test is named after the rule it pins.
 */
use crate::common;

use common::eval;

/** Promise.all on a subclass builds through the subclass and its own `resolve`. */
#[test]
fn promise_all_uses_this_constructor() {
    let out = eval(
        "let out = '';
         class Sub extends Promise {}
         let resolves = 0;
         Sub.resolve = function (v) { resolves++; return Promise.resolve.call(this, v); };
         const p = Sub.all([1, 2]);
         out += (p instanceof Sub) + ':' + resolves;
         p.then(v => { out += ':' + v.join(','); });",
        "out",
    );
    assert_eq!(out, "true:2:1,2");
}

/** Promise.all on a non-constructor throws synchronously instead of returning a promise. */
#[test]
fn promise_all_rejects_non_constructor_this() {
    let out = eval(
        "let out; try { Promise.all.call({}, []); out = 'no'; } catch (e) { out = e instanceof TypeError; }",
        "out",
    );
    assert_eq!(out, "true");
}

/** An iterator that fails after the first value is closed and the result rejects. */
#[test]
fn promise_race_closes_iterator_on_abrupt_then() {
    let out = eval(
        "let out = '';
         let closed = 0;
         const iter = { [Symbol.iterator]() { return { next() { return { value: 1, done: false }; },
           return() { closed++; return {}; } }; } };
         const Bad = function (ex) { return new Promise(ex); };
         Bad.resolve = function () { throw new Error('boom'); };
         Promise.race.call(Bad, iter).catch(e => { out += e.message + ':' + closed; });",
        "out",
    );
    assert_eq!(out, "boom:1");
}

/** Promise.any with every element rejected rejects with an AggregateError listing them. */
#[test]
fn promise_any_rejects_with_aggregate_error() {
    let out = eval(
        "let out = '';
         Promise.any([Promise.reject(1), Promise.reject(2)]).catch(e => {
           out = (e instanceof AggregateError) + ':' + e.errors.join(',');
         });",
        "out",
    );
    assert_eq!(out, "true:1,2");
}

/** then() honours Symbol.species, and finally passes the value through. */
#[test]
fn promise_then_uses_species_and_finally_passes_value() {
    let out = eval(
        "let out = '';
         class Sub extends Promise {}
         out += (Sub.resolve(1).then(() => {}) instanceof Sub);
         Promise.resolve(7).finally(() => 9).then(v => { out += ':' + v; });",
        "out",
    );
    assert_eq!(out, "true:7");
}

/** Reflect.construct refuses a newTarget that is not a constructor. */
#[test]
fn reflect_construct_checks_new_target() {
    let out = eval(
        "let out;
         try { Reflect.construct(function () {}, [], Math.max); out = 'no'; }
         catch (e) { out = e instanceof TypeError; }",
        "out",
    );
    assert_eq!(out, "true");
}

/** A proxy's getPrototypeOf, isExtensible and preventExtensions traps are called and checked. */
#[test]
fn proxy_integrity_traps_run_and_are_checked() {
    let out = eval(
        "const proto = {};
         const p = new Proxy({}, { getPrototypeOf() { return proto; } });
         let lied;
         try { Object.isExtensible(new Proxy({}, { isExtensible() { return false; } })); lied = 'no'; }
         catch (e) { lied = e instanceof TypeError; }
         let threw;
         try { Object.freeze(new Proxy({}, { preventExtensions() { throw new RangeError('x'); } })); }
         catch (e) { threw = e instanceof RangeError; }
         const out = [Object.getPrototypeOf(p) === proto, lied, threw].join(',');",
        "out",
    );
    assert_eq!(out, "true,true,true");
}

/** defineProperties throws on a refused define and reads every descriptor before defining. */
#[test]
fn define_properties_throws_and_reads_first() {
    let out = eval(
        "const o = {};
         Object.defineProperty(o, 'a', { value: 1 });
         let threw;
         try { Object.defineProperties(o, { a: { value: 2 } }); } catch (e) { threw = e instanceof TypeError; }
         const p = {};
         try { Object.defineProperties(p, { x: { value: 1 }, y: { get: 5 } }); } catch (e) {}
         let nulls;
         try { Object.create({}, null); } catch (e) { nulls = e instanceof TypeError; }
         const out = [threw, 'x' in p, nulls].join(',');",
        "out",
    );
    assert_eq!(out, "true,false,true");
}

/** Redefining a non-configurable accessor with the same getter is allowed. */
#[test]
fn define_property_same_accessor_is_unchanged() {
    let out = eval(
        "const o = {}; const g = () => 10;
         Object.defineProperty(o, 'f', { get: g });
         Object.defineProperties(o, { f: { get: g } });
         const out = String(o.f);",
        "out",
    );
    assert_eq!(out, "10");
}

/** __lookupSetter__ finds a setter along the chain. */
#[test]
fn lookup_setter_walks_the_chain() {
    let out = eval(
        "const s = function (v) {};
         const o = Object.create({ set x(v) {} });
         const own = {}; own.__defineSetter__('y', s);
         const out = [typeof o.__lookupSetter__('x'), own.__lookupSetter__('y') === s,
           ({}).__lookupSetter__('z')].join(',');",
        "out",
    );
    assert_eq!(out, "function,true,");
}

/** Map methods refuse a Set receiver and the weak ones refuse strong collections. */
#[test]
fn collection_methods_check_their_own_kind() {
    let out = eval(
        "const refuses = f => { try { f(); return false; } catch (e) { return e instanceof TypeError; } };
         const out = [
           refuses(() => Map.prototype.has.call(new Set(), 1)),
           refuses(() => Set.prototype.clear.call(new Map())),
           refuses(() => WeakMap.prototype.set.call(new Map(), {}, 1)),
           Map[Symbol.species] === Map,
         ].join(',');",
        "out",
    );
    assert_eq!(out, "true,true,true,true");
}

/** Number.prototype and Boolean.prototype are wrappers of +0 and false. */
#[test]
fn number_and_boolean_prototypes_are_wrappers() {
    let out = eval(
        "const out = [Number.prototype.toFixed(1), Boolean.prototype.valueOf(),
           Object.prototype.toString.call(Number.prototype),
           Object.prototype.toString.call(Boolean.prototype)].join(',');",
        "out",
    );
    assert_eq!(out, "0.0,false,[object Number],[object Boolean]");
}

/** A setter on an invalid date converts its argument but leaves the date as it is. */
#[test]
fn date_setter_on_invalid_date_keeps_time() {
    let out = eval(
        "const d = new Date(NaN);
         const r = d.setDate({ valueOf() { d.setTime(0); return 1; } });
         const out = r + ':' + d.getTime();",
        "out",
    );
    assert_eq!(out, "NaN:0");
}

/** Date's toPrimitive refuses bad hints and skips non-callable methods. */
#[test]
fn date_to_primitive_follows_ordinary_to_primitive() {
    let out = eval(
        "const d = new Date(0);
         let bad; try { d[Symbol.toPrimitive]('nope'); } catch (e) { bad = e instanceof TypeError; }
         const o = { toString: null, valueOf() { return 5; } };
         const out = [bad, Date.prototype[Symbol.toPrimitive].call(o, 'string')].join(',');",
        "out",
    );
    assert_eq!(out, "true,5");
}
