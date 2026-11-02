/** Screenshots capture the actual native content view into an operator-granted directory, or return
it inline as base64 PNG when no directory was granted (attached agents have no shared filesystem). */
use crate::protocol::{EngineError, EngineResult};
use objc2_app_kit::{NSBitmapImageFileType, NSColorRenderingIntent, NSColorSpace, NSView};
use objc2_foundation::NSDictionary;
use serde_json::{Value, json};
use std::path::PathBuf;

/// The agent chooses no path or filename and cannot overwrite an existing capture.
pub(super) struct Capture {
    directory: Option<PathBuf>,
    sequence: u64,
}
impl Capture {
    /** Resolve the startup grant once before accepting agent messages. */
    pub fn new(directory: Option<PathBuf>) -> EngineResult<Self> {
        let directory = directory
            .map(|path| {
                std::fs::create_dir_all(&path)?;
                std::fs::canonicalize(path)
            })
            .transpose()
            .map_err(|_| error("Cannot open the startup capture directory."))?;
        Ok(Self {
            directory,
            sequence: 0,
        })
    }
    /** Whether the operator granted a capture directory at startup. */
    pub fn granted(&self) -> bool {
        self.directory.is_some()
    }
    /** Display completion precedes a native bitmap read on the main thread. Without a startup
    directory the PNG is returned inline, so nothing is written to disk. */
    pub fn save(&mut self, view: &NSView, inspection: &Value) -> EngineResult<Value> {
        view.displayIfNeeded();
        let bounds = view.bounds();
        let bitmap = view
            .bitmapImageRepForCachingDisplayInRect(bounds)
            .ok_or_else(|| error("Native capture could not allocate a bitmap."))?;
        view.cacheDisplayInRect_toBitmapImageRep(bounds, &bitmap);
        let bitmap = bitmap
            .bitmapImageRepByConvertingToColorSpace_renderingIntent(
                &NSColorSpace::sRGBColorSpace(),
                NSColorRenderingIntent::RelativeColorimetric,
            )
            .ok_or_else(|| error("Native capture could not convert display colors to sRGB."))?;
        let data = unsafe {
            bitmap.representationUsingType_properties(
                NSBitmapImageFileType::PNG,
                &NSDictionary::new(),
            )
        }
        .ok_or_else(|| error("Native PNG encoding failed."))?;
        let mut result = json!({"page_id":inspection["page_id"],"revision":inspection["revision"],"layout_generation":inspection["layout_generation"],"width":bounds.size.width,"height":bounds.size.height,"unit":"logical_points"});
        let Some(directory) = self.directory.as_ref() else {
            result["mime"] = json!("image/png");
            result["png_base64"] = json!(crate::runtime::base64(&data.to_vec()));
            return Ok(result);
        };
        self.sequence += 1;
        let path = directory.join(format!(
            "oya-native-{}-{}.png",
            std::process::id(),
            self.sequence
        ));
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|_| error("Cannot create a new native capture."))?;
        file.write_all(&data.to_vec())
            .map_err(|_| error("Cannot write the native capture."))?;
        result["path"] = json!(path);
        Ok(result)
    }
}
/** Capture errors never silently switch to a different screenshot provider. */
fn error(message: &str) -> EngineError {
    EngineError::new(
        "capture_unavailable",
        message,
        "Launch desktop with an accessible --capture-dir directory.",
    )
}
