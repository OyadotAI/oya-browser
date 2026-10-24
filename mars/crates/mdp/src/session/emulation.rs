/**
 * Emulation and Performance domains: the viewport (Chromium's size bounds), the colour scheme
 * through setEmulatedMedia, timezone and locale overrides for the page's scripts, the user agent,
 * and the few metrics Mars can report. The viewport is shared by every tab, as Mars has one
 * display; see docs/cdp.md.
 */
use super::{Fault, Handler, Session};
use serde_json::{Value, json};

/// Chromium's bound on an emulated dimension.
const MAX_SIZE: f64 = 10_000_000.0;
/// Seconds per minute, for timezone offsets.
const SECONDS_PER_MINUTE: f64 = 60.0;
/// The system zone database, where IANA ids are files.
const ZONEINFO: &str = "/usr/share/zoneinfo";

unsafe extern "C" {
    /// Rereads TZ; the libc crate does not bind it on every platform.
    fn tzset();
}

/** The Emulation and Performance commands. */
pub(super) const HANDLERS: &[(&str, Handler)] = &[
    (
        "Emulation.setDeviceMetricsOverride",
        Session::device_metrics,
    ),
    (
        "Emulation.clearDeviceMetricsOverride",
        Session::clear_device_metrics,
    ),
    ("Emulation.setEmulatedMedia", Session::emulated_media),
    ("Emulation.setTimezoneOverride", Session::timezone),
    ("Emulation.setLocaleOverride", Session::locale),
    (
        "Emulation.setUserAgentOverride",
        Session::user_agent_override,
    ),
    (
        "Emulation.setTouchEmulationEnabled",
        Session::touch_emulation,
    ),
    ("Performance.getMetrics", Session::metrics),
];

impl Session {
    /** Emulation.setDeviceMetricsOverride: width and height become the viewport; 0 keeps a side. */
    fn device_metrics(&mut self, session: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let tab = self.tab(session)?;
        let (width, height) = (
            params["width"].as_f64().unwrap_or(0.0),
            params["height"].as_f64().unwrap_or(0.0),
        );
        if width < 0.0 || height < 0.0 || width > MAX_SIZE || height > MAX_SIZE {
            return Err(Fault::invalid(&format!(
                "Width and height values must be positive, not greater than {}",
                MAX_SIZE as u64
            )));
        }
        if params["deviceScaleFactor"].as_f64().unwrap_or(0.0) < 0.0 {
            return Err(Fault::invalid("deviceScaleFactor must be non-negative"));
        }
        // A blank tab has no page to read the viewport from; zero keeps the default side.
        let current = self
            .call_tab(&tab, "page.nodes", json!({}))
            .map(|nodes| nodes["viewport"].clone())
            .unwrap_or_else(|_| json!({}));
        let width = if width == 0.0 {
            current["width"]
                .as_f64()
                .unwrap_or(mars::constants::DEFAULT_VIEWPORT_WIDTH)
        } else {
            width
        };
        let height = if height == 0.0 {
            current["height"]
                .as_f64()
                .unwrap_or(mars::constants::DEFAULT_VIEWPORT_HEIGHT)
        } else {
            height
        };
        self.mutate(
            &tab,
            "page.emulate",
            json!({"viewport":{"width":width,"height":height}}),
        )?;
        Ok(json!({}))
    }

    /** Emulation.clearDeviceMetricsOverride: back to the default viewport. */
    fn clear_device_metrics(&mut self, session: Option<&str>, _: &Value) -> Result<Value, Fault> {
        let tab = self.tab(session)?;
        self.mutate(&tab, "page.emulate", json!({"viewport":{"width":mars::constants::DEFAULT_VIEWPORT_WIDTH,"height":mars::constants::DEFAULT_VIEWPORT_HEIGHT}}))?;
        Ok(json!({}))
    }

    /** Emulation.setEmulatedMedia: prefers-color-scheme drives the page's appearance. */
    fn emulated_media(&mut self, session: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let tab = self.tab(session)?;
        let scheme = params["features"].as_array().and_then(|features| {
            features
                .iter()
                .find(|feature| feature["name"] == "prefers-color-scheme")
                .and_then(|feature| feature["value"].as_str().map(str::to_owned))
        });
        match scheme.as_deref() {
            Some("dark") => {
                self.mutate(&tab, "page.emulate", json!({"appearance":"dark"}))?;
            }
            Some("light") | Some("") | None => {
                self.mutate(&tab, "page.emulate", json!({"appearance":"light"}))?;
            }
            Some(other) => {
                return Err(Fault::invalid(&format!(
                    "Unsupported media feature value: {other}"
                )));
            }
        }
        Ok(json!({}))
    }

