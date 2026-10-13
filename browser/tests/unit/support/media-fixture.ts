/**
 * A MediaPermissions over fake native devices, OS prompts and an in-memory
 * grant store: what the camera and screen-share tests drive. No real hardware
 * is touched and nothing is written to disk.
 */
import { EventEmitter } from 'node:events';
import { MediaPermissions } from '../../../src/main/app/media-permissions.ts';
import { SitePermissionStore } from '../../../src/main/app/site-permissions.ts';
import { memoryFile } from './main-ctx.cjs';

/** Requests describe the actual secure requesting frame. */
export const details = {
  mediaTypes: ['audio'],
  requestingUrl: 'https://call.test/room',
  securityOrigin: 'https://call.test',
  isMainFrame: true,
};

/** The service, its fakes, and what was prompted and asked of the OS. */
export function fixture(platform = 'darwin', file = memoryFile()) {
  const state = {
    url: details.requestingUrl,
    interactive: true,
    response: 1,
    os: true,
    destroyed: false,
    crashed: false,
  };
  const contents = Object.assign(new EventEmitter(), {
    getURL: () => state.url,
    isDestroyed: () => state.destroyed,
    isCrashed: () => state.crashed,
  });
  const prompts: any[] = [],
    devices: string[] = [];
  const deps = {
    electron: {
      dialog: {
        showMessageBox: async (options: any) => {
          prompts.push(options);
          return { response: state.response };
        },
      },
      systemPreferences: {
        getMediaAccessStatus: () => (state.os ? 'granted' : 'denied'),
        askForMediaAccess: async (type: string) => {
          devices.push(type);
          return state.os;
        },
      },
    },
    shell: { window: null },
    tabs: { activeTabId: 1, getActiveView: () => ({ webContents: contents }) },
    agentTab: { humanMay: (id: number) => id === 1 && state.interactive },
    governance: { configuration: null as object | null },
    persona: { partitionName: () => 'persist:one' },
    sitePermissions: new SitePermissionStore(file),
  };
  const service = new MediaPermissions(deps as any, platform);
  const check = (mediaType = 'audio') =>
    service.check(contents as any, 'https://call.test', { mediaType, isMainFrame: true } as any);
  return { state, contents, deps, prompts, devices, service, check, file };
}
