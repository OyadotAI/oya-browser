/**
 * Page translation on a person's own tabs: offers it when a page's declared
 * language is not one of the person's, runs it on a click, keeps translating
 * the tab's later pages in that language, and undoes it the moment the tab stops
 * being the person's (an agent claims it, a recording starts). The tab's
 * `translation` is what the toolbar's chip draws; main owns it.
 */
import type { AppServices } from '../app/services.ts';
import type { Tab, TabView, TranslateKeep } from '../tabs/types.ts';
import type { TabTranslation } from '../../shared/ipc.ts';
import { primaryLanguage } from '../../shared/language.ts';
import { desktopRuntime } from '../app/runtime.ts';
import { runInWorld } from '../native/stock-world.ts';
import { ServerApi } from '../connection/server-api.ts';
import { detectJs } from '../../page/translate.ts';
import { TRANSLATE_KEEP_PAGES, TRANSLATE_WORLD_ID } from './constants.ts';
import { TranslateError, failureOf, restorePage, translatePage } from './translate-run.ts';

/** What translation reads: the server, who holds each tab, the recording, governance and the OS languages. */
export type TranslatorDeps = Pick<
  AppServices,
  'config' | 'socket' | 'governance' | 'recorder' | 'agentTab' | 'tabs' | 'electron'
> &
  Partial<Pick<AppServices, 'persona'>>;

/** The server calls translation makes: whether it can, and the batch POST. */
type Api = Pick<ServerApi, 'canCall' | 'postToBrowser'>;

/** The site part of an address, or '' when it has none. */
function siteOf(url: string): string {
  try {
    return new URL(url).origin;
  } catch {
    return '';
  }
}

/** What one click on `tab` keeps translating: its language on its site; nothing for a page that declared no language. */
function keepFrom(tab: Tab): TranslateKeep | undefined {
  const lang = primaryLanguage(tab.pageLang);
  return lang ? { lang, origin: siteOf(tab.url), url: tab.url, pages: 0 } : undefined;
}

/** Whether `keep` covers the tab's new page: same language, same site, not the page just translated, within the page cap. */
function keeps(keep: TranslateKeep, tab: Tab): boolean {
  const same = keep.lang === primaryLanguage(tab.pageLang) && keep.origin === siteOf(tab.url);
  return same && keep.url !== tab.url && keep.pages < TRANSLATE_KEEP_PAGES;
}

/** Offers, runs and undoes page translation on the person's tabs. */
export class Translator {
  /** The services translation reads. */
  private readonly deps: TranslatorDeps;
  /** Resends every window's strip, so the chip follows. */
  private readonly changed: () => void;
  /** The server, called as this browser. */
  private readonly api: Api;
  /** Each running translation's stop. */
  private readonly runs = new WeakMap<Tab, AbortController>();
  /** The last yield's restores: a page command waits on them, so an agent never reads translated text. */
  private yielding: Promise<unknown> = Promise.resolve();

  /** `changed` resends the tab strips; `api` defaults to the server behind the control socket. */
  constructor(deps: TranslatorDeps, changed: () => void, api?: Api) {
    this.deps = deps;
    this.changed = changed;
    this.api = api ?? new ServerApi(deps);
  }

  /** Whether translation exists for this tab at all: the person's own desktop, not governed, no recording, a web page. */
  offered(tab: Tab): boolean {
    const { governance, config, recorder } = this.deps;
    if (governance.configuration || !desktopRuntime(config) || recorder.recording) return false;
    return this.personaAllows() && !tab.home && !tab.collection && /^https?:/i.test(tab.url);
  }

  /**
   * Whether the active persona could plausibly read the person's language. A page
   * sees text rewritten into it: under a persona that claims other languages, that
   * would tell the site the claim is false and point at who is behind it.
   */
  private personaAllows(): boolean {
    const languages = this.deps.persona?.active?.navigator?.languages;
    if (!Array.isArray(languages) || !languages.length) return true;
    return languages.map((l) => primaryLanguage(String(l))).includes(primaryLanguage(this.target()));
  }

  /** Whether the person may translate this tab now: it is offered, and no agent holds it. */
  mayTranslate(tab: Tab): boolean {
    return this.offered(tab) && this.deps.agentTab.humanMay(tab.id);
  }

  /** Whether the tab shows, or is getting, a translation: what "Show Original" undoes. */
  translated(tab: Tab): boolean {
    return this.runs.has(tab) || tab.translation?.state === 'translated' || tab.translation?.state === 'partial';
  }

  /** The person's language: the OS's first, never the persona's (a persona's navigator.languages are someone else's). */
  target(): string {
    const { app } = this.deps.electron;
    return app.getPreferredSystemLanguages?.()[0] || app.getLocale();
  }

  /** Whether `lang` is one the person reads: any of the OS's preferred languages, compared by primary subtag. */
  private reads(lang: string): boolean {
    const { app } = this.deps.electron;
    const theirs = [...(app.getPreferredSystemLanguages?.() ?? []), app.getLocale()].map(primaryLanguage);
    return theirs.includes(primaryLanguage(lang));
  }

  /** The chip a page shows before anything runs: an offer when the person does not read its language, else nothing. */
  private idle(tab: Tab): TabTranslation | undefined {
    const lang = tab.pageLang || '';
    return lang && !this.reads(lang) ? { state: 'offer', lang, target: this.target() } : undefined;
  }

