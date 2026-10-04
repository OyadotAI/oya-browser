/**
 * The connection feature's entry: the welcome screen, the reconnect dialog,
 * the shell dialog (command palette and account page), the toolbar's pills
 * and version line, and the ViewModels the composition root builds for them.
 */
export { SetupScreen } from './components/setup-screen.tsx';
export { ReconnectOverlay } from './components/reconnect-overlay.tsx';
export { ShellDialog, type ShellDialogProps, type AccountModels } from './components/shell-dialog.tsx';
export { UpdatePill, ConnectionPill, AppVersion } from './components/toolbar-pills.tsx';
export { AccountViewModel } from './view-models/account-view-model.ts';
export { SyncViewModel } from './view-models/sync-view-model.ts';
export { ImportViewModel } from './view-models/import-view-model.ts';
export { ProfileViewModel } from './view-models/profile-view-model.ts';
export { ShellDialogViewModel } from './view-models/shell-dialog-view-model.ts';
export { SetupViewModel } from './view-models/setup-view-model.ts';
export { ReconnectViewModel } from './view-models/reconnect-view-model.ts';
export { UpdatesViewModel } from './view-models/updates-view-model.ts';
export { PaletteViewModel } from './view-models/palette-view-model.ts';
export { ConnectionPillViewModel } from './view-models/connection-pill-view-model.ts';
