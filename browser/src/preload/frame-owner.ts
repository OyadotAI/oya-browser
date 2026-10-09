/** Only native owner-token lookup is exposed to the isolated recorder, not renderer internals. */
import { NATIVE_RECORDING } from '../shared/native-recording.ts';
/** Capability provided by the patched renderer engine. */
export interface FrameOwnerLookup {
  /** Resolve one unique selector across author roots to its actual child frame token. */
  _oyaFrameTokenForSelector?: (selector: string) => string;
}
/** Unsupported engines fail explicitly; no DOM index or URL matching is substituted. */
export function frameOwnerToken(frame: FrameOwnerLookup, selector: unknown): string {
  if (typeof selector !== 'string' || !selector || selector.length > NATIVE_RECORDING.MAX_SELECTOR_CHARS) {
    throw new Error('Invalid frame owner selector');
  }
  if (!frame._oyaFrameTokenForSelector) throw new Error('Native frame owner lookup is unsupported');
  return frame._oyaFrameTokenForSelector(selector);
}
