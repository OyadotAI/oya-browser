/**
 * The shell's preload: exposes window.oyaBrowser (bridge.ts) to the shell page.
 * Only the shell window loads it; tabs and the control shield get no preload,
 * so pages cannot reach any of this. It runs sandboxed, so it imports nothing
 * but electron; the rest is bundled in.
 */
import { contextBridge, ipcRenderer } from 'electron';
import { buildBridge } from './bridge.ts';

contextBridge.exposeInMainWorld(
  'oyaBrowser',
  buildBridge(ipcRenderer, (query) => window.matchMedia(query).matches),
);
