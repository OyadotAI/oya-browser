/**
 * The checkpoint a background run takes between page-changing steps: clear a
 * CAPTCHA, sign in, or answer an MFA prompt, or park the run on a person.
 */
import { registry } from '../browsers/registry.ts';
import { container } from '../../app/container.ts';
import * as captcha from '../challenges/captcha.ts';
import * as mfa from '../challenges/mfa.ts';
import * as siteLogin from '../challenges/login.ts';
import * as credentials from '../personas/credentials.ts';
import * as keyConfig from '../config/service.ts';
import { NATIVE_CAPTCHA, announce, evaluateIn } from '../../app/http.ts';

// Not yet layered: reads the persona service from the composition root.
const { personas } = container;

/** What every checkpoint of one run shares. */
interface RunContext {
  /** The key the run belongs to. */
  apiKey: string;
  /** The browser the run drives. */
  browserId: string;
  /** Parks the run on a person until they respond; absent for the agent's own tries. */
  requestHuman?: (attention: object) => Promise<any>;
  /** Where a person finishes a challenge by hand. */
  liveViewUrl: string;
  /** Runs an expression in the browser's page. */
  evaluate: (expr: string) => Promise<any>;
  /** The key's own LLM settings. */
  llm: any;
}

/** Who the page is signed in as, and where it is. */
interface PageContext {
  /** The browser as the registry holds it. */
  browser: any;
  /** The persona whose credentials and mailbox apply. */
  personaId: string;
  /** The site the page is on. */
  domain: string;
}

/** What every step of one run shares, before a person is involved. */
function runContext(apiKey, browserId, requestHuman?): RunContext {
  const liveViewUrl = `/dashboard/?browser=${encodeURIComponent(browserId)}`;
  const evaluate = (expr) => evaluateIn(browserId, expr);
  // Resolved once for the run, not per step: it decrypts this key's settings,
  // and the key cannot change underneath a run that is already going.
  const llm = keyConfig.resolve(apiKey);
  return { apiKey, browserId, requestHuman, liveViewUrl, evaluate, llm };
}

/** Who the page is signed in as, and where it is. */
function pageOf(run: RunContext): PageContext {
  const browser = registry.get(run.browserId);
  const personaId = browser?.persona?.id || personas.defaultFor(run.apiKey).id;
  return { browser, personaId, domain: credentials.domainOf(browser?.currentUrl || '') };
}

/**
 * Between page-changing steps of a run: clear a CAPTCHA, sign in, or answer an
 * MFA prompt, or park the run on a person.
 * ponytail: three detection evals per page-changing step.
 */
export function checkpointFor(apiKey, browserId, requestHuman) {
  const run = runContext(apiKey, browserId, requestHuman);
  return () => checkpoint(run);
}

/**
 * The same three challenges, each tried without a person, for the agent to call
 * as tools: it decides what to do with a challenge that could not be cleared.
 */
export function challengesFor(apiKey, browserId) {
  const run = runContext(apiKey, browserId);
  return {
    captcha: () => tryCaptcha(run, pageOf(run)),
    signIn: () => trySignIn(run, pageOf(run)),
    mfa: (login?) => tryMfa(run, pageOf(run), login),
    liveViewUrl: run.liveViewUrl,
  };
}

/**
 * A checkpoint with no person to hand to (a chat): each challenge is tried, and one
 * that could not be cleared is left for the agent, which sees it and can ask.
 */
export function quietCheckpointFor(apiKey, browserId) {
  const tries = challengesFor(apiKey, browserId);
  return async () => {
    await tries.captcha();
    await tries.mfa(await tries.signIn());
  };
}

/** One checkpoint: CAPTCHA, then sign-in, then MFA. */
async function checkpoint(run: RunContext) {
  const page = pageOf(run);
  await clearCaptcha(run, page);
  // Credentials before the code: the code prompt only exists once the site has
  // accepted a password, and a login page can carry a CAPTCHA of its own,
  // which is why this sits between the two.
  const login = await signIn(run, page);
  await completeMfa(run, page, login);
}

