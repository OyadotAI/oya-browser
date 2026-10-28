mod icu;
/** Test-host timezone delivery reads bounded TZif data only from the canonical system zoneinfo tree. */
mod tzif;
use std::io::Read;
use std::path::{Component, Path};

/** System timezone files are bounded independently of VM budgets and never interpreted as source code. */
const MAX_TZIF_BYTES: usize = 1_048_576;
/** A timezone is inert native transition data, without a filesystem capability inside the VM. */
pub(super) struct Zone {
    pub initial: i32,
    pub initial_daylight: bool,
    pub daylight: Vec<bool>,
    pub display_names: Option<(String, String)>,
    pub transitions: Vec<(f64, i32)>,
    pub posix: Option<String>,
}
/** Match V8's first Environment Variables directive and its whitespace-delimited key=value entries. */
fn requested(source: &str) -> Result<Option<&str>, String> {
    metadata_value(source, "TZ")
}
/** All locale and timezone facts come from the same first metadata line, matching upstream's parser. */
fn metadata_value<'a>(source: &'a str, variable: &str) -> Result<Option<&'a str>, String> {
    let Some(metadata) = source.lines().find_map(|line| {
        line.trim_start()
            .strip_prefix("//")?
            .trim_start()
            .strip_prefix("Environment Variables:")
    }) else {
        return Ok(None);
    };
    let mut zone = None;
    for pair in metadata.split_whitespace() {
        let (key, value) = pair
            .split_once('=')
            .ok_or("Malformed Environment Variables metadata")?;
        if key == variable {
            zone = Some(value);
        }
    }
    Ok(zone)
}
/** A test's declared process locale becomes its page default, without reading the runner's locale. */
pub(super) fn locale(source: &str) -> Result<Option<String>, String> {
    let value = metadata_value(source, "LC_ALL")?.or(metadata_value(source, "LANG")?);
    Ok(value.map(|tag| tag.split('.').next().unwrap_or(tag).replace('_', "-")))
}
/** Names cannot select absolute paths, parent directories, symlink escapes or arbitrary host files. */
fn confined_path(root: &Path, name: &str) -> Result<std::path::PathBuf, String> {
    let name = name.strip_prefix(':').unwrap_or(name);
    let relative = Path::new(name);
    if name.is_empty()
        || relative
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err("TZ must name a system zoneinfo entry".into());
    }
    let root = root
        .canonicalize()
        .map_err(|error| format!("System zoneinfo: {error}"))?;
    let file = root
        .join(relative)
        .canonicalize()
        .map_err(|error| format!("Timezone {name}: {error}"))?;
    if !file.starts_with(&root) {
        return Err("TZ symlink escapes the system zoneinfo tree".into());
    }
    Ok(file)
}
/** Host-only IO is confined before parsing; an absent zone remains an explicit test failure. */
pub(super) fn load(source: &str) -> Result<Option<Zone>, String> {
    let Some(name) = requested(source)? else {
        return Ok(None);
    };
    let file = confined_path(Path::new("/usr/share/zoneinfo"), name)?;
    let file = std::fs::File::open(&file).map_err(|error| format!("Timezone {name}: {error}"))?;
    let mut bytes = Vec::new();
    file.take((MAX_TZIF_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Timezone {name}: {error}"))?;
    if bytes.len() > MAX_TZIF_BYTES {
        return Err("TZif exceeds the bounded host input size".into());
    }
    let mut zone = tzif::parse(&bytes)?;
    let locale = metadata_value(source, "LC_ALL")?.or(metadata_value(source, "LANG")?);
    zone.display_names = icu::names(name, locale)?;
    Ok(Some(zone))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    /** Source strings and later directives cannot impersonate V8's actual environment metadata. */
    fn environment_metadata_is_not_javascript_or_later_comments() {
        assert_eq!(requested("const value='TZ=wrong'"), Ok(None));
        assert_eq!(
            requested(
                "// Environment Variables: LC_ALL=en TZ=Asia/Seoul\n// Environment Variables: TZ=wrong"
            ),
            Ok(Some("Asia/Seoul"))
        );
        assert!(requested("// Environment Variables: TZ").is_err());
    }
    #[test]
    /** V8's declared locale reaches Intl without consulting the runner process environment. */
    fn locale_metadata_removes_encoding_and_uses_the_first_directive() {
        assert_eq!(locale("const x='LC_ALL=fr-FR'"), Ok(None));
        assert_eq!(locale("// Environment Variables: LC_ALL=pt-BR.UTF8\n// Environment Variables: LC_ALL=fr-FR"), Ok(Some("pt-BR".into())));
    }
    #[test]
    /** Canonical confinement rejects all lexical escape forms before any host file read. */
    fn timezone_names_cannot_escape_the_fixed_prefix() {
        for name in [
            "",
            "/etc/passwd",
            "../etc/passwd",
            "America/../../etc/passwd",
        ] {
            assert!(confined_path(Path::new("/usr/share/zoneinfo"), name).is_err());
        }
    }
}
