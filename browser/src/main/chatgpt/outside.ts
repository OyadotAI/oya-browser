/**
 * Opens a sign-in page (ChatGPT, the Oya dashboard) in a browser other than this one. When Oya is the
 * default browser, "open in the browser" hands the page straight back to Oya,
 * which leaves the first-run checklist and shows OpenAI's login in a fresh
 * persona with no ChatGPT session, where it keeps asking to sign in. The person
 * is already signed in to ChatGPT in their usual browser, so the page opens there.
 */
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import type { AppServices } from '../app/services.ts';
import * as C from './constants.ts';

/** The services opening a page uses. */
type Deps = Pick<AppServices, 'electron'>;

/** Runs a command; tests replace it. */
export type Run = (file: string, args: string[]) => Promise<unknown>;

/** The default `run`: the command, its output ignored. */
const execute: Run = promisify(execFile);

/** Opens `url` in another browser when Oya is the default, else in the default browser. */
export async function openOutside(deps: Deps, url: string, platform = process.platform, run = execute) {
  const { app, shell } = deps.electron;
  if (!app.isDefaultProtocolClient('https')) return shell.openExternal(url);
  if (platform === 'darwin') return openOnMac(url, run);
  return shell.openExternal(platform === 'win32' ? C.EDGE_SCHEME + url : url);
}

/** Whether `browser` opened `url`; `open -a` fails at once for an app this Mac does not have. */
const opened = (run: Run, browser: string, url: string) =>
  run('open', ['-a', browser, url]).then(
    () => true,
    () => false,
  );

/** The first of the usual browsers this Mac has. */
async function openOnMac(url: string, run: Run): Promise<void> {
  for (const browser of C.OTHER_MAC_BROWSERS) if (await opened(run, browser, url)) return;
  throw Error('No other browser to sign in with. Install Chrome or Safari and try again.');
}
