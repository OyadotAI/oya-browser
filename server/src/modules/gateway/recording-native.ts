/** Sample Oya frames through authorized native commands, retaining one exact browser and tab. */
import { registry } from '../browsers/registry.ts';
import { sendCommand } from '../browsers/socket.ts';
import { NATIVE_RECORD_INTERVAL_MS } from './constants.ts';

/** Resolve only native fleet attachments; vendor sessions retain their external-provider transport. */
export async function nativeRecording(session) {
  const browser = browserFor(session);
  if (!browser || browser.driver.kind !== 'oya') return null;
  const check = () => assertOwner(session, browser);
  check();
  const answer = await sendCommand(session.nativeBrowserId || session.attachedTo, 'list_tabs');
  check();
  const tab = firstTab(answer);
  return new NativeRecording(() => capture(session, tab.id, check));
}
/** A relay endpoint without its exact browser cannot fall back to protocol recording. */
function browserFor(session) {
  const browser = registry.get(session.nativeBrowserId || session.attachedTo);
  const native = /^oya(?:-|$)/.test(session.provider || '') || !session.endpoint?.url;
  if (native && browser?.driver.kind !== 'oya') throw Error('Native recording browser is unavailable');
  return browser;
}
/** Select one public web tab before recording; never fall back to the active page. */
function firstTab(answer) {
  const tab = answer.ok && answer.data?.tabs?.find((tab) => /^https?:\/\//.test(tab.url));
  if (!tab || !Number.isSafeInteger(tab.id)) throw Error('No native recording page is available');
  return tab;
}
/** Reconnects, ownership changes and destroyed gateway sessions cannot retarget an existing recording. */
function assertOwner(session, browser): void {
  if (
    session.closed ||
    registry.get(session.nativeBrowserId || session.attachedTo) !== browser ||
    browser.apiKey !== session.apiKey
  )
    throw Error('Native recording owner is no longer available');
}
/** Native screenshot selection is exact and rechecks browser ownership after the asynchronous command. */
async function capture(session, tabId, check): Promise<string> {
  check();
  const params = { tab_id: tabId, format: 'jpeg' };
  const answer = await sendCommand(session.nativeBrowserId || session.attachedTo, 'screenshot', params);
  check();
  if (!answer.ok || typeof answer.data?.screenshot !== 'string') throw Error(answer.error || 'Native capture failed');
  const match = /^data:image\/jpeg;base64,([A-Za-z0-9+/]+={0,2})$/.exec(answer.data.screenshot);
  if (!match) throw Error('Native capture returned an invalid JPEG');
  return match[1];
}
/** A bounded sequential sampler; stop drains the in-flight capture and prevents late frame publication. */
export class NativeRecording {
  /** Native operation injected so timer and shutdown behavior can be tested without a browser. */
  private readonly capture: () => Promise<string>;
  /** Next sample, scheduled only after the previous sample settles. */
  private timer: ReturnType<typeof setTimeout> | undefined;
  /** Explicit stop permanently prevents this sampler from restarting. */
  private stopped = false;
  /** The one in-flight capture, drained before the final manifest is written. */
  private pending: Promise<void> = Promise.resolve();
  /** Capture contains the exact owner and tab checks. */
  constructor(capture: () => Promise<string>) {
    this.capture = capture;
  }
  /** First capture failures reject startup instead of reporting a recording that never captured. */
  async start(frame: (data: string) => void, failed: (error: unknown) => void): Promise<void> {
    this.pending = this.sample(frame);
    await this.pending;
    this.schedule(frame, failed);
  }
  /** Samples remain serial even when capture takes longer than the requested interval. */
  private schedule(frame: (data: string) => void, failed: (error: unknown) => void): void {
    if (this.stopped) return;
    this.timer = setTimeout(() => this.tick(frame, failed), NATIVE_RECORD_INTERVAL_MS);
    this.timer.unref();
  }
  /** Run one sample and route failures without overlapping the next native operation. */
  private tick(frame: (data: string) => void, failed: (error: unknown) => void): void {
    this.pending = this.sample(frame);
    void this.pending.then(
      () => this.schedule(frame, failed),
      (error) => this.failure(error, failed),
    );
  }
  /** Stop sampling permanently after the first failure and expose the cause to the recording owner. */
  private failure(error: unknown, failed: (error: unknown) => void): void {
    this.stopped = true;
    failed(error);
  }
  /** Discard late pixels once shutdown or control loss has revoked this sampler. */
  private async sample(frame: (data: string) => void): Promise<void> {
    const data = await this.capture();
    if (!this.stopped) frame(data);
  }
  /** Cancel future samples and settle the outstanding command before archiving. */
  async stop(): Promise<void> {
    this.stopped = true;
    clearTimeout(this.timer);
    await this.pending.catch(() => {});
  }
}
