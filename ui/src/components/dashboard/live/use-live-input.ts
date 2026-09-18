/**
 * Everything the live view does with input, wired together: page mapping,
 * the ordered queue, typing, keyboard capture, the wheel and the mouse.
 */
import { useCallback, useEffect, type KeyboardEvent, type RefObject } from 'react';
import { toPagePoint } from './geometry';
import { handleKey } from './keys';
import { useCapture } from './use-capture';
import { useInputQueue } from './use-input-queue';
import { usePointer } from './use-pointer';
import { useTyping } from './use-typing';
import { attachWheel } from './wheel-scroller';
import type { LiveIo, OnInput, Ripple, Send, WrapRef } from './types';

/** What the live view hands its input hook. */
export interface LiveInputOptions {
  /** The frame image, for page mapping. */
  img: RefObject<HTMLImageElement | null>;
  /** The focusable box around it. */
  wrap: WrapRef;
  /** Sends one command to the browser. */
  send: Send;
  /** The activity feed's optimistic row. */
  onInput?: OnInput;
  /** False while the agent holds control. */
  interactive: boolean;
  /** Whether mouse movement is streamed. */
  hover: boolean;
  /** Draws the click ripple. */
  setRipple: (ripple: Ripple) => void;
}

/** Attaches the wheel listener and passes `io` through; re-attached whenever the queue, mapping or control changes. */
function useWheel(wrap: WrapRef, io: LiveIo): LiveIo {
  const { enqueue, onInput, toPage, interactive } = io;
  useEffect(() => {
    const node = wrap.current;
    if (node) return attachWheel(node, { enqueue, onInput, toPage, interactive });
  }, [wrap, enqueue, onInput, toPage, interactive]);
  return io;
}

/** Handlers and capture state for the frame box. */
export function useLiveInput({ img, wrap, send, onInput, interactive, hover, setRipple }: LiveInputOptions) {
  const toPage = useCallback((x: number, y: number) => toPagePoint(img.current, x, y), [img]);
  const enqueue = useInputQueue(send);
  const typing = useTyping(enqueue, onInput);
  const { captured, grab, release, onBlur } = useCapture(interactive, wrap, typing.flush);
  const io: LiveIo = useWheel(wrap, { enqueue, onInput, toPage, interactive });
  const pointer = usePointer({ ...io, hover, flush: typing.flush, grab, setRipple, send });
  const onKeyDown = (e: KeyboardEvent) => handleKey(e, { captured, typing, enqueue, onInput, release });
  return { captured, onBlur, onKeyDown, ...pointer };
}
