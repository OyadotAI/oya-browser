/**
 * The toolbar's connection pieces: the update pill and the connection pill
 * (index.html's #update-pill and #conn-pill), and the version line in the
 * shell dialog's footer (#app-version).
 */
import { useViewModel } from '../../../hooks/index.ts';
import type { UpdatesViewModel } from '../view-models/updates-view-model.ts';
import type { ConnectionPillViewModel } from '../view-models/connection-pill-view-model.ts';
import type { ViewProps } from '../model/models.ts';
import './toolbar-pills.css';

/** The update pill: shown while there is something to say; checks, or restarts into a staged update. */
export function UpdatePill({ vm }: ViewProps<UpdatesViewModel>) {
  const s = useViewModel(vm);
  return (
    <button
      className={s.attention ? 'update-pill attention' : 'update-pill'}
      id="update-pill"
      hidden={s.hidden}
      disabled={s.disabled}
      title={s.title}
      onClick={() => void vm.click()}
    >
      {s.text}
    </button>
  );
}

/** The version line in the shell dialog's footer. */
export function AppVersion({ vm }: ViewProps<UpdatesViewModel>) {
  return <span id="app-version">{useViewModel(vm).version}</span>;
}

/** The connection pill: connected, connecting or offline; opens the account page. */
export function ConnectionPill({ vm }: ViewProps<ConnectionPillViewModel>) {
  const s = useViewModel(vm);
  return (
    <button
      className={s.className}
      id="conn-pill"
      title={s.title}
      aria-label="Account and connection"
      onClick={() => vm.click()}
    >
      <span className="dot"></span>
      <span id="conn-label">{s.label}</span>
    </button>
  );
}
