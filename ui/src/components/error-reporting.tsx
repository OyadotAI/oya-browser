/**
 * Starts browser error reporting once the page is up. The layout renders this
 * only when the operator set SENTRY_DSN.
 */
'use client';

import { useEffect } from 'react';
import { startSentry } from '@/lib/sentry';

/** What the server handed the page. */
type Props = {
  /** The Sentry DSN. */
  dsn: string;
};

/** Renders nothing; starts Sentry once. */
export function ErrorReporting({ dsn }: Props) {
  useEffect(() => {
    void startSentry(dsn);
  }, [dsn]);
  return null;
}
