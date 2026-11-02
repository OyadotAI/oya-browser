/** Main-owned translation consent binds the disclosed scope to one exact document. */
import type { TabTranslation } from '../../shared/ipc.ts';
import { TRANSLATE_KEEP_PAGES } from './constants.ts';
import { randomUUID } from 'node:crypto';
import type { Tab } from '../tabs/types.ts';
import type { TranslationDocument, TranslationDocuments } from './documents.ts';
import { offered, siteOf, targetLanguage, type TranslatorDeps } from './eligibility.ts';
/** Consent captures the exact main-owned page and disclosed language/site scope. */
export interface Proposal {
  /** Opaque confirmation id; renderer approval cannot invent a new scope. */
  id: string;
  /** Exact document epoch, invalidated even by a same-URL reload. */
  page: TranslationDocument;
  /** Source language shown in the confirmation. */
  lang: string;
  /** OS language shown in the confirmation. */
  target: string;
  /** Exact origin shown in the confirmation. */
  origin: string;
}

/** Capture the scope from main rather than accepting it from the renderer. */
export function proposalFor(tab: Tab, page: TranslationDocument, target: string): Proposal {
  return { id: randomUUID(), page, lang: tab.pageLang || '', target, origin: siteOf(tab.url) };
}

/** A grant stops being valid when control, document, source, target or origin changes. */
export function validProposal(
  deps: TranslatorDeps,
  pages: TranslationDocuments,
  tab: Tab,
  proposal: Proposal,
): boolean {
  const own = offered(deps, tab) && deps.agentTab.humanMay(tab.id);
  const scope = proposal.target === targetLanguage(deps) && proposal.origin === siteOf(tab.url);
  return own && scope && pages.current(tab, proposal.page) && proposal.lang === (tab.pageLang || '');
}

/** The shell receives only the disclosure fields and opaque confirmation id. */
export function confirmationState(proposal: Proposal): TabTranslation {
  const { id, origin, lang, target } = proposal;
  return { state: 'confirming', lang, target, confirmation: { id, origin, pages: lang ? TRANSLATE_KEEP_PAGES : 0 } };
}
