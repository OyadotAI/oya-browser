/** Intl.NumberFormat and Intl.PluralRules read CLDR locale data: separators and grouping sizes,
currency and percent patterns, compact names, range collapsing and plural rules per locale. */
mod common;
use common::eval;

/** Each locale formats with its own decimal and group separators and grouping sizes. */
#[test]
fn separators_and_grouping_follow_the_locale() {
    let source = "const f = (l, x, o) => new Intl.NumberFormat(l, o).format(x);
        const out = [f('de-DE', 1234567.5), f('fr', 1234.5), f('en-IN', 12345678),
            f('es', 1234), f('es', 12345), f('pt-PT', 1234567), f('ja-JP', 1234.5),
            f('de', 1000, { useGrouping: 'always' }), f('hi', 100000)].join('|');";
    assert_eq!(
        eval(source, "out"),
        "1.234.567,5|1\u{202f}234,5|1,23,45,678|1234|12.345|1\u{a0}234\u{a0}567|1,234.5|1.000|1,00,000"
    );
}

/** Currency position, spacing, local symbols and accounting parentheses follow the locale. */
#[test]
fn currency_patterns_follow_the_locale() {
    let source = "const f = (l, c, x, o) => new Intl.NumberFormat(l,
            Object.assign({ style: 'currency', currency: c }, o)).format(x);
        const acc = { currencySign: 'accounting' };
        const out = [f('de-DE', 'USD', -987, acc), f('ko-KR', 'USD', -987, acc),
            f('ja-JP', 'USD', -987, acc), f('fr', 'EUR', 5), f('pt-BR', 'BRL', 5),
            f('en-GB', 'USD', 5), f('en', 'CHF', 5)].join('|');";
    assert_eq!(
        eval(source, "out"),
        "-987,00\u{a0}$|(US$987.00)|($987.00)|5,00\u{a0}€|R$\u{a0}5,00|US$5.00|CHF\u{a0}5.00"
    );
}

/** Percent spacing, NaN and compact names are the locale's, with its compact exponents. */
#[test]
fn percent_nan_and_compact_follow_the_locale() {
    let source = "const f = (l, x, o) => new Intl.NumberFormat(l, o).format(x);
        const c = { notation: 'compact' }, cl = { notation: 'compact', compactDisplay: 'long' };
        const out = [f('de', 0.5, { style: 'percent' }), f('zh-TW', NaN),
            f('de-DE', 987654321, c), f('de-DE', 98765, c), f('de-DE', 9876, cl),
            f('ja-JP', 98765432, c), f('ko-KR', 9876, c), f('zh-TW', 987654321, c)].join('|');";
    assert_eq!(
        eval(source, "out"),
        "50\u{a0}%|非數值|988\u{a0}Mio.|98.765|9,9 Tausend|9877万|9.9천|9.9億"
    );
}

/** Ranges collapse equal affixes wider than one code point and space the locale separator. */
#[test]
fn ranges_collapse_like_icu() {
    let source = "const nf = (l, o) => new Intl.NumberFormat(l, o);
        const eur = { style: 'currency', currency: 'EUR', maximumFractionDigits: 0 };
        const usd = { style: 'currency', currency: 'USD', signDisplay: 'always' };
        const out = [nf('pt-PT', eur).formatRange(3, 5), nf('pt-PT', eur).formatRange(2.9, 3.1),
            nf('en-US', usd).formatRange(2.9, 3.1),
            nf('en-US', { style: 'currency', currency: 'USD' }).formatRange(3, 5)].join('|');";
    assert_eq!(
        eval(source, "out"),
        "3 - 5\u{a0}€|~3\u{a0}€|+$2.90–3.10|$3.00 – $5.00"
    );
}

/** Plural rules and their categories are the locale's, with the compact exponent as `e`. */
#[test]
fn plural_rules_follow_the_locale() {
    let source = "const s = (l, x, o) => new Intl.PluralRules(l, o).select(x);
        const cats = (l) => new Intl.PluralRules(l).resolvedOptions().pluralCategories.join(' ');
        const out = [s('fr', 1e6), s('fr', 1.5e6), s('fr', 1.5e6, { notation: 'compact' }),
            s('fr', 0), s('ru', 3), s('ru', 11), s('ru', 21), s('ja', 1), s('ar', 0),
            s('ar', 105), cats('ar'), cats('fr'), cats('ko')].join('|');";
    assert_eq!(
        eval(source, "out"),
        "many|other|many|one|few|many|one|other|zero|few|zero one two few many other|one many other|other"
    );
}
