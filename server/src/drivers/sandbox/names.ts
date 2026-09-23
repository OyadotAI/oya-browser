/**
 * The names every sandbox runtime shares: how a browser's sandbox is named,
 * how its owner is tagged, and how a setting is read from the environment.
 * A leaf module, so the runtimes in workers/ can use it without importing
 * the runtime selection that imports them.
 */
import { createHash } from 'crypto';
import { OWNER_TAG_CHARS, SHORT_ID_CHARS } from '../constants.ts';

/** Sandboxes are named this plus the browser id, so a restart can still find one. */
export const PREFIX = 'oya-browser-';

/** Stable, non-reversible tag for the owning API key. Never label with the key itself. */
export const ownerTag = (apiKey) => createHash('sha256').update(apiKey).digest('hex').slice(0, OWNER_TAG_CHARS);

/**
 * The ExternalId a key's AWS role is assumed with. Oya's, never the key's: a key
 * puts it in its role's trust policy, and no other key can make it be sent.
 */
export const ecsExternalId = (apiKey) => `oya-${ownerTag(apiKey)}`;

/** A cloud browser's name: the one given, or one made from its id. */
export const displayName = (name, browserId) => name || `Cloud browser ${browserId.slice(0, SHORT_ID_CHARS)}`;

/** An OYA_CLOUD_* setting, with the older DAYTONA_* name as a fallback. */
export const setting = (env, name) => env[`OYA_CLOUD_${name}`] || env[`DAYTONA_${name}`] || '';

/** The env names from `required` whose value is empty. */
export const unset = (required: [string, string][]) => required.filter(([, value]) => !value).map(([name]) => name);
