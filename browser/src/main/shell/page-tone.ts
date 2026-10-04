/**
 * Whether a page reads as dark or light. The control shield dims the page while an
 * agent reads it, and a dim that suits a white page vanishes on a black one, so the
 * shield is told which kind it is drawing over. It is read from the page's own
 * background colours, in the isolated world: a capture taken while the page is
 * still loading is black, and called every half-loaded page dark.
 */
import { DARK_PAGE_LUMA } from './constants.ts';

/** How a page reads: the shield dims a dark one more deeply. */
export type PageTone = 'dark' | 'light';

/** Evaluates a script in the page's isolated world and gives back its answer. */
export type ReadPage = (js: string) => unknown;

/**
 * The page's brightness from 0 (black) to 1 (white): the background behind nine
 * points across the window, each the first solid colour from the element there up
 * through its parents, then the page's own, then white (a browser's blank page).
 */
export const BRIGHTNESS_JS = `(() => {
  const luma = (colour) => {
    const [r, g, b, a = 1] = (String(colour).match(/[\\d.]+/g) || []).map(Number);
    return r === undefined || a < 0.5 ? null : (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;
  };
  const behind = (el) => {
    for (; el; el = el.parentElement) {
      const v = luma(getComputedStyle(el).backgroundColor);
      if (v !== null) return v;
    }
    return null;
  };
  const page = [document.body, document.documentElement].map((el) => el && luma(getComputedStyle(el).backgroundColor)).find((v) => v !== null && v !== undefined);
  const seen = [];
  for (const fx of [0.2, 0.5, 0.8]) for (const fy of [0.25, 0.5, 0.75]) {
    const v = behind(document.elementFromPoint(innerWidth * fx, innerHeight * fy));
    seen.push(v ?? page ?? 1);
  }
  return seen.reduce((a, b) => a + b, 0) / seen.length;
})()`;

/** 'dark' or 'light' for a brightness, or null when it is not a number. */
export function toneOf(brightness: unknown): PageTone | null {
  if (typeof brightness !== 'number' || !Number.isFinite(brightness)) return null;
  return brightness < DARK_PAGE_LUMA ? 'dark' : 'light';
}

/** The tone of the page `read` evaluates scripts in (the isolated world), or null when it cannot be read. */
export function pageTone(read: ReadPage): Promise<PageTone | null> {
  return Promise.resolve()
    .then(() => read(BRIGHTNESS_JS))
    .then(toneOf)
    .catch(() => null);
}
