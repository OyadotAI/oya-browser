/** Translation's desktop, ownership and language policy, shared by offers and confirmed runs. */
import type { AppServices } from '../app/services.ts';
import type { Tab, TranslateKeep } from '../tabs/types.ts';
import type { TabTranslation } from '../../shared/ipc.ts';
import { primaryLanguage } from '../../shared/language.ts';
import { TRANSLATE_KEEP_PAGES } from './constants.ts';
import { desktopRuntime } from '../app/runtime.ts';

/** Services read by translation; windows supplies all tabs after a tear-off. */
export type TranslatorDeps = Pick<
  AppServices,
  'config' | 'socket' | 'governance' | 'recorder' | 'agentTab' | 'tabs' | 'electron'
> &
  Partial<Pick<AppServices, 'persona' | 'windows'>>;

/** The person's preferred language comes from the OS, never the persona. */
export function targetLanguage(deps: TranslatorDeps): string {
  const { app } = deps.electron;
  return app.getPreferredSystemLanguages?.()[0] || app.getLocale();
}

/** A persona must plausibly read the target language because the page sees rewritten text. */
function personaAllows(deps: TranslatorDeps): boolean {
  const languages = deps.persona?.active?.navigator?.languages;
  return (
    !Array.isArray(languages) ||
    !languages.length ||
    languages.some((lang) => primaryLanguage(String(lang)) === primaryLanguage(targetLanguage(deps)))
  );
}

/** Only personal desktop web tabs can offer translation. */
export function offered(deps: TranslatorDeps, tab: Tab): boolean {
  if (deps.governance.configuration || !desktopRuntime(deps.config) || deps.recorder.recording) return false;
  return personaAllows(deps) && !tab.home && !tab.collection && /^https?:/i.test(tab.url);
}

/** The offer stays hidden for languages the person already reads. */
export function idleTranslation(deps: TranslatorDeps, tab: Tab): TabTranslation | undefined {
  const lang = tab.pageLang || '';
  const { app } = deps.electron;
  const reads = [...(app.getPreferredSystemLanguages?.() ?? []), app.getLocale()].map(primaryLanguage);
  return lang && !reads.includes(primaryLanguage(lang))
    ? { state: 'offer', lang, target: targetLanguage(deps) }
    : undefined;
}

/** Consent is scoped to an origin, including its scheme and port. */
export function siteOf(url: string): string {
  try {
    return new URL(url).origin;
  } catch {
    return '';
  }
}

/** Unknown-language pages authorize only the current document. */
export function keepFrom(tab: Tab, target: string): TranslateKeep | undefined {
  const lang = primaryLanguage(tab.pageLang);
  return lang ? { lang, target, origin: siteOf(tab.url), url: tab.url, pages: 0 } : undefined;
}

/** Leaving any disclosed language or site scope revokes continued translation. */
export function sameScope(keep: TranslateKeep, tab: Tab, target: string): boolean {
  return keep.origin === siteOf(tab.url) && keep.lang === primaryLanguage(tab.pageLang) && keep.target === target;
}

/** Continued consent remains bounded and subordinate to current control. */
export function continuationAllowed(deps: TranslatorDeps, tab: Tab, keep: TranslateKeep): boolean {
  return (
    keep.pages < TRANSLATE_KEEP_PAGES &&
    sameScope(keep, tab, targetLanguage(deps)) &&
    offered(deps, tab) &&
    deps.agentTab.humanMay(tab.id)
  );
}
