/** Shared authentication framing puts the task first on phones and the product context beside it on larger screens. */
import type { ReactNode } from 'react';
import { Loader2 } from 'lucide-react';
import { OyaWordmark } from '@/components/oya-logo';
import styles from './auth-shell.module.css';

/** Existing callers can retain their accent choice without affecting authentication state. */
interface ShellProps {
  /** Selects the page's existing accent family. */
  glow: 'accent' | 'indigo';
  /** The authentication form and its feedback. */
  children: ReactNode;
  /** The alternate authentication destination. */
  footer: ReactNode;
  /** Optional context placed after the form on phones. */
  pitch?: ReactNode;
}

/** A solid panel keeps validation and credentials readable in either saved theme. */
export function AuthShell({ glow, children, footer, pitch }: ShellProps) {
  return (
    <div className={styles.shell} data-accent={glow}>
      <div className={styles.frame} data-wide={Boolean(pitch)}>
        <div className={styles.brand}>
          <OyaWordmark />
        </div>
        <div className={styles.columns}>
          <div className={styles.form}>
            <div className={styles.card}>{children}</div>
            <p className={styles.footer}>{footer}</p>
          </div>
          {pitch && <div className={styles.pitch}>{pitch}</div>}
        </div>
      </div>
    </div>
  );
}

/** Session resolution uses the same frame and exposes its loading state to assistive technology. */
export function AuthLoading() {
  return (
    <AuthShell glow="accent" footer={null}>
      <div className={styles.loading} role="status">
        <Loader2 className="h-6 w-6 animate-spin" />
        <span>Loading your session…</span>
      </div>
    </AuthShell>
  );
}
