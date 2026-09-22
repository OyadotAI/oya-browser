/**
 * The driver for a browser this server dialled over CDP: a vendor's hosted
 * browser, or a plain Chrome. It wraps the CDP engine, which does the protocol
 * work, in the port the rest of the server speaks.
 */
import type { CDPDriver } from '../../../drivers/cdp.ts';
import { endpointAt } from '../../../drivers/cdp.ts';
import { actionsFor } from '../../../drivers/vocabulary.ts';
import type { Call } from '../connection/reporter.ts';
import { DriverTransport } from '../connection/transports/driver-transport.ts';
import type { BrowserDriver, CdpEndpoint } from './port.ts';

/** Drives one CDP browser through its engine. */
export class CdpDriver implements BrowserDriver {
  /** A browser reached over CDP. */
  readonly kind = 'cdp';
  /** We drive it; it never reports in. Its open socket is the liveness signal. */
  readonly heartbeat = false;
  /** The CDP engine holding the connection. */
  declare readonly engine: CDPDriver;

  /** Wraps a connected engine. */
  constructor(engine: CDPDriver) {
    this.engine = engine;
  }

  /** Runs the command on the engine and records what it learned about the page. */
  send(call: Call) {
    return new DriverTransport(this.engine).send(call);
  }

  /** Whether the engine's CDP socket is open. An engine that cannot say is taken to be alive. */
  isAlive() {
    return typeof this.engine.isAlive !== 'function' || this.engine.isAlive();
  }

  /** Starts a CDP screencast, handing each frame to `onFrame`. */
  startScreencast(onFrame: (dataUrl: string) => void) {
    return this.engine.startScreencast(onFrame);
  }

  /** Stops the CDP screencast. */
  stopScreencast() {
    return this.engine.stopScreencast();
  }

  /** Every cookie the browser holds, read over CDP. */
  cookies() {
    return this.engine.cookies();
  }

  /** The vendor's CDP address, dialled afresh for each caller. */
  cdpEndpoint(): CdpEndpoint | null {
    return this.engine.wsUrl ? endpointAt(this.engine.wsUrl) : null;
  }

  /** The CDP list: every vendor shares one set of handlers. */
  actions() {
    return actionsFor('cdp');
  }

  /** Closes the engine's CDP socket. */
  close() {
    this.engine.close();
  }
}
