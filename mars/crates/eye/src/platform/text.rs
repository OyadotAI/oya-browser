/** AppKit attributed strings use identical OS font shaping for measurement and display. */
use super::{Face, Family};
use objc2::{
    AnyThread,
    rc::{Retained, autoreleasepool},
    runtime::AnyObject,
};
use objc2_app_kit::{
    NSAttributedStringNSStringDrawing, NSColor, NSFont, NSFontAttributeName, NSFontDescriptor,
    NSFontDescriptorSymbolicTraits, NSFontVariationAttribute, NSForegroundColorAttributeName,
    NSKernAttributeName, NSMutableParagraphStyle, NSParagraphStyleAttributeName,
    NSWritingDirection,
};
use objc2_core_foundation::{CFData, CFRetained};
use objc2_core_text::CTFontManagerCreateFontDescriptorFromData;
use objc2_foundation::{
    NSAttributedString, NSDictionary, NSMutableAttributedString, NSNumber, NSRange, NSString,
};

/// Loaded fonts this thread holds at once; past this the registry empties and the next
/// `install_fonts` loads the current page's fonts again.
const FONT_REGISTRY: usize = 128;
/// The OpenType `wght` axis tag, the one variable axis CSS weight drives.
const WEIGHT_AXIS: u32 = 0x7767_6874;

/// One registered web font: a descriptor made from the page's bytes, or an installed font's name.
enum WebFont {
    /// A descriptor CoreText built from font data for this process only.
    Data(Retained<NSFontDescriptor>),
    /// An installed font, by the name `local()` gave.
    Local(String),
    /// An installed family a page names in `font-family`, styled per run by weight and posture.
    Family(String),
}
thread_local! {
    /** Fonts registered from documents, by content key; never reaches the system registry. */
    static WEB_FONTS: std::cell::RefCell<std::collections::BTreeMap<u64, WebFont>> = const { std::cell::RefCell::new(std::collections::BTreeMap::new()) };
}
/// Whether this thread already holds the font with `key`.
pub fn has_font(key: u64) -> bool {
    WEB_FONTS.with(|fonts| fonts.borrow().contains_key(&key))
}
/// Registers sfnt bytes under `key` for this process only, through
/// CTFontManagerCreateFontDescriptorFromData (never CTFontManagerRegisterFontsForURL, which
/// would expose a page's font to every application). False when CoreText rejects the bytes.
pub fn register_font(key: u64, bytes: &[u8]) -> bool {
    let data = CFData::from_bytes(bytes);
    // SAFETY: a CFData of the bytes; CoreText parses it and returns NULL for anything it rejects.
    let Some(descriptor) = (unsafe { CTFontManagerCreateFontDescriptorFromData(&data) }) else {
        return false;
    };
    // CTFontDescriptor is toll-free bridged with NSFontDescriptor; into_raw hands over the +1.
    let raw = CFRetained::into_raw(descriptor).cast::<NSFontDescriptor>();
    // SAFETY: an owned, retained NSFontDescriptor pointer from the bridged CF object.
    let Some(descriptor) = (unsafe { Retained::from_raw(raw.as_ptr()) }) else {
        return false;
    };
    insert(key, WebFont::Data(descriptor));
    true
}
/// Registers an installed font by name under `key`; false when no such font is installed.
pub fn register_local(key: u64, name: &str) -> bool {
    if !local_font_exists(name) {
        return false;
    }
    insert(key, WebFont::Local(name.to_owned()));
    true
}
/// Registers an installed family by name under `key`, so `font-family: Verdana` shapes with
/// Verdana as Chromium does instead of a generic face; false when it is not installed.
pub fn register_family(key: u64, name: &str) -> bool {
    if !local_font_exists(name) {
        return false;
    }
    insert(key, WebFont::Family(name.to_owned()));
    true
}
/// Whether a font with this full or PostScript name is installed.
pub fn local_font_exists(name: &str) -> bool {
    name.len() <= crate::document::FONT_NAME_BYTES
        && NSFont::fontWithName_size(&NSString::from_str(name), 12.0).is_some()
}
/** Keeps the registry bounded: a full registry empties rather than grow. */
fn insert(key: u64, font: WebFont) {
    WEB_FONTS.with(|fonts| {
        let mut fonts = fonts.borrow_mut();
        if fonts.len() >= FONT_REGISTRY {
            fonts.clear();
            FONTS.with(|cache| cache.borrow_mut().clear());
        }
        fonts.insert(key, font);
    });
}
/// The page font for a face, with the face's weight (already clamped to the declaration's
/// range by the selection) set on a variable font's `wght` axis, as Blink does even for a
/// single-weight declaration: a variable file declared as 700 renders bold, not at its default
/// instance; a font without the axis ignores the attribute. None when the key is not
/// registered on this thread (the generic sans then stands in).
fn web_font(key: u64, face: Face) -> Option<Retained<NSFont>> {
    WEB_FONTS.with(|fonts| match fonts.borrow().get(&key)? {
        WebFont::Local(name) => NSFont::fontWithName_size(&NSString::from_str(name), face.size),
        WebFont::Family(name) => styled_family(name, face),
        WebFont::Data(descriptor) => {
            NSFont::fontWithDescriptor_size(&varied(descriptor, f64::from(face.weight)), face.size)
        }
    })
}
/** An installed family's regular face, or its bold or italic member when the run asks for one
 * and the family has it. */
