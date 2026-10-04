/**
 * The Source pane: the page as the agent reads it and as HTML, refreshed on
 * demand, from View Page Source, or from an element the person inspected. The
 * read is shown in the format picked in the pane (markdown, TOON or JSONL),
 * which starts at the default chosen in settings; switching renders the same
 * analysis again, without reading the page again. It says which page it read,
 * marks the read stale when the page changes, shows errors where the read
 * would be, and never lets a slow switch overwrite a newer one.
 */
import { ViewModel } from '../../../core/view-model.ts';
import { RendererConstants as C } from '../../../core/constants.ts';
import type { Payload } from '../../../../shared/ipc.ts';
import { COPY_TEXT, DEFAULT_FORMAT, FORMAT_LABELS, SOURCE_TEXT } from '../model/constants.ts';
import type { InspectServices } from '../model/services.ts';

/** An analysis kept to render again, with the format it was last written in. */
export type Analysis = Payload & {
  /** The format its page is written in. */
  format?: string;
};

/** A read of the page (or of an inspected element). */
export interface SourceRead {
  /** The page's HTML ('' for an element). */
  html: string;
  /** The read, in the analysis's format. */
  markdown: string;
  /** The analysis behind it, or null. */
  analysis: Analysis | null;
  /** The page read, or ''. */
  url: string;
  /** Why it could not be read. */
  error?: string;
  /** Why there is nothing to read (the agent holds the page). */
  notice?: string;
}

/** Either half of the pane. */
export type SourceHalf = 'page' | 'html';

/** What the Source pane shows. */
export interface SourceState {
  /** The last read, or null before one (and after Clear). */
  read: SourceRead | null;
  /** The format picked. */
  format: string;
  /** A read is under way: Refresh is off and says so. */
  refreshing: boolean;
  /** The page changed since it was read. */
  stale: boolean;
  /** What each half's Copy button says. */
  copyLabels: Readonly<Record<SourceHalf, string>>;
}

/** One half as it shows: its text (or placeholder), whether it is empty, and whether it is an error. */
export interface HalfLook {
  /** The text shown. */
  text: string;
  /** There is nothing to show (the placeholder shows; Copy is off). */
  empty: boolean;
  /** The text is why the read failed. */
  error: boolean;
}

/** What the pane shows. */
export interface SourceLook {
  /** The line naming the page read. */
  url: string;
  /** The read's half. */
  page: HalfLook;
  /** The HTML half. */
  html: HalfLook;
  /** The read's heading: the format's name. */
  label: string;
}

/** A half: `text`, or `placeholder` when there is none. */
const half = (text: string, placeholder: string, error = false): HalfLook => ({
  text: text || placeholder,
  empty: !text,
  error,
});

/** The line naming the page read, or saying it changed. */
function urlLine({ read, stale }: SourceState): string {
  if (stale) return SOURCE_TEXT.changed;
  return read?.url ? SOURCE_TEXT.readFrom(read.url) : SOURCE_TEXT.prompt;
}

/** Everything the pane shows for `state`. */
export function sourceLook(state: SourceState): SourceLook {
  const { read } = state;
  const base = { url: urlLine(state), label: FORMAT_LABELS[state.format] ?? '' };
  if (!read) return { ...base, page: half('', SOURCE_TEXT.readPrompt), html: half('', SOURCE_TEXT.noHtmlYet) };
  const text = read.error ? SOURCE_TEXT.couldNotRead(read.error) : read.markdown;
  const page = half(text, read.notice || SOURCE_TEXT.nothing, !!read.error);
  return { ...base, page, html: half(read.html, SOURCE_TEXT.noHtml) };
}

/** A read from whatever the main process sent. */
function readOf(source: Partial<SourceRead> | null | undefined): SourceRead {
  const { html = '', markdown = '', analysis = null, url = '', error, notice } = source ?? {};
  return { html, markdown, analysis, url, error, notice };
}

/** An Inspect answer: the element's read alone, or the error; the page's HTML is not the element's. */
function inspectedRead(result: Payload | null | undefined): SourceRead {
  const data = result?.data as Analysis | undefined;
  const page = result?.ok && typeof data?.page === 'string' ? data.page : '';
  if (!page) return readOf({ error: String(result?.error || SOURCE_TEXT.unknownError) });
  return readOf({ markdown: page, analysis: data ?? null });
}

/** The Source pane. */
export class SourceViewModel extends ViewModel<SourceState> {
  /** The main process, the panel and the clipboard. */
  private readonly services: Pick<InspectServices, 'bridge' | 'panel' | 'clipboard'>;
  /** Bumped by every show, so only the latest one's answer is shown. */
  private generation = 0;

