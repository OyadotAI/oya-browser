/**
 * The toolbar's right-hand actions: stop, code, pair a desktop, start a browser.
 */
import { Code2, MonitorSmartphone, Square } from 'lucide-react';
import Kbd from '@/components/ui/kbd';
import type { BrowserRow } from '../types';
import { useDesktopPairing } from './use-desktop-pairing';

/** What the actions act on and call. */
export interface ToolbarActionsProps {
  /** Every browser, filtered or not: "Stop all" stops them all. */
  rows: BrowserRow[];
  /** Checked browser ids. */
  checked: Set<string>;
  /** Stops the given browsers. */
  onStop: (ids: string[]) => void;
  /** Opens the start-browser dialog. */
  onStart: () => void;
  /** Opens the code snippets for the fleet. */
  onCode: () => void;
  /** Key the desktop pairing code is minted for. */
  apiKey: string;
}

/** Stop checked (or all), Code, Connect desktop browser, Start browser. */
export default function ToolbarActions({ rows, checked, onStop, onStart, onCode, apiKey }: ToolbarActionsProps) {
  const { pairing, connectDesktop } = useDesktopPairing(apiKey);
  return (
    <div className="ml-auto flex shrink-0 flex-wrap items-center gap-2">
      {checked.size > 0 && (
        <button className="btn-danger h-9" onClick={() => onStop([...checked])}>
          <Square className="h-3 w-3" /> Stop {checked.size}
        </button>
      )}
      {rows.length > 0 && checked.size === 0 && (
        <button className="btn-ghost h-9 text-[12px]" onClick={() => onStop(rows.map((r) => r.id))}>
          Stop all
        </button>
      )}
      <button className="btn-ghost h-9" onClick={onCode} title="Code that starts browsers here">
        <Code2 className="h-3.5 w-3.5" /> Code
      </button>
      <button
        className="btn-ghost h-9"
        onClick={connectDesktop}
        disabled={pairing}
        title="Pair the browser on this machine, so cloud browsers inherit its logins"
      >
        <MonitorSmartphone className="h-3.5 w-3.5" /> Connect desktop browser
      </button>
      <button className="btn-primary h-9" onClick={onStart}>
        Start browser <Kbd>N</Kbd>
      </button>
    </div>
  );
}
