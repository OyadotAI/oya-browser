/** Intl.NumberFormat and Intl.PluralRules read CLDR locale data: separators and grouping sizes,
currency and percent patterns, compact names, range collapsing and plural rules per locale. */
use crate::common;
use common::eval;

/** Omitted Intl locales use the page's host locale across formatter families. */
#[test]
fn omitted_locales_follow_the_host_environment() {
    let mut environment = common::environment();
    environment.locale = "pt-BR".into();
    let source = "console.log([Intl.Collator().resolvedOptions().locale,Intl.DateTimeFormat().resolvedOptions().locale,Intl.NumberFormat().resolvedOptions().locale,new Intl.DisplayNames(undefined,{type:'language'}).resolvedOptions().locale].join(','))";
    let output = common::load_with(source, &environment);
    assert!(output.failed.is_empty(), "{:?}", common::failure(&output));
    assert_eq!(common::console(&output), ["pt-BR,pt-BR,pt-BR,pt-BR"]);
}

/** Arabic numbering retains typed directional marks and the same text across both APIs. */
#[test]
fn arabic_numbering_formats_parts_and_resolves_options() {
    let source = "const f = new Intl.NumberFormat('ar-IQ', {style:'percent', minimumFractionDigits:2});
        const p = f.formatToParts(-135.32);
        const out = [p.map(x => x.type + ':' + x.value).join('|'), p.map(x => x.value).join('') === f.format(-135.32), f.resolvedOptions().numberingSystem].join(';');";
    assert_eq!(
        eval(source, "out"),
        "literal:؜|minusSign:-|integer:١٣|group:٬|integer:٥٣٢|decimal:٫|fraction:٠٠|percentSign:٪|literal:؜;true;arab"
    );
}

/** Explicit numbering wins over the Unicode extension and other locales retain their default. */
#[test]
fn numbering_system_selection_obeys_options_and_extensions() {
    let source = "const a = new Intl.NumberFormat('en-US-u-nu-arab');
        const b = new Intl.NumberFormat('en-US-u-nu-arab', {numberingSystem:'latn'});
        const c = new Intl.NumberFormat('ar-MA');
        const out = [a.format(12.5), a.resolvedOptions().numberingSystem,
            b.format(12.5), b.resolvedOptions().numberingSystem, c.resolvedOptions().numberingSystem].join('|');";
    assert_eq!(eval(source, "out"), "١٢٫٥|arab|12.5|latn|latn");
}

/** Negative zero, explicit Latin digits, and unsupported options follow the chosen system. */
#[test]
fn arabic_numbering_preserves_signs_and_ignores_unsupported_options() {
    let source = "const a = new Intl.NumberFormat('ar-IQ');
        const b = new Intl.NumberFormat('ar-IQ', {numberingSystem:'latn'});
        const c = new Intl.NumberFormat('en-US-u-nu-arab', {numberingSystem:'blah'});
        const out = [a.format(-0), a.format(1234.5), b.format(-1234.5),
            c.resolvedOptions().locale, c.resolvedOptions().numberingSystem].join('|');";
    assert_eq!(
        eval(source, "out"),
        "؜-٠|١٬٢٣٤٫٥|‎-1,234.5|en-US-u-nu-arab|arab"
    );
}

/** Arabic numbering selects matching patterns across locales and survives locale round trips. */
#[test]
fn arabic_numbering_uses_its_own_patterns_and_canonical_locale() {
    let source = "const a = new Intl.NumberFormat('en-US-u-nu-arab-x-foo');
        const loc = a.resolvedOptions().locale;
        const p = new Intl.NumberFormat('de-DE-u-nu-arab', {style:'percent'});
        const c = new Intl.NumberFormat('en-US-u-nu-arab', {style:'currency', currency:'USD'});
        const s = new Intl.NumberFormat('en-US-u-nu-arab', {notation:'scientific'});
        const out = [loc, new Intl.NumberFormat(loc).format(1234), p.format(-1234.5), c.format(-1234.5), s.format(-1234.5)].join('|');";
    assert_eq!(
        eval(source, "out"),
        "en-US-u-nu-arab|١٬٢٣٤|؜-١٢٣٬٤٥٠٪؜|؜-١٬٢٣٤٫٥٠ $|؜-١٫٢٣٥اس٣"
    );
}

/** Arabic accounting and negative scientific exponents use directional sign marks. */
#[test]
fn arabic_numbering_formats_accounting_and_negative_exponents() {
    let source = "const c = new Intl.NumberFormat('en-US-u-nu-arab', {style:'currency', currency:'USD', currencySign:'accounting'});
        const s = new Intl.NumberFormat('en-US-u-nu-arab', {notation:'scientific'});
        const out = [c.format(-0.0012), s.format(-0.0012)].join('|');";
    assert_eq!(eval(source, "out"), "؜-٠٫٠٠ $|؜-١٫٢اس؜-٣");
}

/** Private-use tags cannot alter the resolved locale's default numbering system. */
#[test]
fn arabic_default_ignores_private_use_extensions() {
    let source = "const f = new Intl.NumberFormat('ar-IQ-x-foo');
        const out = [f.resolvedOptions().locale, f.resolvedOptions().numberingSystem, f.format(-1234.5)].join('|');";
    assert_eq!(eval(source, "out"), "ar-IQ|arab|؜-١٬٢٣٤٫٥");
}

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
