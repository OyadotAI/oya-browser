/**
 * Routes that clear what stands between an agent and a page: CAPTCHAs and MFA
 * prompts.
 */
import { registry } from '../registry.ts';
import { NATIVE_CAPTCHA, evaluateIn, getKey } from '../../../app/http.ts';
import { container } from '../../../app/container.ts';
import * as captcha from '../../challenges/captcha.ts';
import * as mfa from '../../challenges/mfa.ts';
import * as credentials from '../../personas/credentials.ts';
import * as keyConfig from '../../config/service.ts';
import { auditBrowser } from './helpers.ts';

/** Not yet layered: reads the persona service from the composition root. */
const { personas } = container;

/** Detects a CAPTCHA on the page and, unless `solve` is false, solves it. */
export async function solveCaptcha(req, res) {
  const { browserId } = req.params;
  const browser = registry.get(browserId);
  const result = await captcha.handle((expr) => evaluateIn(browserId, expr), captchaOptions(req, browser));
  const meta = { type: result.type, method: result.method };
  if (result.present) auditBrowser(req, 'captcha.handle', browserId, meta, result.solved ? 'ok' : 'error');
  res.json(result);
}

/** A provider that solves natively is not paid twice or raced. */
const captchaOptions = (req, browser) => ({
  solve: req.body?.solve !== false,
  providerSolves: NATIVE_CAPTCHA.includes(browser.provider),
  env: keyConfig.envFor(getKey(req)),
});

/** Completes an MFA prompt on the page for the browser's persona. */
export async function completeMfa(req, res) {
  const { browserId } = req.params;
  const browser = registry.get(browserId);
  const personaId = browser.persona?.id || personas.defaultFor(getKey(req)).id;
  const result = await mfa.complete((expr) => evaluateIn(browserId, expr), personaId, mfaOptions(req, browser));
  const meta = { method: result.method };
  if (result.present) auditBrowser(req, 'mfa.complete', browserId, meta, result.completed ? 'ok' : 'error');
  res.json(result);
}

/** Where a person can step in, which site it is, and what counts as a fresh code. */
const mfaOptions = (req, browser) => ({
  liveViewUrl: `/dashboard/?browser=${encodeURIComponent(req.params.browserId)}`,
  domain: credentials.domainOf(browser?.currentUrl || ''),
  since: Number(req.body?.since) || 0,
  llm: keyConfig.resolve(getKey(req)),
});
