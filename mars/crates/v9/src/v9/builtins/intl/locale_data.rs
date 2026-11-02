/*!
 * Compact CLDR number data for the locales pages and test262 use most, hand-copied from the CLDR
 * JSON (numbers.json `latn` symbols, decimal, percent, currency and accounting patterns, compact
 * short and long patterns, misc range and approximately patterns, currencies.json symbols and
 * plurals.json / ordinals.json rules). Only what `number_format` reads is kept; every other
 * language falls back to `en`.
 */

/** One locale's number symbols and patterns. */
pub(super) struct Symbols {
    /** Decimal separator. */
    pub decimal: &'static str,
    /** Grouping separator. */
    pub group: &'static str,
    /** Primary grouping size (the digits nearest the decimal point). */
    pub primary: usize,
    /** Secondary grouping size (3, or 2 for the Indian lakh and crore). */
    pub secondary: usize,
    /** minimumGroupingDigits: digits above the first group before `auto` groups at all. */
    pub min_grouping: usize,
    /** The NaN symbol. */
    pub nan: &'static str,
    /** The approximately sign of the misc patterns. */
    pub approx: &'static str,
    /** The range pattern's separator between `{0}` and `{1}`. */
    pub range: &'static str,
    /** The text between a number and its percent sign. */
    pub percent_gap: &'static str,
    /** Whether the currency symbol leads the number (`¤#,##0.00`) or trails it. */
    pub currency_first: bool,
    /** The pattern's own text between symbol and number (`¤ #` in pt, ` ¤` in de). */
    pub currency_gap: &'static str,
    /** Whether the accounting pattern wraps negatives in parentheses. */
    pub accounting_parens: bool,
    /** Compact short suffixes per exponent, leading space included when CLDR has one. */
    pub compact_short: &'static [(i64, &'static str)],
    /** Compact long suffixes per exponent (the plural "other" forms). */
    pub compact_long: &'static [(i64, &'static str)],
}

/** No-break space. */
const NBSP: &str = "\u{a0}";
/** Narrow no-break space (French grouping and percent). */
const NNBSP: &str = "\u{202f}";

/** en compact short. */
const EN_SHORT: &[(i64, &str)] = &[(3, "K"), (6, "M"), (9, "B"), (12, "T")];
/** en compact long. */
const EN_LONG: &[(i64, &str)] = &[
    (3, " thousand"),
    (6, " million"),
    (9, " billion"),
    (12, " trillion"),
];

/** en: the default and fallback. */
const EN: Symbols = Symbols {
    decimal: ".",
    group: ",",
    primary: 3,
    secondary: 3,
    min_grouping: 1,
    nan: "NaN",
    approx: "~",
    range: "–",
    percent_gap: "",
    currency_first: true,
    currency_gap: "",
    accounting_parens: true,
    compact_short: EN_SHORT,
    compact_long: EN_LONG,
};

