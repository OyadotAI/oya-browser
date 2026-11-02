/**
 * The seeded device tables a persona's device is drawn from. They are Oya's own
 * (server/src/modules/personas/devices.ts), exported to data/devices.json by data/generate.mjs:
 * the seeded picks index into them, so their contents and order are part of every existing
 * persona's fingerprint and are read, never restated.
 */
use serde::Deserialize;
use serde_json::Number;
use std::{collections::BTreeMap, sync::LazyLock};

/// One GPU as WebGL reports it.
#[derive(Clone, Debug, Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Gpu {
    /// The masked vendor string.
    pub vendor: String,
    /// The masked renderer string.
    pub renderer: String,
    /// The real vendor, behind WEBGL_debug_renderer_info.
    pub unmasked_vendor: String,
    /// The real renderer, behind WEBGL_debug_renderer_info.
    pub unmasked_renderer: String,
}

/// One screen resolution with its pixel ratio, kept as JSON numbers so 1 stays 1.
#[derive(Clone, Debug, Deserialize)]
pub struct Resolution {
    /// Width in CSS pixels.
    pub width: u32,
    /// Height in CSS pixels.
    pub height: u32,
    /// Device pixel ratio.
    pub dpr: Number,
}

/// Every table, by the names devices.ts exports.
#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
pub struct Devices {
    /// GPUs by platform.
    pub GPU_DB: BTreeMap<String, Vec<Gpu>>,
    /// Screens by platform.
    pub SCREEN_RESOLUTIONS: BTreeMap<String, Vec<Resolution>>,
    /// Installed fonts by platform.
    pub FONT_SETS: BTreeMap<String, Vec<String>>,
    /// Logical core counts.
    pub HARDWARE_CONCURRENCY: Vec<u32>,
    /// navigator.deviceMemory values.
    pub DEVICE_MEMORY: Vec<u32>,
    /// Seeded timezones by platform.
    pub TIMEZONES: BTreeMap<String, Vec<String>>,
    /// Seeded locales by platform.
    pub LOCALES: BTreeMap<String, Vec<String>>,
    /// Platforms a persona may be, in seeded order.
    pub PLATFORMS: Vec<String>,
    /// Timezones a checked persona may choose.
    pub TIMEZONE_CHOICES: Vec<String>,
    /// Locales a checked persona may choose.
    pub LOCALE_CHOICES: Vec<String>,
}

impl Devices {
    /// A per-platform table entry, falling back to Win32's as Oya's `TABLE[platform] || TABLE.Win32`.
    pub fn of<'a, T>(table: &'a BTreeMap<String, Vec<T>>, platform: &str) -> &'a [T] {
        table
            .get(platform)
            .or_else(|| table.get("Win32"))
            .map_or(&[], Vec::as_slice)
    }
}

/// The tables, parsed once from the generated copy compiled into the crate.
pub static DEVICES: LazyLock<Devices> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../data/devices.json"))
        .expect("data/devices.json is generated from Oya's device tables")
});
