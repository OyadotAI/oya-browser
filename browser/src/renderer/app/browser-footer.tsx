/** Persistent keyboard discovery, outside the native page and workspace bounds. */
import { commandShortcut } from '../../shared/shortcuts.ts';
import type { ShellRootProps } from './shell-root.tsx';
import './browser-footer.css';

/** Both entry points are native buttons: Tab to reach, Enter or Space to open. */
export function BrowserFooter({ vms }: ShellRootProps) {
  const { dialog, palette } = vms.connection;
  return (
    <footer className="browser-footer" aria-label="Browser keyboard tools">
      <button type="button" onClick={() => void dialog.open()}>
        Commands <kbd>{commandShortcut('commands', palette.platform)}</kbd>
      </button>
      <button type="button" onClick={() => void dialog.openShortcuts()}>
        Keyboard shortcuts <kbd>{commandShortcut('shortcuts', palette.platform)}</kbd>
      </button>
    </footer>
  );
}
