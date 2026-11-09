/**
 * The landing page's motion timings, named once so every reveal moves alike.
 */

/** How far inside the viewport an element must come before it reveals. */
export const REVEAL_MARGIN = '-80px';
/** Milliseconds the hero replay spends on each line of the playbook. */
export const SHOWCASE_TICK_MS = 1100;
/** Ticks the finished replay holds still before it starts over. */
export const SHOWCASE_HOLD = 3;
/** Pixels of the Oya mark at the centre of the control-plane diagram. */
export const HUB_LOGO = 34;

/** The kit's phone height for a section whose stacked content needs two screens (`--hm`, layout-v4.md). */
export const TWO_ON_A_PHONE = { '--hm': 2 } as import('react').CSSProperties;

/** The split frames' height over their width: shallower than the kit's 80%, so a split fits one phone screen. */
export const SPLIT_RATIO = { '--ratio': '70%' } as import('react').CSSProperties;

/** The tabs-tour frame's height over its width: the kit's desktop 80% kept on a phone too, so the section fills one screen each. */
export const TOUR_RATIO = { '--ratio': '80%' } as import('react').CSSProperties;

/** Width in pixels of the prior-auth video poster image. */
export const POSTER_WIDTH = 1280;
/** Height in pixels of the prior-auth video poster image. */
export const POSTER_HEIGHT = 860;

/** Width in pixels of the console screenshot images. */
export const SCREENSHOT_WIDTH = 1280;
/** Height in pixels of the console screenshot images. */
export const SCREENSHOT_HEIGHT = 688;

/** The architecture overview has one stop per permanent system layer. */
export const CONTROL_STAGE_COUNT = 4;
/** Time for one illustrated routing stage to remain readable. */
export const CONTROL_TICK_MS = 1800;
/** Ownership examples retain only a bounded recent history. */
export const HANDOFF_EVENT_LIMIT = 4;
