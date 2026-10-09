/** Pause only Oya-owned UI motion through native page execution, never a debugger or site media override. */
import type { WebContents } from 'electron';
/** Only trusted shell surfaces are passed to this helper. */
interface ShellPage {
  /** Native execution and document lifecycle, without debugger capabilities. */
  webContents: Pick<WebContents, 'on' | 'isDestroyed' | 'executeJavaScript'>;
}
/** Window focus determines whether decorative shell motion is useful. */
interface FocusEvents {
  /** Subscribes for this window's lifetime. */
  on(event: 'focus' | 'blur', listener: () => void): unknown;
}
/** The container always conserves animation work. */
export const inContainer = (env: NodeJS.ProcessEnv = process.env): boolean => env.OYA_DOCKER === 'true';
/** Set the local UI policy; never modify matchMedia or a visited page's fingerprint. */
async function apply(view: ShellPage, still: boolean): Promise<void> {
  try {
    if (!view.webContents.isDestroyed()) {
      await view.webContents.executeJavaScript(`document.documentElement.dataset.oyaStill = '${still}';`);
    }
  } catch {
    /* Closing or replacing a shell document is harmless; dom-ready reapplies its policy. */
  }
}
/** Containers retain their policy across shell reloads; the URL carries it before first paint. */
export async function holdStill(view: ShellPage, still: boolean): Promise<void> {
  if (!still) return;
  view.webContents.on('dom-ready', () => void apply(view, true));
  await apply(view, true);
}
/** Latest focus state survives document replacement without a global theme/media override. */
export function stillWhileAway(win: ShellPage & FocusEvents): void {
  let still = false;
  /** Focus changes update the policy before sending it to the current document. */
  const update = (value: boolean) => {
    still = value;
    void apply(win, still);
  };
  win.on('blur', () => update(true));
  win.on('focus', () => update(false));
  win.webContents.on('dom-ready', () => void apply(win, still));
}
