/**
 * The live view: the frame, and a way to act on it. Clicks are scaled from the
 * displayed image to the page's own pixels; keys are batched so typing a word
 * is one command, not seven; Escape hands the keyboard back to the console.
 * The input logic lives in live/.
 */
'use client';

import { useRef, useState } from 'react';
import LiveToolbar from './live/toolbar';
import { CaptureHint, ClickRipple, Frame } from './live/frame';
import { surfaceClass } from './live/classes';
import { useLiveInput } from './live/use-live-input';
import type { Fit, Ripple, Send } from './live/types';

/** What the page or panel hosting the view provides. */
interface Props {
  /** The latest frame as an image URL, or null while connecting. */
  frameSrc: string | null;
  /** Frames in the last second. */
  fps: number;
  /** How old the latest frame is. */
  frameAgeMs: number | null;
  /** Sends one command to the browser. */
  send: Send;
  /** Called with a one-line description whenever the user does something, for the activity feed's optimistic row. */
  onInput?: (line: string) => void;
  /** False while the agent holds control: the view is watch-only, since human input is refused until taken. */
  interactive?: boolean;
  /** Fill the viewport instead of the panel's bounded 420px window, for the dedicated full-page live route. */
  large?: boolean;
}

/** Bounded on purpose. The fleet is the page; this is a window into one row. */
export default function LiveView({
  frameSrc,
  fps,
  frameAgeMs,
  send,
  onInput,
  interactive = true,
  large = false,
}: Props) {
  const img = useRef<HTMLImageElement>(null);
  const wrap = useRef<HTMLDivElement>(null);
  const [fit, setFit] = useState<Fit>('fit');
  const [hover, setHover] = useState(false);
  const [ripple, setRipple] = useState<Ripple | null>(null);
  const { captured, ...handlers } = useLiveInput({ img, wrap, send, onInput, interactive, hover, setRipple });
  return (
    <div className="overflow-hidden rounded-lg border border-border bg-black">
      <LiveToolbar
        {...{ frameSrc, fps, frameAgeMs, hover, fit }}
        onHover={() => setHover(!hover)}
        onFit={() => setFit(fit === 'fit' ? 'actual' : 'fit')}
      />
      <div
        ref={wrap}
        tabIndex={0}
        data-captures-keys={captured ? '' : undefined}
        {...handlers}
        className={surfaceClass(fit, large, captured)}
        style={{ cursor: interactive ? 'crosshair' : 'default' }}
        aria-label={
          interactive
            ? 'Live view, click to control, Esc to release the keyboard'
            : 'Live view, watch only while the agent has control'
        }
      >
        <Frame img={img} src={frameSrc} fit={fit} large={large} />
        {ripple && <ClickRipple ripple={ripple} onDone={() => setRipple(null)} />}
        <CaptureHint captured={captured} interactive={interactive} />
      </div>
    </div>
  );
}
