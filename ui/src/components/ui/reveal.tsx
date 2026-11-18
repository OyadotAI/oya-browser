/** Small client islands enhance server-rendered sections without hiding content before hydration. */
'use client';
import { useEffect, useRef, type ReactNode } from 'react';
import { attachReveal } from './motion';
import type { RevealPreset } from './constants';

/** The wrapper owns only finite visual motion; its children retain their server boundaries and state. */
export function Reveal({
  children,
  preset = 'section',
  className,
  order = 0,
}: {
  /** Complete server-rendered content. */ children: ReactNode;
  /** An approved cadence for the region. */ preset?: RevealPreset;
  /** Existing layout styles stay with the caller. */ className?: string;
  /** Small stagger within a related visual sequence. */ order?: number;
}) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => (ref.current ? attachReveal(ref.current, preset, order) : undefined), [preset, order]);
  return (
    <div ref={ref} className={className} data-reveal={preset}>
      {children}
    </div>
  );
}
