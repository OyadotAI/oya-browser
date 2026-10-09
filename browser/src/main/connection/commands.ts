/**
 * Commands from the server (`cmd` messages): run each one, answer it once, and
 * race it against a JavaScript dialog that would otherwise block it forever.
 */
import { DIALOG_SAFE_ACTIONS, DIALOG_HELD, describeDialog } from '../dialogs/index.ts';
import type { AppServices } from '../app/services.ts';
import type { CommandId, CommandParams } from '../actions/types.ts';
import { resultSummary } from './result-summary.ts';
import { TAB_COMMANDS } from './tab-commands.ts';
import { RESULT_CODES } from './constants.ts';

/** What the commands use: the shell, the tabs and page actions, the socket, and the recorder and studio. */
export type CommandDeps = Pick<
  AppServices,
  | 'windows'
  | 'shell'
  | 'tabs'
  | 'actions'
  | 'socket'
  | 'workspace'
  | 'recorder'
  | 'observer'
  | 'dialogs'
  | 'library'
  | 'notifications'
>;

/** A command's arguments: a page command's, plus those of the commands answered here. */
export interface RemoteParams extends CommandParams {
  /** List unread inbox entries without changing their state. */
  unread_only?: boolean;
  /** Explicit inbox entries to mark read. */
  ids?: string[];
  /** The one notification to dismiss; omission never clears the inbox. */
  notification_id?: string;
  /** Local history/bookmark substring search. */
  query?: string;
  /** Matching library entries to skip. */
  offset?: number;
  /** Saved bookmark title. */
  title?: string;
  /** Explicit authorization to clear the current profile's stored history. */
  confirm?: boolean;
  /** The tab to switch to or close. */
  tab_id?: number;
  /** record: start, stop, pause and the like. */
  mode?: string;
  /** handle_dialog: accept rather than dismiss. */
  accept?: boolean;
  /** handle_dialog: a prompt's answer. */
  prompt_text?: unknown;
  /** handle_dialog: a prompt's answer, as older callers spell it. */
  promptText?: unknown;
  /** workflow: the draft to play. */
  draft?: object;
  /** workflow: values for its variables. */
  variables?: Record<string, unknown>;
  /** workflow: false turns off healing a step whose target moved. */
  autoHeal?: boolean;
}

/** A `cmd` message: the command's id, what to do, and its arguments. */
export interface CommandMessage {
  /** Carried back on the result. */
  id?: CommandId;
  /** The action; anything but a string is refused. */
  action?: unknown;
  /** The action's arguments. */
  params?: RemoteParams;
}

/** The code to send with a failed result: one of ours, or none. */
const codeOf = (err: unknown): string | undefined => {
  const code = (err as Partial<NodeJS.ErrnoException> | null | undefined)?.code;
  return RESULT_CODES.has(code) ? code : undefined;
};

/** A thrown value as the error text a result carries. */
const messageOf = (err: unknown): string => (err as Error | null | undefined)?.message || String(err);

/** Runs server commands and sends their results. */
export class CommandRunner {
  /** The main-process services; the tab commands read them too. */
  readonly deps: CommandDeps;
  /**
   * Commands already answered early because a dialog interrupted them. Each entry
   * is removed by the late answer it is waiting for.
   * ponytail: an entry outlives the session if that answer never lands; bounded by
   * the number of dialogs raised.
   */
  private readonly answeredCommands = new Set<CommandId | undefined>();

  /** `deps` is the main-process services (see src/main/main.ts). */
  constructor(deps: CommandDeps) {
    this.deps = deps;
  }

