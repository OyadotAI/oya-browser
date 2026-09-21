/**
 * The console's data feeds as plain functions: browsers, fleet, personas and
 * configuration. Each drops a response that arrives after the credential
 * changed, so a previous project's data is never painted over the new one.
 */
import type { Dispatch, RefObject, SetStateAction } from 'react';
import { api, errorMessage } from '@/lib/api-client';
import { loadConfig, type KeyConfig } from '@/components/dashboard/config';
import type { BrowserRow, Fleet, Persona } from '@/components/dashboard/types';
import { commandRate, type Rate, type RateSample } from './rate';

/** Refs that say whether a response still matters. */
export interface Liveness {
  /** The credential in force now. */
  keyRef: RefObject<string>;
  /** Whether the tab is hidden (polls pause). */
  hidden: RefObject<boolean>;
}

/** A state setter. */
type Set<T> = Dispatch<SetStateAction<T>>;

/** Whether a poll should run now. */
const polling = (apiKey: string, live: Liveness) => !!apiKey && !live.hidden.current;

/** Where the browsers feed writes. */
export interface BrowsersSink {
  /** The table's rows. */
  setBrowsers: Set<BrowserRow[]>;
  /** Why the last refresh failed, or null. */
  setLoadError: Set<string | null>;
}

/** Loads the browser table; a failure keeps the last rows and says why. */
export async function loadBrowsers(apiKey: string, live: Liveness, sink: BrowsersSink) {
  if (!polling(apiKey, live)) return;
  try {
    const rows = await api<BrowserRow[]>('/browsers', { key: apiKey });
    if (live.keyRef.current === apiKey) showRows(sink, rows);
  } catch (err) {
    if (live.keyRef.current === apiKey) sink.setLoadError(errorMessage(err));
  }
}

/** Fresh rows arrived: show them and clear the error. */
function showRows(sink: BrowsersSink, rows: BrowserRow[]) {
  sink.setBrowsers(rows);
  sink.setLoadError(null);
}

/** Where the fleet feed writes. */
export interface FleetSink {
  /** The fleet summary. */
  setFleet: Set<Fleet | null>;
  /** The throughput. */
  setRate: Set<Rate | null>;
  /** The previous poll's counters. */
  sample: RefObject<RateSample | null>;
}

/** Loads the fleet summary and updates the throughput. */
export async function loadFleet(apiKey: string, live: Liveness, sink: FleetSink) {
  if (!polling(apiKey, live)) return;
  try {
    const f = await api<Fleet>('/fleet', { key: apiKey });
    if (live.keyRef.current !== apiKey) return;
    sink.setFleet(f);
    track(f, sink);
  } catch {
    /* strip shows dashes */
  }
}

/** Turns this poll's counters into a rate against the last one. */
function track(f: Fleet, sink: FleetSink) {
  const cur = { at: Date.now(), commands: f.browsers.commands, errors: f.browsers.errors };
  const rate = commandRate(sink.sample.current, cur);
  if (rate) sink.setRate(rate);
  sink.sample.current = cur;
}

/** The answer of /personas. */
interface PersonaList {
  /** The personas. */
  personas: Persona[];
}

/** Loads the personas; a failure keeps the last list. */
export async function loadPersonas(apiKey: string, live: Liveness, set: Set<Persona[]>) {
  if (!polling(apiKey, live)) return;
  try {
    const { personas } = await api<PersonaList>('/personas', { key: apiKey });
    if (live.keyRef.current === apiKey) set(personas || []);
  } catch {
    /* keep last */
  }
}

/** Where the config feed writes. */
export interface ConfigSink {
  /** The key's configuration. */
  setConfig: Set<KeyConfig | null>;
  /** Opens or closes the setup wizard. */
  setOnboarding: (show: boolean) => void;
  /**
   * The wizard decision is made once per project, on first load. Later
   * refreshes (after Settings, after Skip, after a credential renewal) must
   * not re-open it.
   */
  decidedFor: RefObject<string | null>;
}

/** Loads the configuration and, once per project, decides whether the wizard shows. */
export async function loadKeyConfig(apiKey: string, project: string | null, live: Liveness, sink: ConfigSink) {
  if (!apiKey) return sink.setConfig(null);
  try {
    const cfg = await loadConfig(apiKey);
    if (live.keyRef.current !== apiKey) return;
    sink.setConfig(cfg);
    decideOnboarding(cfg, project || apiKey, sink);
  } catch {
    /* an invalid key already shows as an empty fleet */
  }
}

/** Shows the wizard for an un-onboarded key, the first time this scope loads. */
function decideOnboarding(cfg: KeyConfig, scope: string, sink: ConfigSink) {
  if (sink.decidedFor.current === scope) return;
  sink.decidedFor.current = scope;
  sink.setOnboarding(!cfg.onboarded);
}
