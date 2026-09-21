/**
 * Collects typed characters so typing a word is one command, not seven.
 */
import { TYPE_FLUSH_MS } from './constants';

/** Characters typed since the last send, and the quiet timer that sends them. */
export class TypingBuffer {
  /** What has been typed and not yet sent. */
  private text = '';
  /** Fires once typing pauses. */
  private timer: ReturnType<typeof setTimeout> | null = null;

  /** Adds a character and restarts the quiet timer that calls `flush`. */
  add(char: string, flush: () => void) {
    this.text += char;
    this.cancel();
    this.timer = setTimeout(flush, TYPE_FLUSH_MS);
  }

  /** Stops a pending flush. */
  cancel() {
    if (this.timer) clearTimeout(this.timer);
    this.timer = null;
  }

  /** Takes what was typed, leaving the buffer empty and the timer stopped. */
  take(): string {
    this.cancel();
    const text = this.text;
    this.text = '';
    return text;
  }
}
