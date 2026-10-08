/** Project playbooks exposed to the trusted shell through existing server APIs. */
import type { AppServices } from '../app/services.ts';
import type { Payload } from '../../shared/ipc.ts';
import type { HandlersOf } from './handle.ts';
import { PlaybookRunner } from '../connection/playbook-run.ts';
import { ServerApi } from '../connection/server-api.ts';
import { importPlaybook, exportPlaybook } from './playbook-files.ts';

/** Services used by the library and current-browser replay. */
type Deps = Pick<AppServices, 'config' | 'socket' | 'shell' | 'electron' | 'control' | 'recorder' | 'chatAbort'>;
/** A required string received from the shell. */
function nameOf(value: unknown): string {
  if (typeof value !== 'string' || !value.trim()) throw new Error('Choose a playbook first.');
  return value.trim();
}
/** Library operations, constrained to known routes rather than arbitrary shell HTTP. */
export class PlaybookHandlers {
  /** Services kept in the main process. */
  private readonly deps: Deps;
  /** Authenticated project API. */
  private readonly api: ServerApi;
  /** Background replay ownership and exclusion. */
  private readonly runner: PlaybookRunner;
  /** Fixed command map. */
  private readonly commands: Record<string, (command: Payload) => Promise<unknown>>;
  /** Channels registered behind the shell-only IPC guard. */
  readonly handlers: HandlersOf<'playbooks'> = { playbooks: (_event, command) => this.call(command) };
  /** Builds the allowlisted library operations. */
  constructor(deps: Deps) {
    this.deps = deps;
    this.api = new ServerApi(deps);
    this.runner = new PlaybookRunner(deps);
    this.commands = this.operations();
  }
  /** Rejects unknown commands and reports recoverable API failures to the pane. */
  async call(command: Payload): Promise<Payload> {
    try {
      if (!this.api.canCall()) throw new Error('Reconnect to Oya to open your playbooks.');
      const action = String(command?.action);
      if (!Object.hasOwn(this.commands, action)) throw new Error('Unknown playbook action.');
      return (await this.commands[action](command)) as Payload;
    } catch (error) {
      return { error: (error as Error).message };
    }
  }
  /** Read and write operations all use the current project credential. */
  private operations(): Record<string, (command: Payload) => Promise<unknown>> {
    return {
      list: () => this.api.get('playbooks'),
      rename: (c) => this.api.send('PATCH', this.route(c), { name: nameOf(c.to) }),
      delete: (c) => this.api.send('DELETE', this.route(c)),
      export: (c) => exportPlaybook(this.deps, nameOf(c.name)),
      import: () => importPlaybook(this.deps),
      ...this.runOperations(),
    };
  }
  /** Escape names as one path segment. */
  private route(command: Payload): string {
    return `playbooks/${encodeURIComponent(nameOf(command.name))}`;
  }
  /** Status and human assistance stay within the same project API. */
  private runOperations(): Record<string, (command: Payload) => Promise<unknown>> {
    return {
      run: (c) => this.run(c),
      status: (c) => this.api.get(`runs/${encodeURIComponent(nameOf(c.id))}`),
      respond: (c) => this.respond(c),
    };
  }
  /** Hand back a human hold before telling the waiting run to continue. */
  private async respond(command: Payload): Promise<unknown> {
    const state = this.deps.control.snapshot();
    if (state.mode === 'human' && state.mine) await this.deps.control.change('return');
    return this.api.post(`runs/${encodeURIComponent(nameOf(command.id))}/respond`, { response: command.response });
  }
  /** Starts replay through the same local exclusion gate as Ask and routines. */
  private run(command: Payload): Promise<unknown> {
    const body = { playbook: nameOf(command.name), data: command.values || {}, autoHeal: command.autoHeal !== false };
    return this.runner.start(body);
  }
}
