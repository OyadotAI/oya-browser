/**
 * The browser a persona says it is: User-Agent string, navigator.userAgentData and client-hint
 * headers built together from one device, ported from Oya's browser/src/main/identity/identity.ts
 * and the validated native subset of browser/src/main/native-policy/persona.ts. Mars has no Chrome
 * engine of its own, so the Chrome version claimed is an explicit input, validated the way Oya
 * validates its engine's; a persona never inherits this machine's identity by default.
 */
use crate::constants::{MAX_POLICY_TEXT, MAX_PROCESSORS, MIN_PROCESSORS};
use crate::profile::Profile;
use serde::Serialize;

/// Chromium's GREASE alphabet (components/embedder_support/user_agent_utils.cc).
const GREASE_CHARS: [char; 11] = [' ', '(', ':', '-', '.', '/', ')', ';', '=', '?', '_'];
/// The versions a GREASE brand takes.
const GREASE_VERSIONS: [&str; 3] = ["8", "99", "24"];
/// Slots the GREASE, Chromium and Google Chrome brands take, by `major % 6`.
const BRAND_ORDERS: [[usize; 3]; 6] = [
    [0, 1, 2],
    [0, 2, 1],
    [1, 0, 2],
    [1, 2, 0],
    [2, 0, 1],
    [2, 1, 0],
];
/// Platforms the identity can present: navigator.platform, UA OS part, hint platform and version.
const PLATFORMS: [(&str, &str, &str, &str); 3] = [
    ("Win32", "Windows NT 10.0; Win64; x64", "Windows", "15.0.0"),
    (
        "MacIntel",
        "Macintosh; Intel Mac OS X 10_15_7",
        "macOS",
        "14.6.1",
    ),
    ("Linux x86_64", "X11; Linux x86_64", "Linux", ""),
];
/// IANA areas a system timezone may be in. ponytail: no tz database in the crate; an area check
/// refuses invented zones like `Mars/Olympus`, a real tzdb lookup can replace it when V9 has one.
const ZONE_AREAS: [&str; 11] = [
    "Africa",
    "America",
    "Antarctica",
    "Arctic",
    "Asia",
    "Atlantic",
    "Australia",
    "Europe",
    "Indian",
    "Pacific",
    "Etc",
];

/// Why a persona's identity was refused; the text names the field, as Oya's errors do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityError(pub String);

impl std::fmt::Display for IdentityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for IdentityError {}

/// The refusal for `message`.
fn refuse<T>(message: &str) -> Result<T, IdentityError> {
    Err(IdentityError(message.into()))
}

/// One entry of a brand list.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Brand {
    /// The brand's name.
    pub brand: String,
    /// Its major version, or its full one in a full-version list.
    pub version: String,
}

/// Everything navigator.userAgentData can say.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UserAgentMetadata {
    /// Brands with major versions, in Chrome's seeded order.
    pub brands: Vec<Brand>,
    /// Brands with full versions.
    pub full_version_list: Vec<Brand>,
    /// The claimed engine's full version.
    pub full_version: String,
    /// The OS name.
    pub platform: String,
    /// The OS version.
    pub platform_version: String,
    /// `arm` for an Apple-Silicon Mac, else `x86`.
    pub architecture: String,
    /// Empty on a desktop.
    pub model: String,
    /// Never, on a desktop.
    pub mobile: bool,
    /// Always 64.
    pub bitness: String,
    /// Never, on a 64-bit browser.
    pub wow64: bool,
    /// Always `Desktop`.
    pub form_factors: Vec<String>,
}

/// The validated identity subset a persona presents: the same fields as Oya's NativePolicy.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NativePolicy {
    /// IANA timezone.
    pub time_zone: String,
    /// Canonical BCP 47 locale.
    pub locale: String,
    /// navigator.hardwareConcurrency.
    pub hardware_concurrency: u32,
    /// navigator.languages: the locale and its primary language.
    pub languages: Vec<String>,
    /// navigator.platform.
    pub platform: String,
    /// The User-Agent string, with the reduced `major.0.0.0` version Chrome sends.
    pub user_agent: String,
    /// navigator.userAgentData.
    pub user_agent_metadata: UserAgentMetadata,
}

/// The full Chrome version a persona claims, taken from an engine User-Agent; never a fallback.
pub fn chrome_version(engine_user_agent: &str) -> Result<String, IdentityError> {
    // Oya: the first `Chrome/a.b.c.d` followed by whitespace or the end, then no leading zero.
    let version = engine_user_agent.split("Chrome/").skip(1).find_map(|rest| {
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        Some(&rest[..end]).filter(|token| four_numbers(token))
    });
    match version {
        Some(version) if !version.starts_with('0') => Ok(version.into()),
        _ => refuse("Native persona requires the actual engine Chrome version"),
    }
}

