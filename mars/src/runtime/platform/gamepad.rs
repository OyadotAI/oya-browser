/**
 * Gamepad API's device side: the GameController framework's connected controllers, read as
 * W3C "standard" mapping snapshots (17 buttons, 4 axes). As in Chrome, nothing is exposed until
 * a button is pressed after a page started watching (the gamepad user gesture); after that each
 * platform poll compares the snapshot with the last one and sends changes to subscribed pages,
 * whose getGamepads() then answers from it.
 */
use crate::runtime::supervisor::Runtime;
use serde_json::{Value, json};

/** Controller state when watching started, and the last snapshot sent. */
#[derive(Default)]
pub(in crate::runtime) struct Gamepads {
    watching: bool,
    exposed: bool,
    last: Vec<Value>,
}

impl Gamepads {
    /** Begin reading controllers for subscribed pages. */
    pub fn start(&mut self) {
        self.watching = true;
    }
}

/** Whether any button of any pad is pressed. */
fn any_pressed(pads: &[Value]) -> bool {
    pads.iter().any(|pad| {
        pad["buttons"]
            .as_array()
            .is_some_and(|buttons| buttons.iter().any(|button| button["pressed"] == true))
    })
}

impl Runtime {
    /** Send a new controller snapshot to subscribers when it changed; true when sent. */
    pub(in crate::runtime) fn poll_gamepads(&mut self) -> bool {
        let gamepads = &mut self.platform.gamepads;
        if !gamepads.watching {
            return false;
        }
        let pads = native::snapshot();
        gamepads.exposed |= any_pressed(&pads);
        let visible = if gamepads.exposed { pads } else { Vec::new() };
        if visible == gamepads.last {
            return false;
        }
        gamepads.last = visible.clone();
        let event = json!({"topic":"gamepad","pads":visible});
        for identity in self.platform.events.subscribers("gamepad") {
            self.platform.events.push(&identity, event.clone());
        }
        true
    }
}

/** GameController framework access through the Objective-C runtime. */
#[cfg(target_os = "macos")]
mod native {
    use objc2::msg_send;
    use objc2::rc::Retained;
    use objc2::runtime::{AnyClass, AnyObject, Bool};
    use objc2_foundation::{NSArray, NSString};
    use serde_json::{Value, json};

    #[link(name = "GameController", kind = "framework")]
    unsafe extern "C" {
        /** Referenced so the framework is linked and GCController is registered. */
        static GCControllerDidConnectNotification: &'static NSString;
    }

    /** Buttons of GCExtendedGamepad in the standard mapping's order (dpad and home follow). */
    const BUTTONS: [&std::ffi::CStr; 12] = [
        c"buttonA",
        c"buttonB",
        c"buttonX",
        c"buttonY",
        c"leftShoulder",
        c"rightShoulder",
        c"leftTrigger",
        c"rightTrigger",
        c"buttonOptions",
        c"buttonMenu",
        c"leftThumbstickButton",
        c"rightThumbstickButton",
    ];
    /** The d-pad directions in standard order: up, down, left, right. */
    const DPAD: [&std::ffi::CStr; 4] = [c"up", c"down", c"left", c"right"];
    /** The standard mapping's slots. */
    const SLOTS: usize = 4;

    /** One property object by selector name, or None. */
    fn property(object: &AnyObject, name: &std::ffi::CStr) -> Option<Retained<AnyObject>> {
        let selector = objc2::runtime::Sel::register(name);
        // SAFETY: only called with GameController property selectors that return objects or nil;
        // respondsToSelector: guards controllers that lack a control.
        unsafe {
            let responds: Bool = msg_send![object, respondsToSelector: selector];
            if !responds.as_bool() {
                return None;
            }
            msg_send![object, performSelector: selector]
        }
    }

    /** A GCControllerButtonInput as a GamepadButton record. */
    fn button(input: Option<Retained<AnyObject>>) -> Value {
        let Some(input) = input else {
            return json!({"pressed":false,"touched":false,"value":0.0});
        };
        // SAFETY: GCControllerButtonInput's value and isPressed accessors.
        let (value, pressed): (f32, Bool) =
            unsafe { (msg_send![&input, value], msg_send![&input, isPressed]) };
        json!({"pressed":pressed.as_bool(),"touched":pressed.as_bool() || value > 0.0,"value":value})
    }

    /** One thumbstick's x and y in web orientation (down is positive). */
    fn stick(pad: &AnyObject, name: &std::ffi::CStr) -> [f64; 2] {
        let Some(stick) = property(pad, name) else {
            return [0.0, 0.0];
        };
        let axis = |name| {
            property(&stick, name).map_or(0.0, |input| {
                // SAFETY: GCControllerAxisInput's value accessor.
                let value: f32 = unsafe { msg_send![&input, value] };
                f64::from(value)
            })
        };
        [axis(c"xAxis"), -axis(c"yAxis")]
    }

    /** One controller as a standard-mapping Gamepad record, or None without an extended profile. */
    fn record(controller: &AnyObject, index: usize) -> Option<Value> {
        let pad = property(controller, c"extendedGamepad")?;
        let mut buttons: Vec<Value> = BUTTONS
            .iter()
            .map(|name| button(property(&pad, name)))
            .collect();
        let dpad = property(&pad, c"dpad");
        buttons.extend(
            DPAD.iter()
                .map(|name| button(dpad.as_ref().and_then(|dpad| property(dpad, name)))),
        );
        buttons.push(button(property(&pad, c"buttonHome")));
        let name = property(controller, c"vendorName").map_or_else(
            || "Gamepad".to_string(),
            |name| {
                // SAFETY: vendorName is an NSString.
                unsafe { Retained::cast_unchecked::<NSString>(name) }.to_string()
            },
        );
        let [lx, ly] = stick(&pad, c"leftThumbstick");
        let [rx, ry] = stick(&pad, c"rightThumbstick");
        Some(
            json!({"index":index,"id":format!("{name} (STANDARD GAMEPAD)"),"connected":true,
            "mapping":"standard","timestamp":0,"axes":[lx,ly,rx,ry],"buttons":buttons}),
        )
    }

    /** Every connected controller with an extended gamepad profile, in the first four slots. */
    pub fn snapshot() -> Vec<Value> {
        // SAFETY: touching the linked notification name keeps the framework loaded.
        let _ = unsafe { GCControllerDidConnectNotification };
        let Some(class) = AnyClass::get(c"GCController") else {
            return Vec::new();
        };
        // SAFETY: +[GCController controllers] returns an NSArray of GCController.
        let controllers: Retained<NSArray<AnyObject>> = unsafe { msg_send![class, controllers] };
        controllers
            .iter()
            .take(SLOTS)
            .enumerate()
            .filter_map(|(index, controller)| record(&controller, index))
            .collect()
    }
}

/** Platforms without GameController have no gamepads Mars can read. */
#[cfg(not(target_os = "macos"))]
mod native {
    /** No controllers. */
    pub fn snapshot() -> Vec<serde_json::Value> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /** Pads stay hidden until a button press is seen; reading with no device yields none. */
    #[test]
    fn pads_hidden_until_a_press() {
        assert!(!any_pressed(&[json!({"buttons":[{"pressed":false}]})]));
        assert!(any_pressed(&[
            json!({"buttons":[{"pressed":false},{"pressed":true}]})
        ]));
        let pads = native::snapshot();
        assert!(pads.len() <= 4);
    }
}
