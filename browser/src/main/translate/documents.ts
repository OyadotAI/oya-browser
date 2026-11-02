/** Main-owned document epochs and restoration barriers for translation's isolated page scripts. */
import { randomUUID } from 'node:crypto';
import type { WebContents } from 'electron';
import type { Tab } from '../tabs/types.ts';
import { runInWorld } from '../native/stock-world.ts';
import { identifyJs } from '../../page/translation-guard.ts';
import { TRANSLATE_WORLD_ID, TRANSLATE_RESTORE_TIMEOUT_MS } from './constants.ts';
import { withinTime } from '../../shared/within-time.ts';
import { TranslateError, restorePage } from './translate-run.ts';

/** One isolated document's identity and declared language, read together. */
interface Identified {
  /** Stable only for this document's isolated world. */
  document: string;
  /** Empty when the document declares no supported language. */
  lang: string;
}

/** Read through the existing native isolated-world bridge. */
const identify = (contents: WebContents) =>
  runInWorld<Identified>(contents, TRANSLATE_WORLD_ID, identifyJs(randomUUID()));

/** Captures the exact native target for all requests, writes and later restoration. */
const runFor = (state: TranslationDocument, serial: number): TranslationRun => ({
  document: state.document!,
  serial,
  contents: state.contents,
  stop: new AbortController(),
});

/** A settled cleanup releases only its in-flight promise, retaining failed text for the next retry. */
function restoring(state: TranslationDocument, work: Promise<void>): Promise<void> {
  state.restoring = work.finally(() => {
    state.restoring = undefined;
  });
  return state.restoring;
}

/** A run keeps its original contents and document, including after the tab navigates. */
export interface TranslationRun {
  /** The isolated world's document identity. */
  document: string;
  /** Monotonic run identity; page restoration tombstones it. */
  serial: number;
  /** The exact native contents, never looked up by active tab. */
  contents: WebContents;
  /** Stops requests and rejects late results in main. */
  stop: AbortController;
}

/** One document's mutable state; replacing the object revokes every captured confirmation. */
export interface TranslationDocument {
  /** The native contents captured before detection. */
  contents: WebContents;
  /** The page-side document token, once detection settles. */
  document?: string;
  /** The language of this exact document; undefined means detection has not finished. */
  lang?: string;
  /** The run whose written text may still need restoration. */
  last?: TranslationRun;
  /** Whether that run is still fetching translations. */
  running?: boolean;
  /** Shared barrier while restoring; failure retains last so the next read must retry. */
  restoring?: Promise<void>;
}

/** Keeps page identity and cleanup independent from toolbar state. */
export class TranslationDocuments {
  /** Weak tab ownership survives window moves without retaining closed tabs. */
  private readonly states = new WeakMap<Tab, TranslationDocument>();
  /** Never reuse a run number, even after a document changes. */
  private serial = 0;

  /** The current native document epoch for a tab. */
  state(tab: Tab): TranslationDocument {
    if (this.states.get(tab)?.contents !== tab.view.webContents) this.navigated(tab);
    return this.states.get(tab)!;
  }

  /** A document replacement invalidates pending reads, requests, confirmations and restore failures. */
  navigated(tab: Tab): void {
    this.states.get(tab)?.last?.stop.abort();
    this.states.set(tab, { contents: tab.view.webContents });
  }

  /** Whether an awaited result still belongs to this live document. */
  current(tab: Tab, state: TranslationDocument): boolean {
    return this.state(tab) === state && !state.contents.isDestroyed();
  }

  /** Reads language and identity together; a delayed answer from a discarded document is ignored. */
  async identify(tab: Tab): Promise<TranslationDocument | undefined> {
    const state = this.state(tab);
    const reply = await identify(state.contents);
    if (!this.current(tab, state)) return;
    state.document = reply.document;
    state.lang = reply.lang;
    return state;
  }

  /** Begins only after the document has been identified and consent checked. */
  begin(tab: Tab): TranslationRun {
    const state = this.state(tab);
    if (!state.document || state.restoring) throw new TranslateError('restore_failed');
    const run = runFor(state, ++this.serial);
    state.last = run;
    state.running = true;
    return run;
  }

  /** A stopped or replaced run cannot publish results or send another batch. */
  live(tab: Tab, run: TranslationRun): boolean {
    const state = this.state(tab);
    return state.last === run && !run.stop.signal.aborted && this.current(tab, state);
  }

  /** Stops fetching without forgetting the text that Show original must restore. */
  finished(tab: Tab, run: TranslationRun): void {
    if (this.state(tab).last === run) this.state(tab).running = false;
  }

  /** Restores the exact document, sharing one barrier and retaining failed work for a retry. */
  restore(tab: Tab): Promise<void> {
    const state = this.state(tab);
    state.last?.stop.abort();
    state.running = false;
    if (state.restoring) return state.restoring;
    if (!state.last) return Promise.resolve();
    return restoring(state, this.restoreRun(tab, state, state.last));
  }

  /** A live document's restore failure blocks inspection; a discarded document has no text left to expose. */
  private async restoreRun(tab: Tab, state: TranslationDocument, run: TranslationRun): Promise<void> {
    try {
      await withinTime(restorePage(run.contents, run.document, run.serial), TRANSLATE_RESTORE_TIMEOUT_MS);
      if (state.last === run) state.last = undefined;
    } catch {
      if (this.current(tab, state))
        throw new TranslateError('restore_failed', 'Could not restore the original page. Reload it before continuing.');
    }
  }
}
