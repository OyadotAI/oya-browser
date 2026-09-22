/**
 * The agent's own way past the walls a site puts up: a CAPTCHA, a sign-in form, an
 * MFA code. Each tool runs the same code a background run's checkpoint runs
 * (playbooks/checkpoint.ts challengesFor), with no person involved, and says what
 * happened in words; the agent decides what to do with a wall it could not clear
 * (ask a person, or report FAILED). The site's credentials and codes never reach
 * the model: they come from the persona and are typed by the browser.
 */
import { bare } from './tools.ts';

/** The challenges a run can clear (challengesFor), or none when the run was not given them. */
export type Challenges = {
  /** Solve a CAPTCHA on the page. */
  captcha: () => Promise<any>;
  /** Sign in with the persona's stored credentials for this site. */
  signIn: () => Promise<any>;
  /** Answer an MFA prompt from the persona's factors. */
  mfa: (login?: any) => Promise<any>;
  /** Where a person can finish a challenge by hand. */
  liveViewUrl: string;
};

/** The three tools, as the model is offered them. */
export const CHALLENGE_TOOLS = [
  bare(
    'solve_captcha',
    'Solve a CAPTCHA (reCAPTCHA, hCaptcha, Turnstile) on the current page with the configured solver. Call it when a CAPTCHA blocks you, then analyze_page again.',
  ),
  bare(
    'sign_in',
    "Sign in on the current page with the credentials stored for this site in the browser's persona. You never see them. Call it on a login form you were not given values for.",
  ),
  bare(
    'complete_mfa',
    "Fill and submit a one-time code (MFA / 2FA) the page asks for, from the persona's authenticator, email or SMS. Call it on a code prompt after signing in.",
  ),
];

/** What a CAPTCHA try says. */
function captchaSaid(c: any) {
  if (!c) return 'Error: could not check the page for a CAPTCHA.';
  if (!c.present) return 'No CAPTCHA on this page.';
  if (c.solved) return `Solved the CAPTCHA (${c.method}). Analyze the page to continue.`;
  if (c.method === 'provider')
    return 'The browser provider solves this CAPTCHA itself; wait a moment, then analyze the page.';
  return `A CAPTCHA is here but was not solved: ${c.error || 'no solver could'}. Use request_human, or report FAILED.`;
}

/** What a sign-in try says. */
function signInSaid(l: any) {
  if (!l) return 'Error: could not check the page for a sign-in form.';
  if (!l.present) return l.locked ? 'The account is locked on this site.' : 'No sign-in form on this page.';
  if (l.completed) return 'Signed in with the stored credentials. Analyze the page to continue.';
  return `Could not sign in: ${l.error || 'no stored credentials for this site'}. Use request_human, or report FAILED.`;
}

/** What an MFA try says. */
function mfaSaid(m: any) {
  if (!m) return 'Error: could not check the page for a code prompt.';
  if (!m.present) return 'No one-time code prompt on this page.';
  if (m.completed) return `Entered the one-time code (${m.method}). Analyze the page to continue.`;
  return `Could not complete the code prompt: ${m.error || 'no factor configured for this site'}. Use request_human, or report FAILED.`;
}

/** Tool name → how it runs, given the run's challenges. */
export const CHALLENGE_HANDLERS: Record<string, (c: Challenges) => Promise<string>> = {
  solve_captcha: async (c) => captchaSaid(await c.captcha()),
  sign_in: async (c) => signInSaid(await c.signIn()),
  complete_mfa: async (c) => mfaSaid(await c.mfa()),
};
