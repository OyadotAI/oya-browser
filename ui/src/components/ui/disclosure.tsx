/** Native details keep optional depth accessible while a brief body entrance answers the opening action. */
'use client';
import { useEffect, useRef, type ReactNode } from 'react';
import { ChevronDown } from 'lucide-react';
import { attachDisclosure } from './motion';

/** Native summary owns focus and expanded state; animation never delays its toggle. */
export function Disclosure({
  summary,
  children,
  className = '',
}: {
  /** The explicit purpose of the optional detail. */ summary: string;
  /** Complete detail is included in server HTML. */ children: ReactNode;
  /** Domain styles can refine the common disclosure. */ className?: string;
}) {
  const ref = useRef<HTMLDetailsElement>(null),
    body = useRef<HTMLDivElement>(null);
  useEffect(() => (ref.current && body.current ? attachDisclosure(ref.current, body.current) : undefined), []);
  return (
    <details ref={ref} className={`motion-disclosure ${className}`}>
      <summary>
        {summary}
        <ChevronDown size={16} aria-hidden="true" />
      </summary>
      <div ref={body} className="motion-disclosure-body">
        {children}
      </div>
    </details>
  );
}
