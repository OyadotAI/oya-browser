/**
 * In a container the shell's own pages hold still. There is no GPU there, so
 * Chromium paints every frame on the CPU, and the app keeps rendering while no
 * one watches (KEEP_RENDERING_SWITCHES); the start page's looping animations
 * alone kept an idle container at several CPUs. Each shell page is told to
 * prefer reduced motion, which its stylesheets already honour. Only the
 * shell's pages: visited pages keep the persona's own setting, so the
 * fingerprint does not change.
 *
 * On a desktop the same switch holds the shell still while the window is not
 * the app in front: KEEP_RENDERING_SWITCHES keep it reporting "visible", so
 * visibility never pauses anything, and the start page's loops under its
 * frosted glass cost a third of a core behind other apps.
 */
import { cdp, type PageView } from '../cdp/cdp.ts';

/** The media the shell's pages are told they are on. */
const STILL = { features: [{ name: 'prefers-reduced-motion', value: 'reduce' }] };

/** No emulated media: the page follows the system's own motion setting again. */
const MOVING = { features: [] };

/** The window events that move the shell, or hold it still. */
interface FocusEvents {
  /** Subscribes to the window's focus and blur. */
  on(event: 'focus' | 'blur', listener: () => void): unknown;
}

/** Whether this browser runs in a container (the image sets OYA_DOCKER). */
export const inContainer = (env: NodeJS.ProcessEnv = process.env): boolean => env.OYA_DOCKER === 'true';

/** Tells one of the shell's pages to prefer reduced motion, when `still`; a failure only leaves it animating. */
export async function holdStill(view: PageView, still: boolean): Promise<void> {
  if (!still) return;
  await cdp(view, 'Emulation.setEmulatedMedia', STILL).catch(() => {});
}

/** Holds the window's shell page still while another app is in front, and lets it move when the window comes back. */
export function stillWhileAway(win: PageView & FocusEvents): void {
  win.on('blur', () => void emulate(win, STILL));
  win.on('focus', () => void emulate(win, MOVING));
}

/** Sends one media emulation; a failure only leaves the page as it was. */
const emulate = (view: PageView, media: typeof STILL | typeof MOVING) =>
  cdp(view, 'Emulation.setEmulatedMedia', media).catch(() => {});
