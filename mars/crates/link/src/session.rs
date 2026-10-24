/**
 * One control-socket session with the Oya server, independent of the transport: the `auth`
 * message Mars sends, and what each server message does (browser/src/main/connection/
 * server-messages.ts is the Electron map this follows). The server learns Mars as a browser,
 * sends the persona it runs as, and sends `cmd`s that run through `Commands`. Ownership only ever
 * tightens here: the server's Human Mode refuses commands and revokes agents, and nothing the
 * server says grants an agent control of a Mars tab; that stays a native operator action.
 */
use crate::commands::{ACTIONS, Browser, Commands};
use crate::persona::Persona;
use serde_json::{Value, json};
use std::{collections::HashMap, sync::Arc};

/// What the link reports to the app.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// Signed in: the id the server knows this browser by, and the persona to load as a unit.
    Ready {
        /// The server's id for this browser.
        browser_id: String,
        /// The persona, immutable.
        persona: Arc<Persona>,
    },
    /// The server switched control: `human` or `agent`. Human already revoked agents.
    ControlMode(String),
    /// Cookies for the persona's jar (AC-03); Relay applies them.
    CookieSync(Vec<Value>),
    /// The persona could not be loaded; the session closed instead of running as another device.
    PersonaRefused(String),
    /// The socket closed with this code; the link reconnects unless the code is final.
    Disconnected(u16),
}

/// What handling one message produced.
#[derive(Debug, Default)]
pub struct Step {
    /// Messages to send, in order.
    pub replies: Vec<Value>,
    /// What the app hears.
    pub events: Vec<Event>,
    /// Close the socket with this code.
    pub close: Option<u16>,
}

/// Who this browser is, for the `auth` message.
#[derive(Clone, Debug)]
pub struct Identity {
    /// The project key.
    pub api_key: String,
    /// The name the person gave this browser.
    pub browser_name: String,
    /// The persona to run as.
    pub persona: Option<String>,
    /// This app's version, so the server sees which versions run.
    pub app_version: String,
    /// The browser id kept across launches, or None for the server to mint one.
    pub browser_id: Option<String>,
}

/// One message type's handler.
type Handler = fn(&mut Session, &Value, &mut dyn Browser, &mut Step);

/// Server message type to handler; an unknown type is ignored, as in Oya.
const HANDLERS: [(&str, Handler); 6] = [
    ("auth_ok", Session::auth_ok),
    ("ping", |_, _, _, step| {
        step.replies.push(json!({"type": "pong"}))
    }),
    ("pong", |_, _, _, _| {}),
    ("cmd", Session::cmd),
    ("control_mode", Session::control_mode),
    ("cookie_sync", |_, msg, _, step| {
        step.events.push(Event::CookieSync(
            msg.get("cookies")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
        ))
    }),
];

/// The session's state across messages.
pub struct Session {
    /// Who this browser is.
    identity: Identity,
    /// The server accepted our auth.
    ready: bool,
    /// The server says the person holds control.
    human: bool,
    /// Command state: the agent's tab and element numbers.
    commands: Commands,
    /// Every persona seen this run, by id: a known id must keep its device.
    devices: HashMap<String, Arc<Persona>>,
}

impl Session {
    /// A session that will authenticate as `identity`.
    pub fn new(identity: Identity) -> Self {
        Self {
            identity,
            ready: false,
            human: false,
            commands: Commands::default(),
            devices: HashMap::new(),
        }
    }

    /// Whether the server accepted our auth.
    pub fn ready(&self) -> bool {
        self.ready
    }

    /// The browser id to reconnect with.
    pub fn browser_id(&self) -> Option<&str> {
        self.identity.browser_id.as_deref()
    }

    /// The `auth` message; sent first on every connection.
    pub fn auth_message(&self) -> Value {
        let id = &self.identity;
        json!({
            "type": "auth", "api_key": id.api_key, "browser_id": id.browser_id, "browser_name": id.browser_name,
            "host_platform": host_platform(), "app_version": id.app_version, "persona": id.persona,
            "provider": "oya-desktop", "cdp": false, "actions": ACTIONS,
        })
    }

