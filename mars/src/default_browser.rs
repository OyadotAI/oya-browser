/** Asks macOS to make the running Mars.app the default web browser, only when the person runs
`mars set-default`. Nothing calls this at launch: Mars never claims links without being asked. */
use mars::protocol::{EngineError, EngineResult};
use std::ffi::{CStr, c_char, c_void};

/** CoreFoundation's UTF-8 string encoding constant. */
const UTF8: u32 = 0x0800_0100;
/** LaunchServices role mask for "view this document type". */
const ROLE_VIEWER: u32 = 0x0000_0002;
/** URL schemes a default browser owns; macOS shows its confirmation dialog for these. */
const SCHEMES: [&CStr; 2] = [c"http", c"https"];
/** Uniform type identifiers Mars opens from Finder. */
const DOCUMENT_TYPES: [&CStr; 2] = [c"public.html", c"public.xhtml"];

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    /** The bundle of the running process; it has no identifier outside an .app. */
    fn CFBundleGetMainBundle() -> *const c_void;
    /** Borrowed bundle identifier, or null for a bare binary. */
    fn CFBundleGetIdentifier(bundle: *const c_void) -> *const c_void;
    /** Owned URL of the .app folder. */
    fn CFBundleCopyBundleURL(bundle: *const c_void) -> *const c_void;
    /** Owned CFString built from a C string. */
    fn CFStringCreateWithCString(
        allocator: *const c_void,
        text: *const c_char,
        encoding: u32,
    ) -> *const c_void;
    /** Releases an owned CoreFoundation object. */
    fn CFRelease(object: *const c_void);
}

#[link(name = "CoreServices", kind = "framework")]
unsafe extern "C" {
    /** Adds or refreshes the bundle's Info.plist claims in the LaunchServices database. */
    fn LSRegisterURL(bundle: *const c_void, update: u8) -> i32;
    /** Requests the default handler for a URL scheme; macOS confirms browser changes. */
    fn LSSetDefaultHandlerForURLScheme(scheme: *const c_void, bundle_id: *const c_void) -> i32;
    /** Requests the default viewer for a document type. */
    fn LSSetDefaultRoleHandlerForContentType(
        content_type: *const c_void,
        role: u32,
        bundle_id: *const c_void,
    ) -> i32;
}

/** Register this Mars.app, then request http, https and HTML ownership for its identifier. */
pub fn set_default() -> EngineResult<()> {
    let bundle = unsafe { CFBundleGetMainBundle() };
    let identifier = if bundle.is_null() {
        std::ptr::null()
    } else {
        unsafe { CFBundleGetIdentifier(bundle) }
    };
    if identifier.is_null() {
        return Err(not_bundled());
    }
    register(bundle)?;
    for scheme in SCHEMES {
        with_string(scheme, |scheme| unsafe {
            LSSetDefaultHandlerForURLScheme(scheme, identifier)
        })?;
    }
    for kind in DOCUMENT_TYPES {
        with_string(kind, |kind| unsafe {
            LSSetDefaultRoleHandlerForContentType(kind, ROLE_VIEWER, identifier)
        })?;
    }
    Ok(())
}

/** A copied or freshly unpacked app is unknown to LaunchServices until registered. */
fn register(bundle: *const c_void) -> EngineResult<()> {
    let url = unsafe { CFBundleCopyBundleURL(bundle) };
    if url.is_null() {
        return Err(not_bundled());
    }
    let status = unsafe { LSRegisterURL(url, 1) };
    unsafe { CFRelease(url) };
    checked(status)
}

/** Owns one temporary CFString around a LaunchServices call. */
fn with_string(text: &CStr, call: impl FnOnce(*const c_void) -> i32) -> EngineResult<()> {
    let value = unsafe { CFStringCreateWithCString(std::ptr::null(), text.as_ptr(), UTF8) };
    if value.is_null() {
        return Err(refused());
    }
    let status = call(value);
    unsafe { CFRelease(value) };
    checked(status)
}

/** LaunchServices reports success as zero. */
fn checked(status: i32) -> EngineResult<()> {
    if status == 0 { Ok(()) } else { Err(refused()) }
}

/** A bare cargo binary has no bundle identifier for macOS to record. */
fn not_bundled() -> EngineError {
    EngineError::new(
        "invalid_request",
        "Only an installed Mars.app can become the default browser.",
        "Run /Applications/Mars.app/Contents/MacOS/mars set-default.",
    )
}

/** The person declined, or LaunchServices could not record the change. */
fn refused() -> EngineError {
    EngineError::new(
        "default_browser_refused",
        "macOS did not record Mars as the default browser.",
        "Choose Mars in System Settings > Desktop & Dock > Default web browser.",
    )
}
