/**
 * The console's error boundary: a render error shows a message and a retry
 * instead of a blank page.
 */
'use client';

/** The id Next.js gives a server error, for matching it in the logs. */
interface Digest {
  /** Hash of the server-side error. */
  digest?: string;
}

/** What Next.js passes an error boundary. */
interface Props {
  /** The error that was thrown while rendering. */
  error: Error & Digest;
  /** Renders the segment again. */
  reset: () => void;
}

/** A render error in the console shows this instead of a blank page; the session is untouched. */
export default function DashboardError({ error, reset }: Props) {
  return (
    <div role="alert" className="flex min-h-[60vh] flex-col items-center justify-center gap-3 px-6 text-center">
      <p className="text-sm text-text">The console hit an error: {error.message || 'something went wrong'}.</p>
      <button className="btn-ghost" onClick={reset}>
        Try again
      </button>
    </div>
  );
}
