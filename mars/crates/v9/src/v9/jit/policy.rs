/**
 * The JIT's switches: whether it compiles at all and how hot a function must get first. Both are
 * read from the environment once per process, so the interpreter never reads the environment on
 * its hot path. Tests in the same process can override them, because the two tiers must be
 * compared without restarting the process.
 */
use super::constants::{
    DEFAULT_JIT_ENABLED, JIT_ENV, JIT_KILL_SWITCH_ENV, JIT_THRESHOLD_ENV, JIT_TRACE_ENV,
    JIT_VALUE_OFF, JIT_VALUE_ON, TIER_UP_THRESHOLD,
};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/** Whether this build's target can run the JIT at all: macOS on Apple silicon for now. */
const TARGET_SUPPORTED: bool = cfg!(all(target_os = "macos", target_arch = "aarch64"));

/** The settings in force for the process, shared by every thread's VM. */
struct Settings {
    enabled: AtomicBool,
    threshold: AtomicU32,
    trace: bool,
}

/** The process settings, read from the environment on first use. */
fn settings() -> &'static Settings {
    static SETTINGS: OnceLock<Settings> = OnceLock::new();
    SETTINGS.get_or_init(|| Settings {
        enabled: AtomicBool::new(env_enabled()),
        threshold: AtomicU32::new(env_threshold()),
        trace: std::env::var_os(JIT_TRACE_ENV).is_some_and(|value| value == JIT_VALUE_ON),
    })
}

/** Whether each compilation is reported on stderr, for checking which functions the JIT took. */
pub(in crate::v9) fn tracing() -> bool {
    settings().trace
}

/** Reads the switches once: the kill switch wins, then `MARS_V9_JIT` (`1` on, `0` off), then the default. */
fn env_enabled() -> bool {
    let kill = std::env::var(JIT_KILL_SWITCH_ENV).ok();
    let switch = std::env::var(JIT_ENV).ok();
    decide_enabled(kill.as_deref(), switch.as_deref())
}

/** The enable decision from the two raw environment values. Pure, so the precedence is testable. */
fn decide_enabled(kill: Option<&str>, switch: Option<&str>) -> bool {
    if kill == Some(JIT_VALUE_OFF) {
        return false;
    }
    match switch {
        Some(JIT_VALUE_ON) => true,
        Some(JIT_VALUE_OFF) => false,
        _ => DEFAULT_JIT_ENABLED,
    }
}

/** Reads the tier-up threshold, falling back to the default when unset or not a number. */
fn env_threshold() -> u32 {
    std::env::var(JIT_THRESHOLD_ENV)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(TIER_UP_THRESHOLD)
}

/** Whether functions may be compiled in this process: the target supports it and it is switched on. */
pub(in crate::v9) fn enabled() -> bool {
    TARGET_SUPPORTED && settings().enabled.load(Ordering::Relaxed)
}

/** Calls, and taken backward jumps, a function needs before it is compiled. */
pub(in crate::v9) fn threshold() -> u32 {
    settings().threshold.load(Ordering::Relaxed)
}

/** Replaces the process settings. Only tests call this, to compare the tiers within one process. */
#[allow(dead_code)]
pub(in crate::v9) fn configure(enabled: bool, threshold: u32) {
    settings().enabled.store(enabled, Ordering::Relaxed);
    settings().threshold.store(threshold, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    /** The enable decision follows the kill switch first, then the switch, then the default. */
    use super::*;

    /** `MARS_JIT=0` turns the JIT off even when `MARS_V9_JIT=1` asks for it. */
    #[test]
    fn kill_switch_overrides_the_switch() {
        assert!(!decide_enabled(Some("0"), Some("1")));
        assert!(!decide_enabled(Some("0"), None));
    }

    /** Without the kill switch, `MARS_V9_JIT` decides, and anything else keeps the default. */
    #[test]
    fn switch_decides_when_the_kill_switch_is_not_set() {
        assert!(decide_enabled(None, Some("1")));
        assert!(!decide_enabled(None, Some("0")));
        assert_eq!(decide_enabled(None, Some("yes")), DEFAULT_JIT_ENABLED);
        assert_eq!(decide_enabled(Some("1"), None), DEFAULT_JIT_ENABLED);
    }
}
