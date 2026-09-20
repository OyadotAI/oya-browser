/**
 * Loads the key's settings when the editor opens, with a retry after a failure.
 */
import { useEffect, useState } from 'react';
import { errorMessage } from '@/lib/api-client';
import { loadConfig, type KeyConfig } from '../config';

/** Where a load reports to. */
interface LoadTarget {
  /** Receives the loaded settings. */
  setConfig: (config: KeyConfig) => void;
  /** Receives the failure, or '' once loaded. */
  setError: (error: string) => void;
}

/** Loads settings into `target`; the returned cleanup ignores an answer that arrives too late. */
function loadInto(apiKey: string, target: LoadTarget) {
  let cancelled = false;
  loadConfig(apiKey)
    .then((data) => void (!cancelled && (target.setConfig(data), target.setError(''))))
    .catch((err) => void (!cancelled && target.setError(errorMessage(err, 'Could not load settings.'))));
  return () => void (cancelled = true);
}

/** The saved settings (null while loading), the current error, and `retry`. */
export function useKeyConfig(apiKey: string) {
  const [config, setConfig] = useState<KeyConfig | null>(null);
  const [error, setError] = useState('');
  const [attempt, setAttempt] = useState(0);
  useEffect(() => loadInto(apiKey, { setConfig, setError }), [apiKey, attempt]);
  return { config, error, setError, retry: () => setAttempt((a) => a + 1) };
}
