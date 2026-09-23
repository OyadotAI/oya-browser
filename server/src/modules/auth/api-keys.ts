/**
 * Per-user API keys: registering, listing, revoking and minting them, and
 * resolving which account owns one. Only a key's digest is ever stored.
 */

import { sealText } from '../../platform/secrets.ts';
import { control, projectId } from '../control/service.ts';
import { db as supabase } from '../../platform/db.ts';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { generateKey, isEnvKey, isFleetToken, keyCache, keyDigest, keyPrefix, noteAgentKey } from './keys.ts';
import { MAX_PROJECT_NAME } from './constants.ts';

/** What a new project is called until its key's label names it. */
const DEFAULT_PROJECT_NAME = /^Project [0-9a-f]{6}$/;

/**
 * Resolve the account that owns an API key, or null for keys with no account
 * (env API_KEYS admin keys, the fleet token, or an unknown key).
 * Cached, this sits on the chat request path.
 */
const ownerCache = new Map();

/** The user id that owns a key, or null for env keys, the fleet token and unknown keys. Hits are cached. */
export async function getKeyOwner(key) {
  if (!key || isEnvKey(key) || isFleetToken(key)) return null;
  if (ownerCache.has(key)) return ownerCache.get(key);
  return supabase ? lookupOwner(key) : null;
}

/** Reads a key's owner from Supabase and caches a hit; a failure is logged and means no owner. */
async function lookupOwner(key) {
  try {
    const owner = (await ownerRow(keyDigest(key)))?.user_id || null;
    // Only cache a hit. Caching null would pin a key registered on another
    // instance to "no account" for the life of the process, silently falling
    // back to the server-wide OpenAI config.
    if (owner) ownerCache.set(key, owner);
    return owner;
  } catch (e) {
    console.error('[auth] Failed to resolve key owner:', e.message);
    return null;
  }
}

/** The api_keys row for a digest (its user_id, and agent_email for a key an agent made), or null. */
async function ownerRow(digest) {
  const { data, error } = await supabase
    .from('api_keys')
    .select('user_id, agent_email')
    .eq('key_hash', digest)
    .maybeSingle();
  if (error) throw error;
  return data;
}

/** A key's api_keys row: digest, display prefix and project, never the key. */
const keyRow = (key) => ({
  key_hash: keyDigest(key),
  key_prefix: keyPrefix(key),
  project: projectId(key),
  created_at: new Date().toISOString(),
});

/** Stores a key's digest for a user and claims its project. Re-importing a key already registered to the same user re-seals its project; an agent's unclaimed key becomes this user's (resolving true); a key owned by someone else is refused. */
export async function registerApiKey(key, userId, label) {
  const digest = keyDigest(key);
  if (supabase && userId) {
    const existing = await ownerRow(digest);
    if (existing) return reimport(key, digest, userId, existing);
    await insertKeyRow(key, userId, label);
  }
  // Claim first: a refused claim must not leave the key cached as the refused user's.
  await claimNewProject(key, userId, label);
  remember(key, digest, userId);
}

/** A key this user already registered: re-seal its project under this server's secret. */
async function reimport(key, digest, userId, existing) {
  const adopted = isUnclaimedAgent(existing);
  if (adopted) await adoptAgentKey(digest, userId);
  else if (existing.user_id !== userId) throw new HttpError(Status.FORBIDDEN, 'Key cannot be imported');
  await claimProject(key, userId);
  remember(key, digest, userId);
  return adopted;
}

/** Opens the key's project and makes it `userId`'s, re-sealed under this server's secret. */
async function claimProject(key, userId) {
  await control().project(key);
  await control().store.transact(async (tx) => {
    claim(await tx.get('project', projectId(key)), key, userId);
  });
}

/** A row an agent made for itself that no person has claimed yet. */
const isUnclaimedAgent = (row) => Boolean(row?.agent_email && !row.user_id);

/**
 * Hands an agent's key to the person claiming it. Conditional on the row still
 * being unowned, so two people racing for one claim link cannot both win.
 */
async function adoptAgentKey(digest, userId) {
  const rows = supabase.from('api_keys').update({ user_id: userId });
  const { data, error } = await rows.eq('key_hash', digest).is('user_id', null).select('key_hash');
  if (error) throw error;
  if (!data?.length) throw new HttpError(Status.FORBIDDEN, 'Key cannot be imported');
  noteAgentKey(digest, false);
}

/** Mints a key for an AI agent, owned by nobody until a person imports it, and opens its project. */
export async function registerAgentKey(email: string) {
  if (!supabase) throw new HttpError(Status.UNAVAILABLE, 'Agent signup needs Supabase');
  const key = generateKey();
  const { error } = await supabase.from('api_keys').insert({ ...keyRow(key), agent_email: email, label: 'Agent' });
  if (error) throw error;
  keyCache.add(keyDigest(key));
  noteAgentKey(keyDigest(key), true);
  await control().project(key);
  return key;
}