    /** Emulation.setTimezoneOverride: the zone's current offset, from the C library. */
    fn timezone(&mut self, session: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let tab = self.tab(session)?;
        let zone = params["timezoneId"].as_str().unwrap_or_default();
        if zone.is_empty() {
            return Ok(json!({}));
        }
        let offset =
            zone_offset_minutes(zone).ok_or_else(|| Fault::invalid("Invalid timezone id"))?;
        self.mutate(
            &tab,
            "page.emulate",
            json!({"timezone_offset_minutes":offset}),
        )?;
        Ok(json!({}))
    }

    /** Emulation.setLocaleOverride: the locale leads navigator.languages. */
    fn locale(&mut self, session: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let tab = self.tab(session)?;
        let Some(locale) = params["locale"]
            .as_str()
            .filter(|locale| !locale.is_empty())
        else {
            return Ok(json!({}));
        };
        self.mutate(&tab, "page.emulate", json!({"languages":[locale]}))?;
        Ok(json!({}))
    }

    /** Emulation.setTouchEmulationEnabled: Mars has no touch events; off is the one state. */
    fn touch_emulation(&mut self, _: Option<&str>, params: &Value) -> Result<Value, Fault> {
        if params["enabled"].as_bool() == Some(true) {
            return Err(Fault::server("Mars has no touch events to emulate."));
        }
        Ok(json!({}))
    }

    /** Performance.getMetrics: what Mars counts; the rest is zero rather than invented. */
    fn metrics(&mut self, session: Option<&str>, _: &Value) -> Result<Value, Fault> {
        let tab = self.tab(session)?;
        let nodes = self.call_tab(&tab, "page.nodes", json!({}))?;
        let count = nodes["nodes"].as_array().map_or(0, Vec::len);
        let metrics = [
            ("Timestamp", self.timestamp()),
            ("Documents", 1.0),
            ("Frames", 1.0),
            ("JSEventListeners", 0.0),
            ("Nodes", count as f64),
            ("LayoutCount", nodes["revision"].as_f64().unwrap_or(0.0)),
            (
                "RecalcStyleCount",
                nodes["revision"].as_f64().unwrap_or(0.0),
            ),
            ("LayoutDuration", 0.0),
            ("RecalcStyleDuration", 0.0),
            ("ScriptDuration", 0.0),
            ("TaskDuration", 0.0),
            ("JSHeapUsedSize", 0.0),
            ("JSHeapTotalSize", 0.0),
        ];
        Ok(
            json!({"metrics":metrics.iter().map(|(name, value)| json!({"name":name,"value":value})).collect::<Vec<_>>()}),
        )
    }
}

/**
 * Minutes east of UTC for an IANA zone right now, asked of the C library through TZ. The
 * variable is restored at once; the front door's own process is the only one affected.
 */
fn zone_offset_minutes(zone: &str) -> Option<f64> {
    if zone.is_empty()
        || zone.len() > 64
        || !zone
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "/_+-".contains(c))
    {
        return None;
    }
    // The C library answers UTC for a zone it does not know; the zone database says which exist.
    if !std::path::Path::new(ZONEINFO).join(zone).is_file() {
        return None;
    }
    let previous = std::env::var_os("TZ");
    // SAFETY: the front door is single-purpose; TZ is set and restored around one localtime_r.
    unsafe { std::env::set_var("TZ", zone) };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs()) as libc::time_t;
    let mut local: libc::tm = unsafe { std::mem::zeroed() };
    unsafe {
        tzset();
        libc::localtime_r(&now, &mut local);
    }
    match previous {
        Some(value) => unsafe { std::env::set_var("TZ", value) },
        None => unsafe { std::env::remove_var("TZ") },
    }
    unsafe { tzset() };
    Some(local.tm_gmtoff as f64 / SECONDS_PER_MINUTE)
}

#[cfg(test)]
mod tests {
    use super::zone_offset_minutes;

    /** Known zones give their offset; unknown ones are refused. */
    #[test]
    fn zones_resolve_to_offsets() {
        assert_eq!(zone_offset_minutes("UTC"), Some(0.0));
        let tokyo = zone_offset_minutes("Asia/Tokyo").unwrap();
        assert_eq!(tokyo, 540.0);
        assert!(zone_offset_minutes("Nowhere/Nope").is_none());
    }
}
