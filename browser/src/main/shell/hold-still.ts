/**
 * In a container the shell's own pages hold still. There is no GPU there, so
 * Chromium paints every frame on the CPU, and the app keeps rendering while no
 * one watches (KEEP_RENDERING_SWITCHES); the start page's looping animations
 * alone kept an idle container at several CPUs. Each shell page is told to
 * prefer reduced motion, which its stylesheets already honour. Only the
 * shell's pages: visited pages keep the persona's own setting, so the
 * fingerprint does not change.
 */
import { cdp, type PageView } from '../cdp/cdp.ts';

/** The media the shell's pages are told they are on. */
const STILL = { features: [{ name: 'prefers-reduced-motion', value: 'reduce' }] };

/** Whether this browser runs in a container (the image sets OYA_DOCKER). */
export const inContainer = (env: NodeJS.ProcessEnv = process.env): boolean => env.OYA_DOCKER === 'true';

/** Tells one of the shell's pages to prefer reduced motion, when `still`; a failure only leaves it animating. */
export async function holdStill(view: PageView, still: boolean): Promise<void> {
  if (!still) return;
  await cdp(view, 'Emulation.setEmulatedMedia', STILL).catch(() => {});
}
