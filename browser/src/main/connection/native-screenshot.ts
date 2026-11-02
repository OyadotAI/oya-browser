/** Exact-tab recording capture never follows focus or exposes internal and unprotected surfaces. */
import { capturePage } from '../native/index.ts';
import { isWebAddress } from '../tabs/navigation.ts';
import { SCREENSHOT_JPEG_QUALITY } from '../actions/constants.ts';
import type { CommandDeps } from './commands.ts';
import type { Tab } from '../tabs/types.ts';

/** Check control and exact live object identity both before capture and before returning its pixels. */
function target(deps: CommandDeps, id: number): Tab {
  const state = deps.control.snapshot();
  if (state.mode !== 'agent' || state.mine) throw Error('Recording capture requires agent control');
  const tab = (deps.windows?.allTabs() || deps.tabs.list).find((tab) => tab.id === id);
  if (!tab || tab.home || tab.protection !== 'protected' || tab.view.webContents.isDestroyed())
    throw Error('Recording target is unavailable or unprotected');
  if (!isWebAddress(tab.view.webContents.getURL())) throw Error('Recording target must be a web page');
  return tab;
}
/** A specified tab is mandatory and cannot be coerced into another tab or active-page fallback. */
export async function captureSpecifiedTab(deps: CommandDeps, id: unknown, format?: unknown): Promise<string> {
  if (!Number.isSafeInteger(id)) throw Error('Invalid recording tab id');
  const tab = target(deps, id as number);
  const revision = deps.control.snapshot().revision;
  await deps.translation?.released(tab.view);
  if (target(deps, id as number) !== tab) throw Error('Recording target changed during restoration');
  const image = await capturePage(tab.view, format === 'jpeg' ? SCREENSHOT_JPEG_QUALITY : undefined);
  screenshotUnchanged(deps, id as number, tab, revision);
  return image;
}

/** Pixels cannot cross a control revision or a replaced tab after an asynchronous capture. */
function screenshotUnchanged(deps: CommandDeps, id: number, tab: Tab, revision: number | undefined): void {
  if (target(deps, id) !== tab || deps.control.snapshot().revision !== revision)
    throw Error('Recording target or owner changed during capture');
}