/** Whether `key` is an agent's own key that no person has claimed: those may not run browsers this server pays for. */
export async function isUnclaimedAgentKey(key) {
  if (!supabase || !key || isEnvKey(key) || isFleetToken(key)) return false;
  return isUnclaimedAgent(await ownerRow(keyDigest(key)));
}

/** Records a new key for a user. */
async function insertKeyRow(key, userId, label) {
  const { key_hash, key_prefix, project, created_at } = keyRow(key);
  const row = { key_hash, key_prefix, project, user_id: userId, label: label || 'Default', created_at };
  const { error } = await supabase.from('api_keys').insert(row);
  if (error) throw error;
}

/** Caches the digest, and the owner when there is one. */
function remember(key, digest, userId) {
  keyCache.add(digest);
  if (userId) ownerCache.set(key, userId);
}

/**
 * Claiming only works on an unowned project. Assigning unconditionally let
 * an import of someone else's key, one with a project but no api_keys row,
 * such as a key from API_KEYS, hand its browsers and the right to mint
 * credentials to whoever imported it.
 */
function claim(p, key, userId) {
  if (p.ownerUser && p.ownerUser !== userId) throw new HttpError(Status.FORBIDDEN, 'Key belongs to another account');
  p.ownerUser = userId;
  // Re-seal under this server's secret, which is what makes "add the original key" repair an unreadable project.
  p.key = sealText(`control:${p.id}`, key);
}

/** Opens the key's project and, for a user's key, claims it and names it after the label. */
async function claimNewProject(key, userId, label) {
  const project = await control().project(key);
  if (!userId) return;
  await control().store.transact(async (tx) => {
    const p = await tx.get('project', project.id);
    claim(p, key, userId);
    // A new project takes its key's label as its name, so every view calls it the same thing.
    if (label && DEFAULT_PROJECT_NAME.test(p.name)) p.name = String(label).slice(0, MAX_PROJECT_NAME);
  });
}

/**
 * Key metadata, never a key. `id` is the digest, the handle the console uses to
 * delete one, and `project` is what a project is opened with, via
 * POST /auth/projects/:id/access, which mints a scoped, expiring credential.
 */
export async function listApiKeys(userId) {
  if (!supabase) return [];
  const { data, error } = await supabase
    .from('api_keys')
    .select('key_hash, key_prefix, project, label, created_at, last_used_at')
    .eq('user_id', userId)
    .order('created_at', { ascending: false });
  if (error) throw error;
  return (data || []).map(({ key_hash, key_prefix, ...rest }) => ({ id: key_hash, prefix: key_prefix, ...rest }));
}

/** By digest: the server has no way to look a key up by its plaintext any more. */
export async function deleteApiKey(id, userId) {
  if (!supabase) throw new HttpError(Status.CONFLICT, 'Accounts need Supabase');
  const deleted = await deleteKeyRow(id, userId);
  if (!deleted?.length) throw new HttpError(Status.NOT_FOUND, 'Key not found');
  keyCache.delete(id);
  for (const [key, owner] of ownerCache) if (owner === userId && keyDigest(key) === id) ownerCache.delete(key);
}

/** Deletes a user's key row by digest; returns the rows removed. */
async function deleteKeyRow(id, userId) {
  const { data, error } = await supabase
    .from('api_keys')
    .delete()
    .eq('key_hash', id)
    .eq('user_id', userId)
    .select('key_hash');
  if (error) throw error;
  return data;
}

/** Records when a key was last used; failures are logged, never thrown. */
export async function touchApiKey(key) {
  if (supabase) {
    try {
      await supabase.from('api_keys').update({ last_used_at: new Date().toISOString() }).eq('key_hash', keyDigest(key));
    } catch (e) {
      console.error('[auth] touchApiKey failed:', e.message);
    }
  }
}

/** Mints `count` new unowned API keys, stores their digests and creates a project for each. Returns the plaintext keys. */
export async function provisionKeys(count) {
  const keys = [];
  for (let i = 0; i < count; i++) keys.push(generateKey());
  if (supabase && keys.length > 0) await upsertRows(keys);
  for (const key of keys) {
    keyCache.add(keyDigest(key));
    await control().project(key);
  }
  return keys;
}

/** Stores the rows for freshly minted keys, upserting on the digest. */
async function upsertRows(keys) {
  const { error } = await supabase.from('api_keys').upsert(
    keys.map((key) => keyRow(key)),
    { onConflict: 'key_hash' },
  );
  if (error) throw error;
}