/** A CAPTCHA on the page solved by the solver or the provider; null when detection failed. */
function tryCaptcha(run: RunContext, { browser }: PageContext) {
  const options = { providerSolves: NATIVE_CAPTCHA.includes(browser?.provider), env: keyConfig.envFor(run.apiKey) };
  return captcha.handle(run.evaluate, options).catch(() => null);
}

/** Solves a CAPTCHA on the page, or hands it to a person. */
async function clearCaptcha(run: RunContext, page: PageContext) {
  const c = await tryCaptcha(run, page);
  if (c?.present && !c.solved && !c.invisible && c.method !== 'provider') {
    const message = c.error || 'A CAPTCHA needs solving. Solve it in the live view, then respond.';
    await run.requestHuman({ reason: 'captcha', message, liveViewUrl: run.liveViewUrl });
  }
}

/** A sign-in form completed with the persona's credentials, its outcome recorded; null when detection failed. */
async function trySignIn(run: RunContext, page: PageContext) {
  const where = { domain: page.domain, browserId: run.browserId, liveViewUrl: run.liveViewUrl };
  const l = await siteLogin.complete(run.evaluate, page.personaId, where).catch(() => null);
  if (l?.present) announceLogin(run, page, l);
  return l;
}

/** Completes a sign-in form with the persona's credentials, or hands it to a person. */
async function signIn(run: RunContext, page: PageContext) {
  const l = await trySignIn(run, page);
  if (l?.present && !l.completed) {
    const message = l.error || 'A sign-in needs completing in the live view.';
    await run.requestHuman({ reason: 'login', message, liveViewUrl: run.liveViewUrl });
  }
  return l;
}

/** Records the sign-in's outcome on the key's event log. */
function announceLogin(run: RunContext, { personaId, domain }: PageContext, l) {
  const type = l.completed ? 'login.completed' : 'login.failed';
  announce(run.apiKey, type, run.browserId, { personaId, domain, method: l.method ?? null });
}

/** An MFA prompt answered from the persona's factors (TOTP, mailbox, SMS), its outcome recorded; null when detection failed. */
async function tryMfa(run: RunContext, { personaId, domain }: PageContext, l?) {
  // `since` is the moment this login asked for a code, so a code sitting in
  // the mailbox from the previous run is not mistaken for this one's.
  const since = l?.submittedAt || l?.requestedAt || 0;
  // The tenant's own LLM reads the code out of the message: these email
  // templates are rewritten constantly and the code is not always digits.
  const m = await mfa
    .complete(run.evaluate, personaId, { liveViewUrl: run.liveViewUrl, domain, since, llm: run.llm })
    .catch(() => null);
  // Provider and outcome only: the code itself never leaves this process.
  if (m?.present && m.completed)
    announce(run.apiKey, 'mfa.completed', run.browserId, { personaId, domain, method: m.method ?? null });
  return m;
}

/** Answers an MFA prompt from the persona's mailbox, or asks a person for the code. */
async function completeMfa(run: RunContext, page: PageContext, l) {
  const m = await tryMfa(run, page, l);
  if (m?.present && !m.completed) await askForCode(run, m);
}

/** Parks the run on a person for the MFA code, and types it if their reply carries one. */
async function askForCode(run: RunContext, m) {
  const message = m.error || 'MFA needs completing in the live view.';
  const answer = await run.requestHuman({ reason: 'mfa', message, liveViewUrl: run.liveViewUrl });
  // Someone who replies with the code, in Slack, the dashboard or the SDK,
  // has answered the challenge. Type it for them rather than sending them to
  // the live view to do the same thing again.
  const code = mfa.codeInReply(answer);
  if (code) await mfa.submitCode(run.evaluate, code).catch(() => null);
}
