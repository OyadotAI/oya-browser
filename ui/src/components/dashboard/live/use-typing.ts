/**
 * Typing into the live view: characters are batched and sent as one
 * `keyboard_type` once the user pauses, or sooner when something else happens.
 */
import { useCallback, useEffect, useState } from 'react';
import { TypingBuffer } from './typing-buffer';
import type { OnInput, Send } from './types';

/** Sends what was typed as one command. Nothing typed, nothing sent. */
function sendTyped(text: string, enqueue: Send, onInput?: OnInput) {
  if (!text) return;
  onInput?.(`type ${text.length} chars`);
  void enqueue('keyboard_type', { text });
}

/** `add` buffers a character; `flush` sends the buffer now. A pending flush is dropped on unmount. */
export function useTyping(enqueue: Send, onInput?: OnInput) {
  const [buffer] = useState(() => new TypingBuffer());
  useEffect(() => () => buffer.cancel(), [buffer]);
  const flush = useCallback(() => sendTyped(buffer.take(), enqueue, onInput), [buffer, enqueue, onInput]);
  const add = useCallback((char: string) => buffer.add(char, flush), [buffer, flush]);
  return { flush, add };
}
