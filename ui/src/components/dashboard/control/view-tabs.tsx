/**
 * The Control tab's header strip: one tab per view, and a manual refresh.
 */
import { RefreshCw } from 'lucide-react';
import { VIEWS } from './constants';
import type { View } from './types';

/** Tab strip props. */
interface Props {
  /** The view on screen. */
  view: View;
  /** Switches view. */
  onView: (view: View) => void;
  /** Reloads the data now. */
  onRefresh: () => void;
}

/** Tabs for each Control view, with Refresh at the far end. */
export default function ViewTabs({ view, onView, onRefresh }: Props) {
  return (
    <div
      className="flex items-center gap-1 px-4 lg:px-6 py-3 border-b border-border overflow-x-auto shrink-0"
      role="tablist"
      aria-label="Control views"
    >
      {VIEWS.map((v) => (
        <button
          key={v.key}
          role="tab"
          aria-selected={view === v.key}
          onClick={() => onView(v.key)}
          className={`flex items-center gap-1.5 px-3 py-2 rounded-md text-[12px] font-medium whitespace-nowrap transition-colors ${
            view === v.key ? 'bg-bg-elevated text-text' : 'text-text-dim hover:text-text-muted'
          }`}
        >
          <v.icon className="w-3.5 h-3.5" />
          {v.label}
        </button>
      ))}
      <button
        onClick={onRefresh}
        className="ml-auto flex items-center gap-1.5 px-2.5 py-1.5 rounded text-xs text-text-dim hover:text-text"
      >
        <RefreshCw className="w-3.5 h-3.5" /> Refresh
      </button>
    </div>
  );
}