/// `\d+\.\d+\.\d+\.\d+`.
fn four_numbers(token: &str) -> bool {
    let parts: Vec<&str> = token.split('.').collect();
    parts.len() == 4
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

/// The platform row for a navigator.platform the identity can present.
fn platform_row(
    platform: &str,
) -> Option<(&'static str, &'static str, &'static str, &'static str)> {
    PLATFORMS.into_iter().find(|row| row.0 == platform)
}

/// Chrome's brand list for a major version, with Chrome's GREASE entry, both seeded by the major.
pub fn brands_for(major: &str) -> Vec<Brand> {
    let seed: usize = major.parse().unwrap_or_default();
    let grease = format!(
        "Not{}A{}Brand",
        GREASE_CHARS[seed % GREASE_CHARS.len()],
        GREASE_CHARS[(seed + 1) % GREASE_CHARS.len()]
    );
    let named = [
        (
            grease,
            GREASE_VERSIONS[seed % GREASE_VERSIONS.len()].to_owned(),
        ),
        ("Chromium".into(), major.into()),
        ("Google Chrome".into(), major.into()),
    ];
    let mut list = vec![None; named.len()];
    for (slot, (brand, version)) in BRAND_ORDERS[seed % BRAND_ORDERS.len()].iter().zip(named) {
        list[*slot] = Some(Brand { brand, version });
    }
    list.into_iter().flatten().collect()
}

/// The brands with full versions: the claimed Chrome's, and the GREASE brand's padded out.
fn full_versions(brands: &[Brand], full: &str) -> Vec<Brand> {
    let version = |b: &Brand| {
        if b.brand.starts_with("Not") {
            format!("{}.0.0.0", b.version)
        } else {
            full.into()
        }
    };
    brands
        .iter()
        .map(|b| Brand {
            brand: b.brand.clone(),
            version: version(b),
        })
        .collect()
}

/// An Apple GPU is Apple Silicon: arm, while navigator.platform stays MacIntel.
fn architecture(platform: &str, profile: &Profile) -> String {
    let gpu = format!(
        "{} {}",
        profile.webgl.renderer.as_deref().unwrap_or_default(),
        profile
            .webgl
            .unmasked_renderer
            .as_deref()
            .unwrap_or_default()
    );
    let apple = gpu.to_lowercase().contains("apple m");
    if platform == "MacIntel" && apple {
        "arm"
    } else {
        "x86"
    }
    .into()
}

/// navigator.userAgentData for a supported platform and full version.
fn metadata(platform: &str, profile: &Profile, full: &str) -> UserAgentMetadata {
    let row = platform_row(platform).expect("platform validated before metadata");
    let brands = brands_for(full.split('.').next().unwrap_or_default());
    UserAgentMetadata {
        full_version_list: full_versions(&brands, full),
        brands,
        full_version: full.into(),
        platform: row.2.into(),
        platform_version: row.3.into(),
        architecture: architecture(platform, profile),
        model: String::new(),
        mobile: false,
        bitness: "64".into(),
        wow64: false,
        form_factors: vec!["Desktop".into()],
    }
}

/// Chrome's frozen UA string on a platform: only the major version moves.
fn user_agent(platform: &str, major: &str) -> String {
    let os = platform_row(platform).map_or("", |row| row.1);
    format!(
        "Mozilla/5.0 ({os}) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{major}.0.0.0 Safari/537.36"
    )
}

/// The validated identity of a persona's device, claiming the Chrome in `engine_user_agent`.
pub fn native_policy(
    profile: &Profile,
    engine_user_agent: &str,
) -> Result<NativePolicy, IdentityError> {
    let platform = profile.navigator.platform.as_str();
    if platform_row(platform).is_none() {
        return refuse("Native persona requires a supported platform");
    }
    let full = chrome_version(engine_user_agent)?;
    let locale = canonical_locale(profile.locale.as_deref().unwrap_or_default())?;
    let time_zone = system_zone(profile.timezone.as_deref().unwrap_or_default())?;
    let hardware_concurrency = processors(profile.navigator.hardware_concurrency)?;
    let languages = languages(&profile.navigator.languages, &locale)?;
    let major = full.split('.').next().unwrap_or_default();
    Ok(NativePolicy {
        time_zone,
        locale,
        hardware_concurrency,
        languages,
        platform: platform.into(),
        user_agent: user_agent(platform, major),
        user_agent_metadata: metadata(platform, profile, &full),
    })
}

/// Bounded printable ASCII, as Oya's policy text check.
fn text(value: &str) -> Result<&str, IdentityError> {
    let printable = value.bytes().all(|b| (0x21..=0x7e).contains(&b));
    if value.is_empty() || value.len() > MAX_POLICY_TEXT || !printable {
        return refuse("Invalid native policy text");
    }
    Ok(value)
}

/// A canonical BCP 47 tag (`en-us` becomes `en-US`); a tag without a language is refused.
pub fn canonical_locale(value: &str) -> Result<String, IdentityError> {
    let mut parts = text(value)?.split('-');
    let language = parts.next().unwrap_or_default().to_ascii_lowercase();
    let alpha = |s: &str, n: std::ops::RangeInclusive<usize>| {
        n.contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphabetic())
    };
    if !(alpha(&language, 2..=3) || alpha(&language, 5..=8)) || language == "und" {
        return refuse("Native policy requires a language");
    }
    let mut tag = language;
    for part in parts {
        let canonical = match part.len() {
            4 if alpha(part, 4..=4) => {
                part[..1].to_ascii_uppercase() + &part[1..].to_ascii_lowercase()
            }
            2 if alpha(part, 2..=2) => part.to_ascii_uppercase(),
            3 if part.bytes().all(|b| b.is_ascii_digit()) => part.into(),
            5..=8 if part.bytes().all(|b| b.is_ascii_alphanumeric()) => part.to_ascii_lowercase(),
            _ => return refuse("Invalid native policy locale"),
        };
        tag = format!("{tag}-{canonical}");
    }
    Ok(tag)
}

