/** Browser-owned isolated recording bridge, installed before any tab or popup document can execute. */
import path from 'node:path';
import type { WebPreferences } from 'electron';
/** Subframes run only the sandboxed preload; Node remains unavailable in every website world. */
export function recordingPreferences(appDir: string): WebPreferences {
  return {
    sandbox: true,
    contextIsolation: true,
    nodeIntegration: false,
    nodeIntegrationInSubFrames: true,
    preload: path.join(appDir, 'out', 'preload', 'recording.js'),
  };
}
