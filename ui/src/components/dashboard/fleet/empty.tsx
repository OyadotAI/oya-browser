/**
 * What the table shows when no browsers are running at all.
 */
import { Monitor } from 'lucide-react';

/** The one action the empty state offers. */
interface EmptyProps {
  /** Opens the start-browser dialog. */
  onStart: () => void;
}

/** An invitation to start the first browser. */
export default function Empty({ onStart }: EmptyProps) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-3 py-24 text-center">
      <div className="flex h-12 w-12 items-center justify-center rounded-xl border border-border bg-bg-card">
        <Monitor className="h-5 w-5 text-text-muted" />
      </div>
      <div>
        <p className="text-[15px] font-medium text-text">No browsers running</p>
        <p className="mt-1 text-[13px] text-text-muted">
          Start one here, from the CLI with <code className="font-mono">oya start</code>, or from the SDK.
        </p>
      </div>
      <button className="btn-primary mt-1" onClick={onStart}>
        Start a browser
      </button>
    </div>
  );
}
