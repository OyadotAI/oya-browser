/**
 * The server's health, polled for the badge in the header.
 */
import { useEffect, useState } from 'react';
import { apiUrl } from '@/lib/api';
import { HEALTH_POLL_MS } from './constants';

/** The badge's text and colour. */
interface Health {
  /** What the badge says: "healthy", the server's own status, or "offline". */
  status: string;
  /** Whether the server answered "ok". */
  ok: boolean;
}

/** Asks /health once; any failure, including a non-2xx answer, reads as offline. */
async function fetchHealth(): Promise<Health> {
  try {
    const res = await fetch(apiUrl('/health'));
    if (!res.ok) throw new Error();
    const data = await res.json();
    return { status: data.status === 'ok' ? 'healthy' : data.status, ok: data.status === 'ok' };
  } catch {
    return { status: 'offline', ok: false };
  }
}

/** Polls /health now and every HEALTH_POLL_MS while mounted. */
export function useHealth(): Health {
  const [health, setHealth] = useState<Health>({ status: 'connecting...', ok: false });
  useEffect(() => {
    const poll = () => void fetchHealth().then(setHealth);
    poll();
    const interval = setInterval(poll, HEALTH_POLL_MS);
    return () => clearInterval(interval);
  }, []);
  return health;
}