  /** Nothing read, at the default format; follows View Page Source, Inspect and page changes. */
  constructor(services: Pick<InspectServices, 'bridge' | 'panel' | 'clipboard'>) {
    const copyLabels = { page: COPY_TEXT.idle, html: COPY_TEXT.idle };
    super({ read: null, format: DEFAULT_FORMAT, refreshing: false, stale: false, copyLabels });
    this.services = services;
    this.listen();
    services.bridge.getUiPreferences().then(
      (prefs) => this.setDefault(String(prefs?.pageFormat)),
      () => {},
    );
  }

  /** Hears View Page Source, Inspect and page changes, and registers the pane's Clear. */
  private listen(): void {
    const { bridge, panel } = this.services;
    this.own(bridge.onViewSource((data) => void this.viewSource(data)));
    this.own(bridge.onInspectResult((result) => void this.inspected(result)));
    this.own(bridge.onUrlChanged(() => this.pageChanged()));
    this.own(panel.onClear('source', () => this.clear()));
  }

  /** Reads the current page, with Refresh off until it is back. */
  async refresh(): Promise<void> {
    this.set({ refreshing: true });
    const source = await this.services.bridge.getPageSource().catch((e: Error) => ({ error: e.message }));
    await this.loaded(source);
    this.set({ refreshing: false });
  }

  /** Keeps and shows a read. */
  loaded(source: Partial<SourceRead> | null | undefined): Promise<void> {
    this.set({ read: readOf(source), stale: false });
    return this.show();
  }

  /** View Page Source: shows the pane with the page's source. */
  viewSource(data: Partial<SourceRead> | null | undefined): Promise<void> {
    this.services.panel.show('source');
    return this.loaded(data);
  }

  /** An inspected element: shows the pane with its read alone, or the error. */
  inspected(result: Payload | null | undefined): Promise<void> {
    this.services.panel.show('source');
    this.set({ read: inspectedRead(result), stale: false });
    return this.show();
  }

  /** The page changed since it was read: say so (nothing to say before a read of a page). */
  pageChanged(): void {
    if (this.state.read?.url) this.set({ stale: true });
  }

  /** Picks `format` in the pane and shows the read in it. */
  pick(format: string): Promise<void> {
    this.set({ format });
    return this.show();
  }

  /** The default format chosen in settings: the pane picks it (a format this app does not know is ignored). */
  setDefault(format: string): void {
    if (Object.hasOwn(FORMAT_LABELS, format)) this.set({ format });
  }

  /** The person chose a default format in settings: saves it, and shows the read in it. */
  chooseDefault(format: string): Promise<void> {
    void this.services.bridge.saveUiPreferences({ pageFormat: format });
    this.setDefault(format);
    return this.show();
  }

  /** Shows the read in the picked format, rendering its analysis again when it was written in another. */
  async show(): Promise<void> {
    const generation = ++this.generation;
    const { format, read } = this.state;
    const analysis = read?.analysis;
    if (!read || !analysis || analysis.format === format) return;
    const page = await this.rerender(analysis, format);
    if (page === undefined || generation !== this.generation || this.state.read !== read) return;
    this.set({ read: { ...read, markdown: page, analysis: { ...analysis, format } } });
  }

  /** The analysis written in `format` (undefined when nothing came back); a failed render is said in place of the read. */
  private async rerender(analysis: Analysis, format: string): Promise<string | undefined> {
    try {
      const page: unknown = await this.services.bridge.renderPage(analysis, format);
      return typeof page === 'string' ? page : undefined;
    } catch (error) {
      return SOURCE_TEXT.couldNotFormat((error as Error).message);
    }
  }

  /** Copies one half and says so on its button for a moment. */
  async copy(which: SourceHalf): Promise<void> {
    const look = sourceLook(this.state)[which];
    if (look.empty) return;
    const copied = await this.services.clipboard.writeText(look.text).then(
      () => true,
      () => false,
    );
    this.label(which, copied ? COPY_TEXT.copied : COPY_TEXT.failed);
    setTimeout(() => this.label(which, COPY_TEXT.idle), C.COPIED_MS);
  }

  /** Sets what one half's Copy button says. */
  private label(which: SourceHalf, text: string): void {
    this.set({ copyLabels: { ...this.state.copyLabels, [which]: text } });
  }

  /** Forgets the read and shows the prompt again. */
  clear(): void {
    this.set({ read: null, stale: false });
  }
}
