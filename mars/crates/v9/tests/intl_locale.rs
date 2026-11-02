/**
 * ECMA-402 locale handling (test262 intl402/Locale, intl402/Intl/getCanonicalLocales and
 * intl402/DisplayNames): canonical tags, invalid tags throwing RangeError, `Intl.Locale`
 * options and getters, likely subtags, Locale Info and `Intl.DisplayNames`. formatjs builds on
 * these when a page like craigslist starts.
 */
use crate::common;

use common::eval;

/** getCanonicalLocales canonicalizes case, aliases and order, and drops duplicates. */
#[test]
fn canonical_locales_are_canonical_and_unique() {
    let out = eval(
        "const out = Intl.getCanonicalLocales(['DE-de', 'de-DE', 'cmn-hans-cn', 'art-lojban',
           'sl-rozaj-biske-1994', 'en-u-kn-true']).join('|');",
        "out",
    );
    assert_eq!(out, "de-DE|zh-Hans-CN|jbo|sl-1994-biske-rozaj|en-u-kn");
}

/** Structurally invalid tags throw RangeError; non-string, non-object elements TypeError. */
#[test]
fn invalid_tags_throw() {
    let out = eval(
        "const kind = f => { try { f(); return 'ok' } catch (e) { return e.name } };
         const out = [kind(() => Intl.getCanonicalLocales('en-GB-oed')),
           kind(() => Intl.getCanonicalLocales([1])), kind(() => new Intl.Locale('x-private')),
           kind(() => Intl.Locale('en'))].join('|');",
        "out",
    );
    assert_eq!(out, "RangeError|TypeError|RangeError|TypeError");
}

/** Options override the tag's subtags and keywords, and the getters read them back. */
#[test]
fn locale_options_and_getters() {
    let out = eval(
        "const l = new Intl.Locale('en-latn-us-u-ca-gregory', {region: 'GB', hourCycle: 'h23',
           numeric: true});
         const out = [l.toString(), l.baseName, l.language, l.script, l.region, l.calendar,
           l.hourCycle, l.numeric, Object.prototype.toString.call(l)].join('|');",
        "out",
    );
    assert_eq!(
        out,
        "en-Latn-GB-u-ca-gregory-hc-h23-kn|en-Latn-GB|en|Latn|GB|gregory|h23|true|[object Intl.Locale]"
    );
}

/** maximize and minimize follow CLDR likely subtags. */
#[test]
fn likely_subtags() {
    let out = eval(
        "const out = [new Intl.Locale('und').maximize(), new Intl.Locale('zh-TW').maximize(),
           new Intl.Locale('en-Latn-US').minimize(), new Intl.Locale('en-Arab-US').minimize()]
           .join('|');",
        "out",
    );
    assert_eq!(out, "en-Latn-US|zh-Hant-TW|en|en-Arab");
}

/** Locale Info answers from the keyword first, then the likely region. */
#[test]
fn locale_info() {
    let out = eval(
        "const us = new Intl.Locale('en-US'), de = new Intl.Locale('de-u-fw-sun');
         const out = [us.getHourCycles(), de.getHourCycles(), us.getWeekInfo().firstDay,
           de.getWeekInfo().firstDay, new Intl.Locale('ar').getTextInfo().direction,
           new Intl.Locale('ja').getTimeZones()].join('|');",
        "out",
    );
    assert_eq!(out, "h12|h23|7|7|rtl|");
}

/** DisplayNames names known codes, falls back to the canonical code, and checks its options. */
#[test]
fn display_names() {
    let out = eval(
        "const r = new Intl.DisplayNames('en', {type: 'region'});
         const n = new Intl.DisplayNames('en', {type: 'script', fallback: 'none'});
         let missing; try { new Intl.DisplayNames('en') } catch (e) { missing = e.name }
         const out = [r.of('us'), r.of('zz'), n.of('zzzz'), missing,
           Object.keys(r.resolvedOptions())].join('|');",
        "out",
    );
    assert_eq!(
        out,
        "United States|ZZ||TypeError|locale,style,type,fallback"
    );
}
