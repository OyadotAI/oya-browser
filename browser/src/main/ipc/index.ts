/**
 * Facade: every IPC channel the shell page can call, gathered into one table
 * and registered in one place through the shell-only guard (handle.ts).
 */
import type { AppServices } from '../app/services.ts';
import { ShellIpc, type ShellHandlers } from './handle.ts';
import { NavigationHandlers } from './navigation.ts';
import { SessionHandlers } from './session.ts';
import { RecordingHandlers } from './recording.ts';
import { ShellPageHandlers } from './shell.ts';
import { DevHandlers } from './dev.ts';
import { RoutineHandlers } from './routines.ts';
import { PlaybookHandlers } from './playbooks.ts';
import { UpdateHandlers } from './updates.ts';

export { ShellIpc };
export type { Handler, ShellHandlers } from './handle.ts';

/** The services the shell's calls reach. */
export type IpcDeps = ConstructorParameters<typeof NavigationHandlers>[0] &
  ConstructorParameters<typeof SessionHandlers>[0] &
  ConstructorParameters<typeof RecordingHandlers>[0] &
  ConstructorParameters<typeof ShellPageHandlers>[0] &
  ConstructorParameters<typeof DevHandlers>[0] &
  ConstructorParameters<typeof RoutineHandlers>[0] &
  ConstructorParameters<typeof UpdateHandlers>[0] &
  Pick<AppServices, 'electron' | 'shell'>;

/** The page's own channels: navigation and tabs, the shell's overlays and panel, and the dev panel. */
function pageHandlers(deps: IpcDeps) {
  return {
    ...new NavigationHandlers(deps).handlers,
    ...new ShellPageHandlers(deps).handlers,
    ...new DevHandlers(deps).handlers,
  };
}

/** The app's channels: settings and the session, recording, routines and updates. */
function appHandlers(deps: IpcDeps) {
  return {
    ...new SessionHandlers(deps).handlers,
    ...new RecordingHandlers(deps).handlers,
    ...new RoutineHandlers(deps).handlers,
    ...new PlaybookHandlers(deps).handlers,
    ...new UpdateHandlers(deps).handlers,
  };
}

/** Every channel's handler, one group per area of the shell; a channel without one does not compile. */
export function shellHandlers(deps: IpcDeps): ShellHandlers {
  return { ...pageHandlers(deps), ...appHandlers(deps) };
}

/** Registers every channel on ipcMain, behind the shell-only guard. */
export function registerIpc(deps: IpcDeps): void {
  new ShellIpc(deps).register(shellHandlers(deps));
}
