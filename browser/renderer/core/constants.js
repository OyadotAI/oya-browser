/**
 * Every number the shell's renderer uses, by name. Loaded first; the other
 * renderer scripts read it as the global `RendererConstants`.
 */
/* exported RendererConstants */

/** Timeouts (milliseconds), panel sizes (CSS pixels) and display limits. */
const RendererConstants = Object.freeze({
  /** How long Connect waits for the server before saying it could not connect. */
  CONNECT_TIMEOUT_MS: 5000,
  /** How long a profile save may take before the button comes back. */
  PROFILE_SAVE_TIMEOUT_MS: 15000,
  /** How long "Copied!" shows on a chat message. */
  COPIED_MS: 1500,
  /** The tallest the Ask box grows before it scrolls. */
  CHAT_INPUT_MAX_PX: 160,
  /** The most attached file data a chat may carry, in base64 characters: the server's cap, 10 MiB of file. */
  CHAT_FILES_MAX_B64: 13_981_016,
  /** How often the Routines pane redraws its times ("Next 10:02", a run's elapsed time). */
  ROUTINES_CLOCK_MS: 30_000,
  /** How long a routine's note (why Run now could not start, say) stays under it. */
  ROUTINE_NOTE_MS: 6000,
  /** Characters of the prompt suggested as a playbook's name. */
  PLAYBOOK_NAME_SUGGESTION: 48,
  /** How often the Ask pane's step line redraws its elapsed count. */
  CHAT_PROGRESS_TICK_MS: 1000,
  /** Characters of a step's detail (a URL, typed text) the step line shows. */
  CHAT_STEP_DETAIL_CHARS: 48,
  /** The workspace panel's narrowest and widest. */
  PANEL_MIN_WIDTH: 320,
  PANEL_MAX_WIDTH: 560,
  /** The panel's width in compact studio mode. */
  PANEL_COMPACT_WIDTH: 360,
  /** The page keeps at least this much width beside the panel. */
  PAGE_MIN_WIDTH: 480,
  /** Below this window width the panel cannot be dragged. */
  RESIZE_MIN_WINDOW: 960,
  /** One arrow-key press on the resize handle. */
  PANEL_KEY_STEP: 16,
  /** The activity log keeps this many entries. */
  NET_LOG_LIMIT: 500,
  /** The activity log follows new entries while scrolled within this of the bottom. */
  NET_LOG_STICK_PX: 60,
  /** Characters of an analysis, and of any other action result, shown. */
  ANALYZE_PREVIEW: 5000,
  RESULT_PREVIEW: 3000,
  /** Elements of an analyzed page listed in the Actions pane to pick from. */
  ACTION_ELEMENTS_SHOWN: 60,
  /** The most steps a draft may have (shown as "n / 500"). */
  MAX_STEPS: 500,
  /** Steps from which a recording warns that the limit is near. */
  STEPS_WARNING: 450,
  /** The run timeline shows this many latest events. */
  RUN_EVENTS_SHOWN: 100,
  /** Zero-padded widths: step numbers, clock fields, milliseconds. */
  STEP_NUMBER_DIGITS: 2,
  /** The most characters of a URL or typed text a step row shows. */
  ROW_TEXT_CHARS: 60,
  /** How many levels of a long CSS path a step row shows, from the element up. */
  SELECTOR_LEVELS_SHOWN: 2,
  CLOCK_DIGITS: 2,
  MS_DIGITS: 3,
  /** Indentation of JSON shown to people. */
  JSON_INDENT: 2,
  /** Milliseconds in a second, for elapsed counts shown to people. */
  MS_PER_SECOND: 1000,
  /** The control shield's scan runs at least this long, so a quick analysis still reads as one. */
  SHIELD_MIN_SCAN_MS: 1000,
  /** How long the outlines of what an analysis found stay up. */
  SHIELD_HOLD_MS: 2600,
  /** How long the outlines take to fade away. */
  SHIELD_FADE_MS: 600,
  /** How long the reveal's wash of light takes to cross the page; each outline lights up as it passes. */
  SHIELD_REVEAL_MS: 1100,
  /** How much wider and taller than its element an outline's brackets start, in pixels (half on each side), before they lock on. */
  SHIELD_LOCK_REACH_PX: 16,
  /** How much wider and taller than its element the ripple grows as it leaves, in pixels. */
  SHIELD_RIPPLE_REACH_PX: 28,
  /** The most sparks that fly into the orb; past this, every few elements send one. */
  SHIELD_SPARKS_MAX: 36,
  /** How long after the beam reaches an element its spark leaves, so the brackets lock first. */
  SHIELD_SPARK_LAG_MS: 260,
  /** A found element covering more than this share of the page is a container, not something to outline. */
  SHIELD_MAX_BOX_SHARE: 0.25,
  /** Past this many outlines a page is busy: their numbers step back once they have locked on. */
  SHIELD_BUSY_COUNT: 40,
  /** A box covering this share of a smaller outlined box wraps or repeats it, and is left out. */
  SHIELD_WRAP_SHARE: 0.6,
  /** A number badge's height, and its width for one digit, in pixels (matches .box b in control-shield.html). */
  SHIELD_TAG_PX: 16,
  /** How far a badge sits up and left of its outline's corner, in pixels (matches .box b). */
  SHIELD_TAG_OFFSET_PX: 7,
  /** How much wider a badge grows for each further digit, in pixels. */
  SHIELD_TAG_DIGIT_PX: 6,
  /** Half, for the middle of a box. */
  SHIELD_HALF: 0.5,
});
