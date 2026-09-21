/**
 * Slack notifications: a run that failed, or one parked on a person.
 *
 * Two ways in, one thing stored. A customer either installs the hosted Slack app
 * (OAuth) or pastes a bot token from their own app; both write the same sealed
 * `_slack` row in key-config, so nothing downstream knows which path was used.
 * Sending rides the control plane's existing event -> outbox -> delivery worker,
 * so retries, backoff and replay are the ones that were already there.
 *
 * The message's primary action is a share link, an expiring credential scoped to
 * one browser (control/service.js share()), so whoever sees the alert can take the
 * browser over and finish the login or CAPTCHA without an Oya account.
 */

export { oauthConfigured, consoleUrl, call, isDeadInstall } from './client.ts';
export { blocksFor } from './message.ts';
export { verifySignature } from './signature.ts';
export { slackRouter } from './routes.ts';
export { slackActionsRouter } from './actions.ts';
