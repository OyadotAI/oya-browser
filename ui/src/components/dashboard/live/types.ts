/**
 * Shapes shared by the live view's hooks: how a command reaches the browser,
 * and how the feed hears about what the user did.
 */
import type { RefObject } from 'react';

/** Sends one browser command, e.g. `click_coordinates` with `{ x, y }`. */
export type Send = (action: string, params?: Record<string, unknown>) => Promise<unknown>;
/** Tells the activity feed, in one line, what the user just did. */
export type OnInput = (line: string) => void;
/** Maps a point on screen to the page's own pixels, or null when it is off the frame. */
export type ToPage = (clientX: number, clientY: number) => PagePoint | null;
/** The focusable box around the frame, which owns the keyboard while captured. */
export type WrapRef = RefObject<HTMLDivElement | null>;

/** A point on the remote page, plus where it sits inside the displayed element. */
export interface PagePoint {
  /** Page x, in the page's own pixels. */
  x: number;
  /** Page y, in the page's own pixels. */
  y: number;
  /** x inside the displayed element, where the click ripple is drawn. */
  localX: number;
  /** y inside the displayed element. */
  localY: number;
}

/** The expanding ring drawn where a click landed. */
export interface Ripple {
  /** Left, in element pixels. */
  x: number;
  /** Top, in element pixels. */
  y: number;
  /** Changes on every click so the animation restarts. */
  id: number;
}

/** Fit the frame to the panel, or show it at the page's actual size. */
export type Fit = 'fit' | 'actual';

/** What every input path needs: the ordered command queue and the page mapping. */
export interface LiveIo {
  /** Sends a command after every earlier one has finished. */
  enqueue: Send;
  /** The activity feed's optimistic row. */
  onInput?: OnInput;
  /** Screen → page pixels. */
  toPage: ToPage;
  /** False while the agent holds control: input is refused. */
  interactive: boolean;
}
