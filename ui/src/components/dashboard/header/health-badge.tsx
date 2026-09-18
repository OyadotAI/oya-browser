/**
 * The pill in the header that says whether the server is up.
 */
'use client';

import { useHealth } from './use-health';

/** Green "healthy", or red with the server's own status or "offline". */
export default function HealthBadge() {
  const { status, ok } = useHealth();
  return (
    <div
      className={`inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium ${
        ok ? 'bg-accent/10 text-accent' : 'bg-red-500/10 text-red-400'
      }`}
    >
      <span className={`w-2 h-2 rounded-full mr-1.5 ${ok ? 'bg-accent' : 'bg-red-400'}`} />
      <span className="hidden sm:inline">{status}</span>
    </div>
  );
}
