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
  /** Parks the run on a person until they respond. */
  requestHuman: (attention: object) => Promise<any>;
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

/**
 * Between page-changing steps of a run: clear a CAPTCHA, sign in, or answer an
 * MFA prompt, or park the run on a person.
 * ponytail: three detection evals per page-changing step.
 */
export function checkpointFor(apiKey, browserId, requestHuman) {
  const liveViewUrl = `/dashboard/?browser=${encodeURIComponent(browserId)}`;
  const evaluate = (expr) => evaluateIn(browserId, expr);
  // Resolved once for the run, not per step: it decrypts this key's settings,
  // and the key cannot change underneath a run that is already going.
  const llm = keyConfig.resolve(apiKey);
  const run: RunContext = { apiKey, browserId, requestHuman, liveViewUrl, evaluate, llm };
  return () => checkpoint(run);
}

/** One checkpoint: CAPTCHA, then sign-in, then MFA. */
async function checkpoint(run: RunContext) {
  const browser = registry.get(run.browserId);
  const personaId = browser?.persona?.id || personas.defaultFor(run.apiKey).id;
  const domain = credentials.domainOf(browser?.currentUrl || '');
  const page: PageContext = { browser, personaId, domain };
  await clearCaptcha(run, page);
  // Credentials before the code: the code prompt only exists once the site has
  // accepted a password, and a login page can carry a CAPTCHA of its own,
  // which is why this sits between the two.
  const login = await signIn(run, page);
  await completeMfa(run, page, login);
}

/** Solves a CAPTCHA on the page, or hands it to a person. */
async function clearCaptcha(run: RunContext, { browser }: PageContext) {
  const options = { providerSolves: NATIVE_CAPTCHA.includes(browser?.provider), env: keyConfig.envFor(run.apiKey) };
  const c = await captcha.handle(run.evaluate, options).catch(() => null);
  if (c?.present && !c.solved && !c.invisible && c.method !== 'provider') {
    const message = c.error || 'A CAPTCHA needs solving. Solve it in the live view, then respond.';
    await run.requestHuman({ reason: 'captcha', message, liveViewUrl: run.liveViewUrl });
  }
}

/** Completes a sign-in form with the persona's credentials, or hands it to a person. */
async function signIn(run: RunContext, page: PageContext) {
  const where = { domain: page.domain, browserId: run.browserId, liveViewUrl: run.liveViewUrl };
  const l = await siteLogin.complete(run.evaluate, page.personaId, where).catch(() => null);
  if (l?.present) announceLogin(run, page, l);
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

/** Answers an MFA prompt from the persona's mailbox, or asks a person for the code. */
async function completeMfa(run: RunContext, { personaId, domain }: PageContext, l) {
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
