/**
 * Auth, Supabase-backed user accounts, API keys, and fleet tokens.
 *
 * Three layers:
 *   1. Supabase Auth (signup/login), users get JWTs
 *   2. API keys (per user), browsers connect with these. Only sha256(key) is
 *      stored, in oya_browser.api_keys.key_hash, alongside an 8-character
 *      prefix for display: a read of that table yields no working credential.
 *   3. Admin keys (env var) + fleet token (env var), for ops/fleet management
 *
 * This file is the module's facade; the work lives in keys.ts, api-keys.ts,
 * accounts.ts and middleware.ts.
 */

export { keyDigest, knownKeys, authReady, validateApiKey, isFleetToken, agentKeyUnclaimed } from './keys.ts';
export {
  getKeyOwner,
  registerApiKey,
  registerAgentKey,
  isUnclaimedAgentKey,
  listApiKeys,
  deleteApiKey,
  touchApiKey,
  provisionKeys,
} from './api-keys.ts';
export { issueChallenge, spendChallenge, validEmail, clientAddress, claimUrl } from './agents.ts';
export { signup, login, refreshSession, oauthUrl, oauthSignup, getProfile, updateProfile } from './accounts.ts';
export { verifyCaptcha } from './captcha.ts';
export { userAuthMiddleware, authenticateToken, authMiddleware } from './middleware.ts';
