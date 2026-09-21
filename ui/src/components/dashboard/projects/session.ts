/**
 * The open project's lifecycle: open one with a fresh one-hour credential,
 * renew it while the tab stays open, fall back to another when access goes,
 * and pick the project back up after a reload. Every step is a plain function
 * of the Session so the hook that owns it stays a thin wire.
 */
import { CREDENTIAL_GONE_EVENT, PROJECT_CREDENTIAL, PROJECT_ID, RENEW_INTERVAL_MS } from './constants';
import { call, isGone, listOwnedKeys, message } from './project-api';
import type { AccessAnswer, Project, Session } from './types';

/** Stores the credential and tells the console which project is open. */
function remember(s: Session, id: string, credential: string) {
  sessionStorage.setItem(PROJECT_CREDENTIAL, credential);
  sessionStorage.setItem(PROJECT_ID, id);
  s.setCurrentId(id);
  s.setApiKey(credential, id);
}

/** The request that exchanges the account token for a project credential. */
const ACCESS_INIT = { method: 'POST', body: '{}' };

/** Resolves false when a newer switch superseded this one. A renewal never supersedes a switch. */
export async function openProject(s: Session, id: string, renew = false) {
  if (!s.token) return false;
  const seq = renew ? s.opening.current : ++s.opening.current;
  const path = `/auth/projects/${encodeURIComponent(id)}/access`;
  const data = await call<AccessAnswer>(s.token, path, ACCESS_INIT, 'Could not open that project');
  if (seq !== s.opening.current) return false;
  remember(s, id, data.token);
  return true;
}

/** Fetches the projects and owned keys, and stores them. */
export async function loadProjects(s: Session): Promise<Project[]> {
  if (!s.token) return [];
  const [projects, keys] = await Promise.all([
    call<Project[]>(s.token, '/auth/projects', {}, 'Could not load projects'),
    listOwnedKeys(s.token),
  ]);
  s.setListing({ projects, keys });
  return projects;
}

/** Drops the stored credential; also cancels any switch in flight. */
export function forget(s: Session) {
  ++s.opening.current;
  sessionStorage.removeItem(PROJECT_CREDENTIAL);
  sessionStorage.removeItem(PROJECT_ID);
  s.setCurrentId(null);
  s.setApiKey('', null);
}

/** Marks an attempt that opened (or was superseded, which also ends the search). */
const OPENED = Symbol('opened');

/** One try at opening: OPENED, or the error. */
const attempt = (s: Session, id: string) =>
  openProject(s, id).then(
    () => OPENED,
    (e: unknown) => e,
  );

/** Tries each project in turn; returns the last failure, or null once one opened (or there were none). */
async function firstThatOpens(s: Session, list: Project[]) {
  let failure: unknown = null;
  for (const p of list) {
    const outcome = await attempt(s, p.id);
    if (outcome === OPENED) return null;
    failure = outcome;
  }
  return failure;
}

/** Open the first project that opens, yours before shared ones, so one unreadable project never leaves the console empty. */
export async function openAny(s: Session, list: Project[]) {
  const failure = await firstThatOpens(s, [...list.filter((p) => p.owner), ...list.filter((p) => !p.owner)]);
  if (!failure) return;
  forget(s);
  s.toast(`No project could be opened: ${message(failure)}`, 'error');
}

/** A failed renewal: when access is really gone, move the console to another project. */
async function renewalFailed(s: Session, id: string, seq: number, e: unknown) {
  // A switch that started meanwhile decides what the console shows; a stale renewal must not forget it.
  if (seq !== s.opening.current || !isGone(e) || sessionStorage.getItem(PROJECT_ID) !== id) return;
  s.toast('You no longer have access to that project', 'info');
  forget(s);
  const rest = (await loadProjects(s)).filter((p) => p.id !== id);
  await openAny(s, rest);
}

/** Renews the open project's credential; losing access moves the console to another project. */
export async function renew(s: Session, id: string) {
  const seq = s.opening.current;
  try {
    await openProject(s, id, true);
  } catch (e) {
    await renewalFailed(s, id, seq, e);
  }
}

/** Reopens the stored project with a fresh credential (the stored one may have expired while the tab slept), or the first that opens. */
function resume(s: Session, list: Project[]) {
  const id = sessionStorage.getItem(PROJECT_ID),
    credential = sessionStorage.getItem(PROJECT_CREDENTIAL);
  if (id && credential && list.some((p) => p.id === id)) {
    s.setCurrentId(id);
    s.setApiKey(credential, id);
    return renew(s, id);
  }
  return openAny(s, list);
}

/** On sign-in: load the projects and open one. */
export function startup(s: Session) {
  // Key-only sign-in has no account and no projects; its console credential is not ours to replace.
  if (!s.token) return;
  void loadProjects(s)
    .then((list) => resume(s, list))
    .catch((e) => s.toast(message(e, 'Could not load projects'), 'error'));
}

/** Renews on an interval, and at once when the server refuses the credential (project deleted, access removed). Returns the cleanup. */
export function watchRenewal(s: Session, id: string) {
  const timer = setInterval(() => void renew(s, id), RENEW_INTERVAL_MS);
  const gone = () => void renew(s, id);
  window.addEventListener(CREDENTIAL_GONE_EVENT, gone);
  return () => {
    clearInterval(timer);
    window.removeEventListener(CREDENTIAL_GONE_EVENT, gone);
  };
}