    /// The connection closed: commands stop until the next auth, and element numbers are forgotten.
    pub fn disconnected(&mut self) {
        self.ready = false;
        self.commands = Commands::default();
    }

    /// Handles one parsed server message.
    pub fn handle(&mut self, msg: &Value, browser: &mut dyn Browser) -> Step {
        let mut step = Step::default();
        let kind = msg.get("type").and_then(Value::as_str).unwrap_or_default();
        if let Some((_, handler)) = HANDLERS.iter().find(|(name, _)| *name == kind) {
            handler(self, msg, browser, &mut step);
        }
        step
    }

    /// `auth_ok`: load the persona whole, refuse a known persona whose device changed, go online.
    fn auth_ok(&mut self, msg: &Value, browser: &mut dyn Browser, step: &mut Step) {
        let persona = match Persona::from_welcome(msg) {
            Ok(persona) => persona,
            Err(error) => return refuse(step, error.0),
        };
        if let Some(known) = self.devices.get(persona.id())
            && !known.same_device(&persona)
        {
            return refuse(
                step,
                format!(
                    "Persona {} came back as a different device; refusing to re-roll it.",
                    persona.id()
                ),
            );
        }
        self.devices
            .insert(persona.id().to_owned(), persona.clone());
        if let Some(id) = msg.get("browser_id").and_then(Value::as_str) {
            self.identity.browser_id = Some(id.to_owned());
        }
        self.ready = true;
        let browser_id = self.identity.browser_id.clone().unwrap_or_default();
        step.events.push(Event::Ready {
            browser_id,
            persona,
        });
        self.set_mode(
            msg.get("control")
                .and_then(|c| c.get("mode"))
                .and_then(Value::as_str),
            browser,
            step,
        );
    }

    /// `control_mode`: the mode from its full state, else the bare `mode`.
    fn control_mode(&mut self, msg: &Value, browser: &mut dyn Browser, step: &mut Step) {
        let mode = msg
            .get("state")
            .and_then(|s| s.get("mode"))
            .or_else(|| msg.get("mode"))
            .and_then(Value::as_str);
        self.set_mode(mode, browser, step);
    }

    /// Human Mode revokes agents at once; agent mode only stops the link refusing.
    fn set_mode(&mut self, mode: Option<&str>, browser: &mut dyn Browser, step: &mut Step) {
        let Some(mode) = mode else { return };
        self.human = mode == "human";
        if self.human {
            browser.revoke_agents();
        }
        step.events.push(Event::ControlMode(mode.to_owned()));
    }

    /// `cmd`: run it and answer once with `cmd_result`.
    fn cmd(&mut self, msg: &Value, browser: &mut dyn Browser, step: &mut Step) {
        let id = msg.get("id").cloned().unwrap_or(Value::Null);
        let params = msg.get("params").cloned().unwrap_or_else(|| json!({}));
        let outcome = match msg.get("action").and_then(Value::as_str) {
            _ if !self.ready => Err(("Browser not ready".to_owned(), None)),
            None => Err(("action must be a string".to_owned(), None)),
            Some(_) if self.human => Err((
                "The person is in control (Human Mode). Wait for control to return.".to_owned(),
                None,
            )),
            Some(action) => self
                .commands
                .run(browser, action, &params)
                .map_err(|e| (e.message, e.code)),
        };
        step.replies.push(match outcome {
            Ok(data) => json!({"type": "cmd_result", "id": id, "ok": true, "data": data, "error": null}),
            Err((error, code)) => {
                let mut result = json!({"type": "cmd_result", "id": id, "ok": false, "data": null, "error": error});
                if let Some(code) = code {
                    result["code"] = code.into();
                }
                result
            }
        });
    }
}

/// Closes the session rather than run as a persona that failed to load.
fn refuse(step: &mut Step, reason: String) {
    step.events.push(Event::PersonaRefused(reason));
    step.close = Some(crate::constants::CLOSE_REJECTED);
}

/// This machine's platform as navigator.platform spells it, so a first default persona matches it.
fn host_platform() -> Option<&'static str> {
    match std::env::consts::OS {
        "macos" => Some("MacIntel"),
        "windows" => Some("Win32"),
        "linux" => Some("Linux x86_64"),
        _ => None,
    }
}