fn styled_family(name: &str, face: Face) -> Option<Retained<NSFont>> {
    let regular = NSFont::fontWithName_size(&NSString::from_str(name), face.size)?;
    let mut traits = NSFontDescriptorSymbolicTraits::empty();
    if face.bold() {
        traits |= NSFontDescriptorSymbolicTraits::TraitBold;
    }
    if face.italic {
        traits |= NSFontDescriptorSymbolicTraits::TraitItalic;
    }
    if traits.is_empty() {
        return Some(regular);
    }
    // SAFETY: the selector is NSFontDescriptor's own; AppKit answers nil when the family has
    // no face with the traits (browserleaks.com's fonts do), which the generated binding
    // declares impossible and turns into a panic that took the whole document service down.
    let descriptor: Option<Retained<NSFontDescriptor>> = unsafe {
        objc2::msg_send![&*regular.fontDescriptor(), fontDescriptorWithSymbolicTraits: traits]
    };
    descriptor
        .and_then(|descriptor| NSFont::fontWithDescriptor_size(&descriptor, face.size))
        .or(Some(regular))
}
/** The descriptor with `wght` set, so one variable file serves every weight the page uses. */
fn varied(descriptor: &NSFontDescriptor, weight: f64) -> Retained<NSFontDescriptor> {
    let axes: Retained<NSDictionary<NSNumber, NSNumber>> = NSDictionary::from_retained_objects(
        &[&*NSNumber::new_u32(WEIGHT_AXIS)],
        &[NSNumber::new_f64(weight)],
    );
    // SAFETY: NSFontVariationAttribute is Apple's immutable attribute key; its documented value
    // is a dictionary of axis tag numbers to axis value numbers, which `axes` is.
    unsafe {
        let attributes =
            NSDictionary::from_slices(&[NSFontVariationAttribute], &[axes.as_ref() as &AnyObject]);
        descriptor.fontDescriptorByAddingAttributes(&attributes)
    }
}

/// Native interface font for controls and legacy display lists.
pub fn font(size: f64, bold: bool) -> Retained<NSFont> {
    if bold {
        NSFont::boldSystemFontOfSize(size)
    } else {
        NSFont::systemFontOfSize(size)
    }
}

/** CSS numeric weights map onto AppKit's system weight scale. */
fn system_weight(weight: u16) -> f64 {
    match weight {
        0..=150 => -0.8,
        151..=250 => -0.6,
        251..=350 => -0.4,
        351..=450 => 0.0,
        451..=550 => 0.23,
        551..=650 => 0.3,
        651..=750 => 0.4,
        751..=850 => 0.56,
        _ => 0.62,
    }
}

/// The document selects family, size and weight; the OS supplies glyph metrics.
/// Named faces have only regular and bold, as Chromium synthesizes nothing between them.
fn face_font(face: Face) -> Retained<NSFont> {
    let name = match (face.family, face.bold()) {
        (Family::System, _) => {
            return NSFont::systemFontOfSize_weight(face.size, system_weight(face.weight));
        }
        (Family::Web(key), _) => {
            return web_font(key, face).unwrap_or_else(|| {
                face_font(Face {
                    family: Family::SansSerif,
                    ..face
                })
            });
        }
        (Family::Serif, false) if face.italic => "Times-Italic",
        (Family::Serif, false) => "Times-Roman",
        (Family::Serif, true) if face.italic => "Times-BoldItalic",
        (Family::Serif, true) => "Times-Bold",
        (Family::SansSerif, false) if face.italic => "Helvetica-Oblique",
        (Family::SansSerif, false) => "Helvetica",
        (Family::SansSerif, true) if face.italic => "Helvetica-BoldOblique",
        (Family::SansSerif, true) => "Helvetica-Bold",
        (Family::Monospace, false) if face.italic => "Menlo-Italic",
        (Family::Monospace, false) => "Menlo-Regular",
        (Family::Monospace, true) if face.italic => "Menlo-BoldItalic",
        (Family::Monospace, true) => "Menlo-Bold",
    };
    NSFont::fontWithName_size(&NSString::from_str(name), face.size)
        .unwrap_or_else(|| font(face.size, face.bold()))
}

