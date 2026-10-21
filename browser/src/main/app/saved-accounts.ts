/**
 * Accounts this browser signed out of, as a mail app keeps them: the welcome
 * screen lists them and signs back in with one click. Each key is sealed with
 * the OS keychain; with no real keychain (Linux without a keyring) nothing is
 * kept, so a key never lingers in plain text after a log out.
 */
import { createHash } from 'node:crypto';
import type { AppServices } from './services.ts';
import type { SavedAccountView } from '../../shared/ipc.ts';
import { sealable, type ConfigValues } from './config-store.ts';
import { SAVED_ACCOUNT_LIMIT, SAVED_ACCOUNT_ID_CHARS } from './constants.ts';

/** One remembered account, as config.json keeps it. */
export interface SavedAccount extends SavedAccountView {
  /** The server it signs in to. */
  serverUrl: string;
  /** The persona it used. */
  persona?: string;
  /** The key, sealed by the OS keychain. */
  sealedKey: string;
}

/** A saved account opened for signing in. */
export interface OpenedAccount extends Pick<ConfigValues, 'apiKey' | 'serverUrl'> {
  /** The persona it used. */
  persona: string;
}

/** Settings and the keychain. */
type Deps = Pick<AppServices, 'config' | 'electron'>;

/** Who the server said this was, loosely typed as config.json holds it. */
interface KnownAccount {
  /** The person's name. */
  name?: string;
  /** The person's email. */
  email?: string;
  /** The project the key belongs to. */
  project?: {
    /** Its name. */
    name?: string;
  };
}

/** A stable id for a server and key that reveals neither; a saved draft also records it, to know which project it was saved to. */
export const idOf = (serverUrl: string, apiKey: string): string =>
  createHash('sha256').update(`${serverUrl}\n${apiKey}`).digest('hex').slice(0, SAVED_ACCOUNT_ID_CHARS);

/** This browser's server and key now, as the id a saved draft records (see workflow/published.ts). */
export const accountOf = ({ serverUrl, apiKey }: Partial<Pick<ConfigValues, 'serverUrl' | 'apiKey'>>): string =>
  idOf(String(serverUrl ?? ''), String(apiKey ?? ''));

/** The remembered accounts; anything malformed in the file counts as none. */
const savedOf = (deps: Deps): SavedAccount[] => {
  const list = deps.config.values.savedAccounts;
  return Array.isArray(list) ? (list as SavedAccount[]) : [];
};

/** The server's host, for the detail line. */
function serverHost(serverUrl: string): string {
  return URL.parse(serverUrl.replace(/^ws/, 'http'))?.host || serverUrl;
}

/** What the chooser shows: name over email, then the email when it is not the label, the project and host. */
function accountLabels(known: KnownAccount | null | undefined, serverUrl: string): Omit<SavedAccountView, 'id'> {
  const label = known?.name || known?.email || 'Signed in with an API key';
  const email = known?.name ? known.email || '' : '';
  return { label, email, project: known?.project?.name || '', host: serverHost(serverUrl) };
}

/** Before a log out forgets the key: keep it sealed, newest first, replacing an older copy of the same one. */
export function rememberAccount(deps: Deps): void {
  const { apiKey, serverUrl, persona, account } = deps.config.values;
  const safe = deps.electron.safeStorage;
  if (!apiKey || !sealable(safe)) return;
  const id = idOf(serverUrl, apiKey);
  const sealedKey = safe.encryptString(apiKey).toString('base64');
  const entry = { id, ...accountLabels(account as KnownAccount, serverUrl), serverUrl, persona, sealedKey };
  const others = savedOf(deps).filter((saved) => saved.id !== id);
  deps.config.merge({ savedAccounts: [entry, ...others].slice(0, SAVED_ACCOUNT_LIMIT) });
}

/** The accounts the welcome screen may offer. */
export function listAccounts(deps: Deps): SavedAccountView[] {
  return savedOf(deps).map(({ id, label, email, project, host, serverUrl }) => ({
    id,
    label: label || 'Signed in with an API key',
    email: email || '',
    project: project || '',
    host: host || serverHost(serverUrl),
  }));
}

/** The key and server of `id`, or null when it is unknown or this keychain cannot open it. */
export function openAccount(deps: Deps, id: string): OpenedAccount | null {
  const saved = savedOf(deps).find((entry) => entry.id === id);
  if (!saved || !sealable(deps.electron.safeStorage)) return null;
  try {
    const apiKey = deps.electron.safeStorage.decryptString(Buffer.from(saved.sealedKey, 'base64'));
    return { apiKey, serverUrl: saved.serverUrl, persona: saved.persona || 'default' };
  } catch {
    return null;
  }
}

/** Drops `id` from the list and saves. */
export function forgetAccount(deps: Deps, id: string): SavedAccountView[] {
  deps.config.merge({ savedAccounts: savedOf(deps).filter((entry) => entry.id !== id) });
  deps.config.save();
  return listAccounts(deps);
}
