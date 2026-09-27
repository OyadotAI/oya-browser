/**
 * The admin page's building blocks: a section with a title, a number tile,
 * and a plain table.
 */
import type { ReactNode } from 'react';

/** A titled block of the page. */
export function Section({
  title,
  children,
}: {
  /** Its title. */ title: string;
  /** Its content. */ children: ReactNode;
}) {
  return (
    <section className="flex flex-col gap-3 rounded-md border border-border bg-bg-elevated/40 p-4">
      <h2 className="text-sm font-medium text-text">{title}</h2>
      {children}
    </section>
  );
}

/** One number and what it counts. */
export function Tile({ label, value }: { /** What it counts. */ label: string; /** The number. */ value: ReactNode }) {
  return (
    <div className="flex flex-col gap-1 rounded-md border border-border px-3 py-2">
      <span className="text-[11px] text-text-dim">{label}</span>
      <span className="text-lg font-semibold text-text">{value}</span>
    </div>
  );
}

/** A table of rows under headings; says so when there are none. */
export function Table({
  head,
  rows,
}: {
  /** Column headings. */ head: string[];
  /** Each row's cells. */ rows: ReactNode[][];
}) {
  if (!rows.length) return <p className="text-xs text-text-dim">Nothing yet.</p>;
  return (
    <div className="overflow-x-auto">
      <table className="w-full text-left text-xs">
        <thead className="text-text-dim">
          <tr>
            {head.map((h) => (
              <th key={h} className="px-2 py-1 font-medium">
                {h}
              </th>
            ))}
          </tr>
        </thead>
        <tbody className="text-text">
          {rows.map((cells, i) => (
            <tr key={i} className="border-t border-border">
              {cells.map((c, j) => (
                <td key={j} className="px-2 py-1">
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