/// Attribute types are fixed here so page-controlled markup cannot inject native objects.
pub fn attributed(text: &str, size: f64, bold: bool) -> Retained<NSAttributedString> {
    colored(text, size, bold, &NSColor::textColor())
}

/// Link color changes presentation only; the exact font metrics remain shared.
pub fn colored(text: &str, size: f64, bold: bool, color: &NSColor) -> Retained<NSAttributedString> {
    run(text, Face::system(size, bold), color, None)
}

/// Right-to-left paragraphs set the base writing direction so AppKit shapes and orders them as RTL.
pub fn directed(
    text: &str,
    size: f64,
    bold: bool,
    color: &NSColor,
    rtl: bool,
) -> Retained<NSAttributedString> {
    styled(text, Face::system(size, bold), color, rtl)
}

/// One attributed run for a full face: family, weight, letter spacing, color and direction.
/// A painted run's base direction is always its paragraph's: left unset, CoreText takes it from
/// the first strong character and lays `א > a` out right to left in an LTR paragraph.
pub fn styled(text: &str, face: Face, color: &NSColor, rtl: bool) -> Retained<NSAttributedString> {
    run(text, face, color, Some(rtl))
}

/// `styled` with an optional base direction; None leaves CoreText's natural direction, which
/// measurement keeps because it does not know the paragraph (joining Arabic measured in a forced
/// LTR base came out wider than it paints). Kerning is added only when it changes shaping.
fn run(text: &str, face: Face, color: &NSColor, rtl: Option<bool>) -> Retained<NSAttributedString> {
    let font = cached_font(face);
    let kern = NSNumber::new_f64(face.spacing);
    let style = NSMutableParagraphStyle::new();
    style.setBaseWritingDirection(match rtl {
        Some(true) => NSWritingDirection::RightToLeft,
        _ => NSWritingDirection::LeftToRight,
    });
    let mut objects: Vec<&AnyObject> = vec![font.as_ref(), color.as_ref()];
    // SAFETY: Apple's attribute key statics are immutable NSString constants.
    let mut keys = unsafe { vec![NSFontAttributeName, NSForegroundColorAttributeName] };
    // SAFETY: as above.
    unsafe {
        if rtl.is_some() {
            keys.push(NSParagraphStyleAttributeName);
            objects.push(style.as_ref());
        }
        if face.spacing != 0.0 {
            keys.push(NSKernAttributeName);
            objects.push(kern.as_ref());
        }
    }
    // SAFETY: each key is Apple's attribute name paired with its documented value class.
    let run = unsafe {
        let attributes = NSDictionary::from_slices(&keys, &objects);
        NSAttributedString::initWithString_attributes(
            NSAttributedString::alloc(),
            &NSString::from_str(text),
            Some(&attributes),
        )
    };
    let run = unspaced(run, text, face);
    if face.autospace != super::autospace::OFF {
        return autospaced(run, text, face);
    }
    run
}

/// Whether the attributed run holds exactly `text`'s UTF-16 units, so ranges computed from
/// `text` address it. Foundation drops a leading byte order mark (U+FEFF) when it builds the
/// string, and a range past its end raises an exception that would abort the process.
fn same_units(run: &NSAttributedString, text: &str) -> bool {
    run.length() == text.encode_utf16().count()
}

/// A run whose zero-width controls (joiners, bidi marks and the like) take no letter spacing,
/// as Blink's ShapeResultSpacing skips them; otherwise each added a gap of its own.
fn unspaced(
    run: Retained<NSAttributedString>,
    text: &str,
    face: Face,
) -> Retained<NSAttributedString> {
    let ranges = super::ignorable::ranges(text);
    if face.spacing == 0.0 || ranges.is_empty() || !same_units(&run, text) {
        return run;
    }
    let bare = NSMutableAttributedString::initWithAttributedString(
        NSMutableAttributedString::alloc(),
        &run,
    );
    let none = NSNumber::new_f64(0.0);
    for (start, length) in ranges {
        // SAFETY: Apple's kern key paired with its documented NSNumber value, on one scalar's
        // range inside the string.
        unsafe {
            bare.addAttribute_value_range(NSKernAttributeName, &none, NSRange::new(start, length))
        };
    }
    Retained::into_super(bare)
}

