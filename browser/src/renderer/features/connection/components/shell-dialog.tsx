/**
 * The shell dialog (index.html's #shell-overlay): the command palette or the
 * account page, with the footer the root hands in (appearance, page format
 * and the version line). Escape, the close button and a click on the backdrop
 * close it; Tab stays inside.
 */
import type { ReactNode } from 'react';
import { useViewModel } from '../../../hooks/index.ts';
import { Dialog, IconButton } from '../../../ui/index.ts';
import './shell-dialog.css';
import { pageLabels, type ShellDialogViewModel } from '../view-models/shell-dialog-view-model.ts';
import type { AccountViewModel } from '../view-models/account-view-model.ts';
import type { SyncViewModel } from '../view-models/sync-view-model.ts';
import type { ImportViewModel } from '../view-models/import-view-model.ts';
import type { ProfileViewModel } from '../view-models/profile-view-model.ts';
import type { ReconnectViewModel } from '../view-models/reconnect-view-model.ts';
import type { PaletteViewModel } from '../view-models/palette-view-model.ts';
import { CommandPalette } from './command-palette.tsx';
import { ShortcutGuide } from './shortcut-guide.tsx';
import { AccountPage } from './account-page.tsx';

/** The ViewModels of the account page. */
export interface AccountModels {
  /** The dialog itself: open, page, server, browser id, Advanced, Copy. */
  dialog: ShellDialogViewModel;
  /** The account card and its actions. */
  account: AccountViewModel;
  /** Sync. */
  sync: SyncViewModel;
  /** Imported logins. */
  imports: ImportViewModel;
  /** Browsing as, and this device. */
  profile: ProfileViewModel;
  /** Connection settings… */
  reconnect: ReconnectViewModel;
}

/** What the shell dialog is given. */
export interface ShellDialogProps extends AccountModels {
  /** The command palette. */
  palette: PaletteViewModel;
  /** The footer's controls (theme and page format from their features, then <AppVersion>). */
  footer: ReactNode;
}

/** The commands and account dialog. */
export function ShellDialog({ palette, footer, ...models }: ShellDialogProps) {
  const { dialog } = models;
  const s = useViewModel(dialog);
  const profile = s.page === 'profile';
  const offer = useViewModel(models.imports).offer && profile;
  const close = () => {
    if (offer) void models.imports.dismissOffer();
    dialog.close();
  };
  const labels = pageLabels(s.page);
  return (
    <Dialog
      className="shell-overlay"
      id="shell-overlay"
      hidden={!s.open}
      label={labels.label}
      open={s.open}
      onClose={close}
      focusId={s.page === 'shortcuts' ? 'shortcut-search' : profile ? 'shell-dialog-close' : 'command-search'}
    >
      <div className="shell-dialog">
        <header>
          <h2 id="shell-dialog-title">{offer ? 'Import your logins' : labels.title}</h2>
          <IconButton
            className="nav-btn"
            id="shell-dialog-close"
            aria-label="Close dialog"
            data-icon="close"
            onClick={close}
            icon="close"
          />
        </header>
        <CommandPalette vm={palette} hidden={s.page !== 'commands'} />
        {s.open && s.page === 'shortcuts' && <ShortcutGuide platform={palette.platform} />}
        <AccountPage {...models} hidden={!profile} />
        {!offer && <footer>{footer}</footer>}
      </div>
    </Dialog>
  );
}
