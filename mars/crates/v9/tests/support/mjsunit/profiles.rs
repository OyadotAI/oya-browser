/** The test host writes actual native profile artifacts only below its explicitly owned results directory. */
use std::path::{Path, PathBuf};

/** Generated components preserve shell's fixed filename while isolating concurrently executing upstream cases. */
pub(super) fn write(test: &str, profiles: Vec<(Option<u64>, String)>) -> Result<(), String> {
    if profiles.is_empty() {
        return Ok(());
    }
    let digest = ring::digest::digest(&ring::digest::SHA256, test.as_bytes());
    let name: String = digest
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let root = crate::test262_runner::out_dir();
    std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    let root = root.canonicalize().map_err(|error| error.to_string())?;
    for (endpoint, json) in profiles {
        let isolate = endpoint.map_or_else(|| "root".into(), |id| format!("worker-{id}"));
        let directory = confined(&root, &root.join("cpu-profiles").join(&name).join(isolate))?;
        let file = directory.join("v8.prof");
        if let Ok(metadata) = std::fs::symlink_metadata(&file)
            && (metadata.file_type().is_symlink() || !metadata.is_file())
        {
            return Err("CPU profile artifact must be a regular owned file".into());
        }
        std::fs::write(file, json).map_err(|error| error.to_string())?;
    }
    Ok(())
}

/** Existing host-created symlinks cannot redirect generated output into unrelated directories. */
fn confined(root: &Path, directory: &Path) -> Result<PathBuf, String> {
    let relative = directory
        .strip_prefix(root)
        .map_err(|error| error.to_string())?;
    let mut current = root.to_owned();
    for component in relative.components() {
        if !matches!(component, std::path::Component::Normal(_)) {
            return Err("CPU profile directory contains an unowned component".into());
        }
        current.push(component);
        if let Ok(metadata) = std::fs::symlink_metadata(&current) {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err("CPU profile directory must be an owned directory".into());
            }
        } else {
            std::fs::create_dir(&current).map_err(|error| error.to_string())?;
        }
    }
    Ok(current)
}
