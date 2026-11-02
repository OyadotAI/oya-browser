/** Intl.NumberFormat and Intl.PluralRules, ported from ECMA-402: option order and validation,
exact rounding, notations, sign display, currencies, units, parts and ranges. */
use crate::common;
use common::eval;

/** Options are read in the spec's order (unit options, notation, digit options, the rest). */
#[test]
fn options_are_read_in_spec_order() {
    let source = "const reads = [];
        const options = new Proxy({}, { get(t, k) { reads.push(k); return undefined; } });
        new Intl.NumberFormat(undefined, options);
        const out = reads.join();";
    assert_eq!(
        eval(source, "out"),
        "localeMatcher,numberingSystem,style,currency,currencyDisplay,currencySign,unit,\
         unitDisplay,notation,minimumIntegerDigits,minimumFractionDigits,maximumFractionDigits,\
         minimumSignificantDigits,maximumSignificantDigits,roundingIncrement,roundingMode,\
         roundingPriority,trailingZeroDisplay,compactDisplay,useGrouping,signDisplay"
    );
}

/** Bad options throw RangeError, a missing currency TypeError, and an increment with
significant rounding TypeError. */
#[test]
fn invalid_options_throw_the_spec_errors() {
    let source = "const kind = (o) => { try { new Intl.NumberFormat('en', o); return 'ok'; }
            catch (e) { return e.constructor.name; } };
        const out = [kind({ maximumFractionDigits: 101 }), kind({ style: 'currency' }),
            kind({ roundingIncrement: 3 }), kind({ roundingIncrement: 5, maximumSignificantDigits: 2 }),
            kind({ minimumFractionDigits: 3, maximumFractionDigits: 1 }), kind({ unit: 'furlong' })].join();";
    assert_eq!(
        eval(source, "out"),
        "RangeError,TypeError,RangeError,TypeError,RangeError,RangeError"
    );
}

/** Rounding is exact on the shortest decimal, by mode, increment and significant digits. */
#[test]
fn rounding_modes_increments_and_digits() {
    let source = "const f = (o, x) => new Intl.NumberFormat('en-US', o).format(x);
        const out = [f({ maximumFractionDigits: 1, roundingMode: 'halfEven' }, 1.25),
            f({ maximumFractionDigits: 1, roundingMode: 'ceil' }, -1.21),
            f({ maximumFractionDigits: 2, minimumFractionDigits: 2, roundingIncrement: 25 }, 1.13),
            f({ maximumSignificantDigits: 2 }, 987654),
            f({ maximumFractionDigits: 20 }, '1.0000000000000001'),
            f({}, 12345678901234567890n),
            f({ minimumFractionDigits: 2, trailingZeroDisplay: 'stripIfInteger' }, 3)].join('|');";
    assert_eq!(
        eval(source, "out"),
        "1.2|-1.2|1.25|990,000|1.0000000000000001|12,345,678,901,234,567,890|3"
    );
}

/** Notations: compact (short and long, re-chosen on rounding), scientific and engineering. */
#[test]
fn compact_scientific_and_engineering() {
    let source = "const f = (o, x) => new Intl.NumberFormat('en-US', o).format(x);
        const out = [f({ notation: 'compact' }, 1234), f({ notation: 'compact' }, 999999),
            f({ notation: 'compact', compactDisplay: 'long' }, 987654321),
            f({ notation: 'scientific' }, 0.000345), f({ notation: 'engineering' }, 98765)].join('|');";
    assert_eq!(eval(source, "out"), "1.2K|1M|988 million|3.45E-4|98.765E3");
}

/** Sign display, accounting currency, currency names and units. */
#[test]
fn signs_currencies_and_units() {
    let source = "const f = (o, x) => new Intl.NumberFormat('en-US', o).format(x);
        const out = [f({ signDisplay: 'exceptZero' }, -0.0001), f({ signDisplay: 'always' }, NaN),
            f({ style: 'currency', currency: 'USD', currencySign: 'accounting' }, -987),
            f({ style: 'currency', currency: 'EUR', currencyDisplay: 'name' }, 2),
            f({ style: 'currency', currency: 'JPY' }, 5.5),
            f({ style: 'unit', unit: 'kilometer-per-hour', unitDisplay: 'long' }, 987),
            f({ style: 'percent' }, 0.256)].join('|');";
    assert_eq!(
        eval(source, "out"),
        "0|+NaN|($987.00)|2.00 euros|¥6|987 kilometers per hour|26%"
    );
}

/** formatToParts types each part, formatRange joins with an en dash or marks equal ends. */
#[test]
fn parts_and_ranges() {
    let source = "const nf = new Intl.NumberFormat('en-US');
        const parts = nf.formatToParts(-1234.5).map(p => p.type).join();
        const out = [parts, nf.formatRange(3, 5), nf.formatRange(3, 3),
            typeof nf.format, nf.format === undefined].join('|');";
    assert_eq!(
        eval(source, "out"),
        "minusSign,integer,group,integer,decimal,fraction|3–5|~3|function|false"
    );
}

/** PluralRules follows the digit options and orders its categories as CLDR does. */
#[test]
fn plural_rules_select_and_categories() {
    let source = "const one = new Intl.PluralRules('en');
        const two = new Intl.PluralRules('en', { minimumFractionDigits: 1 });
        const ord = new Intl.PluralRules('en', { type: 'ordinal' });
        const out = [one.select(1), two.select(1), ord.select(22), ord.select(13),
            ord.resolvedOptions().pluralCategories.join('/'), one.selectRange(1, 5)].join();";
    assert_eq!(
        eval(source, "out"),
        "one,other,two,other,one/two/few/other,other"
    );
}
