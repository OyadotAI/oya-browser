/**
 * The Chrome DevTools Protocol catalog, read from the pinned `.pdl` definitions vendored under
 * `protocol/` (Chromium 2c592105 for browser_protocol, V8 fedd021b for js_protocol): every
 * domain with its commands, events and types, each flagged experimental or deprecated. A small
 * line parser is enough: the format is indentation-based and the files are fixed at build time,
 * so no grammar library and no network are involved.
 */
/// Lines one definition file may hold; the vendored files are a third of this.
const LINES: usize = 60_000;
/// Indentation of a domain member (`  command x`).
const MEMBER_INDENT: usize = 2;
/// Indentation of a parameter block (`    parameters`).
const BLOCK_INDENT: usize = 4;
/// Indentation of a parameter line (`      string name`).
const FIELD_INDENT: usize = 6;

/// The browser protocol index and the V8 protocol, exactly as pinned.
const INDEX: &str = include_str!("../protocol/browser_protocol.pdl");
const JS: &str = include_str!("../protocol/js_protocol.pdl");
/// The browser protocol's domain files, in the index's include order.
const DOMAINS: &[(&str, &str)] = &[
    (
        "Accessibility",
        include_str!("../protocol/domains/Accessibility.pdl"),
    ),
    ("Ads", include_str!("../protocol/domains/Ads.pdl")),
    (
        "Animation",
        include_str!("../protocol/domains/Animation.pdl"),
    ),
    ("Audits", include_str!("../protocol/domains/Audits.pdl")),
    ("Autofill", include_str!("../protocol/domains/Autofill.pdl")),
    (
        "BackgroundService",
        include_str!("../protocol/domains/BackgroundService.pdl"),
    ),
    (
        "BluetoothEmulation",
        include_str!("../protocol/domains/BluetoothEmulation.pdl"),
    ),
    ("Browser", include_str!("../protocol/domains/Browser.pdl")),
    ("CSS", include_str!("../protocol/domains/CSS.pdl")),
    (
        "CacheStorage",
        include_str!("../protocol/domains/CacheStorage.pdl"),
    ),
    ("Cast", include_str!("../protocol/domains/Cast.pdl")),
    (
        "CrashReportContext",
        include_str!("../protocol/domains/CrashReportContext.pdl"),
    ),
    ("DOM", include_str!("../protocol/domains/DOM.pdl")),
    (
        "DOMDebugger",
        include_str!("../protocol/domains/DOMDebugger.pdl"),
    ),
    (
        "DOMSnapshot",
        include_str!("../protocol/domains/DOMSnapshot.pdl"),
    ),
    (
        "DOMStorage",
        include_str!("../protocol/domains/DOMStorage.pdl"),
    ),
    (
        "DeviceAccess",
        include_str!("../protocol/domains/DeviceAccess.pdl"),
    ),
    (
        "DeviceOrientation",
        include_str!("../protocol/domains/DeviceOrientation.pdl"),
    ),
    (
        "DigitalCredentials",
        include_str!("../protocol/domains/DigitalCredentials.pdl"),
    ),
    (
        "Emulation",
        include_str!("../protocol/domains/Emulation.pdl"),
    ),
    (
        "EventBreakpoints",
        include_str!("../protocol/domains/EventBreakpoints.pdl"),
    ),
    (
        "Extensions",
        include_str!("../protocol/domains/Extensions.pdl"),
    ),
    ("FedCm", include_str!("../protocol/domains/FedCm.pdl")),
    ("Fetch", include_str!("../protocol/domains/Fetch.pdl")),
    (
        "FileSystem",
        include_str!("../protocol/domains/FileSystem.pdl"),
    ),
    (
        "HeadlessExperimental",
        include_str!("../protocol/domains/HeadlessExperimental.pdl"),
    ),
    ("IO", include_str!("../protocol/domains/IO.pdl")),
    (
        "IndexedDB",
        include_str!("../protocol/domains/IndexedDB.pdl"),
    ),
    ("Input", include_str!("../protocol/domains/Input.pdl")),
    (
        "Inspector",
        include_str!("../protocol/domains/Inspector.pdl"),
    ),
    (
        "LayerTree",
        include_str!("../protocol/domains/LayerTree.pdl"),
    ),
    ("Log", include_str!("../protocol/domains/Log.pdl")),
    ("Media", include_str!("../protocol/domains/Media.pdl")),
    ("Memory", include_str!("../protocol/domains/Memory.pdl")),
    ("Network", include_str!("../protocol/domains/Network.pdl")),
    ("Overlay", include_str!("../protocol/domains/Overlay.pdl")),
    ("PWA", include_str!("../protocol/domains/PWA.pdl")),
    ("Page", include_str!("../protocol/domains/Page.pdl")),
    (
        "Performance",
        include_str!("../protocol/domains/Performance.pdl"),
    ),
    (
        "PerformanceTimeline",
        include_str!("../protocol/domains/PerformanceTimeline.pdl"),
    ),
    ("Preload", include_str!("../protocol/domains/Preload.pdl")),
    ("Security", include_str!("../protocol/domains/Security.pdl")),
    (
        "ServiceWorker",
        include_str!("../protocol/domains/ServiceWorker.pdl"),
    ),
    (
        "SmartCardEmulation",
        include_str!("../protocol/domains/SmartCardEmulation.pdl"),
    ),
    ("Storage", include_str!("../protocol/domains/Storage.pdl")),
    (
        "SystemInfo",
        include_str!("../protocol/domains/SystemInfo.pdl"),
    ),
    ("Target", include_str!("../protocol/domains/Target.pdl")),
    (
        "Tethering",
        include_str!("../protocol/domains/Tethering.pdl"),
    ),
    ("Tracing", include_str!("../protocol/domains/Tracing.pdl")),
    ("WebAudio", include_str!("../protocol/domains/WebAudio.pdl")),
    ("WebAuthn", include_str!("../protocol/domains/WebAuthn.pdl")),
    ("WebMCP", include_str!("../protocol/domains/WebMCP.pdl")),
];

