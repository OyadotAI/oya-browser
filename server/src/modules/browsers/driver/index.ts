/**
 * The browser driver port and the one place a driver is chosen. Everything
 * else imports from here.
 */
import type { CDPDriver } from '../../../drivers/cdp.ts';
import { OyaDriver } from './oya-driver.ts';
import { CdpDriver } from './cdp-driver.ts';
import type { BrowserDriver } from './port.ts';

export type { BrowserDriver, CdpEndpoint, CdpSocket } from './port.ts';

/** What decides the driver: a CDP engine we dialled, or the socket an Oya browser dialled us on. */
type DriverSpec = {
  /** 'oya' or 'cdp', as the caller registered it. */
  clientType?: string;
  /** The CDP engine this server dialled the browser with; its presence picks the CDP driver. */
  engine?: CDPDriver | null;
  /** The control socket an Oya browser dialled us on. */
  ws?: any;
  /** Whether that Oya browser offers CDP over a relay. */
  cdp?: boolean;
  /** The actions that Oya browser said it does, already checked. */
  actions?: readonly string[] | null;
};

/**
 * The driver for a browser that is registering: CDP when we hold an engine for
 * it, Oya otherwise. A cdp browser with no engine is refused: given an Oya
 * driver it would register, look healthy, and fail every command it was sent.
 */
export function driverFor(spec: DriverSpec, browserId: string): BrowserDriver {
  if (spec.engine) return new CdpDriver(spec.engine);
  if (spec.clientType === 'cdp') throw new Error('A cdp browser needs the engine that drives it');
  return new OyaDriver({ ws: spec.ws, cdp: spec.cdp, actions: spec.actions }, browserId);
}
