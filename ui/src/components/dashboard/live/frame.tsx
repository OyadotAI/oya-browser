/**
 * What sits inside the live view's box: the frame (or a placeholder), the
 * click ripple, and the keyboard hint in the corner.
 */
'use client';

import type { RefObject } from 'react';
import { Keyboard } from 'lucide-react';
import Kbd from '@/components/ui/kbd';
import { frameClass, hintClass } from './classes';
import type { Fit, Ripple } from './types';

/** The frame image, or a placeholder until the first one arrives. */
export function Frame({
  img,
  src,
  fit,
  large,
}: {
  /** The image element, for page mapping. */
  img: RefObject<HTMLImageElement | null>;
  /** The frame, or null. */
  src: string | null;
  /** Fitted or actual size. */
  fit: Fit;
  /** The full-page route. */
  large: boolean;
}) {
  if (!src) {
    return (
      <div className="flex h-[240px] items-center justify-center text-[13px] text-text-dim">
        Waiting for the first frame…
      </div>
    );
  }
  // eslint-disable-next-line @next/next/no-img-element
  return <img ref={img} src={src} alt="" draggable={false} className={frameClass(fit, large)} />;
}

/** A ring where a click landed; it removes itself when its animation ends. */
export function ClickRipple({
  ripple,
  onDone,
}: {
  /** Where, and a key that restarts the animation. */
  ripple: Ripple;
  /** Called when the animation ends. */
  onDone: () => void;
}) {
  return (
    <span
      key={ripple.id}
      className="pointer-events-none absolute h-6 w-6 -translate-x-1/2 -translate-y-1/2 animate-ping rounded-full border-2 border-accent"
      style={{ left: ripple.x, top: ripple.y }}
      onAnimationEnd={onDone}
    />
  );
}

/** Tells the user who has the keyboard and how to get it back. */
export function CaptureHint({
  captured,
  interactive,
}: {
  /** The frame owns the keyboard. */
  captured: boolean;
  /** The user may take control. */
  interactive: boolean;
}) {
  return (
    <div className={hintClass(captured)}>
      <Keyboard className="h-3 w-3" />
      {captured ? (
        <>
          keyboard captured · <Kbd>Esc</Kbd> releases
        </>
      ) : interactive ? (
        'click to control'
      ) : (
        'watching · take control to interact'
      )}
    </div>
  );
}