/** A command, event or type of a domain. */
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    /// The member name (`navigate`).
    pub name: String,
    pub experimental: bool,
    pub deprecated: bool,
    /// Parameter names for commands and events; property names for types.
    pub fields: Vec<String>,
    /// Return names for commands.
    pub returns: Vec<String>,
}

/** One protocol domain. */
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Domain {
    pub name: String,
    pub experimental: bool,
    pub deprecated: bool,
    pub commands: Vec<Item>,
    pub events: Vec<Item>,
    pub types: Vec<Item>,
}

/** The whole protocol. */
#[derive(Clone, Debug, Default)]
pub struct Catalog {
    pub version: String,
    pub domains: Vec<Domain>,
}

impl Catalog {
    /** Parse the vendored definitions; the browser domains come first, then V8's. */
    pub fn pinned() -> Self {
        let mut catalog = Self {
            version: version(INDEX),
            domains: Vec::new(),
        };
        for (_, text) in DOMAINS {
            catalog.domains.extend(parse(text));
        }
        catalog.domains.extend(parse(JS));
        catalog
    }
    /** `Domain.command`, if the protocol has it. */
    pub fn command(&self, method: &str) -> Option<(&Domain, &Item)> {
        let (domain, name) = method.split_once('.')?;
        let domain = self.domains.iter().find(|entry| entry.name == domain)?;
        let item = domain.commands.iter().find(|entry| entry.name == name)?;
        Some((domain, item))
    }
    /** Every command as `Domain.command`, in protocol order. */
    pub fn methods(&self) -> impl Iterator<Item = (String, &Domain, &Item)> + '_ {
        self.domains.iter().flat_map(|domain| {
            domain
                .commands
                .iter()
                .map(move |item| (format!("{}.{}", domain.name, item.name), domain, item))
        })
    }
}

/** `major.minor` from the index file. */
fn version(index: &str) -> String {
    let mut major = "1";
    let mut minor = "3";
    for line in index.lines().take(LINES) {
        if let Some(value) = line.trim().strip_prefix("major ") {
            major = value.trim();
        }
        if let Some(value) = line.trim().strip_prefix("minor ") {
            minor = value.trim();
        }
    }
    format!("{major}.{minor}")
}

/** Where the parser is inside a domain. */
#[derive(Clone, Copy, PartialEq, Eq)]
enum Block {
    None,
    Fields,
    Returns,
}