  /**
   * A confirm() or prompt() opens *during* the command that triggered it and
   * blocks the renderer, so the command it interrupted can never finish. Race the
   * held dialog against it: the caller hears about the dialog now instead of
   * waiting out a timeout that cannot succeed. The interrupted command is left to
   * settle on its own; sendResult drops its late answer.
   */
  async handleCommand(msg: CommandMessage): Promise<void> {
    const { held, release } = this.deps.dialogs.held();
    const ran = this.runSafely(msg);
    const outcome = await Promise.race([ran, held]);
    release();
    if (outcome !== DIALOG_HELD) return;
    // Answer first: marking the id before this would make sendResult drop the very
    // result being sent, and the caller would wait out the timeout after all.
    this.sendResult(msg.id, false, null, describeDialog(this.deps.dialogs.current()!, false));
    // The interrupted command is still out there; drop its answer when it lands.
    this.answeredCommands.add(msg.id);
  }

  /**
   * handleCommand is called fire-and-forget by the socket router, so a throw
   * escaping here would take down the main process rather than one command.
   */
  private runSafely(msg: CommandMessage): Promise<null> {
    const failed = (e: unknown): null => {
      this.sendResult(msg.id, false, null, messageOf(e), codeOf(e));
      return null;
    };
    return this.runCommand(msg).then(() => null, failed);
  }

  /** Answers one command. */
  private async runCommand(msg: CommandMessage): Promise<void> {
    const { id, action, params } = msg;
    if (!this.deps.shell.browsingMode) return this.sendResult(id, false, null, 'Browser not ready');
    // The command maps look an action up as a key, which would read `["evaluate_raw"]` as the string.
    if (typeof action !== 'string') return this.sendResult(id, false, null, 'action must be a string');
    // A held dialog blocks the renderer: anything that touches the page would sit
    // there until its timeout and tell the caller nothing. Answer with the dialog
    // instead, so the next move is obvious and costs no wall clock.
    if (this.blockedByDialog(id, action)) return;
    if (action === 'handle_dialog') return this.handleDialog(id, params);
    await this.dispatch(id, action, params).catch((err: unknown) =>
      this.sendResult(id, false, null, messageOf(err), codeOf(err)),
    );
  }

  /** Answers with the held dialog when it blocks this action. */
  private blockedByDialog(id: CommandId | undefined, action: string): boolean {
    const dialog = this.deps.dialogs.current();
    if (!dialog || DIALOG_SAFE_ACTIONS.has(action)) return false;
    this.sendResult(id, false, null, describeDialog(dialog, false));
    return true;
  }

  /** Accepts or dismisses the open dialog. */
  private async handleDialog(id: CommandId | undefined, params: RemoteParams | undefined): Promise<void> {
    const r = await this.deps.dialogs.answer(params?.accept, params?.prompt_text ?? params?.promptText);
    this.sendResult(id, r.ok, r.data, r.error);
  }

  /** Tab-level commands here; everything else acts on the active page. */
  private async dispatch(id: CommandId | undefined, action: string, params: RemoteParams | undefined) {
    if (Object.hasOwn(TAB_COMMANDS, action)) return TAB_COMMANDS[action](this, id, params);
    // All remaining actions need an active tab
    const view = this.deps.tabs.getActiveView();
    if (!view || view.webContents.isDestroyed()) return this.sendResult(id, false, null, 'No active tab');
    await this.deps.actions.runPageAction(id!, action, params, view);
  }

  /** Sends one result, with any dialog that fired during it, and its code when it has one of ours; drops a late answer already given. */
  sendResult(id: CommandId | undefined, ok: boolean, data?: unknown, error?: string | null, code?: string): void {
    if (!this.deps.socket.isOpen()) return;
    if (this.answeredCommands.delete(id)) return;
    // Every command result passes through here, which makes it the one place a
    // dialog that fired mid-action can be reported without editing 40 call sites.
    const dialog = this.deps.dialogs.takeNotes();
    if (dialog) data = { ...((data as object | null) || {}), dialog };
    this.deps.shell.devLog('out', ok ? 'result: ok' : 'result: error', resultSummary(id, ok, data, error));
    const coded = RESULT_CODES.has(code) ? { code } : {};
    this.deps.socket.send({ type: 'cmd_result', id, ok, data: data || null, error: error || null, ...coded });
  }
}
