/**
 * One RecordingChannel per tab: it arms the page's recorder in the isolated
 * world and carries its events out, surviving navigations.
 */
import { randomBytes } from 'node:crypto';
import type { AppServices } from '../app/services.ts';
import { RecordingChannel as UntypedChannel } from '../../page/recording.ts';
import { withinTime } from '../../shared/within-time.ts';
import { cdpAttach, cdp, type PageView } from '../cdp/cdp.ts';
import { framePorts } from './frame-sessions.ts';
import { ANALYZER_ATTR_BYTES, RECORDING_CDP_MS } from './constants.ts';
import type { PageOutput, RecordingTab } from './types.ts';

/** The services the channels use; the recorder receives what each channel delivers. */
type Deps = Pick<AppServices, 'tabs' | 'isolatedWorld' | 'analyzerScript' | 'recorder'>;

/** One view's channel, as the channels use it. */
interface Channel {
  /** Settles once the page's recorder is running; set as the channel is armed. */
  ready?: Promise<unknown>;
  /** Arms the page's recorder. */
  start(): Promise<unknown>;
  /** Disarms it, collecting nothing more. */
  stop(): Promise<unknown>;
  /** Collects what the page buffered; `final` takes even unfinished typing. */
  drain(final?: boolean): Promise<unknown>;
}

/** What a channel is built with: its transport, the iframes it can arm, and where its steps go. */
interface ChannelOptions {
  /** Sends one CDP command to the view's page. */
  send: (method: string, params?: object) => Promise<unknown>;
  /** Subscribes to one CDP event of the view's page; returns the unsubscribe. */
  on: (method: string, fn: (params: unknown) => void) => () => void;
  /** Turns the Runtime domain off again on stop. */
  disableRuntimeOnStop: boolean;
  /** The view's cross-site iframes, or null when untracked. */
  frames: ReturnType<typeof framePorts>;
  /** The isolated world the recorder runs in. */
  worldName: string;
  /** The analyzer's source, tagged and not yet recording. */
  analyzer: string;
  /** Hands on what the page delivered. */
  receive: (out: PageOutput) => void;
}

/** The page channel (src/page/recording.ts, shared with the server), typed for how it is built here. */
const RecordingChannel = UntypedChannel as unknown as new (options: ChannelOptions) => Channel;

/** Subscribes to one CDP event of the view's own page, not its iframes' sessions; returns the unsubscribe. */
function listenOnView(view: PageView, method: string, fn: (params: unknown) => void): () => void {
  const dbg = cdpAttach(view);
  if (!dbg) throw new Error('View is destroyed');
  const listener = (_event: unknown, name: string, params: unknown, sessionId?: string) => {
    if (name === method && !sessionId) fn(params);
  };
  dbg.on('message', listener);
  return () => dbg.off('message', listener);
}

/** Stops one channel; resolves to a live tab's failure, or null. */
async function stopOne(view: PageView, channel: Channel): Promise<unknown> {
  await channel.ready?.catch(() => {});
  try {
    await channel.stop();
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
      this.channels.delete(view);
      throw err;
    }
  }

  /** A channel for one view, remembering where the view started and which tab it is. */
  private createChannel(view: PageView): Channel {
    const startingUrl = view.webContents.getURL();
    const tabId = this.deps.tabs.list.find((t: RecordingTab) => t.view === view)?.id;
    return new RecordingChannel(this.channelOptions(view, startingUrl, tabId));
  }

  /** How the channel talks to the view (never waiting forever) and where its steps go. */
  private channelOptions(view: PageView, startingUrl: string, tabId?: number): ChannelOptions {
    return {
      send: (method: string, params?: object) =>
        withinTime(cdp(view, method, params), RECORDING_CDP_MS, 'The page did not answer'),
      on: (method: string, fn: (params: unknown) => void) => listenOnView(view, method, fn),
      ...{ disableRuntimeOnStop: true, frames: framePorts(view) },
      ...{ worldName: this.deps.isolatedWorld, analyzer: this.analyzer() },
      receive: (out: PageOutput) => this.deps.recorder.receive(view, startingUrl, out, tabId),
    };
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
