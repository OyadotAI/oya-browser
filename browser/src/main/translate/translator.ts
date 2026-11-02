/** Page translation consent, bounded continuation and ownership handoff; document fencing lives in documents.ts. */
import type { Tab, TabView } from '../tabs/types.ts';
import type { TabTranslation } from '../../shared/ipc.ts';
import { ServerApi } from '../connection/server-api.ts';
import { TranslationDocuments } from './documents.ts';
import { proposalFor, validProposal, confirmationState, type Proposal } from './consent.ts';
import { runConfirmed, failTranslation, showFailure, type TranslationContext } from './session.ts';
import {
  offered,
  idleTranslation,
  targetLanguage,
  continuationAllowed,
  siteOf,
  type TranslatorDeps,
} from './eligibility.ts';
import { TranslateError } from './translate-run.ts';
export type { TranslatorDeps } from './eligibility.ts';

/** The model endpoint is replaceable in hermetic tests. */
type Api = Pick<ServerApi, 'canCall' | 'postToBrowser'>;
/** Offers translation and admits only confirmed, current-document runs. */
export class Translator {
  /** Services consulted again after every admission await. */
  private readonly deps: TranslatorDeps;
  /** Publishes state to every window, including a moved tab. */
  private readonly changed: () => void;
  /** Run execution shares the same document owner and restoration callback. */
  private readonly session: TranslationContext;
  /** Native document and run identities, with per-document restoration barriers. */
  private readonly pages = new TranslationDocuments();
  /** Unaccepted confirmations; navigation and ownership changes revoke them. */
  private readonly proposals = new WeakMap<Tab, Proposal>();

  /** The production endpoint is injected only when a fixture supplies one. */
  constructor(deps: TranslatorDeps, changed: () => void, api?: Api) {
    this.deps = deps;
    this.changed = changed;
    this.session = this.execution(api);
  }

  /** Compose the run engine with this translator's consent revocation and state publication. */
  private execution(api?: Api): TranslationContext {
    return {
      deps: this.deps,
      pages: this.pages,
      api: api ?? new ServerApi(this.deps),
      show: (tab, state) => this.show(tab, state),
      undo: (tab) => this.undo(tab),
    };
  }

  /** Personal desktop web pages only, under the existing persona policy. */
  offered(tab: Tab): boolean {
    return offered(this.deps, tab);
  }
  /** Agent claims always take precedence over translation consent. */
  mayTranslate(tab: Tab): boolean {
    return this.offered(tab) && this.deps.agentTab.humanMay(tab.id);
  }
  /** The actual run owns restoration even if its toolbar state was cleared. */
  translated(tab: Tab): boolean {
    return !!this.pages.state(tab).last;
  }
  /** OS target, never the persona's claimed language. */
  target(): string {
    return targetLanguage(this.deps);
  }
  /** All windows participate in claim and recording barriers. */
  private tabs(): Tab[] {
    return this.deps.windows?.allTabs() ?? this.deps.tabs?.list ?? [];
  }

  /** Detects only this document; stale reads cannot publish language or start continuation. */
  async detect(tab: Tab): Promise<void> {
    if (!this.mayTranslate(tab)) return;
    const page = await this.pages.identify(tab).catch(() => undefined);
    if (!page || !this.mayTranslate(tab)) return;
    tab.pageLang = page.lang || '';
    if (!this.translated(tab) && !this.proposals.has(tab)) this.show(tab, idleTranslation(this.deps, tab));
    if (!tab.view.webContents.isLoading?.()) this.loaded(tab);
  }

  /** Continues only the disclosed site/language/target scope and ends it when that scope is left. */
  loaded(tab: Tab): void {
    const keep = tab.translateKeep;
    if (!keep || this.pages.state(tab).lang === undefined) return;
    if (!continuationAllowed(this.deps, tab, keep)) return void (tab.translateKeep = undefined);
    if (keep.url === tab.url || this.translated(tab)) return;
    keep.pages++;
    keep.url = tab.url;
    void runConfirmed(this.session, tab, proposalFor(tab, this.pages.state(tab), this.target()), true);
  }

