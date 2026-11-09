/** One bounded routing illustration runs only while visible and explicitly allowed to move. */
'use client';
import { useCallback, useEffect, useRef, useState } from 'react';
import { useInView, useReducedMotion } from 'framer-motion';
import { CONTROL_STAGE_COUNT, CONTROL_TICK_MS, REVEAL_MARGIN } from './constants';

/** Releases the routing timer on pause, reduced motion, leaving view and unmount. */
function useRouteTimer(active: boolean, advance: () => void) {
  useEffect(() => {
    if (!active) return;
    const timer = setInterval(advance, CONTROL_TICK_MS);
    return () => clearInterval(timer);
  }, [active, advance]);
}

/** The full route renders immediately; motion only indicates the current illustrative stop. */
export function useControlPlaneMotion() {
  const ref = useRef<HTMLElement>(null);
  const seen = useInView(ref, { margin: REVEAL_MARGIN }),
    still = !!useReducedMotion();
  const [paused, setPaused] = useState(false),
    [phase, setPhase] = useState(CONTROL_STAGE_COUNT - 1);
  useRouteTimer(seen && !still && !paused, useAdvance(setPhase));
  return { ref, phase, paused, still, toggle: () => setPaused((value) => !value) };
}

/** A stable callback prevents unrelated renders from resetting the illustration clock. */
function useAdvance(setPhase: import('react').Dispatch<import('react').SetStateAction<number>>) {
  return useCallback(() => setPhase((value) => (value + 1) % CONTROL_STAGE_COUNT), [setPhase]);
}
