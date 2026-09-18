/**
 * Project operations' state: the durable overview polled from /control, the
 * settings drafts seeded from it, the action runner, and the small forms.
 */
import { useCallback, useEffect, useMemo, useState } from 'react';
import { api, errorMessage } from '@/lib/api-client';
import { DURABLE_POLL_MS, JSON_INDENT } from './constants';
import { failure } from './requests';
import type { Member, Overview, ProjectSettings } from './types';

/** A call under /control: path, method (GET), body. */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type ControlRequest = (path?: string, method?: string, body?: unknown) => Promise<any>;

/** Settings as they are being edited; rates and policy are JSON text. */
export interface Drafts {
  /** Numeric settings, once seeded. */
  settings: ProjectSettings | null;
  /** Rate cards, as JSON. */
  rates: string;
  /** Managed policy, as JSON. */
  policy: string;
}

/** The small inputs around the page. */
export interface DurableForm {
  /** Inventory state filter; '' is all. */
  filter: string;
  /** Role for new invitations and credentials. */
  role: string;
  /** Label for a new credential. */
  label: string;
  /** URL for a new webhook. */
  url: string;
}

/** The overview and its team, as one poll delivers them. */
interface Loaded {
  /** GET /control. */
  value: Overview;
  /** GET /control/members, when the key may see credentials. */
  members: Member[];
}

/** A state setter, as the loaders see it. */
type Setter<T> = (value: T) => void;

/** Nothing typed yet: the first poll fills these in. */
const EMPTY_DRAFTS: Drafts = { settings: null, rates: '{}', policy: '{}' };
/** The forms' starting values. */
const EMPTY_FORM: DurableForm = { filter: '', role: 'operator', label: '', url: '' };

/** Binds /control requests to a key. */
export const controlRequest =
  (apiKey: string): ControlRequest =>
  (path = '', method = 'GET', body) =>
    api(`/control${path}`, { key: apiKey, method, body });

/** Fills drafts the user has not touched from the server's settings; edits are kept. */
export const seedDrafts = (d: Drafts, s: ProjectSettings): Drafts => ({
  settings: d.settings || s,
  policy: d.policy === '{}' ? JSON.stringify(s.policy, null, JSON_INDENT) : d.policy,
  rates: d.rates === '{}' ? JSON.stringify(s.rates, null, JSON_INDENT) : d.rates,
});

/** PATCH /control/project body; throws when rates or policy is not JSON. */
export const settingsBody = (d: Drafts) => ({
  ...d.settings,
  rates: JSON.parse(d.rates),
  policy: JSON.parse(d.policy),
});

/** A secret the server shows once: an invitation code, token or webhook secret. */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export const shownOnce = (result: any): string => result.token || result.secret || result.code;

/** Everything Project operations reads and does. */
export function useDurable(apiKey: string) {
  const request = useMemo(() => controlRequest(apiKey), [apiKey]);
  const drafts = useDrafts();
  const overview = useOverview(request, drafts.seed);
  const actions = useDurableActions(request, overview.refresh, overview.setError);
  const form = useFields(EMPTY_FORM);
  return { ...overview, ...drafts, ...actions, form: form.values, setForm: form.set };
}

/** Project operations' state and actions. */
export type Durable = ReturnType<typeof useDurable>;

/** A small record of fields with a patching setter. */
function useFields<T>(initial: T) {
  const [values, setValues] = useState(initial);
  const set = useCallback((patch: Partial<T>) => setValues((v) => ({ ...v, ...patch })), []);
  return { values, set };
}

/** Settings drafts, and a seeder that fills only what the user has not touched. */
function useDrafts() {
  const [drafts, setDrafts] = useState(EMPTY_DRAFTS);
  const editDrafts = useCallback((patch: Partial<Drafts>) => setDrafts((d) => ({ ...d, ...patch })), []);
  const seed = useCallback((s: ProjectSettings) => setDrafts((d) => seedDrafts(d, s)), []);
  return { drafts, editDrafts, seed };
}

