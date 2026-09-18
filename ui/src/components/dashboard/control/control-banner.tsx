/**
 * The line under the Control tabs: a load or action error, or a success
 * notice. Errors win.
 */
import { AlertTriangle, Circle } from 'lucide-react';

/** An error to report, or a notice to show when there is none. */
export default function ControlBanner({
  error,
  notice,
}: {
  /** Error to report, or empty. */ error: string;
  /** Success message, or empty. */ notice: string;
}) {
  if (!error && !notice) return null;
  return (
    <div
      role={error ? 'alert' : 'status'}
      className={`px-4 lg:px-6 py-2 text-xs flex items-center gap-2 shrink-0 ${error ? 'text-red' : 'text-accent'}`}
    >
      {error ? <AlertTriangle className="w-3.5 h-3.5" /> : <Circle className="w-3 h-3 fill-current" />}
      {error || notice}
    </div>
  );
}
