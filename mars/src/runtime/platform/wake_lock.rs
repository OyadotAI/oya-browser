/**
 * Screen Wake Lock: while any visible page holds a `screen` sentinel, Mars holds one IOKit
 * "PreventUserIdleDisplaySleep" power assertion, so the display does not dim or sleep. A request
 * from a page that is not visible (not in the selected tab) is NotAllowedError; when a page stops
 * being visible or goes away, its sentinels are released and their `release` events fire, as the
 * spec's visibility rules require.
 */
use super::{Ctx, Handler, Reply, fail, ok};
use crate::protocol::EngineResult;
use crate::runtime::supervisor::Runtime;
use serde_json::{Value, json};
use std::collections::BTreeSet;

/** Wake lock operations. */
pub(super) const HANDLERS: &[(&str, Handler)] =
    &[("wakelock.acquire", acquire), ("wakelock.release", release)];

/** One held sentinel: its context, the tab it lives in and its id there. */
#[derive(Clone, Debug, PartialEq)]
struct Sentinel {
    identity: String,
    tab: String,
    id: u64,
}

/** Held sentinels and the assertion they keep. */
#[derive(Default)]
pub(in crate::runtime) struct WakeLocks {
    sentinels: Vec<Sentinel>,
    assertion: Option<native::Assertion>,
}

impl WakeLocks {
    /** Hold the assertion exactly while a sentinel exists. */
    fn sync(&mut self) {
        if self.sentinels.is_empty() {
            self.assertion = None;
        } else if self.assertion.is_none() {
            self.assertion = native::Assertion::take();
        }
    }
}

/** navigator.wakeLock.request('screen'). */
fn acquire(runtime: &mut Runtime, ctx: &Ctx, args: &Value, _: &[u8]) -> EngineResult<Reply> {
    if !ctx.selected {
        return fail("NotAllowedError", "The requesting page is not visible");
    }
    let wake = &mut runtime.platform.wake;
    wake.sentinels.push(Sentinel {
        identity: ctx.identity.clone(),
        tab: ctx.tab.clone(),
        id: args["id"].as_u64().unwrap_or_default(),
    });
    wake.sync();
    if wake.assertion.is_none() {
        wake.sentinels.pop();
        return fail("NotAllowedError", "The system refused the wake lock.");
    }
    ok(Value::Null)
}

/** WakeLockSentinel.release(). */
fn release(runtime: &mut Runtime, ctx: &Ctx, args: &Value, _: &[u8]) -> EngineResult<Reply> {
    let id = args["id"].as_u64().unwrap_or_default();
    let wake = &mut runtime.platform.wake;
    wake.sentinels
        .retain(|s| !(s.identity == ctx.identity && s.id == id));
    wake.sync();
    ok(Value::Null)
}

impl Runtime {
    /** Release sentinels of pages that went away or stopped being visible; true when any did. */
    pub(in crate::runtime) fn poll_wake_locks(&mut self, live: &BTreeSet<String>) -> bool {
        let selected = self.selected_tab_id().to_string();
        let wake = &mut self.platform.wake;
        let (dropped, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut wake.sentinels)
            .into_iter()
            .partition(|s| !live.contains(&s.identity) || s.tab != selected);
        wake.sentinels = kept;
        wake.sync();
        for sentinel in &dropped {
            self.platform.events.push(
                &sentinel.identity,
                json!({"topic":"wakelock","released":[sentinel.id]}),
            );
        }
        !dropped.is_empty()
    }
}

/** IOKit power assertions. */
#[cfg(target_os = "macos")]
mod native {
    use objc2_foundation::NSString;
    use std::ffi::c_void;

    /** kIOPMAssertionLevelOn. */
    const LEVEL_ON: u32 = 255;
    /** kIOReturnSuccess. */
    const SUCCESS: i32 = 0;

    #[link(name = "IOKit", kind = "framework")]
    unsafe extern "C" {
        fn IOPMAssertionCreateWithName(
            kind: *const c_void,
            level: u32,
            name: *const c_void,
            id: *mut u32,
        ) -> i32;
        fn IOPMAssertionRelease(id: u32) -> i32;
    }

    /** One held assertion; dropping it releases it. */
    pub struct Assertion(u32);

    impl Assertion {
        /** Take a display-sleep assertion, or None when IOKit refuses. */
        pub fn take() -> Option<Self> {
            let kind = NSString::from_str("PreventUserIdleDisplaySleep");
            let name = NSString::from_str("Mars: a page holds a screen wake lock");
            let mut id = 0;
            // SAFETY: NSString is toll-free bridged to CFString; `id` is a valid out pointer.
            let status = unsafe {
                IOPMAssertionCreateWithName(
                    (&*kind as *const NSString).cast(),
                    LEVEL_ON,
                    (&*name as *const NSString).cast(),
                    &mut id,
                )
            };
            (status == SUCCESS).then_some(Self(id))
        }
    }

    impl Drop for Assertion {
        /** Release the assertion. */
        fn drop(&mut self) {
            // SAFETY: the id came from a successful IOPMAssertionCreateWithName.
            unsafe {
                IOPMAssertionRelease(self.0);
            }
        }
    }
}

/** Platforms without IOKit cannot keep the display awake, so requests are refused. */
#[cfg(not(target_os = "macos"))]
mod native {
    /** Never held. */
    pub struct Assertion;
    impl Assertion {
        /** No power management to ask. */
        pub fn take() -> Option<Self> {
            None
        }
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    /** The assertion exists exactly while a sentinel does. */
    #[test]
    fn assertion_follows_sentinels() {
        let mut wake = WakeLocks::default();
        wake.sentinels.push(Sentinel {
            identity: "a".into(),
            tab: "tab-1".into(),
            id: 1,
        });
        wake.sync();
        assert!(
            wake.assertion.is_some(),
            "IOKit granted a display assertion"
        );
        wake.sentinels.clear();
        wake.sync();
        assert!(wake.assertion.is_none());
    }
}
