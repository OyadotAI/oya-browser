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
  /** How long after the agent's last command the chrome stops showing it at work. */
  AGENT_ACTIVE_MS: 3000,
  /** How long the panel's orb shows a run's outcome before it rests. */
  CHAT_ORB_REST_MS: 4000,
  /** How long the launch plays before it dissolves on its own (matches the launch keyframes in start.css). */
  LAUNCH_MS: 1500,
  /** How long the launch takes to dissolve (matches .launch.leaving in start.css). */
  LAUNCH_LEAVE_MS: 420,
  /** The control shield's scan runs at least this long, so a quick analysis still reads as one. */
  SHIELD_MIN_SCAN_MS: 1600,
  /** How long the outlines of what an analysis found stay up. */
  SHIELD_HOLD_MS: 3800,
  /** How long the outlines take to fade away. */
  SHIELD_FADE_MS: 1200,
  /** How long the reveal's wash of light takes to cross the page; each outline lights up as it passes. */
  SHIELD_REVEAL_MS: 2400,
  /** How much wider and taller than its element a found element's rim starts, in pixels (half on each side), before it settles on. */
  SHIELD_LOCK_REACH_PX: 14,
  /** How long Oya's caption stays after the agent's last action, before it goes quiet. */
  SHIELD_ACT_HOLD_MS: 4200,
  /** How long an action's flight of light and target ring stay on the page before they are removed (they finish playing first). */
  SHIELD_TARGET_MS: 1500,
  /** One sweep of the beam down the page while the agent reads; the reveal starts as a sweep ends (matches the CSS --loop). */
  SHIELD_SCAN_LOOP_MS: 1800,
  /** How long Oya says it is done after a run ends, before going quiet. */
  SHIELD_DONE_MS: 2200,
  /** How long the show takes to let go when the agent changes the page under it (matches .dismissing in control-shield.html). */
  SHIELD_DISMISS_MS: 500,
  /** The share of the veil's reach, from its middle, that stays at its lightest before darkening towards the edges. */
  SHIELD_VEIL_CLEAR_SHARE: 0.15,
  /** How long a window in the veil takes to open over a found element. */
  SHIELD_VEIL_OPEN_MS: 900,
  /** How long a window in the veil takes to glide after its element moves (matches .box's transform transition). */
  SHIELD_VEIL_GLIDE_MS: 320,
  /** How far a window in the veil reaches past its element on each side, in pixels, so the rim sits in the light. */
  SHIELD_VEIL_PAD_PX: 3,
  /** A window's corner radius, in pixels (matches .box::before). */
  SHIELD_VEIL_RADIUS_PX: 8,
  /**
   * The soft light around each window in the veil, outermost first: how far past the
   * window each step reaches, in pixels, and how much of the smoke it clears.
   */
  SHIELD_VEIL_FALLOFF: Object.freeze([Object.freeze({ px: 22, share: 0.16 }), Object.freeze({ px: 9, share: 0.3 })]),
  /** The share of its full size a window starts at as it opens. */
  SHIELD_VEIL_START: 0.85,
  /** How strongly a window's opening eases out: the power of a cubic ease. */
  SHIELD_VEIL_EASE_POWER: 3,
  /** How far the veil's canvas reaches past the window on each side, in pixels (matches #veil's inset), so its blur never lightens the edges. */
  SHIELD_VEIL_BLEED_PX: 12,
  /** The most sparks that fly into the orb; past this, every few elements send one. */
  SHIELD_SPARKS_MAX: 18,
  /** How long after the beam reaches an element its spark leaves, so its rim settles first. */
  SHIELD_SPARK_LAG_MS: 520,
  /** A found element covering more than this share of the page is a container, not something to outline. */
  SHIELD_MAX_BOX_SHARE: 0.25,
  /** Past this many outlines a page is busy: their numbers step back once they have locked on. */
  SHIELD_BUSY_COUNT: 40,
  /** A box covering this share of a smaller outlined box wraps or repeats it, and is left out. */
  SHIELD_WRAP_SHARE: 0.6,
  /** A number badge's height, and its width for one digit, in pixels (matches .box b in control-shield.html). */
  SHIELD_TAG_PX: 16,
  /** How far a badge sits up and left of its outline's corner, in pixels (matches .box b). */
  SHIELD_TAG_OFFSET_PX: 10,
  /** How far inside its outline's corner a badge sits when its element hugs the window's edge, in pixels (matches .box.tucked b). */
  SHIELD_TAG_TUCK_PX: 3,
  /** How much wider a badge grows for each further digit, in pixels. */
  SHIELD_TAG_DIGIT_PX: 6,
  /** Half, for the middle of a box. */
  SHIELD_HALF: 0.5,
  /** Half, for the middle of a tab. */
  HALF: 0.5,
  /** The widest a tab grows, as in Chrome, in pixels. */
  TAB_MAX_WIDTH: 240,
  /** The narrowest a tab shrinks before the strip scrolls: room for its icon, in pixels (matches tabs.css). */
  TAB_MIN_WIDTH: 44,
  /** Below this width an inactive tab drops its close button, in pixels. */
  TAB_SMALL_WIDTH: 96,
  /** Below this width a tab shows only its icon (the active one its close button), in pixels. */
  TAB_TINY_WIDTH: 60,
  /** Free strip kept right of the new-tab button so the window can always be dragged, in pixels (matches tabs.css). */
  TAB_DRAG_RESERVE: 48,
  /** How far the pointer moves on a tab before a press becomes a drag, in pixels. */
  TAB_DRAG_THRESHOLD: 5,
  /** How close to the strip's edge a dragged tab starts scrolling it, in pixels. */
  TAB_EDGE_PX: 32,
  /** The fastest the strip scrolls under a dragged tab, in pixels a frame. */
  TAB_SCROLL_MAX_PX: 14,
  /** How long the pointer rests on a tab before its hover card shows. */
  TAB_CARD_DELAY_MS: 500,
  /** How long a closing tab takes to fold away (matches tabs.css). */
  TAB_CLOSE_MS: 180,
  /** Under this many seconds ago, a time reads "just now". */
  JUST_NOW_SECONDS: 45,
  /** The units a past time is told in ("3 days ago"), largest first, each with its length in seconds. */
  TIME_UNIT_SECONDS: Object.freeze({
    year: 31_536_000,
    month: 2_592_000,
    week: 604_800,
    day: 86_400,
    hour: 3600,
    minute: 60,
  }),
  /** How many earlier imports the account page lists under the latest one. */
  EARLIER_IMPORTS_SHOWN: 4,
});