/** What kind of member a line opened. */
#[derive(Clone, Copy, PartialEq, Eq)]
enum Member {
    Command,
    Event,
    Type,
}

/** Parse one file's domains. */
fn parse(text: &str) -> Vec<Domain> {
    let mut domains: Vec<Domain> = Vec::new();
    let mut member: Option<Member> = None;
    let mut block = Block::None;
    for line in text.lines().take(LINES) {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = line.len() - trimmed.len();
        let words: Vec<&str> = trimmed.split_whitespace().collect();
        if indent == 0 {
            if let Some(domain) = parse_domain(&words) {
                domains.push(domain);
                member = None;
                block = Block::None;
            }
            continue;
        }
        let Some(domain) = domains.last_mut() else {
            continue;
        };
        if indent == MEMBER_INDENT {
            member = parse_member(domain, &words);
            block = Block::None;
        } else if indent == BLOCK_INDENT {
            block = match words.first().copied() {
                Some("parameters" | "properties") => Block::Fields,
                Some("returns") => Block::Returns,
                _ => Block::None,
            };
        } else if indent == FIELD_INDENT
            && let (Some(member), Some(name)) = (member, words.last())
            && block != Block::None
        {
            add_field(domain, member, block, name);
        }
    }
    domains
}

/** `[experimental] [deprecated] domain Name`. */
fn parse_domain(words: &[&str]) -> Option<Domain> {
    let at = words.iter().position(|word| *word == "domain")?;
    Some(Domain {
        name: words.get(at + 1)?.to_string(),
        experimental: words[..at].contains(&"experimental"),
        deprecated: words[..at].contains(&"deprecated"),
        ..Domain::default()
    })
}

/** `[experimental] [deprecated] command|event|type Name [extends ...]`. */
fn parse_member(domain: &mut Domain, words: &[&str]) -> Option<Member> {
    let at = words
        .iter()
        .position(|word| ["command", "event", "type"].contains(word))?;
    let item = Item {
        name: words.get(at + 1)?.to_string(),
        experimental: words[..at].contains(&"experimental"),
        deprecated: words[..at].contains(&"deprecated"),
        fields: Vec::new(),
        returns: Vec::new(),
    };
    let member = match words[at] {
        "command" => Member::Command,
        "event" => Member::Event,
        _ => Member::Type,
    };
    match member {
        Member::Command => domain.commands.push(item),
        Member::Event => domain.events.push(item),
        Member::Type => domain.types.push(item),
    }
    Some(member)
}

/** A `Type name` line adds its name to the open member's fields or returns. */
fn add_field(domain: &mut Domain, member: Member, block: Block, name: &str) {
    let item = match member {
        Member::Command => domain.commands.last_mut(),
        Member::Event => domain.events.last_mut(),
        Member::Type => domain.types.last_mut(),
    };
    if let Some(item) = item {
        if block == Block::Returns {
            item.returns.push(name.to_string());
        } else {
            item.fields.push(name.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Catalog;

    /** The pinned files parse into the domains, commands and events the protocol has. */
    #[test]
    fn pinned_catalog_has_every_domain() {
        let catalog = Catalog::pinned();
        assert_eq!(catalog.version, "1.3");
        assert_eq!(catalog.domains.len(), 58);
        let (domain, navigate) = catalog.command("Page.navigate").unwrap();
        assert!(!domain.experimental);
        assert_eq!(navigate.fields[0], "url");
        assert_eq!(
            navigate.returns,
            ["frameId", "loaderId", "errorText", "isDownload"]
        );
        let (runtime, evaluate) = catalog.command("Runtime.evaluate").unwrap();
        assert!(
            runtime
                .events
                .iter()
                .any(|event| event.name == "consoleAPICalled")
        );
        assert!(evaluate.fields.contains(&"awaitPromise".to_string()));
        let commands: usize = catalog.domains.iter().map(|d| d.commands.len()).sum();
        let events: usize = catalog.domains.iter().map(|d| d.events.len()).sum();
        assert_eq!(commands, 665);
        assert_eq!(events, 234);
        assert!(catalog.command("Page.nope").is_none());
    }
}
