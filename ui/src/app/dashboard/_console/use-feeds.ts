/**
 * The console's data as hooks: each feed's state, its stable fetcher, and a
 * reset for when another project opens.
 */
'use client';

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { KeyConfig } from '@/components/dashboard/config';
import type { BrowserRow, Fleet, Persona } from '@/components/dashboard/types';
import { loadBrowsers, loadFleet, loadKeyConfig, loadPersonas, type Liveness, type LoadStatus } from './feeds';
import type { Rate, RateSample } from './rate';

/** The refs every feed checks; stable for the page's life. */
export function useLiveness(): Liveness {
  const keyRef = useRef('');
  const hidden = useRef(false);
  return useMemo(() => ({ keyRef, hidden }), []);
}

/** The browser table and its last error. */
export function useBrowsers(apiKey: string, live: Liveness) {
  const [browsers, setBrowsers] = useState<BrowserRow[]>([]);
  const [loadError, setLoadError] = useState<string | null>(null);
  const fetchBrowsers = useCallback(() => loadBrowsers(apiKey, live, { setBrowsers, setLoadError }), [apiKey, live]);
  const resetBrowsers = useCallback(() => (setBrowsers([]), setLoadError(null)), []);
  return { browsers, loadError, fetchBrowsers, resetBrowsers };
}

/** The fleet summary and throughput. */
export function useFleet(apiKey: string, live: Liveness) {
  const [fleet, setFleet] = useState<Fleet | null>(null);
  const [rate, setRate] = useState<Rate | null>(null);
  const sample = useRef<RateSample | null>(null);
  const fetchFleet = useCallback(() => loadFleet(apiKey, live, { setFleet, setRate, sample }), [apiKey, live]);
  const resetFleet = useCallback(() => (setFleet(null), setRate(null), (sample.current = null)), []);
  return { fleet, rate, fetchFleet, resetFleet };
}

/** The personas. */
export function usePersonas(apiKey: string, live: Liveness) {
  const [personas, setPersonas] = useState<Persona[]>([]);
  const [personasStatus, setStatus] = useState<LoadStatus>('loading');
  const fetchPersonas = useCallback(() => loadPersonas(apiKey, live, { set: setPersonas, setStatus }), [apiKey, live]);
  const resetPersonas = useCallback(() => (setPersonas([]), setStatus('loading')), []);
  return { personas, personasStatus, fetchPersonas, resetPersonas };
}

/** Where the config feed writes; stable while `setOnboarding` is. */
function useConfigSink(setOnboarding: (show: boolean) => void) {
  const [config, setConfig] = useState<KeyConfig | null>(null);
  const decidedFor = useRef<string | null>(null);
  const sink = useMemo(() => ({ setConfig, setOnboarding, decidedFor }), [setOnboarding]);
  const resetConfig = useCallback(() => setConfig(null), []);
  return { config, sink, resetConfig };
}

/** Opens or closes the setup wizard. */
type OnboardingSetter = (show: boolean) => void;

/** The key's configuration, loaded whenever the credential or project changes. */
export function useKeyConfig(apiKey: string, project: string | null, live: Liveness, onboard: OnboardingSetter) {
  const { config, sink, resetConfig } = useConfigSink(onboard);
  const fetchConfig = useCallback(() => loadKeyConfig(apiKey, project, live, sink), [apiKey, project, live, sink]);
  useEffect(() => void fetchConfig(), [fetchConfig]);
  return { config, fetchConfig, resetConfig };
}
