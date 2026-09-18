/**
 * The building blocks the docs sections are written in: headings, inline
 * code, code blocks with copy, note and warning boxes, links and tables.
 */
'use client';

import { useState, type PropsWithChildren, type ReactNode } from 'react';
import Link from 'next/link';
import { Copy, Check } from 'lucide-react';
import SyntaxCode from '@/components/ui/syntax-code';
import { COPIED_RESET_MS } from './constants';

/** A section heading's props. */
interface HeadingProps extends PropsWithChildren {
  /** The anchor, sidebar and search target. */
  id: string;
  /** The first heading has no rule above it. */
  first?: boolean;
}

/** A section's h2, with the anchor the sidebar and search jump to. */
export function SectionHeading({ id, children, first }: HeadingProps) {
  return (
    <h2
      id={id}
      className={`text-[26px] font-medium tracking-tight text-text mb-5 scroll-mt-28 ${
        first ? 'mt-0' : 'mt-16 pt-10 border-t border-border'
      }`}
    >
      {children}
    </h2>
  );
}

/** Code inside a sentence. */
export function InlineCode({ children }: PropsWithChildren) {
  return <code className="bg-bg-elevated px-1.5 py-0.5 rounded text-sm font-mono text-text-muted">{children}</code>;
}

/** A code block's props. */
interface CodeBlockProps {
  /** The code. */
  children: string;
}

/** A highlighted example with a copy button, and a manual fallback when the clipboard is refused. */
export function CodeBlock({ children }: CodeBlockProps) {
  const [copied, setCopied] = useState(false);
  const [error, setError] = useState(false);
  /** Copies the code; "Copied" shows for a moment. */
  async function copy() {
    try {
      await navigator.clipboard.writeText(children);
      setCopied(true);
      setTimeout(() => setCopied(false), COPIED_RESET_MS);
    } catch {
      setError(true);
    }
  }
  return (
    <div className="my-5 min-w-0 overflow-hidden rounded-xl border border-border bg-bg-card">
      <div className="flex items-center justify-between border-b border-border px-4 py-2">
        <span className="font-mono text-[10px] text-text-dim">Example</span>
        <button onClick={copy} className="btn-ghost h-7 text-[11px]" aria-label="Copy code">
          {copied ? <Check size={12} /> : <Copy size={12} />} {copied ? 'Copied' : 'Copy'}
        </button>
      </div>
      <pre className="overflow-x-auto p-5 text-[12px] font-mono leading-[1.9]">
        <SyntaxCode code={children} />
      </pre>
      {error && (
        <p role="status" className="px-5 pb-3 text-xs text-text-muted">
          Select the code to copy it.
        </p>
      )}
    </div>
  );
}

/** A highlighted aside. */
export function NoteBox({ children }: PropsWithChildren) {
  return (
    <div className="bg-indigo/10 border border-indigo/20 rounded-xl p-4 text-sm text-indigo mb-4 leading-relaxed">
      {children}
    </div>
  );
}

/** A caution. */
export function WarnBox({ children }: PropsWithChildren) {
  return (
    <div className="bg-yellow/10 border border-yellow/20 rounded-xl p-4 text-sm text-yellow mb-4 leading-relaxed">
      {children}
    </div>
  );
}

/** An inline link's props. */
interface InlineLinkProps extends PropsWithChildren {
  /** Where it goes. */
  href: string;
}

/** A link inside a sentence. */
export function InlineLink({ href, children }: InlineLinkProps) {
  return (
    <Link href={href} className="text-accent hover:text-accent-hover transition-colors">
      {children}
    </Link>
  );
}

/** An in-page link's props. */
interface InlineAnchorProps extends PropsWithChildren {
  /** Goes to the target section. */
  onClick: () => void;
}

/** A link inside a sentence that jumps to another section. */
export function InlineAnchor({ onClick, children }: InlineAnchorProps) {
  return (
    <button onClick={onClick} className="text-accent hover:text-accent-hover transition-colors cursor-pointer">
      {children}
    </button>
  );
}

/** A table's props. */
interface TableProps {
  /** Column headings. */
  headers: string[];
  /** Cells, row by row. */
  rows: ReactNode[][];
}

/** A reference table. */
export function Table({ headers, rows }: TableProps) {
  return (
    <div className="overflow-x-auto mb-4">
      <table className="w-full text-sm">
        <thead>
          <tr>
            {headers.map((h, i) => (
              <th
                key={i}
                className="text-left text-xs font-semibold uppercase tracking-wider text-text-dim py-2 px-3 border-b border-border"
              >
                {h}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((row, ri) => (
            <tr key={ri} className="hover:bg-bg-card/50 transition-colors">
              {row.map((cell, ci) => (
                <td key={ci} className="py-2 px-3 border-b border-border text-[14px] text-text-muted">
                  {cell}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
