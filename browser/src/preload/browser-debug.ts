/** Electron's document-start adapter accepts fixed requests only from native main. */
import { ipcRenderer } from 'electron';
import { BROWSER_DEBUG as C, type DebugRequest } from '../shared/browser-debug.ts';
import { DebugDocument } from './debug-runtime.ts';
import { debugEvaluate } from './debug-eval.ts';
const runtime = new DebugDocument(debugEvaluate, (event) => ipcRenderer.send(C.REGISTER, event));
/** Installed synchronously during preload execution, before any author code. */
export function installBrowserDebug(): void {
  ipcRenderer.on(
    C.REQUEST,
    (_event, request: DebugRequest) =>
      void runtime.execute(request).then((reply) => ipcRenderer.send(C.RESPONSE, reply)),
  );
  runtime.register();
}
