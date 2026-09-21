/**
 * The Oya mark and the "Oya Browser" wordmark that links home.
 */
import Link from 'next/link';

/** Shared Oya identity, from A2ABaseAI/frontend. */
/** The mark's props. */
interface LogoProps {
  /** Width and height in pixels. */
  size?: number;
}

/** The two-ring mark. */
export function OyaLogo({ size = 24 }: LogoProps) {
  return (
    <svg viewBox="0 0 512 512" width={size} height={size} fill="none" aria-hidden="true" className="shrink-0">
      <circle cx="276" cy="276" r="160" stroke="var(--primary)" strokeWidth="80" />
      <circle cx="236" cy="236" r="160" stroke="var(--foreground)" strokeWidth="80" />
    </svg>
  );
}

/** The wordmark's props. */
interface WordmarkProps {
  /** Where the wordmark links. */
  href?: string;
}

/** Mark plus "Browser", linking home. */
export function OyaWordmark({ href = '/' }: WordmarkProps) {
  return (
    <Link href={href} className="inline-flex shrink-0 items-center gap-2.5" aria-label="Oya Browser home">
      <OyaLogo size={26} />
      <span className="font-display text-[15px] tracking-tight">Browser</span>
    </Link>
  );
}