/** The table, longest keys first within a language so `lookup` takes the most specific. */
const LOCALES: &[(&str, Symbols)] = &[
    (
        "en-IN",
        Symbols {
            secondary: 2,
            compact_short: &[(3, "K"), (5, "L"), (7, "Cr")],
            compact_long: &[(3, " thousand"), (5, " lakh"), (7, " crore")],
            ..EN
        },
    ),
    ("en-GB", EN),
    ("en", EN),
    (
        "de",
        Symbols {
            decimal: ",",
            group: ".",
            approx: "≈",
            percent_gap: NBSP,
            currency_first: false,
            currency_gap: NBSP,
            accounting_parens: false,
            compact_short: &[(6, "\u{a0}Mio."), (9, "\u{a0}Mrd."), (12, "\u{a0}Bio.")],
            compact_long: &[
                (3, " Tausend"),
                (6, " Millionen"),
                (9, " Milliarden"),
                (12, " Billionen"),
            ],
            ..EN
        },
    ),
    (
        "fr",
        Symbols {
            decimal: ",",
            group: NNBSP,
            approx: "≃",
            percent_gap: NNBSP,
            currency_first: false,
            currency_gap: NBSP,
            compact_short: &[
                (3, "\u{a0}k"),
                (6, "\u{a0}M"),
                (9, "\u{a0}Md"),
                (12, "\u{a0}Bn"),
            ],
            compact_long: &[
                (3, " mille"),
                (6, " millions"),
                (9, " milliards"),
                (12, " billions"),
            ],
            ..EN
        },
    ),
    (
        "es",
        Symbols {
            decimal: ",",
            group: ".",
            min_grouping: 2,
            range: "-",
            percent_gap: NBSP,
            currency_first: false,
            currency_gap: NBSP,
            accounting_parens: false,
            compact_short: &[
                (3, "\u{a0}mil"),
                (6, "\u{a0}M"),
                (9, "\u{a0}mil\u{a0}M"),
                (12, "\u{a0}B"),
            ],
            compact_long: &[
                (3, " mil"),
                (6, " millones"),
                (9, " mil millones"),
                (12, " billones"),
            ],
            ..EN
        },
    ),
    (
        "it",
        Symbols {
            decimal: ",",
            group: ".",
            range: "-",
            currency_first: false,
            currency_gap: NBSP,
            accounting_parens: false,
            compact_short: &[(6, "\u{a0}Mln"), (9, "\u{a0}Mrd"), (12, "\u{a0}Bln")],
            compact_long: &[
                (3, " mila"),
                (6, " milioni"),
                (9, " miliardi"),
                (12, " mila miliardi"),
            ],
            ..EN
        },
    ),
    (
        "pt-PT",
        Symbols {
            decimal: ",",
            group: NBSP,
            min_grouping: 2,
            range: " - ",
            currency_first: false,
            currency_gap: NBSP,
            compact_short: &[
                (3, "\u{a0}mil"),
                (6, "\u{a0}M"),
                (9, "\u{a0}mM"),
                (12, "\u{a0}Bi"),
            ],
            compact_long: &[
                (3, " mil"),
                (6, " milhões"),
                (9, " mil milhões"),
                (12, " biliões"),
            ],
            ..EN
        },
    ),
    (
        "pt",
        Symbols {
            decimal: ",",
            group: ".",
            range: "–",
            currency_gap: NBSP,
            accounting_parens: false,
            compact_short: &[
                (3, "\u{a0}mil"),
                (6, "\u{a0}mi"),
                (9, "\u{a0}bi"),
                (12, "\u{a0}tri"),
            ],
            compact_long: &[
                (3, " mil"),
                (6, " milhões"),
                (9, " bilhões"),
                (12, " trilhões"),
            ],
            ..EN
        },
    ),
    (
        "ja",
        Symbols {
            approx: "約",
            range: "～",
            compact_short: &[(4, "万"), (8, "億"), (12, "兆")],
            compact_long: &[(4, "万"), (8, "億"), (12, "兆")],
            ..EN
        },
    ),
    (
        "ko",
        Symbols {
            range: "~",
            compact_short: &[(3, "천"), (4, "만"), (8, "억"), (12, "조")],
            compact_long: &[(3, "천"), (4, "만"), (8, "억"), (12, "조")],
            ..EN
        },
    ),
    (
        "zh-TW",
        Symbols {
            nan: "非數值",
            range: "-",
            compact_short: &[(4, "萬"), (8, "億"), (12, "兆")],
            compact_long: &[(4, "萬"), (8, "億"), (12, "兆")],
            ..EN
        },
    ),
    (
        "zh",
        Symbols {
            range: "-",
            compact_short: &[(4, "万"), (8, "亿"), (12, "万亿")],
            compact_long: &[(4, "万"), (8, "亿"), (12, "万亿")],
            ..EN
        },
    ),
    (
        "ru",
        Symbols {
            decimal: ",",
            group: NBSP,
            approx: "≈",
            percent_gap: NBSP,
            currency_first: false,
            currency_gap: NBSP,
            accounting_parens: false,
            compact_short: &[
                (3, "\u{a0}тыс."),
                (6, "\u{a0}млн"),
                (9, "\u{a0}млрд"),
                (12, "\u{a0}трлн"),
            ],
            compact_long: &[
                (3, " тысячи"),
                (6, " миллиона"),
                (9, " миллиарда"),
                (12, " триллиона"),
            ],
            ..EN
        },
    ),
    (
        "hi",
        Symbols {
            secondary: 2,
            compact_short: &[
                (3, "\u{a0}हज़ार"),
                (5, "\u{a0}लाख"),
                (7, "\u{a0}क॰"),
                (9, "\u{a0}अ॰"),
                (11, "\u{a0}ख॰"),
            ],
            compact_long: &[
                (3, " हज़ार"),
                (5, " लाख"),
                (7, " करोड़"),
                (9, " अरब"),
                (11, " खरब"),
            ],
            ..EN
        },
    ),
];

