/**
 * Registering a browser provider for a key: validate the config, check the
 * vendor is known and has a credential, refuse a name already taken, vet the
 * CDP URL, then save it (and the vendor API key, when one came with it).
 */
import { available as availableProviders } from '../../drivers/providers.ts';
import { assertSafeTarget } from '../../platform/net-guard.ts';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import * as keyConfig from '../config/service.ts';
import { pool, validateProviderConfig } from './routing.ts';

/** Refusal for a name already taken by this key. */
const NAME_TAKEN = 'A provider with this name already exists. Choose another name.';

/** Registers the provider described by `body` for `key`; each refusal is an HttpError. */
export async function registerProvider(key, owner, body) {
  const cfg = validateProviderConfig({ ...(body || {}), owner });
  const credential = storableCredential(cfg, body);
  assertSupported(key, cfg, credential?.value || '');
  await vetUrl(cfg);
  const provider = pool.register(cfg);
  if (credential) await keyConfig.set(key, { [credential.field]: credential.value });
  await keyConfig.saveRouting(key, pool);
  return provider;
}

/** The vendor API key sent with the provider, when there is one and it is a secret setting this key may store. */
function storableCredential(cfg, body) {
  const value = typeof body?.apiKey === 'string' ? body.apiKey.trim() : '';
  const field = `${cfg.type}_api_key`;
  return value && keyConfig.FIELDS[field]?.secret ? { field, value } : null;
}

/** The vendor must be known and configured (with the key's saved or supplied credential), and the name free. */
function assertSupported(key, cfg, credential) {
  const env = keyConfig.envFor(key);
  if (credential) env[`${cfg.type.toUpperCase()}_API_KEY`] = credential;
  const supported = availableProviders(env).find((p) => p.name === cfg.type);
  if (!supported) throw new HttpError(Status.BAD_REQUEST, 'Unknown browser provider.');
  if (!supported.configured)
    throw new HttpError(Status.CONFLICT, 'Add an API key for this provider, or save one in Settings → Browsers.');
  assertNameFree(cfg);
}

/** Refuses a name this key already uses. */
function assertNameFree(cfg) {
  if (pool.get(cfg.owner, cfg.name)) throw new HttpError(Status.CONFLICT, NAME_TAKEN);
}

/**
 * The host dials this URL, using its network position rather than the
 * caller's. Unvalidated, that is a server-side request forgery primitive.
 */
async function vetUrl(cfg) {
  if (cfg.wsUrl) {
    const safe = await assertSafeTarget(cfg.wsUrl, { label: 'wsUrl' });
    cfg.wsUrl = safe.href;
  }
  // Recheck after DNS validation, which can yield to a concurrent addition.
  assertNameFree(cfg);
}
