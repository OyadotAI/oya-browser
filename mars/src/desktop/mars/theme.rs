/** Oya's shell palette and native helpers, so Mars mars reads like Oya Browser in both schemes. */
use super::super::canvas::{Canvas, button};
use objc2::{ClassType, MainThreadOnly, msg_send, rc::Retained};
use objc2_app_kit::{
    NSAccessibility, NSAppearanceCustomization, NSAppearanceNameAqua, NSAppearanceNameDarkAqua,
    NSButton, NSColor, NSFont, NSImage, NSImageSymbolConfiguration, NSMenu, NSMenuItem, NSView,
};
use objc2_foundation::{NSArray, NSString};

/// The strip behind the tabs: a deeper band than the toolbar the active tab grows out of.
pub(super) const STRIP: (u32, u32) = (0xedf0f2, 0x0b0d0c);
/// The toolbar and the active tab share one surface, as in Oya's renderer tokens.
pub(super) const BAR: (u32, u32) = (0xffffff, 0x1a1e1b);
/// The resting address field sits slightly recessed into the toolbar.
pub(super) const MUTED: (u32, u32) = (0xf1f3f5, 0x252522);
/// Field and panel borders.
pub(super) const BORDER: (u32, u32) = (0xdde2e6, 0x2f2f2b);
/// Oya's one accent: the agent's green, deeper on light paper.
pub(super) const ACCENT: (u32, u32) = (0x157a13, 0x39ed35);
/// Tinted backgrounds for the agent pill and progress track.
pub(super) const ACCENT_SOFT: (u32, u32) = (0xe2f3df, 0x1c3a1b);
/// Recovery banners use the danger tint so they read as the browser speaking, not the page.
pub(super) const DANGER_SOFT: (u32, u32) = (0xfbe8e6, 0x3d2322);
/// Quiet icons and inactive tab titles.
pub(super) const SECONDARY: (u32, u32) = (0x626b73, 0xa3a39b);
/// Hairlines are translucent so they sit correctly on every band.
pub(super) const HAIRLINE_ALPHA: f64 = 0.11;
/// Find matches use a warm highlight that stays legible on light and dark pages.
pub(super) const MATCH: u32 = 0xffd60a;

/** Effective appearance decides the palette at draw time, so overrides apply without a rebuild. */
pub(super) fn dark(view: &NSView) -> bool {
    let names = unsafe { NSArray::from_slice(&[NSAppearanceNameAqua, NSAppearanceNameDarkAqua]) };
    view.effectiveAppearance()
        .bestMatchFromAppearancesWithNames(&names)
        .is_some_and(|name| unsafe { &*name == NSAppearanceNameDarkAqua })
}

/** Resolve a light/dark token pair for the view's current appearance. */
pub(super) fn color(view: &NSView, pair: (u32, u32)) -> Retained<NSColor> {
    hex(if dark(view) { pair.1 } else { pair.0 }, 1.0)
}

/** A literal sRGB color keeps Oya's exact values instead of a system approximation. */
pub(super) fn hex(value: u32, alpha: f64) -> Retained<NSColor> {
    let channel = |shift: u32| ((value >> shift) & 255) as f64 / 255.0;
    NSColor::colorWithSRGBRed_green_blue_alpha(channel(16), channel(8), channel(0), alpha)
}

/** The hairline is ink on light mars and light on dark mars. */
pub(super) fn hairline(view: &NSView) -> Retained<NSColor> {
    hex(if dark(view) { 0xffffff } else { 0x000000 }, HAIRLINE_ALPHA)
}

/** System font at an explicit weight; the typed wrapper needs a feature this crate does not enable. */
pub(super) fn font(size: f64, weight: f64) -> Retained<NSFont> {
    unsafe { msg_send![NSFont::class(), systemFontOfSize: size, weight: weight] }
}

/** An SF Symbol at a fixed point size, so toolbar glyphs match across macOS versions. */
pub(super) fn symbol(name: &str, size: f64) -> Option<Retained<NSImage>> {
    let image = NSImage::imageWithSystemSymbolName_accessibilityDescription(
        &NSString::from_str(name),
        None,
    )?;
    let config: Retained<NSImageSymbolConfiguration> = unsafe {
        msg_send![NSImageSymbolConfiguration::class(), configurationWithPointSize: size, weight: 0.0f64]
    };
    image.imageWithSymbolConfiguration(&config)
}

/** A symbol pre-coloured for drawing inside custom views, where tint colours do not apply. */
pub(super) fn tinted(name: &str, size: f64, tint: &NSColor) -> Option<Retained<NSImage>> {
    let image = symbol(name, size)?;
    let config = NSImageSymbolConfiguration::configurationWithHierarchicalColor(tint);
    image.imageWithSymbolConfiguration(&config)
}

/** A borderless icon button: the SF Symbol, an accessible name and the same audited action. */
pub(super) fn icon(
    name: &str,
    size: f64,
    label: &str,
    tag: isize,
    target: &Canvas,
) -> Retained<NSButton> {
    let control = button("", tag, target);
    if let Some(image) = symbol(name, size) {
        control.setImage(Some(&image));
    } else {
        control.setTitle(&NSString::from_str(label));
    }
    control.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
    control.setToolTip(Some(&NSString::from_str(label)));
    control.setAccessibilityLabel(Some(&NSString::from_str(label)));
    control
}

/// AppKit's NSLineBreakByTruncatingTail; the typed enum needs a feature this crate does not enable.
const TRUNCATE_TAIL: usize = 4;

/** Long titles end in an ellipsis instead of clipping mid-glyph. */
pub(super) fn truncate_tail(control: &objc2_app_kit::NSControl) {
    let _: () = unsafe { msg_send![control, setLineBreakMode: TRUNCATE_TAIL] };
}

/** One native menu row that sends a browser action tag through the canvas's audited `activate:` selector. */
pub(in crate::desktop) fn menu_item(
    title: &str,
    tag: isize,
    target: &Canvas,
) -> Retained<NSMenuItem> {
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(target.mtm()),
            &NSString::from_str(title),
            Some(objc2::sel!(activate:)),
            &NSString::from_str(""),
        )
    };
    item.setTag(tag);
    unsafe { item.setTarget(Some(target)) };
    item
}

/** A context menu from (title, tag, enabled) rows; an empty title is a separator. */
pub(in crate::desktop) fn context_menu(
    rows: &[(&str, isize, bool)],
    target: &Canvas,
) -> Retained<NSMenu> {
    let menu = NSMenu::new(target.mtm());
    menu.setAutoenablesItems(false);
    for &(title, tag, enabled) in rows {
        if title.is_empty() {
            menu.addItem(&NSMenuItem::separatorItem(target.mtm()));
        } else {
            let item = menu_item(title, tag, target);
            item.setEnabled(enabled);
            menu.addItem(&item);
        }
    }
    menu
}
