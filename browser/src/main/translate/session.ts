/** Execution and failure reporting for an already-confirmed translation of one document. */
import type { Tab } from '../tabs/types.ts';
import type { TabTranslation } from '../../shared/ipc.ts';
import type { ServerApi } from '../connection/server-api.ts';
import type { TranslationDocuments, TranslationRun } from './documents.ts';
import { validProposal, type Proposal } from './consent.ts';
import { offered, keepFrom, targetLanguage, type TranslatorDeps } from './eligibility.ts';
import { TranslateError, failureOf, translatePage } from './translate-run.ts';

/** Main composes execution with its one document owner and consent-revoking restore. */
export interface TranslationContext {
  /** Current desktop and ownership policy. */
  deps: TranslatorDeps;
  /** Document identity and cleanup state. */
  pages: TranslationDocuments;
  /** Project model endpoint. */
  api: Pick<ServerApi, 'canCall' | 'postToBrowser'>;
  /** Publish state to the owning window. */
  show(tab: Tab, state: TabTranslation | undefined): void;
  /** Revoke pending and continued consent before restoring. */
  undo(tab: Tab): Promise<void>;
}

/** Start once and only while the exact confirmed scope remains eligible. */
export async function runConfirmed(ctx: TranslationContext, tab: Tab, scope: Proposal, auto: boolean): Promise<void> {
  if (!validProposal(ctx.deps, ctx.pages, tab, scope) || ctx.pages.state(tab).last) return;
  if (!ctx.api.canCall()) return failTranslation(ctx, tab, new TranslateError('signed_out'));
  const run = ctx.pages.begin(tab);
  ctx.show(tab, stateFor(scope, 'translating'));
  await execute(ctx, tab, scope, run, auto);
}

/** Finish only this run; a navigation or explicit stop already owns its cleanup. */
async function execute(ctx: TranslationContext, tab: Tab, scope: Proposal, run: TranslationRun, auto: boolean) {
  try {
    await finish(ctx, tab, scope, run, auto);
  } catch (error) {
    if (ctx.pages.live(tab, run)) await failTranslation(ctx, tab, failureOf(error));
  } finally {
    ctx.pages.finished(tab, run);
  }
}

/** Every batch and final publication must retain main's grant. */
function checkRun(ctx: TranslationContext, tab: Tab, scope: Proposal, run: TranslationRun): void {
  if (!validProposal(ctx.deps, ctx.pages, tab, scope) || !ctx.pages.live(tab, run))
    throw new TranslateError('cancelled');
}

/** Page-side fences independently reject writes that were queued before a stop. */
async function finish(ctx: TranslationContext, tab: Tab, scope: Proposal, run: TranslationRun, auto: boolean) {
  const check = () => checkRun(ctx, tab, scope, run);
  const request = { api: ctx.api, ...run, signal: run.stop.signal, target: scope.target, check };
  const { partial } = await translatePage(request);
  check();
  if (!auto) tab.translateKeep = keepFrom(tab, scope.target);
  ctx.show(tab, stateFor(scope, partial ? 'partial' : 'translated'));
}

/** Toolbar text always describes the approved source and target. */
const stateFor = (scope: Proposal, state: TabTranslation['state']): TabTranslation => ({
  state,
  lang: scope.lang,
  target: scope.target,
});

/** Failure cleanup must finish before a retry is offered; a restore failure instead requires reload. */
export async function failTranslation(ctx: TranslationContext, tab: Tab, error: TranslateError): Promise<void> {
  const page = ctx.pages.state(tab);
  await ctx.undo(tab).catch((restore) => {
    error = restore;
  });
  if (ctx.pages.current(tab, page)) showFailure(ctx, tab, error);
}

/** A failed live restore remains visible to the person and blocked to agent readers. */
export function showFailure(ctx: TranslationContext, tab: Tab, error: TranslateError): void {
  const own = offered(ctx.deps, tab) && ctx.deps.agentTab.humanMay(tab.id);
  const state: TabTranslation = {
    state: 'failed',
    lang: tab.pageLang || '',
    target: targetLanguage(ctx.deps),
    error: { code: error.code, message: error.message },
  };
  ctx.show(tab, own ? state : undefined);
}
