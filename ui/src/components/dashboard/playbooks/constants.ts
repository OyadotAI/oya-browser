/**
 * The Playbooks tab's numbers and fixed text, by name.
 */
import type { AttentionReason, RunStatus } from './types';

/** How often the playbook list refreshes. */
export const PLAYBOOKS_POLL_MS = 15_000;
/** How often a running recording's steps are fetched. */
export const RECORD_POLL_MS = 800;
/** The window the live view's frames-per-second is counted over. */
export const FPS_WINDOW_MS = 1000;
/**
 * Human input is refused unless a person holds the browser, and the hold expires
 * in five minutes, shorter than plenty of flows, so recording renews it this often.
 */
export const CONTROL_RENEW_MS = 120_000;
/** Shown when the hold could neither be renewed nor taken again. */
export const HOLD_LOST =
  'Could not keep control of the browser. Your steps so far are safe: stop and save, or take control again.';
/** Minutes left at which a recording warns that the server is about to stop it. */
export const RECORD_WARN_MINUTES = 5;
/** How often a replay run is polled. */
export const RUN_POLL_MS = 2000;
/** Failed polls in a row before the person is told contact with the run is lost. */
export const POLL_FAILURES_BEFORE_NOTICE = 3;
/** How long a browser started from here gets to dial in, cloud ones take up to ~90s. */
export const CONNECT_TIMEOUT_MS = 150_000;

/** Said when a browser started for a run never dialled in. */
export const CONNECT_TIMEOUT_MESSAGE =
  'The browser did not connect in time. It may still come up, check the Browsers tab.';

/** A valid playbook name: 1-64 letters, digits, _ or -. */
export const NAME_PATTERN = /^[\w-]{1,64}$/;
/** The name suffix the server uses for a playbook's healed draft. */
export const DRAFT_SUFFIX = ':draft';

/** What the empty list suggests: record from code with ask(), then replay. */
export const RECORD_SNIPPET = `await browser.ask('Fill the order for {{name}}', { data: { name: 'Ada' } });
await browser.toPlaybook('order');           // saved here
await browser.play('order', { name: 'Alan' }); // replayed without the LLM`;

/** The badge on a paused run, by why it paused. */
export const ATTENTION: Record<AttentionReason, string> = {
  captcha: 'CAPTCHA',
  login: 'Sign-in',
  mfa: 'MFA',
  agent: 'Agent question',
  heal_failed: 'Could not heal',
};

/** The reply box's hint, by why the run paused; anything else is just "done". */
export const REPLY_PLACEHOLDER: Partial<Record<AttentionReason, string>> = {
  agent: 'Your answer',
  mfa: 'Paste the code, or finish in the live view and type done',
};

/** The status text's colour, by run status; a running or finished run is accent. */
export const STATUS_CLASS: Partial<Record<RunStatus, string>> = {
  failed: 'text-red',
  needs_attention: 'text-yellow',
};

/** Friendly toasts for the recording refusals the server names by code. */
export const RECORD_ERRORS: Record<string, string> = {
  control_busy: 'Another tab or operator is holding this browser.',
  commands_pending: 'The browser is still finishing a command. Try again in a moment.',
};

/** The error code for a browser someone else holds. */
export const CONTROL_BUSY = 'control_busy';
