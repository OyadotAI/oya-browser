/**
 * A persona's device: the fingerprint profile Oya's server generates from the persona's seed and
 * sends in `auth_ok`. The generator is a port of server/src/modules/personas/profile.ts, pick for
 * pick in the same stream order, so the same seed and prefs give the same device in both, forever.
 * The server stays the source of truth; the port exists so Mars can verify and preview a device.
 */
use crate::constants::{
    AUDIO_NOISE_MAX, CANVAS_NOISE_MAX, COLOR_DEPTH, RECTS_NOISE_MAX, TASKBAR_PX,
};
use crate::devices::{DEVICES, Devices, Gpu, Resolution};
use crate::prng::Prng;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};

/// What navigator reports.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceNavigator {
    /// navigator.platform: `Win32`, `MacIntel` or `Linux x86_64`.
    pub platform: String,
    /// navigator.hardwareConcurrency.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hardware_concurrency: Option<u32>,
    /// navigator.deviceMemory, in GB.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_memory: Option<Number>,
    /// navigator.maxTouchPoints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_touch_points: Option<u32>,
    /// navigator.languages, the locale first.
    #[serde(default)]
    pub languages: Vec<String>,
    /// navigator.vendor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,
}

/// What screen reports.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct DeviceScreen {
    /// screen.width.
    pub width: u32,
    /// screen.height.
    pub height: u32,
    /// screen.availWidth.
    pub avail_width: u32,
    /// screen.availHeight: the height less the taskbar or dock.
    pub avail_height: u32,
    /// screen.colorDepth.
    pub color_depth: u32,
    /// screen.pixelDepth.
    pub pixel_depth: u32,
    /// window.devicePixelRatio.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_pixel_ratio: Option<Number>,
}

/// The GPU WebGL reports; a device mirrored from a real browser may lack some strings.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default)]
pub struct DeviceGpu {
    /// The masked vendor string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,
    /// The masked renderer string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub renderer: Option<String>,
    /// The real vendor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unmasked_vendor: Option<String>,
    /// The real renderer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unmasked_renderer: Option<String>,
}

/// One noise surface's seed.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Noise {
    /// The seed the surface's noise is drawn from.
    pub noise_seed: f64,
}

/// The fonts a device reports as installed.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Fonts {
    /// Font family names.
    pub available: Vec<String>,
}

/// The whole device, in the shape `auth_ok` carries it. Unknown fields are kept, never dropped.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    /// The persona's id.
    pub id: String,
    /// navigator values.
    pub navigator: DeviceNavigator,
    /// screen values; a mirrored device may have none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screen: Option<DeviceScreen>,
    /// Canvas noise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canvas: Option<Noise>,
    /// The GPU.
    #[serde(default)]
    pub webgl: DeviceGpu,
    /// Audio noise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<Noise>,
    /// Client-rects noise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rects: Option<Noise>,
    /// Installed fonts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fonts: Option<Fonts>,
    /// IANA timezone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    /// BCP 47 locale.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    /// Whether WebGL answers with Chrome's ANGLE strings (personas made under current rules).
    #[serde(default)]
    pub webgl_chrome: bool,
    /// The proxy that belongs to the persona, or null for direct egress.
    #[serde(default)]
    pub proxy: Option<Value>,
    /// Anything else the server sent, kept so the unit is never silently trimmed.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A persona's device choices; anything not offered falls back to the seeded pick.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Prefs {
    /// Wanted platform.
    #[serde(default)]
    pub platform: Option<String>,
    /// Wanted timezone.
    #[serde(default)]
    pub timezone: Option<String>,
    /// Wanted locale.
    #[serde(default)]
    pub locale: Option<String>,
    /// Created under validated prefs: the wider choice lists apply.
    #[serde(default)]
    pub checked: Option<bool>,
}

/// What a device is drawn from: an id and seed fixed for the persona's life, its prefs and proxy.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct SeedIdentity {
    /// The persona's id.
    pub id: String,
    /// The 32-bit seed.
    pub seed: u32,
    /// Device choices.
    #[serde(default)]
    pub prefs: Option<Prefs>,
    /// The persona's proxy, carried into the profile as given.
    #[serde(default)]
    pub proxy: Option<Value>,
}

/// The hardware picks, in stream order.
struct Hardware {
    /// The GPU.
    gpu: Gpu,
    /// The screen.
    screen: Resolution,
    /// The platform's font set (not seeded).
    fonts: Vec<String>,
    /// Logical cores.
    hardware_concurrency: u32,
    /// Memory in GB.
    device_memory: u32,
    /// Canvas, audio and client-rects noise seeds.
    noise: [f64; 3],
}