/** The overview, its members and its error, polled and refreshable. */
function useOverview(request: ControlRequest, seed: (s: ProjectSettings) => void) {
  const [data, setData] = useState<Overview | null>(null);
  const [members, setMembers] = useState<Member[]>([]);
  const [error, setError] = useState('');
  const apply = useMemo(() => applier(setData, setMembers, seed), [seed]);
  usePollOverview(request, apply, setError);
  const refresh = useCallback(() => refreshOverview(request, setData, setMembers), [request]);
  return { data, members, error, setError, refresh };
}

/** Stores a poll's result and seeds the drafts from it. */
function applier(setData: Setter<Overview>, setMembers: Setter<Member[]>, seed: Setter<ProjectSettings>) {
  return ({ value, members }: Loaded) => {
    setMembers(members);
    setData(value);
    seed(value.project.settings);
  };
}

/** The overview, then its members when the key may see credentials. */
async function fetchOverview(request: ControlRequest): Promise<Loaded> {
  const value = (await request()) as Overview;
  const team = value.credentials ? await request('/members') : null;
  return { value, members: team?.members || [] };
}

/** Loads now and on an interval; results after unmount are dropped. */
function usePollOverview(request: ControlRequest, apply: Setter<Loaded>, fail: Setter<string>) {
  useEffect(() => pollOverview(request, apply, fail), [request, apply, fail]);
}

/** Starts polling; the returned cleanup stops it and silences loads in flight. */
function pollOverview(request: ControlRequest, apply: Setter<Loaded>, fail: Setter<string>) {
  let mounted = true;
  const live = () => mounted;
  const load = () => loadOverview(request, whenLive(live, apply), whenLive(live, fail));
  void load();
  const timer = setInterval(() => void load(), DURABLE_POLL_MS);
  return () => ((mounted = false), clearInterval(timer));
}

/** Wraps a setter so it does nothing once `live` says the poll is over. */
const whenLive =
  <T>(live: () => boolean, fn: Setter<T>) =>
  (v: T) =>
    live() && fn(v);

/** One poll: the overview to `apply`, or its failure's message to `fail`. */
const loadOverview = (request: ControlRequest, apply: Setter<Loaded>, fail: Setter<string>) =>
  fetchOverview(request).then(apply, (e) => fail(errorMessage(e, 'Control data unavailable')));

/** After an action: reload the overview, then the members if credentials are visible. */
async function refreshOverview(request: ControlRequest, setData: Setter<Overview>, setMembers: Setter<Member[]>) {
  const value = (await request()) as Overview;
  setData(value);
  if (value.credentials) setMembers((await request('/members')).members);
}

/** Runs one action at a time; keeps any once-shown secret and reports failures. */
function useDurableActions(request: ControlRequest, refresh: () => Promise<void>, setError: (e: string) => void) {
  const [busy, setBusy] = useState(false);
  const [secret, setSecret] = useState('');
  const act = (path: string, method = 'POST', body?: unknown) =>
    runAct({ request, refresh, setError, setBusy, setSecret }, [path, method, body]);
  return { busy, secret, setSecret, act };
}

/** What an action touches. */
interface ActDeps {
  /** Sends the request. */
  request: ControlRequest;
  /** Reloads afterwards. */
  refresh: () => Promise<void>;
  /** Reports a failure. */
  setError: (e: string) => void;
  /** Marks the page busy. */
  setBusy: (busy: boolean) => void;
  /** Keeps a secret the server shows once. */
  setSecret: (s: string) => void;
}

/** Busy, clear the error, perform, then record any failure. */
async function runAct(deps: ActDeps, call: [string, string, unknown]) {
  deps.setBusy(true);
  deps.setError('');
  const err = await failure(() => perform(deps, call), 'Action failed');
  if (err) deps.setError(err);
  deps.setBusy(false);
}

/** One action: the request, any secret it returns, then a refresh. */
async function perform({ request, refresh, setSecret }: ActDeps, [path, method, body]: [string, string, unknown]) {
  const once = shownOnce(await request(path, method, body));
  if (once) setSecret(once);
  await refresh();
}
