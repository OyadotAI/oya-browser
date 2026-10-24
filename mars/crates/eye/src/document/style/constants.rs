/** Named bounds for the best-effort CSS grammar; shared engine limits stay in mars-core. */
/** Nested blocks (at-rules and CSS nesting) cannot recurse the parser without bound. */
pub(super) const BLOCK_DEPTH: usize = 8;
/** One complex selector cannot chain an unbounded number of compounds. */
pub(super) const SELECTOR_COMPOUNDS: usize = 128;
/** Arguments of `:not()`, `:is()` and `:where()` stay small enough to match per node. */
pub(super) const SELECTOR_ARGUMENTS: usize = 16;
/** Elements one `:has()` check may visit; past this it does not match, so hostile pages stay linear. */
pub(super) const HAS_VISITS: usize = 512;
/** Custom property values are retained per declaring element, so each one is bounded. */
pub(super) const CUSTOM_PROPERTY_BYTES: usize = 2048;
/** `var()` chains stop here; deeper or cyclic references are invalid at computed-value time. */
pub(super) const VAR_DEPTH: usize = 8;
/** Substituted values cannot double themselves past this size through repeated references. */
pub(super) const VAR_OUTPUT_BYTES: usize = 4096;
/** `calc()` expressions are tokenized and evaluated within this many tokens. */
pub(super) const CALC_TOKENS: usize = 64;
/** `calc()` parentheses nest at most this deep. */
pub(super) const CALC_DEPTH: usize = 8;
/** Painted shadows per box. */
pub(super) const SHADOWS: usize = 8;
/** Background image layers per box. */
pub(super) const BACKGROUND_LAYERS: usize = 8;
/** Color stops per gradient. */
pub(super) const GRADIENT_STOPS: usize = 16;
/** Transform functions per declaration. */
pub(super) const TRANSFORM_FUNCTIONS: usize = 16;
/** Transform matrix entries stay finite and inside this magnitude. */
pub(super) const TRANSFORM_MAX: f64 = 4096.0;
/** Letter spacing magnitude cannot exceed the largest supported font size. */
pub(super) const LETTER_SPACING_MAX: f64 = crate::constants::CSS_MAX_FONT_SIZE;
/** A positioned or shifted box this far outside the page is treated as hidden from agents. */
pub(super) const OFFSCREEN: f64 = 1000.0;
/** Opacity at or below this cannot be seen; dropped rules setting it are hiding rules. */
pub(super) const INVISIBLE_OPACITY: f64 = 0.01;
/** CSS `medium` border width. */
pub(super) const BORDER_MEDIUM: f64 = 3.0;
/** CSS `thin` border width. */
pub(super) const BORDER_THIN: f64 = 1.0;
/** CSS `thick` border width. */
pub(super) const BORDER_THICK: f64 = 5.0;
/** Points convert to CSS pixels at 96/72. */
pub(super) const PX_PER_PT: f64 = 96.0 / 72.0;
/** Percent values divide by this. */
pub(super) const PERCENT: f64 = 100.0;
/** Without a known width, percentage radii below half a box approximate against this size. */
pub(super) const RADIUS_PERCENT_BASIS: f64 = 200.0;
/** A percentage radius of at least half the box makes a pill or circle. */
pub(super) const RADIUS_HALF: f64 = 50.0;
/** Chromium's `normal` line advance for its default Times face. */
pub(super) const NORMAL_LINE_HEIGHT: f64 = 1.15;
/** Normal, bold and the CSS weight range. */
pub(super) const WEIGHT_NORMAL: u16 = 400;
/** Bold keyword weight. */
pub(super) const WEIGHT_BOLD: u16 = 700;
/** Weights at or above this paint with the bold face. */
pub(super) const WEIGHT_BOLD_FACE: u16 = 600;
/** Lightest CSS weight. */
pub(super) const WEIGHT_MIN: u16 = 1;
/** Heaviest CSS weight. */
pub(super) const WEIGHT_MAX: u16 = 1000;
/** Diagnostic names longer than this are reported generically, never verbatim. */
pub(super) const DIAGNOSTIC_NAME: usize = 48;
/** Grid tracks per axis, including expanded `repeat()` groups. */
pub(super) const GRID_TRACKS: usize = 64;
/** A grid area name longer than this is not a name Eye keeps. */
pub(super) const GRID_NAME_BYTES: usize = 64;
/** Grid line numbers clamp into this magnitude. */
pub(super) const GRID_LINE_MAX: i32 = 1000;
/** `line-height: 0` lays out with this tiny positive advance, since advances must stay positive. */
pub(super) const LINE_HEIGHT_FLOOR: f64 = 0.01;
/** An animation that reveals hidden content is trusted for agents only if it finishes within this many seconds. */
pub(super) const REVEAL_SECONDS: f64 = 5.0;
/** Animations one element runs; later names in a longer list are ignored. */
pub(super) const ANIMATIONS: usize = 8;
/** Distinct `@keyframes` names one cascade keeps; past this, unknown names fail closed. */
pub(super) const KEYFRAME_SETS: usize = 1024;
/** Frames one `@keyframes` rule keeps; a longer rule applies nothing and only masks. */
pub(super) const KEYFRAME_STEPS: usize = 64;
/** Flex and grid `order` values clamp into this magnitude, so sorting never sees an extreme value. */
pub(super) const ORDER_MAX: i64 = 1 << 20;
/** `container-name` keeps at most this many names per box. */
pub(super) const CONTAINER_NAMES: usize = 8;
/** An `aspect-ratio` outside this range of width over height is not one Eye lays out. */
pub(super) const ASPECT_RATIO_MAX: f64 = 4096.0;
/** `env()` functions one value may substitute before it is refused. */
pub(super) const ENV_FUNCTIONS: usize = 8;
