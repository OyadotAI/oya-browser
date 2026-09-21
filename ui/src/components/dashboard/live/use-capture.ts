/**
 * Keyboard capture: while the frame owns the keyboard, keys go to the remote
 * page; Escape or a blur hands it back to the console.
 */
import { useState, type Dispatch, type SetStateAction } from 'react';
import type { WrapRef } from './types';

/** Sets whether the keyboard is captured. */
type SetCaptured = Dispatch<SetStateAction<boolean>>;

/**
 * Control handed back mid-capture: release the keyboard with it. Adjusted
 * during render rather than in an effect, the release lands in the same
 * pass that loses control, instead of one cascading render later.
 */
function useReleaseOnLostControl(interactive: boolean, setCaptured: SetCaptured) {
  const [hadControl, setHadControl] = useState(interactive);
  if (hadControl !== interactive) {
    setHadControl(interactive);
    if (!interactive) setCaptured(false);
  }
}

/** Focuses the frame (without scrolling the console) and takes the keyboard. */
function grabKeys(wrap: WrapRef, setCaptured: SetCaptured) {
  wrap.current?.focus({ preventScroll: true });
  setCaptured(true);
}

/** Sends what was typed and gives the keyboard back. */
function dropKeys(flush: () => void, setCaptured: SetCaptured) {
  flush();
  setCaptured(false);
}

/** Whether keys are captured, plus `grab`, `release` (Escape: also blurs) and `onBlur`. */
export function useCapture(interactive: boolean, wrap: WrapRef, flush: () => void) {
  const [captured, setCaptured] = useState(false);
  useReleaseOnLostControl(interactive, setCaptured);
  const onBlur = () => dropKeys(flush, setCaptured);
  const release = () => (onBlur(), wrap.current?.blur());
  return { captured, grab: () => grabKeys(wrap, setCaptured), release, onBlur };
}
