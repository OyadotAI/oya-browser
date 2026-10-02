/**
 * The admin page's building blocks: a section with a title, a headline number,
 * a small number tile, a fold for long tables, and a plain table.
 */
import type { ReactNode } from 'react';

/** A titled block of the page: a quiet heading over its content, no box of its own. */
export function Section({
  title,
  hint,
  children,
}: {
  /** Its title. */ title: string;
  /** A short note beside the title. */ hint?: string;
  /** Its content. */ children: ReactNode;
}) {
  return (
    <section className="flex flex-col gap-3">
      <div className="flex items-baseline gap-3 border-b border-border pb-2">
        <h2 className="text-[13px] font-semibold tracking-tight text-text">{title}</h2>
        {hint && <span className="text-[11px] text-text-dim">{hint}</span>}
      </div>
      {children}
    </section>
  );
}

/** A headline number: big, with what it counts, how it moved and one line of detail. */
export function Kpi({
  label,
  value,
  trend,
  sub,
}: {
  /** What it counts. */ label: string;
  /** The number. */ value: ReactNode;
  /** How it moved, when known. */ trend?: ReactNode;
  /** One line of detail under it. */ sub?: ReactNode;
}) {
  return (
    <div className="flex flex-col gap-2 rounded-lg border border-border bg-bg-card p-4">
      <span className="text-[11px] font-medium tracking-[0.08em] text-text-dim uppercase">{label}</span>
      <span className="font-display text-3xl leading-none text-text tabular-nums">{value}</span>
      <div className="flex min-h-4 flex-col gap-0.5 text-xs text-text-muted">
        {trend}
        {sub && <span>{sub}</span>}
      </div>
    </div>
  );
}

/** One small number and what it counts. */
export function Tile({ label, value }: { /** What it counts. */ label: string; /** The number. */ value: ReactNode }) {
  return (
    <div className="flex flex-col gap-1 rounded-md border border-border bg-bg-card px-3 py-2">
      <span className="text-[11px] text-text-dim">{label}</span>
      <span className="text-base font-semibold text-text tabular-nums">{value}</span>
    </div>
  );
}

/** Long content folded away until asked for. */
export function Fold({
  label,
  children,
}: {
  /** What opening it shows. */ label: string;
  /** The content. */ children: ReactNode;
}) {
  return (
    <details className="group rounded-md border border-border">
      <summary className="cursor-pointer list-none px-3 py-2 text-xs text-text-muted select-none hover:text-text">
        <span className="mr-1 inline-block transition-transform group-open:rotate-90">›</span>
        {label}
      </summary>
      <div className="border-t border-border p-2">{children}</div>
    </details>
  );
}

/** A table of rows under headings; numbers sit right-aligned; says so when there are none. */
export function Table({
  head,
  rows,
}: {
  /** Column headings. */ head: string[];
  /** Each row's cells. */ rows: ReactNode[][];
}) {
  if (!rows.length) return <p className="py-2 text-xs text-text-dim">Nothing yet.</p>;
  const numeric = head.map((_, j) => rows.every((r) => typeof r[j] === 'number'));
  const align = (j: number) => (numeric[j] ? 'text-right tabular-nums' : 'text-left');
  return (
    <div className="overflow-x-auto">
      <table className="w-full text-xs">
        <thead className="text-[11px] text-text-dim">
          <tr>
            {head.map((h, j) => (
              <th key={h || j} className={`px-2 py-1.5 font-medium ${align(j)}`}>
                {h}
              </th>
            ))}
          </tr>
        </thead>
        <tbody className="text-text">
          {rows.map((cells, i) => (
            <tr key={i} className="border-t border-border/60 hover:bg-text/[0.03]">
              {cells.map((c, j) => (
                <td key={j} className={`px-2 py-1.5 ${align(j)}`}>
                  {c}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
