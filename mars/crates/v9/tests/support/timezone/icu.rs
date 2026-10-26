/** macOS's owned ICU calendar supplies actual localized timezone names to the inert test environment. */
#[cfg(target_os = "macos")]
mod native {
    use std::ffi::{CString, c_char, c_void};
    /** Primary ICU4C ucal.h defines STANDARD=0, DST=2 and GREGORIAN=1; UErrorCode succeeds at <=0. */
    const STANDARD: i32 = 0;
    const DAYLIGHT: i32 = 2;
    const GREGORIAN: i32 = 1;
    /** A bounded UTF-16 result prevents native localization data from growing host input without limit. */
    const NAME_UNITS: usize = 1024;
    const INPUT_UNITS: usize = 256;
    #[link(name = "icucore")]
    unsafe extern "C" {
        fn ucal_open(
            zone: *const u16,
            length: i32,
            locale: *const c_char,
            kind: i32,
            status: *mut i32,
        ) -> *mut c_void;
        fn ucal_close(calendar: *mut c_void);
        fn ucal_getTimeZoneDisplayName(
            calendar: *const c_void,
            kind: i32,
            locale: *const c_char,
            result: *mut u16,
            capacity: i32,
            status: *mut i32,
        ) -> i32;
    }
    /** The handle is private to one call and closed on every successful-open path, including errors. */
    struct Calendar(*mut c_void);
    impl Drop for Calendar {
        fn drop(&mut self) {
            unsafe { ucal_close(self.0) }
        }
    }
    /** C input strings and lengths are validated before any FFI call; no source or callbacks enter ICU. */
    pub(super) fn names(
        zone: &str,
        locale: Option<&str>,
    ) -> Result<Option<(String, String)>, String> {
        let zone: Vec<u16> = zone
            .strip_prefix(':')
            .unwrap_or(zone)
            .encode_utf16()
            .collect();
        let locale =
            CString::new(locale.unwrap_or("en_US")).map_err(|_| "Timezone locale contains NUL")?;
        if zone.len() > INPUT_UNITS || locale.as_bytes().len() > INPUT_UNITS {
            return Err("Timezone localization input exceeds host bounds".into());
        }
        let mut status = 0;
        let calendar = unsafe {
            ucal_open(
                zone.as_ptr(),
                zone.len() as i32,
                locale.as_ptr(),
                GREGORIAN,
                &mut status,
            )
        };
        if calendar.is_null() || status > 0 {
            if !calendar.is_null() {
                unsafe { ucal_close(calendar) }
            };
            return Err(format!("ICU timezone calendar failed with status {status}"));
        }
        let calendar = Calendar(calendar);
        Ok(Some((
            name(&calendar, &locale, STANDARD)?,
            name(&calendar, &locale, DAYLIGHT)?,
        )))
    }
    /** ICU writes only into owned fixed-capacity storage, and its returned length/status are checked before slicing. */
    fn name(calendar: &Calendar, locale: &CString, kind: i32) -> Result<String, String> {
        let mut text = [0u16; NAME_UNITS];
        let mut status = 0;
        let length = unsafe {
            ucal_getTimeZoneDisplayName(
                calendar.0,
                kind,
                locale.as_ptr(),
                text.as_mut_ptr(),
                NAME_UNITS as i32,
                &mut status,
            )
        };
        if status > 0 || length < 0 || length as usize > text.len() {
            return Err(format!(
                "ICU timezone display name failed with status {status}"
            ));
        }
        String::from_utf16(&text[..length as usize])
            .map_err(|_| "ICU timezone display name is invalid UTF-16".into())
    }
}
/** Unsupported hosts retain an explicit absent native localization source rather than fabricated city tables. */
pub(super) fn names(zone: &str, locale: Option<&str>) -> Result<Option<(String, String)>, String> {
    #[cfg(target_os = "macos")]
    {
        native::names(zone, locale)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (zone, locale);
        Ok(None)
    }
}
