/**
 * Loads the control shield page for a test: the body of its index.html in the
 * fake DOM, with the Shield built over it the way main.ts builds it, and a
 * frame queue the test runs by hand.
 */
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';
import { documentFrom } from '../../support/fake-dom.cjs';
import { Shield } from '../../../../src/renderer/control-shield/shield.ts';

/** The page under test. */
const PAGE = fileURLToPath(new URL('../../../../src/renderer/control-shield/index.html', import.meta.url));

/** The loaded page and the handles a test drives it by. */
export interface ShieldPage {
  /** The fake document. */
  document: any;
  /** An element by id. */
  $: (id: string) => any;
  /** What the main process calls. */
  oyaShield: (update: unknown) => void;
  /** Runs every animation frame asked for so far, at `now`. */
  runFrames: (now: number) => void;
}

/** Loads the page in a window 1280 by 800, as the legacy harness did. */
export function loadShield(): ShieldPage {
  const document = documentFrom(fs.readFileSync(PAGE, 'utf8'));
  const frames: ((now: number) => void)[] = [];
  const view = { innerWidth: 1280, innerHeight: 800, devicePixelRatio: 1 };
  const shield = new Shield({ document, view, requestFrame: (draw) => frames.push(draw) });
  return {
    document,
    $: (id) => document.getElementById(id),
    oyaShield: (update) => shield.update(update),
    runFrames: (now) => frames.splice(0).forEach((draw) => draw(now)),
  };
}
