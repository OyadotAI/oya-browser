/** A browser window is revealed only after its real chrome and selected tab have painted. */
import type { BrowserWindow } from 'electron';
import { withinTime } from '../../shared/within-time.ts';
import { WINDOW_PAINT_TIMEOUT } from './constants.ts';

/** This runs only in Oya's trusted local shell, never in a website. */
export function chromePaintScript(id: number): string {
  return `(async () => {
    await document.fonts.ready; const frame = () => new Promise(resolve => requestAnimationFrame(resolve));
    const ready = () => document.documentElement.dataset.ready === 'true' &&
      document.body.classList.contains('mode-browsing') &&
      document.querySelector('.tab-item[data-id="${id}"] [aria-selected="true"]')?.getBoundingClientRect().width > 0 &&
      document.querySelector('#url-bar')?.getBoundingClientRect().height > 0;
    while (!ready()) await frame(); await frame(); await frame(); return true;
  })()`;
}
/** A stalled renderer never strands the original page in an invisible destination window. */
export async function waitForChrome(win: BrowserWindow, loaded: Promise<unknown>, id: number): Promise<void> {
  const paint = loaded.then(() => win.webContents.executeJavaScript(chromePaintScript(id)));
  await withinTime(paint, WINDOW_PAINT_TIMEOUT, 'The new window could not draw its toolbar. Your tab was not moved.');
}