  /** A committed new document revokes pending confirmation and old writes, even at the same address. */
  navigated(tab: Tab, url = tab.url): void {
    this.pages.navigated(tab);
    this.proposals.delete(tab);
    if (tab.translateKeep?.origin !== siteOf(url)) tab.translateKeep = undefined;
    tab.pageLang = '';
    this.show(tab, undefined);
  }

  /** Opens consent for chip and context-menu actions; no page text is sent here. */
  async translate(tab: Tab): Promise<void> {
    if (!this.mayTranslate(tab) || this.translated(tab)) return;
    if (!this.session.api.canCall()) return failTranslation(this.session, tab, new TranslateError('signed_out'));
    const page = await this.pages.identify(tab).catch(() => undefined);
    if (!page || !this.mayTranslate(tab)) return;
    tab.pageLang = page.lang || '';
    const proposal = proposalFor(tab, this.pages.state(tab), this.target());
    this.proposals.set(tab, proposal);
    this.show(tab, confirmationState(proposal));
  }

  /** Rechecks the native document and eligibility after the person confirms, before collecting or sending text. */
  async confirm(tab: Tab, id: string): Promise<void> {
    const proposal = this.proposals.get(tab);
    if (!proposal || proposal.id !== id) return;
    const page = await this.pages.identify(tab).catch(() => undefined);
    if (this.proposals.get(tab) !== proposal) return;
    if (!page || !validProposal(this.deps, this.pages, tab, proposal) || page.lang !== proposal.lang)
      return this.cancel(tab);
    this.proposals.delete(tab);
    await runConfirmed(this.session, tab, proposal, false);
  }

  /** Clears a stale confirmation without sending a request or touching another document. */
  private cancel(tab: Tab): void {
    this.proposals.delete(tab);
    this.show(tab, this.mayTranslate(tab) ? idleTranslation(this.deps, tab) : undefined);
  }

  /** Explicit restoration also revokes continued translation and pending confirmation. */
  async showOriginal(tab: Tab): Promise<void> {
    const page = this.pages.state(tab);
    await this.undo(tab).catch((error) => {
      if (this.pages.current(tab, page)) showFailure(this.session, tab, error);
      throw error;
    });
    if (this.pages.current(tab, page))
      this.show(tab, this.mayTranslate(tab) ? idleTranslation(this.deps, tab) : undefined);
  }

  /** Revokes consent synchronously before waiting for native restoration. */
  private undo(tab: Tab): Promise<void> {
    this.proposals.delete(tab);
    tab.translateKeep = undefined;
    if (tab.translation && !this.mayTranslate(tab)) this.show(tab, undefined);
    return this.pages.restore(tab);
  }

  /** Claims and recording starts restore every affected tab, including tabs in another window. */
  async sync(): Promise<void> {
    const lost = this.tabs().filter((tab) => !this.mayTranslate(tab));
    const results = await Promise.allSettled(lost.map((tab) => this.undo(tab)));
    this.publishIdle();
    const failed = results.find((result) => result.status === 'rejected');
    if (failed?.status === 'rejected') throw failed.reason;
  }

  /** Publish eligible idle tabs even when another document still needs recovery. */
  private publishIdle(): void {
    for (const tab of this.tabs())
      if (this.mayTranslate(tab) && !tab.translation && !this.translated(tab))
        tab.translation = idleTranslation(this.deps, tab);
    this.changed();
  }

  /** Every agent read awaits the exact tab's restoration; failed live documents cannot be inspected. */
  async released(view?: TabView): Promise<void> {
    await Promise.all(
      this.tabs()
        .filter((tab) => !view || tab.view === view)
        .map((tab) => this.showOriginal(tab)),
    );
  }

  /** Publish main-owned state to the shell. */
  private show(tab: Tab, translation: TabTranslation | undefined): void {
    tab.translation = translation;
    this.changed();
  }
}
