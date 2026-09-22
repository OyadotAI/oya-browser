/**
 * Commands for outbound clients (CDP: Anchor, Browserbase, Steel, plain Chrome),
 * driven directly with the same action vocabulary as the socket clients.
 */
import { registry } from '../../registry.ts';
import { CdpConnectionError } from '../../../../drivers/cdp.ts';
import { HttpError } from '../../../../platform/errors.ts';
import { reportOutcome, type Call } from '../reporter.ts';
import { noteDialog } from '../dialog-notes.ts';
import type { CommandResult, CommandTransport } from './transport.ts';

/** What the page's refusal is answered as: the same failed result an Oya browser sends, with one code. */
function refused(call: Call, err): CommandResult {
  return { ok: false, error: refusalText(call, err.message), code: 'command_failed' };
}

/** The page's words, except a CDP "no target" which names the tab the caller asked for. */
function refusalText(call: Call, message: string) {
  const asked = call.params as Record<string, unknown>;
  const tab = asked?.id ?? asked?.tab_id;
  return /No target with given id/.test(message) && tab ? `Tab ${tab} not found` : message;
}

/** Calls the browser's driver and records the outcome. */
export class DriverTransport implements CommandTransport {
  /** The browser's CDP driver. */
  declare private readonly driver: {
    /** Runs one action on the engine. */
    send(action: string, params: object, timeout: number): Promise<CommandResult>;
    /** Whether the engine's connection is still open. */
    isAlive?: () => boolean;
  };

  /** Wraps a connected browser's driver. */
  constructor(driver) {
    this.driver = driver;
  }

  /**
   * Runs the command, then records it and what it learned about the page. A
   * page that refused is a failed result, like an Oya browser answers; only a
   * lost connection is thrown, because then the outcome is unknown.
   */
  async send(call: Call) {
    const started = Date.now();
    const result = await this.driver.send(call.action, call.params, call.timeout).catch((err) => {
      if (this.connectionLost(err)) return this.lost(call, err, started);
      return refused(call, err);
    });
    this.observe(call, result, Date.now() - started);
    return result;
  }

  /**
   * Whether the error must be thrown rather than answered as the page's refusal:
   * the connection is gone, so the outcome is unknown; or the error already
   * carries its own HTTP answer, such as a timeout or a refused command slot.
   */
  private connectionLost(err) {
    if (err instanceof HttpError) return true;
    return err instanceof CdpConnectionError || this.driver.isAlive?.() === false;
  }

  /** A lost connection is recorded and thrown: the caller must be told the outcome is unknown. */
  private lost(call: Call, err, started: number): never {
    reportOutcome(call, 'error', Date.now() - started, err.message);
    // A dead engine's own words are a lost connection too, whatever class it threw.
    throw err instanceof HttpError || err instanceof CdpConnectionError ? err : new CdpConnectionError(err.message);
  }

  /** A driven browser does not announce where it is; the result tells us. */
  private observe(call: Call, result: CommandResult, ms: number) {
    const ok = result?.ok !== false;
    reportOutcome(call, ok ? 'ok' : 'error', ms, result?.error);
    if (ok && typeof result?.data?.url === 'string') registry.updateUrl(call.browserId, result.data.url);
    noteDialog(call.browserId, result);
  }
}
