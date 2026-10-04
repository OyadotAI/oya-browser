/**
 * Builds the connection feature's ViewModels over one fake bridge, wired as
 * the composition root will wire them, for the tests in this folder.
 */
import { fakeBridge, manualFrames } from '../../support/bridge.ts';
import { ShellViewModel } from '../../../../../src/renderer/app/shell-view-model.ts';
import { PanelViewModel } from '../../../../../src/renderer/app/panel/panel-view-model.ts';
import { AccountViewModel } from '../../../../../src/renderer/features/connection/view-models/account-view-model.ts';
import { SyncViewModel } from '../../../../../src/renderer/features/connection/view-models/sync-view-model.ts';
import { ImportViewModel } from '../../../../../src/renderer/features/connection/view-models/import-view-model.ts';
import { ProfileViewModel } from '../../../../../src/renderer/features/connection/view-models/profile-view-model.ts';
import { ShellDialogViewModel } from '../../../../../src/renderer/features/connection/view-models/shell-dialog-view-model.ts';
import { SetupViewModel } from '../../../../../src/renderer/features/connection/view-models/setup-view-model.ts';
import { ReconnectViewModel } from '../../../../../src/renderer/features/connection/view-models/reconnect-view-model.ts';
import { UpdatesViewModel } from '../../../../../src/renderer/features/connection/view-models/updates-view-model.ts';
import { PaletteViewModel } from '../../../../../src/renderer/features/connection/view-models/palette-view-model.ts';
import { ConnectionPillViewModel } from '../../../../../src/renderer/features/connection/view-models/connection-pill-view-model.ts';

/** Lets pending promises settle (a few rounds, for chained awaits). */
export async function settle(rounds = 5): Promise<void> {
  for (let i = 0; i < rounds; i++) await new Promise((resolve) => setImmediate(resolve));
}

/** Every ViewModel of the feature over a fake bridge answering from `answers`; `host` records the palette's calls. */
export function build(answers: Record<string, unknown> = {}, clipboard = { writeText: async (_text: string) => {} }) {
  const fake = fakeBridge({ getConfig: {}, getStatus: { connected: false }, importSources: [], ...answers });
  const { bridge } = fake;
  const shell = new ShellViewModel(bridge);
  const panel = new PanelViewModel(bridge, manualFrames());
  const parts = {
    account: new AccountViewModel(bridge),
    sync: new SyncViewModel(bridge),
    imports: new ImportViewModel(bridge),
    profile: new ProfileViewModel(bridge),
  };
  const dialog = new ShellDialogViewModel({ bridge, clipboard, ...parts });
  const setup = new SetupViewModel({ bridge, shell });
  const reconnect = new ReconnectViewModel({ bridge, shell, dialog, setup });
  const updates = new UpdatesViewModel(bridge);
  const host: string[] = [];
  const hostApi = {
    focusAddress: () => void host.push('address'),
    focusChat: () => void host.push('chat'),
    recordButton: () => void host.push('record'),
  };
  const palette = new PaletteViewModel({ bridge, panel, shell, dialog, updates, host: hostApi, platform: 'MacIntel' });
  const pill = new ConnectionPillViewModel({ bridge, shell, account: dialog });
  return { fake, shell, panel, ...parts, dialog, setup, reconnect, updates, palette, pill, host };
}
