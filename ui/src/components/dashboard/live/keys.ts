/**
 * What a key press means while the live view has the keyboard: Escape
 * releases it, printable keys are typed, named keys are pressed.
 */
import type { KeyboardEvent } from 'react';
import { SPECIAL_KEYS } from './constants';
import type { OnInput, Send } from './types';

/** The typing buffer, as keys see it. */
interface Typing {
  /** Buffers a printable character. */
  add: (char: string) => void;
  /** Sends what was typed now. */
  flush: () => void;
}

/** What handling a key needs from the live view. */
export interface KeyContext {
  /** Keys are ignored unless the frame owns the keyboard. */
  captured: boolean;
  /** The typing buffer. */
  typing: Typing;
  /** The ordered command queue. */
  enqueue: Send;
  /** The activity feed's optimistic row. */
  onInput?: OnInput;
  /** Hands the keyboard back to the console. */
  release: () => void;
}

/**
 * Escape is ours alone. React flushes the state change before the native event
 * reaches document, so the console's own Escape binding would otherwise see
 * the keyboard as already released and close the whole panel.
 */
function releaseOnEscape(e: KeyboardEvent, ctx: KeyContext) {
  e.preventDefault();
  e.stopPropagation();
  e.nativeEvent.stopImmediatePropagation();
  ctx.release();
}

/** A named key (Enter, arrows, …): send what was typed first, then press it. Unknown keys are dropped. */
function pressNamed(key: string, ctx: KeyContext) {
  if (!Object.hasOwn(SPECIAL_KEYS, key)) return;
  const name = SPECIAL_KEYS[key];
  ctx.typing.flush();
  ctx.onInput?.(`press ${name}`);
  void ctx.enqueue('press_key', { key: name });
}

/** Routes one keydown. The browser's own shortcuts (copy, reload the dashboard, …) pass through. */
export function handleKey(e: KeyboardEvent, ctx: KeyContext) {
  if (!ctx.captured) return;
  if (e.key === 'Escape') return releaseOnEscape(e, ctx);
  if (e.metaKey || e.ctrlKey || e.altKey) return;
  e.preventDefault();
  if (e.key.length === 1) return ctx.typing.add(e.key);
  pressNamed(e.key, ctx);
}