/// The language subtag of a canonical locale.
fn primary(locale: &str) -> &str {
    locale.split('-').next().unwrap_or(locale)
}

/// A system timezone name; numeric offsets and invented areas are refused.
fn system_zone(value: &str) -> Result<String, IdentityError> {
    let zone = text(value)?;
    let shaped = zone.starts_with(|c: char| c.is_ascii_alphabetic())
        && zone
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_+/-".contains(c));
    let area = zone.split('/').next().unwrap_or_default();
    let known = matches!(zone, "UTC" | "GMT") || (zone.contains('/') && ZONE_AREAS.contains(&area));
    if !shaped || !known {
        return refuse("Native policy requires a system timezone");
    }
    Ok(zone.into())
}

/// A whole processor count the identity can present.
fn processors(value: Option<u32>) -> Result<u32, IdentityError> {
    match value {
        Some(count) if (MIN_PROCESSORS..=MAX_PROCESSORS).contains(&count) => Ok(count),
        _ => refuse("Invalid native processor count"),
    }
}

/// navigator.languages must be the locale and its primary fallback; a lone regional tag gains it.
fn languages(requested: &[String], locale: &str) -> Result<Vec<String>, IdentityError> {
    let expected = if locale == primary(locale) {
        vec![locale.to_owned()]
    } else {
        vec![locale.to_owned(), primary(locale).to_owned()]
    };
    let expanded = match requested {
        [only] => {
            let only = canonical_locale(only)?;
            let base = primary(&only).to_owned();
            if only == base {
                vec![only]
            } else {
                vec![only, base]
            }
        }
        list => list
            .iter()
            .map(|tag| canonical_locale(tag))
            .collect::<Result<_, _>>()?,
    };
    if expanded != expected {
        return refuse("Native languages must match the locale and its primary fallback");
    }
    Ok(expected)
}

/// Chrome's Accept-Language for the persona's languages (a lone regional tag gains its base).
pub fn accept_language(languages: &[String]) -> Option<String> {
    match languages {
        [] => None,
        [only] if only.contains('-') => Some(format!("{only},{}", primary(only))),
        list => Some(list.join(",")),
    }
}

/// The client-hint request headers, by lowercase name, saying what the metadata says.
pub fn client_hints(meta: &UserAgentMetadata) -> Vec<(&'static str, String)> {
    let list = |brands: &[Brand]| {
        brands
            .iter()
            .map(|b| format!("\"{}\";v=\"{}\"", b.brand, b.version))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let quoted = |value: &str| format!("\"{value}\"");
    vec![
        ("sec-ch-ua", list(&meta.brands)),
        ("sec-ch-ua-mobile", "?0".into()),
        ("sec-ch-ua-full-version", quoted(&meta.full_version)),
        ("sec-ch-ua-arch", quoted(&meta.architecture)),
        ("sec-ch-ua-platform", quoted(&meta.platform)),
        ("sec-ch-ua-platform-version", quoted(&meta.platform_version)),
        ("sec-ch-ua-model", quoted(&meta.model)),
        ("sec-ch-ua-bitness", quoted(&meta.bitness)),
        ("sec-ch-ua-wow64", "?0".into()),
        ("sec-ch-ua-full-version-list", list(&meta.full_version_list)),
    ]
}
