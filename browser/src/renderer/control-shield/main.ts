/**
 * The control shield page's entry. The shield is a transparent view over the
 * web page while an agent drives; the site never sees it. The main process
 * drives it through one function, `window.oyaShield(update)`
 * (src/main/shell/control-shield.ts), which this installs.
 */
import { Shield } from './shield.ts';
import type { ShieldUpdate } from './types.ts';

declare global {
  /** The shield page's window. */
  interface Window {
    /** Plays one update from the main process. */
    oyaShield?: (update: ShieldUpdate) => void;
  }
}

const shield = new Shield({ document, view: window, requestFrame: (draw) => window.requestAnimationFrame(draw) });

window.oyaShield = (update) => shield.update(update);