/// A full device from the persona's seed and prefs; every seeded pick happens in Oya's order.
pub fn generate(identity: &SeedIdentity) -> Profile {
    let mut rng = Prng::new(identity.seed);
    let prefs = identity.prefs.clone().unwrap_or_default();
    let platform = pick_platform(&prefs, &mut rng);
    let hardware = pick_hardware(&platform, &mut rng);
    let checked = prefs.checked == Some(true);
    let timezone = pick_place(
        &prefs.timezone,
        checked,
        &DEVICES.TIMEZONES,
        &DEVICES.TIMEZONE_CHOICES,
        &platform,
        &mut rng,
    );
    let locale = pick_place(
        &prefs.locale,
        checked,
        &DEVICES.LOCALES,
        &DEVICES.LOCALE_CHOICES,
        &platform,
        &mut rng,
    );
    build(identity, platform, hardware, timezone, locale, checked)
}

/// The preferred platform when offered, else the seeded one; the pick is drawn either way.
fn pick_platform(prefs: &Prefs, rng: &mut Prng) -> String {
    let seeded = rng.pick(&DEVICES.PLATFORMS).clone();
    prefs
        .platform
        .clone()
        .filter(|platform| DEVICES.PLATFORMS.contains(platform))
        .unwrap_or(seeded)
}

/// GPU, screen, cores, memory and noise seeds, in stream order.
fn pick_hardware(platform: &str, rng: &mut Prng) -> Hardware {
    let gpu = rng.pick(Devices::of(&DEVICES.GPU_DB, platform)).clone();
    let screen = rng
        .pick(Devices::of(&DEVICES.SCREEN_RESOLUTIONS, platform))
        .clone();
    let fonts = Devices::of(&DEVICES.FONT_SETS, platform).to_vec();
    let hardware_concurrency = *rng.pick(&DEVICES.HARDWARE_CONCURRENCY);
    let device_memory = *rng.pick(&DEVICES.DEVICE_MEMORY);
    let noise = [
        rng.next_f64() * CANVAS_NOISE_MAX,
        rng.next_f64() * AUDIO_NOISE_MAX,
        rng.next_f64() * RECTS_NOISE_MAX,
    ];
    Hardware {
        gpu,
        screen,
        fonts,
        hardware_concurrency,
        device_memory,
        noise,
    }
}

/// Timezone or locale: the pref when the persona's rule allows it, else the seeded pick.
fn pick_place(
    wanted: &Option<String>,
    checked: bool,
    seeded: &std::collections::BTreeMap<String, Vec<String>>,
    choices: &[String],
    platform: &str,
    rng: &mut Prng,
) -> String {
    let pick = rng.pick(Devices::of(seeded, platform)).clone();
    let allowed = if checked {
        choices
    } else {
        seeded.get(platform).map_or(&[][..], Vec::as_slice)
    };
    wanted
        .clone()
        .filter(|value| !value.is_empty() && allowed.contains(value))
        .unwrap_or(pick)
}

/// The profile object browsers receive.
fn build(
    identity: &SeedIdentity,
    platform: String,
    hardware: Hardware,
    timezone: String,
    locale: String,
    checked: bool,
) -> Profile {
    let primary = locale.split('-').next().unwrap_or_default().to_owned();
    let screen = &hardware.screen;
    Profile {
        id: identity.id.clone(),
        navigator: DeviceNavigator {
            platform,
            hardware_concurrency: Some(hardware.hardware_concurrency),
            device_memory: Some(hardware.device_memory.into()),
            max_touch_points: Some(0),
            languages: vec![locale.clone(), primary],
            vendor: Some("Google Inc.".into()),
        },
        screen: Some(DeviceScreen {
            width: screen.width,
            height: screen.height,
            avail_width: screen.width,
            avail_height: screen.height.saturating_sub(TASKBAR_PX),
            color_depth: COLOR_DEPTH,
            pixel_depth: COLOR_DEPTH,
            device_pixel_ratio: Some(screen.dpr.clone()),
        }),
        canvas: Some(Noise {
            noise_seed: hardware.noise[0],
        }),
        webgl: DeviceGpu {
            vendor: Some(hardware.gpu.vendor),
            renderer: Some(hardware.gpu.renderer),
            unmasked_vendor: Some(hardware.gpu.unmasked_vendor),
            unmasked_renderer: Some(hardware.gpu.unmasked_renderer),
        },
        audio: Some(Noise {
            noise_seed: hardware.noise[1],
        }),
        rects: Some(Noise {
            noise_seed: hardware.noise[2],
        }),
        fonts: Some(Fonts {
            available: hardware.fonts,
        }),
        timezone: Some(timezone),
        locale: Some(locale),
        webgl_chrome: checked,
        proxy: identity.proxy.clone(),
        extra: Map::new(),
    }
}
