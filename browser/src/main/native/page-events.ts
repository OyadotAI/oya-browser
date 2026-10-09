/** Native page readiness notifications; no renderer instrumentation or debugging transport. */
import type { NativePage } from './page.ts';
import type { NativeEventSink } from './log-stream.ts';
import { NATIVE_SECONDS } from './constants.ts';
/** Only events whose native meaning matches the protocol are advertised. */
const EVENTS: Record<string, string> = {
  'dom-ready': 'Page.domContentEventFired',
  'did-finish-load': 'Page.loadEventFired',
};
/** Main-document readiness uses monotonic timestamps and checks authorization at delivery. */
export function watchNativePage(page: NativePage, allowed: () => boolean, emit: NativeEventSink): () => void {
  const listeners = Object.entries(EVENTS).map(([native, method]) =>
    listen(page, native, () => {
      if (!page.webContents.isDestroyed() && allowed()) emit(method, { timestamp: performance.now() / NATIVE_SECONDS });
    }),
  );
  return () => {
    for (const remove of listeners) remove();
  };
}
/** Each enable owns only its own browser-process event listeners. */
function listen(page: NativePage, event: string, listener: () => void): () => void {
  page.webContents.on(event as 'dom-ready', listener);
  return () => {
    page.webContents.off(event as 'dom-ready', listener);
  };
}
/** Reload and stop operate on the already-authorized exact native page. */
export function nativePageCommand(page: NativePage, action: string, params: Record<string, unknown>): object {
  const contents = page.webContents;
  if (contents.isDestroyed()) throw Error('View is destroyed');
  if (action === 'page:stop') contents.stop();
  else if (action === 'page:reload') reload(page, params);
  else throw Error('Unsupported native page operation');
  return {};
}
/** Cache bypass is explicit; script injection and alternate loader assumptions are not accepted. */
function reload(page: NativePage, params: Record<string, unknown>): void {
  if (params.ignoreCache === true) page.webContents.reloadIgnoringCache();
  else page.webContents.reload();
}
