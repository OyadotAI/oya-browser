/**
 * The pseudo-element and pseudo-class names Blink accepts. Eye's parser asks here whether a name is
 * known: a known pseudo-element or state that Eye cannot render or track is kept as a rule that never
 * matches, while an unknown one makes its selector invalid, as Blink drops it. The lists are Blink's
 * names, kept apart from the parser so the grammar and the vocabulary can change independently.
 */
/// Pseudo-elements Blink accepts without a vendor prefix. Eye's own boxes (`::before`, `::after`,
/// `::placeholder`, `::marker`, `::first-line`, `::first-letter`, scroll markers and buttons) are
/// rendered where they end a selector, so they only reach the parser in a position Blink refuses.
const KNOWN_ELEMENTS: [&str; 30] = [
    "before",
    "after",
    "first-line",
    "first-letter",
    "placeholder",
    "marker",
    "selection",
    "backdrop",
    "file-selector-button",
    "cue",
    "cue-region",
    "highlight",
    "spelling-error",
    "grammar-error",
    "target-text",
    "view-transition",
    "view-transition-group",
    "view-transition-image-pair",
    "view-transition-old",
    "view-transition-new",
    "part",
    "slotted",
    "details-content",
    "scroll-marker",
    "scroll-marker-group",
    "scroll-button",
    "column",
    "checkmark",
    "picker-icon",
    "picker",
];

/// The vendor prefix of Blink's own pseudo-elements, such as `::-webkit-scrollbar-thumb`.
const WEBKIT_PREFIX: &str = "-webkit-";

/// Pseudo-classes Blink accepts that Eye has no state for, so they never match: window focus,
/// viewport and media states, and the scrollbar states that only follow a scrollbar pseudo-element.
const NEVER_CLASSES: [&str; 14] = [
    "window-inactive",
    "current",
    "past",
    "future",
    "horizontal",
    "vertical",
    "decrement",
    "increment",
    "start",
    "end",
    "double-button",
    "single-button",
    "no-button",
    "corner-present",
];

/** Whether a lowercase pseudo-element name is one Blink accepts, with or without a prefix. */
pub(super) fn known_pseudo_element(name: &str) -> bool {
    name.starts_with(WEBKIT_PREFIX) || KNOWN_ELEMENTS.contains(&name)
}

/** Whether a lowercase pseudo-class name is a Blink state Eye keeps but never matches. */
pub(super) fn never_pseudo_class(name: &str) -> bool {
    NEVER_CLASSES.contains(&name)
}
