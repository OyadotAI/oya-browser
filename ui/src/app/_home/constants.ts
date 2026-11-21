/**
 * The landing page's motion timings, named once so every reveal moves alike.
 */

/** How far inside the viewport an element must come before it reveals. */
export const REVEAL_MARGIN = '-80px';
/** Milliseconds the hero replay spends on each line of the playbook. */
export const SHOWCASE_TICK_MS = 1100;
/** Ticks the finished replay holds still before it starts over. */
export const SHOWCASE_HOLD = 3;

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

/** Intrinsic dimensions reserve the generated hero artwork's space before it loads. */
export const HERO_ART_WIDTH = 1536;
/** The sculpture's landscape framing stays consistent across responsive image sources. */
export const HERO_ART_HEIGHT = 1024;
/** The closing mark is decorative and intentionally crops against its panel edge. */
export const CLOSING_MARK_SIZE = 420;
/** Width in pixels of the official Product Hunt launch embed badge. */
export const PRODUCT_HUNT_BADGE_WIDTH = 250;
/** Height in pixels of the official Product Hunt launch embed badge. */
export const PRODUCT_HUNT_BADGE_HEIGHT = 54;
