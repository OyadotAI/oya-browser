/**
 * The landing page's motion: sections that rise into place as they scroll in,
 * and numbers that count up once seen. Both stand still for anyone who asked
 * their system for reduced motion, and render their final state for crawlers
 * and readers without JavaScript.
 */
'use client';

import { animate, motion, useInView, useMotionValue, useReducedMotion, useTransform } from 'framer-motion';
import { useEffect, useRef, type ReactNode } from 'react';
import {
  COUNT_SECONDS,
  EASE_OUT,
  REVEAL_BLUR,
  REVEAL_MARGIN,
  REVEAL_RISE,
  REVEAL_SECONDS,
  STAGGER_SECONDS,
} from './constants';

/** Props for a reveal. */
interface RevealProps {
  /** What rises into place. */
  children: ReactNode;
  /** Its place in a sequence: each step waits one stagger longer. */
  order?: number;
  /** Classes for the wrapper. */
  className?: string;
}

/** Content that fades, rises and sharpens into place the first time it scrolls into view. */
export function Reveal({ children, order = 0, className }: RevealProps) {
  const still = useReducedMotion();
  return (
    <motion.div
      className={className}
      initial={still ? false : { opacity: 0, y: REVEAL_RISE, filter: `blur(${REVEAL_BLUR}px)` }}
      whileInView={{ opacity: 1, y: 0, filter: 'blur(0px)' }}
      viewport={{ once: true, margin: REVEAL_MARGIN }}
      transition={{ duration: REVEAL_SECONDS, ease: EASE_OUT, delay: order * STAGGER_SECONDS }}
    >
      {children}
    </motion.div>
  );
}

/**
 * A number that counts up from zero once it is seen. The server renders the
 * final value, so the page reads right before any script runs, and the count
 * never runs for reduced motion. The value is a motion value written straight
 * to the page, so counting re-renders nothing.
 */
function useCountUp(to: number) {
  const ref = useRef<HTMLSpanElement>(null);
  const seen = useInView(ref, { once: true });
  const still = useReducedMotion();
  const count = useMotionValue(to);
  const shown = useTransform(count, (v) => Math.round(v));
  useEffect(() => {
    if (!seen || still) return;
    const run = animate(count, to, { from: 0, duration: COUNT_SECONDS, ease: EASE_OUT });
    return () => run.stop();
  }, [seen, still, to, count]);
  return { ref, shown };
}

/** Props for a counting number. */
interface CountUpProps {
  /** Where the count ends. */
  to: number;
  /** What follows the number: `%`, `/10`. */
  suffix?: string;
}

/** A number that counts up to `to` when it scrolls into view. */
export function CountUp({ to, suffix = '' }: CountUpProps) {
  const { ref, shown } = useCountUp(to);
  return (
    <span ref={ref}>
      <motion.span>{shown}</motion.span>
      {suffix}
    </span>
  );
}
