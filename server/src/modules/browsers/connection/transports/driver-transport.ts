/**
 * Commands for outbound clients (CDP: Anchor, Browserbase, Steel, plain Chrome),
 * driven directly with the same action vocabulary as the socket clients.
 */
import { registry } from '../../registry.ts';
import { reportOutcome, type Call } from '../reporter.ts';
import { noteDialog } from '../dialog-notes.ts';
import type { CommandResult, CommandTransport } from './transport.ts';

/** Calls the browser's driver and records the outcome. */
export class DriverTransport implements CommandTransport {
  /** The browser's CDP driver. */
  declare private readonly driver: { send(action: string, params: object, timeout: number): Promise<CommandResult> };

  /** Wraps a connected browser's driver. */
  constructor(driver) {
    this.driver = driver;
  }

  /** Runs the command, then records it and what it learned about the page. */
  async send(call: Call) {
    const started = Date.now();
    const result = await this.driver.send(call.action, call.params, call.timeout).catch((err) => {
      reportOutcome(call, 'error', Date.now() - started, err.message);
      throw err;
    });
    this.observe(call, result, Date.now() - started);
    return result;
  }

  /** A driven browser does not announce where it is; the result tells us. */
  private observe(call: Call, result: CommandResult, ms: number) {
    const ok = result?.ok !== false;
    reportOutcome(call, ok ? 'ok' : 'error', ms, result?.error);
    if (ok && typeof result?.data?.url === 'string') registry.updateUrl(call.browserId, result.data.url);
    noteDialog(call.browserId, result);
  }
}