/** Symbols that differ from the en symbol, per (table key, currency code). */
const CURRENCY_SYMBOLS: &[(&str, &str, &str)] = &[
    ("en-GB", "USD", "US$"),
    ("fr", "USD", "$US"),
    ("es", "USD", "US$"),
    ("it", "USD", "USD"),
    ("pt", "USD", "US$"),
    ("pt-PT", "USD", "US$"),
    ("ko", "USD", "US$"),
    ("zh", "USD", "US$"),
    ("zh-TW", "USD", "US$"),
    ("ja", "JPY", "￥"),
    ("zh", "CNY", "¥"),
    ("zh-TW", "TWD", "$"),
    ("ru", "RUB", "₽"),
    ("ko", "KRW", "₩"),
    ("pt", "BRL", "R$"),
    ("fr", "CAD", "$CA"),
];

/** The table key a resolved tag reads: `zh-Hant*` and `zh-TW` share Traditional Chinese, a
region picks its own row when it has one, otherwise the language's. */
fn key(locale: &str) -> &str {
    let mut parts = locale.split('-');
    let lang = parts.next().unwrap_or("");
    let rest: Vec<&str> = parts.collect();
    if lang == "zh"
        && rest
            .iter()
            .any(|p| *p == "Hant" || *p == "TW" || *p == "HK")
    {
        return "zh-TW";
    }
    let region = rest
        .iter()
        .find(|p| p.len() == 2 || p.len() == 3 && p.bytes().all(|b| b.is_ascii_digit()));
    if let Some(region) = region
        && let Some((key, _)) = LOCALES.iter().find(|(k, _)| {
            k.split_once('-')
                .is_some_and(|(l, r)| l == lang && r == *region)
        })
    {
        return key;
    }
    LOCALES
        .iter()
        .find(|(k, _)| *k == lang)
        .map_or("en", |(k, _)| k)
}

/** The symbols a resolved locale formats with. */
pub(super) fn symbols(locale: &str) -> &'static Symbols {
    let key = key(locale);
    LOCALES
        .iter()
        .find(|(k, _)| *k == key)
        .map_or(&EN, |(_, s)| s)
}

/** The locale's symbol for a currency when it differs from en's. */
pub(super) fn currency_symbol(locale: &str, code: &str) -> Option<&'static str> {
    let key = key(locale);
    let lang = key.split('-').next().unwrap_or(key);
    let find = |k: &str| {
        CURRENCY_SYMBOLS
            .iter()
            .find(|row| row.0 == k && row.1 == code)
            .map(|row| row.2)
    };
    find(key).or_else(|| (key != lang).then(|| find(lang)).flatten())
}

/** CLDR plural operands of a formatted number. */
pub(super) struct Operands<'a> {
    /** Integer digits. */
    pub int: &'a str,
    /** Visible fraction digits, trailing zeros kept. */
    pub frac: &'a str,
    /** The compact exponent (CLDR `e`). */
    pub exp: i64,
}

impl Operands<'_> {
    /** `i` modulo 10^7, enough for every rule's modulus. */
    fn i(&self) -> u64 {
        let tail = &self.int[self.int.len().saturating_sub(7)..];
        tail.parse().unwrap_or(0)
    }
    /** Whether the integer part is exactly `value`. */
    fn i_is(&self, value: u64) -> bool {
        self.int.trim_start_matches('0').len() <= 7 && self.i() == value
    }
    /** `v`: the count of visible fraction digits. */
    fn v(&self) -> usize {
        self.frac.len()
    }
    /** Whether `n` is an integer (every fraction digit zero). */
    fn n_integer(&self) -> bool {
        self.frac.bytes().all(|b| b == b'0')
    }
    /** `n = value`. */
    fn n_is(&self, value: u64) -> bool {
        self.n_integer() && self.i_is(value)
    }
    /** The "many" rule French, Spanish, Italian and Portuguese share for millions. */
    fn millions(&self) -> bool {
        let big =
            !self.int.trim_start_matches('0').is_empty() && self.i().is_multiple_of(1_000_000);
        self.exp == 0 && big && self.v() == 0 || !(0..=5).contains(&self.exp)
    }
}

