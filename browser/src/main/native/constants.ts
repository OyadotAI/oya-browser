/** Isolated-world ids above Electron's reserved main and preload worlds. */
import { NATIVE_RECORDING } from '../../shared/native-recording.ts';
/** Electron's native JPEG encoder accepts integer quality percentages. */
export const JPEG_MAX_QUALITY = 100;
/** Keep preload delivery and browser evaluation in the same reserved world. */
export const ANALYZER_WORLD_ID = NATIVE_RECORDING.WORLD_ID;
/** Random bytes in an analyzer's per-document DOM marker. */
export const ANALYZER_ATTRIBUTE_BYTES = 4;
/** A hung renderer must not indefinitely block recording readiness, drains or stop. */
export const NATIVE_RECORDING_COMMAND_MS = 5000;
/** Convert browser-process monotonic milliseconds to protocol seconds. */
export const NATIVE_SECONDS = 1000;
/** Bound native tab-history exports and retained digest work. */
export const NATIVE_HISTORY_ENTRIES = 500;
/** Serialized native history above this size requires a smaller history before agent traversal. */
export const NATIVE_HISTORY_BYTES = 1048576;

/** Hard limits reject oversized input before native numeric conversion. */
export const INPUT_LIMITS = {
  /** Avoid native float overflow from hostile coordinates or wheel deltas. */
  coordinate: 1_000_000,
  /** Standard modifier bitset: Alt, Control, Meta, Shift. */
  modifiers: 15,
  /** Only the three native mouse buttons are implemented. */
  buttons: 7,
  /** Native single, double and triple clicks. */
  clicks: 3,
} as const;
/** Named modifier bits avoid numeric event construction outside this table. */
const INPUT_KEY_FLAGS = { alt: 1, control: 2, meta: 4, shift: 8 } as const;
/** Translate the shared modifier bits to Electron's native event names. */
export const INPUT_MODIFIERS = [
  [INPUT_KEY_FLAGS.alt, 'alt'],
  [INPUT_KEY_FLAGS.control, 'control'],
  [INPUT_KEY_FLAGS.meta, 'meta'],
  [INPUT_KEY_FLAGS.shift, 'shift'],
] as const;
/** Mouse button masks match DOM buttons and external input semantics. */
export const INPUT_BUTTONS = { left: 1, right: 2, middle: 4 } as const;
/** Native held-button flags preserve dragging without a page-visible button registry. */
export const BUTTON_MODIFIERS = [
  [INPUT_BUTTONS.left, 'leftbuttondown'],
  [INPUT_BUTTONS.right, 'rightbuttondown'],
  [INPUT_BUTTONS.middle, 'middlebuttondown'],
] as const;
