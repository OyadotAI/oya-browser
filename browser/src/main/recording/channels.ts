/**
 * One RecordingChannel per tab: it arms the page's recorder in the isolated
 * world and carries its events out, surviving navigations.
 */
import { randomBytes } from 'node:crypto';
import type { AppServices } from '../app/services.ts';
import { NativeRecordingChannel } from './native-channel.ts';
import type { NativePage as PageView } from '../native/index.ts';
import { ANALYZER_ATTR_BYTES } from './constants.ts';
import type { PageOutput, RecordingTab } from './types.ts';

/** The services the channels use; the recorder receives what each channel delivers. */
type Deps = Pick<AppServices, 'tabs' | 'analyzerScript' | 'recorder'>;

/** One view's channel, as the channels use it. */
interface Channel {
  /** Settles once the page's recorder is running; set as the channel is armed. */
  ready?: Promise<unknown>;
  /** Arms the page's recorder. */
  start(): Promise<unknown>;
  /** Disarms it after collecting final buffered typing. */
  stop(): Promise<unknown>;
  /** Collects what the page buffered; `final` takes even unfinished typing. */
  drain(final?: boolean): Promise<unknown>;
}

/** Stops one channel; resolves to a live tab's failure, or null. */
async function stopOne(view: PageView, channel: Channel): Promise<unknown> {
  try {
    await channel.stop();
    await channel.ready?.catch(() => {});
    return null;
  } catch (err) {
    return view.webContents.isDestroyed() ? null : err;
  }
}

/** View → its recording channel. */
export class RecordingChannels {
  /** The services the channels use. */
  private readonly deps: Deps;
  /** View → channel. */
  private readonly channels = new Map<PageView, Channel>();

  /** `deps` is the main-process services (see src/main/main.ts). */
  constructor(deps: Deps) {
    this.deps = deps;
  }

  /** Arms a view; resolves once its recorder is running. */
  async armRecordingView(view: PageView): Promise<unknown> {
    if (this.channels.has(view)) return this.channels.get(view)?.ready;
    const channel = this.createChannel(view);
    this.channels.set(view, channel);
    channel.ready = channel.start();
    await this.armed(view, channel);
  }

  /** Waits for a channel to start; one that fails is forgotten, so the next arm retries. */
  private async armed(view: PageView, channel: Channel): Promise<void> {
    try {
      await channel.ready;
    } catch (err) {
      if (this.channels.get(view) === channel) this.channels.delete(view);
      throw err;
    }
  }

  /** A channel for one view, remembering where the view started and which tab it is. */
  private createChannel(view: PageView): Channel {
    const startingUrl = view.webContents.getURL();
    const tabId = this.deps.tabs.list.find((t: RecordingTab) => t.view === view)?.id;
    const receive = (out: PageOutput) => this.deps.recorder.receive(view, startingUrl, out, tabId);
    return new NativeRecordingChannel(view.webContents, this.analyzer(), receive);
  }

  /** The analyzer with a fresh tag attribute, not yet recording. */
  private analyzer(): string {
    const attr = 'data-' + randomBytes(ANALYZER_ATTR_BYTES).toString('hex');
    return this.deps.analyzerScript.replace('__OYA_ATTR__', attr).replace('__OYA_RECORD__', 'false');
  }

  /** Collects what one view buffered. */
  async drain(view: PageView, final?: boolean): Promise<void> {
    await this.channels.get(view)?.drain(final);
  }

  /**
   * Stops every channel and forgets them all, whatever happens. A closed tab's
   * failure is expected; a live tab's is thrown, but only after the others are
   * stopped, so one refusing page never leaves the rest listening.
   */
  async stopAll(): Promise<void> {
    const channels = [...this.channels];
    this.channels.clear();
    let failure: unknown = null;
    for (const [view, channel] of channels) failure = (await stopOne(view, channel)) || failure;
    if (failure) throw failure;
  }

  /** A closed tab's channel: stopped in the background and forgotten. */
  forget(view: PageView): void {
    const channel = this.channels.get(view);
    this.channels.delete(view);
    if (channel) stopOne(view, channel).catch(() => {});
  }
}
