/** Isolated document/run fences prevent queued translation scripts from writing after navigation or restoration. */
import { applyJs, collectJs, detectJs, restoreJs } from './translate.ts';

/** Identifies this document once, even when two detection requests arrive out of order. */
export const identifyJs = (seed: string): string => `(() => {
  globalThis.__oyaTranslationDocument ??= ${JSON.stringify(seed)};
  return { document: globalThis.__oyaTranslationDocument, lang: ${detectJs} };
})()`;

/** Collection can start a run only once, and never after that run was restored. */
export const collectFor = (document: string, run: number, max: number, batch: number): string => `(() => {
  if (globalThis.__oyaTranslationDocument !== ${JSON.stringify(document)}) throw Error('Translation document changed');
  if ((globalThis.__oyaTranslationSequence || 0) >= ${run}) throw Error('Translation run ended');
  globalThis.__oyaTranslationSequence = ${run};
  const result = ${collectJs(max, batch)};
  globalThis.__oyaTranslation.run = ${run};
  return result;
})()`;

/** A late model answer cannot update another run or a document that replaced this one. */
export const applyFor = (document: string, run: number, start: number, texts: string[]): string => `(() => {
  if (globalThis.__oyaTranslationDocument !== ${JSON.stringify(document)} || globalThis.__oyaTranslation?.run !== ${run}) return 0;
  return ${applyJs(start, texts)};
})()`;

/** Tombstones the run before restoring, so even an already queued collection cannot recreate it. */
export const restoreFor = (document: string, run: number): string => `(() => {
  if (globalThis.__oyaTranslationDocument !== ${JSON.stringify(document)}) return 0;
  globalThis.__oyaTranslationSequence = Math.max(globalThis.__oyaTranslationSequence || 0, ${run});
  if (globalThis.__oyaTranslation?.run !== ${run}) return 0;
  return ${restoreJs};
})()`;
