/** navigator.userAgentData answers as Chrome 152 on a Mac does, with the same values Relay sends
as client hints, so a page's own checks agree with its request headers. */
use crate::common;

/** The low-entropy view: one stable object, the three brands in Sec-CH-UA's order, macOS. */
#[test]
fn low_entropy_values_match_sec_ch_ua() {
    let source = "const d=navigator.userAgentData;const r=[d===navigator.userAgentData,\
        d.brands.map(b=>b.brand+'/'+b.version).join(' '),d.mobile,d.platform,JSON.stringify(d.toJSON().platform)].join('|');";
    let expected = format!(
        "true|Not?A_Brand/24 Chromium/{major} Mars/{major}|false|macOS|\"macOS\"",
        major = mars_v9::constants::CHROME_FULL_VERSION
            .split('.')
            .next()
            .unwrap()
    );
    let mut environment = common::environment();
    environment.user_agent = format!(
        "Mozilla/5.0 Chrome/{} Safari/537.36",
        mars_v9::constants::CHROME_FULL_VERSION
    );
    let execution = common::load_with(source, &environment);
    assert_eq!(common::failure(&execution), None);
    assert_eq!(common::global(&execution, "r"), expected);
}

/** getHighEntropyValues resolves with exactly the hints asked for, plus the low-entropy three. */
#[test]
fn high_entropy_values_resolve_with_the_requested_hints() {
    let source = "let r='';navigator.userAgentData.getHighEntropyValues(['platformVersion','uaFullVersion','bitness','nope'])\
        .then(v=>{r=[v.platformVersion,v.uaFullVersion,v.bitness,'nope' in v,v.platform].join('|')});";
    let expected = format!(
        "{}|{}|64|false|macOS",
        mars_v9::constants::MACOS_PLATFORM_VERSION,
        mars_v9::constants::CHROME_FULL_VERSION
    );
    assert_eq!(common::eval(source, "r"), expected);
}