/// A run with `text-autospace`'s eighth of an em after each scalar before an ideograph-alpha
/// boundary, on top of its letter spacing, so measuring and painting place the same gaps.
fn autospaced(
    run: Retained<NSAttributedString>,
    text: &str,
    face: Face,
) -> Retained<NSAttributedString> {
    let (at, gap) = super::autospace::gaps(text, face.size, face.autospace);
    if at.is_empty() || !same_units(&run, text) {
        return run;
    }
    let spaced = NSMutableAttributedString::initWithAttributedString(
        NSMutableAttributedString::alloc(),
        &run,
    );
    let kern = NSNumber::new_f64(face.spacing + gap);
    for (start, length) in at {
        // SAFETY: Apple's kern key paired with its documented NSNumber value, on one scalar's
        // range inside the string.
        unsafe {
            spaced.addAttribute_value_range(NSKernAttributeName, &kern, NSRange::new(start, length))
        };
    }
    Retained::into_super(spaced)
}

/// UTF-16 units of the characters CSS `word-spacing` widens: the space and the no-break space,
/// the same two layout counts when it adds `word-spacing` to a run's width.
const WORD_SEPARATORS: [u16; 2] = [0x20, 0xa0];

/// A run as `styled` draws it, plus CSS `word-spacing`: each word separator carries `word` more
/// advance after it. AppKit has no word spacing of its own, so without this a painter draws the
/// words of a widened run bunched at its start while layout placed what follows past its width.
pub fn spaced(
    text: &str,
    face: Face,
    color: &NSColor,
    rtl: bool,
    word: f64,
) -> Retained<NSAttributedString> {
    let base = styled(text, face, color, rtl);
    if word == 0.0 || !same_units(&base, text) {
        return base;
    }
    let run = NSMutableAttributedString::initWithAttributedString(
        NSMutableAttributedString::alloc(),
        &base,
    );
    let kern = NSNumber::new_f64(face.spacing + word);
    for (at, unit) in text.encode_utf16().enumerate() {
        if WORD_SEPARATORS.contains(&unit) {
            // SAFETY: Apple's kern key paired with its documented NSNumber value, on a one-unit
            // range inside the string.
            unsafe {
                run.addAttribute_value_range(NSKernAttributeName, &kern, NSRange::new(at, 1))
            };
        }
    }
    Retained::into_super(run)
}

/// Distinct faces kept resolved; the cache resets rather than grow past this.
const FONT_CACHE: usize = 64;
thread_local! {
    /** Resolving a named face is far slower than shaping a short run, so faces are reused. */
    static FONTS: std::cell::RefCell<Vec<(Face, Retained<NSFont>)>> = const { std::cell::RefCell::new(Vec::new()) };
}
/** The resolved font for a face, from this thread's cache. */
fn cached_font(face: Face) -> Retained<NSFont> {
    FONTS.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some((_, font)) = cache.iter().find(|(known, _)| *known == face) {
            return font.clone();
        }
        if cache.len() >= FONT_CACHE {
            cache.clear();
        }
        let font = face_font(face);
        cache.push((face, font.clone()));
        font
    })
}

/// Exact complete-run measurement is reused by wrapping and native painting.
pub fn measure(text: &str, face: Face) -> f64 {
    // AppKit reads a zero point size as the default size, so zero is answered here.
    if face.size <= 0.0 {
        return 0.0;
    }
    autoreleasepool(|_| run(text, face, &NSColor::textColor(), None).size().width)
}

/// Chromium grows the ascent of Times and Helvetica (and Courier) by this share of ascent plus descent
/// so they match their Windows counterparts' vertical metrics, the de facto web standard.
const CLASSIC_ASCENT: f64 = 0.15;

/// Cap height and x-height of a face, where `text-box-edge: cap` and `ex` put a line's edge
/// (Blink's FontMetrics::CapHeight and XHeight read from the font).
pub fn edges(face: Face) -> (f64, f64) {
    if face.size <= 0.0 {
        return (0.0, 0.0);
    }
    autoreleasepool(|_| {
        let font = cached_font(face);
        (font.capHeight(), font.xHeight())
    })
}

/// Ascender and CSS `line-height: normal` of a face, as Chromium computes them on macOS.
pub fn metrics(face: Face) -> (f64, f64) {
    if face.size <= 0.0 {
        return (0.0, 0.0);
    }
    autoreleasepool(|_| {
        let font = cached_font(face);
        let (ascent, descent) = (font.ascender(), -font.descender());
        let extra = if matches!(
            face.family,
            Family::System | Family::Monospace | Family::Web(_)
        ) {
            0.0
        } else {
            CLASSIC_ASCENT * (ascent + descent)
        };
        (ascent + extra, ascent + extra + descent + font.leading())
    })
}
