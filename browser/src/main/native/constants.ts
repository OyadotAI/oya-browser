/** Isolated-world ids above Electron's reserved main and preload worlds. */
import { NATIVE_RECORDING } from '../../shared/native-recording.ts';
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