  /** On dom-ready: reads the page's language and offers translation when the person does not read it. */
  async detect(tab: Tab): Promise<void> {
    if (!this.mayTranslate(tab)) return;
    const lang = await runInWorld<string>(tab.view.webContents, TRANSLATE_WORLD_ID, detectJs).catch(() => '');
    tab.pageLang = typeof lang === 'string' ? lang : '';
    // An agent may have claimed the tab while the page was read.
    if (this.mayTranslate(tab) && !this.runs.has(tab)) this.show(tab, this.idle(tab));
    // A light page can finish loading before its language was read: keep-translating checks again now.
    if (!tab.view.webContents.isLoading?.()) this.loaded(tab);
  }

  /** On a finished load: a tab told to keep translating translates the next page of that site, in that language, on its own. */
  loaded(tab: Tab): void {
    const keep = tab.translateKeep;
    if (!keep) return;
    // Left the translated page for another: coming back to it later is not a reload.
    if (tab.url !== keep.url && !keeps(keep, tab)) keep.url = '';
    if (!keeps(keep, tab) || !this.mayTranslate(tab)) return;
    keep.pages++;
    keep.url = tab.url;
    void this.translate(tab, true);
  }

  /** On a new document: the last page's run is dropped (its document is gone) and its chip with it. */
  navigated(tab: Tab): void {
    this.runs.get(tab)?.abort();
    this.runs.delete(tab);
    tab.pageLang = '';
    if (tab.translation) this.show(tab, undefined);
  }

  /** Translates the tab's page (the chip's click, the menu, or keep-translating when `auto`); a second call while one runs does nothing. */
  async translate(tab: Tab, auto = false): Promise<void> {
    if (!this.mayTranslate(tab) || this.runs.has(tab)) return;
    if (!this.api.canCall()) return this.fail(tab, new TranslateError('signed_out'));
    const stop = new AbortController();
    this.runs.set(tab, stop);
    this.show(tab, { state: 'translating', lang: tab.pageLang || '', target: this.target() });
    await this.run(tab, stop, auto);
  }

  /** Runs the translation and shows how it ended, unless it was stopped. */
  private async run(tab: Tab, stop: AbortController, auto: boolean): Promise<void> {
    try {
      await this.finish(tab, stop, auto);
    } catch (err) {
      if (!stop.signal.aborted) await this.fail(tab, err instanceof TranslateError ? err : failureOf(err));
    } finally {
      if (this.runs.get(tab) === stop) this.runs.delete(tab);
    }
  }

  /** Translates the page and, unless stopped, shows it done; a click starts keep-translating this site and language. */
  private async finish(tab: Tab, stop: AbortController, auto: boolean): Promise<void> {
    const source = tab.pageLang || '';
    const target = this.target();
    const req = { api: this.api, contents: tab.view.webContents, target, signal: stop.signal };
    const { partial } = await translatePage(req);
    if (stop.signal.aborted) return;
    if (!auto) tab.translateKeep = keepFrom(tab);
    this.show(tab, { state: partial ? 'partial' : 'translated', lang: source, target });
  }

  /** Ends a run that failed: what was written is undone, keep-translating ends, and the chip says why. */
  private async fail(tab: Tab, err: TranslateError): Promise<void> {
    tab.translateKeep = undefined;
    await restorePage(tab.view.webContents);
    const error = { code: err.code, message: err.message };
    this.show(tab, { state: 'failed', lang: tab.pageLang || '', target: this.target(), error });
  }

  /** "Show original": stops any run, puts the page's own text back, ends keep-translating, and offers again. */
  async showOriginal(tab: Tab): Promise<void> {
    await this.undo(tab);
    this.show(tab, this.mayTranslate(tab) ? this.idle(tab) : undefined);
  }

  /** Stops the tab's run, restores its page and ends keep-translating. */
  private async undo(tab: Tab): Promise<void> {
    this.runs.get(tab)?.abort();
    this.runs.delete(tab);
    tab.translateKeep = undefined;
    await restorePage(tab.view.webContents);
  }

  /**
   * On every claim, control change and recording start or stop: undoes translation on
   * each tab the person no longer holds, and offers it again on each one handed back.
   */
  sync(): Promise<unknown> {
    const tabs = this.deps.tabs?.list ?? [];
    const lost = tabs.filter((t) => (t.translation || t.translateKeep) && !this.mayTranslate(t));
    const back = tabs.filter((t) => !t.translation && t.pageLang && this.mayTranslate(t));
    for (const t of lost) t.translation = undefined;
    for (const t of back) t.translation = this.idle(t);
    this.yielding = Promise.all([this.yielding, ...lost.map((t) => this.undo(t))]);
    if (lost.length || back.some((t) => t.translation)) this.changed();
    return this.yielding;
  }

  /**
   * Settles once every yielded tab is restored: a page command waits on it, so an
   * agent's first read never sees text the page did not serve. CDP passthrough
   * reads and screenshots by tab id do not wait; they rely on the yield the claim
   * itself started having finished by then.
   */
  released(_view?: TabView): Promise<unknown> {
    return this.yielding;
  }

  /** Sets what the tab's chip draws, and resends the strips. */
  private show(tab: Tab, translation: TabTranslation | undefined): void {
    tab.translation = translation;
    this.changed();
  }
}
