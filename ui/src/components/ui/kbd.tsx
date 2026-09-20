/**
 * The key-cap element shortcut hints are drawn with.
 */
import type { PropsWithChildren } from 'react';

/** A key cap, the way shortcut hints are drawn everywhere in the console. */
export default function Kbd({ children }: PropsWithChildren) {
  return (
    <kbd className="inline-flex h-5 min-w-5 items-center justify-center rounded border border-border bg-bg-elevated px-1.5 font-mono text-[11px] font-medium text-text-secondary">
      {children}
    </kbd>
  );
}
