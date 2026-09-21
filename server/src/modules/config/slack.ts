/**
 * The Slack install for a key: bot token, workspace and the channel notifications
 * go to. Sealed like routing and, like routing, kept out of FIELDS, POST /config
 * must not be able to overwrite a bot token, and this is an object, not a scalar.
 * Both ways in (OAuth install and a pasted bot token) write this one row.
 */
import { fingerprint as ownerOf } from '../../platform/audit.ts';
import { store, seal, unseal, writeField, dropField } from './store.ts';

/** This key's Slack install, or null. */
export function getSlack(apiKey) {
  const owner = ownerOf(apiKey);
  const sealed = store.get(owner)?._slack;
  if (!sealed) return null;
  // Like reveal(): a row sealed under a rotated secret reads as absent rather than throwing.
  try {
    return unseal(owner, sealed);
  } catch {
    return null;
  }
}

/** Store this key's Slack install, replacing any earlier one. */
export async function saveSlack(apiKey, install) {
  const owner = ownerOf(apiKey);
  await writeField(owner, '_slack', seal(owner, install));
}

/** Forget this key's Slack install. */
export async function clearSlack(apiKey) {
  await dropField(ownerOf(apiKey), '_slack');
}