/** The language plural rules dispatch on: the primary subtag, with European Portuguese apart. */
pub(super) fn plural_lang(locale: &str) -> &str {
    let lang = locale.split('-').next().unwrap_or(locale);
    if key(locale) == "pt-PT" {
        "pt-PT"
    } else {
        lang
    }
}

/** The cardinal plural category of `n` in `lang` (CLDR plurals.json). */
pub(super) fn cardinal(lang: &str, n: &Operands) -> &'static str {
    let v0 = n.v() == 0;
    let i = n.i();
    match lang {
        "ar" if n.n_is(0) => "zero",
        "ar" if n.n_is(1) => "one",
        "ar" if n.n_is(2) => "two",
        "ar" if n.n_integer() && (3..=10).contains(&(i % 100)) => "few",
        "ar" if n.n_integer() && (11..=99).contains(&(i % 100)) => "many",
        "sl" if v0 && i % 100 == 1 => "one",
        "sl" if v0 && i % 100 == 2 => "two",
        "sl" if !v0 || (3..=4).contains(&(i % 100)) => "few",
        "gv" if !v0 => "many",
        "gv" if i % 10 == 1 => "one",
        "gv" if i % 10 == 2 => "two",
        "gv" if [0, 20, 40, 60, 80].contains(&(i % 100)) => "few",
        "ja" | "ko" | "zh" => "other",
        "fr" | "pt" if n.i_is(0) || n.i_is(1) => "one",
        "es" if n.n_is(1) => "one",
        "it" | "pt-PT" if n.i_is(1) && v0 => "one",
        "fr" | "pt" | "es" | "it" | "pt-PT" if n.millions() => "many",
        "ru" if v0 && i % 10 == 1 && i % 100 != 11 => "one",
        "ru" if v0 && (2..=4).contains(&(i % 10)) && !(12..=14).contains(&(i % 100)) => "few",
        "ru" if v0 => "many",
        "hi" if n.i_is(0) || n.n_is(1) => "one",
        "en" | "de" if n.i_is(1) && v0 => "one",
        _ => "other",
    }
}

/** The ordinal plural category of `n` in `lang` (CLDR ordinals.json). */
pub(super) fn ordinal(lang: &str, n: &Operands) -> &'static str {
    let i = n.i();
    match lang {
        "en" if i % 10 == 1 && i % 100 != 11 => "one",
        "en" if i % 10 == 2 && i % 100 != 12 => "two",
        "en" if i % 10 == 3 && i % 100 != 13 => "few",
        "fr" if n.n_is(1) => "one",
        "it" if [11, 8, 80, 800].iter().any(|k| n.n_is(*k)) => "many",
        "hi" if n.n_is(1) => "one",
        "hi" if n.n_is(2) || n.n_is(3) => "two",
        "hi" if n.n_is(4) => "few",
        "hi" if n.n_is(6) => "many",
        _ => "other",
    }
}

/** The categories `resolvedOptions().pluralCategories` lists, in CLDR order. */
pub(super) fn categories(lang: &str, ordinal: bool) -> &'static [&'static str] {
    match (lang, ordinal) {
        ("en", true) => &["one", "two", "few", "other"],
        ("fr", true) => &["one", "other"],
        ("it", true) => &["many", "other"],
        ("hi", true) => &["one", "two", "few", "many", "other"],
        (_, true) | ("ja" | "ko" | "zh", false) => &["other"],
        ("fr" | "pt" | "es" | "it" | "pt-PT", false) => &["one", "many", "other"],
        ("ru", false) => &["one", "few", "many", "other"],
        ("ar", false) => &["zero", "one", "two", "few", "many", "other"],
        ("sl", false) => &["one", "two", "few", "other"],
        ("gv", false) => &["one", "two", "few", "many", "other"],
        _ => &["one", "other"],
    }
}
