/** Complete operational tables scroll locally so phones retain every column without widening the page. */
import type { ReactNode } from 'react';

/** Gives keyboard users a named entry point to a horizontally scrolling data surface. */
export function DataRegion({
  label,
  children,
}: {
  /** The data this region contains. */ label: string;
  /** The unchanged table and its loading or empty states. */ children: ReactNode;
}) {
  return (
    <div
      role="region"
      aria-label={label}
      tabIndex={0}
      className="data-scroll mx-4 mb-4 min-h-0 flex-initial overflow-auto rounded-xl border border-border bg-bg-card/25 lg:mx-6 lg:mb-6"
    >
      {children}
    </div>
  );
}
