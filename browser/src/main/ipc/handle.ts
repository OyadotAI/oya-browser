/**
 * Every IPC channel is the shell's. Tabs and the control shield have no preload,
 * so a call from anywhere else is not ours to answer. The handlers are typed
 * against the contract in src/shared/ipc.ts: a channel's arguments and answer
 * are the ones the preload's bridge method declares.
 */
import type { IpcMainInvokeEvent } from 'electron';
import type { CallArgs, CallChannel, CallResult } from '../../shared/ipc.ts';
import type { AppServices } from '../app/services.ts';

/** What a handler may answer: the call's result (or a promise of it); a call answering nothing may answer anything. */
type Answer<T> = [T] extends [void] ? unknown : T | Promise<T>;

/** The handler of call channel `C`: the invoking event, then the call's own arguments. */
export type Handler<C extends CallChannel> = (event: IpcMainInvokeEvent, ...args: CallArgs<C>) => Answer<CallResult<C>>;

/** A handler for every call channel; a channel left out does not compile. */
export type ShellHandlers = { [C in CallChannel]: Handler<C> };

/** The handlers of channels `C`, as one group of them declares. */
export type HandlersOf<C extends CallChannel> = Pick<ShellHandlers, C>;

/** A handler as ipcMain calls it: the page's arguments, unchecked. */
type Untyped = (event: IpcMainInvokeEvent, ...args: unknown[]) => unknown;

/** The services the guard reads: Electron's ipcMain and the shell window. */
type Deps = Pick<AppServices, 'electron' | 'shell'>;

/** Registers the shell's handlers on ipcMain, each behind the check that the shell page called it. */
export class ShellIpc {
  /** The main-process services. */
  private readonly deps: Deps;

  /** `deps` gives ipcMain and the shell window. */
  constructor(deps: Deps) {
    this.deps = deps;
  }

  /** Throws unless the call came from the shell page's main frame. */
  requireShell(event: Pick<IpcMainInvokeEvent, 'sender' | 'senderFrame'>): void {
    const shell = this.deps.shell.window;
    if (event.sender !== shell?.webContents || event.senderFrame !== shell.webContents.mainFrame) {
      throw new Error('Only the Oya workspace can use this command');
    }
  }

  /** Registers every channel of `handlers`. */
  register(handlers: ShellHandlers): void {
    for (const channel of Object.keys(handlers) as CallChannel[]) this.handle(channel, handlers[channel] as Untyped);
  }

  /** ipcMain.handle for one channel, guarded by requireShell. The arguments are the page's, as it sent them. */
  private handle(channel: CallChannel, fn: Untyped): void {
    this.deps.electron.ipcMain.handle(channel, (event, ...args: unknown[]) => {
      this.requireShell(event);
      return fn(event, ...args);
    });
  }
}
