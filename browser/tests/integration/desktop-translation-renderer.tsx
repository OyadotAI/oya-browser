/** Production toolbar and consent UI over a native fixture bridge, with unrelated shell services stubbed. */
/* global window, document */
import { createRoot } from 'react-dom/client';
import { Toolbar, type ToolbarProps } from '../../src/renderer/features/toolbar/components/toolbar.tsx';
import type { TabSummary } from '../../src/shared/ipc.ts';
import { ToolbarViewModel } from '../../src/renderer/features/toolbar/view-models/toolbar-view-model.ts';
import { ViewModel } from '../../src/renderer/core/view-model.ts';
import { fakeBridge } from '../unit/renderer/support/bridge.ts';
import '../../src/renderer/core/tokens.css';
import '../../src/renderer/app/shell.css';
import '../../src/renderer/features/connection/components/shell-dialog.css';

/** The fixture preload's deliberately restricted native surface. */
interface FixtureWindow extends Window {
  /** Main waits until all native-event listeners are installed. */
  translationReady: boolean;
  /** Isolated preload methods only for this owned fixture window. */
  translationFixture: {
    call(name: string, args: unknown[]): Promise<unknown>;
    onTabs(listener: (tabs: TabSummary[]) => void): void;
    onBackdrop(listener: (payload: Backdrop | null) => void): void;
  };
}
/** Frozen native page supplied by the production overlay service. */
interface Backdrop {
  /** Locally captured image; never sent outside the fixture. */
  image: string;
  /** Acknowledges this exact painted backdrop. */
  token: string;
}
const fixture = (window as unknown as FixtureWindow).translationFixture;
const overrides = Object.fromEntries(
  ['translatePage', 'showOriginal', 'confirmTranslation', 'showOverlay', 'hideOverlay'].map((name) => [
    name,
    (...args: unknown[]) => fixture.call(name, args),
  ]),
);
const fake = fakeBridge({ ...overrides, addressSuggestions: [], getStatus: {}, bookmarksBar: { shown: true } });
const vm = new ToolbarViewModel(fake.bridge, {
  openAsk() {},
  openModels() {},
  openClaim() {},
  focusAddress: () => vm.focusAddress(),
});
const panel = new ViewModel({ open: false });
const shell = new ViewModel({ recording: false });
fixture.onTabs((tabs) => {
  fake.emit('onUrlChanged', tabs[0].url);
  fake.emit('onTabsUpdated', tabs);
});
fixture.onBackdrop((payload) => {
  const backdrop = document.getElementById('backdrop')!;
  backdrop.style.backgroundImage = payload ? `url(${payload.image})` : '';
  if (payload) void fixture.call('backdropReady', [payload.token]);
});
createRoot(document.getElementById('root')!).render(
  <Toolbar
    vm={vm}
    panel={panel as unknown as ToolbarProps['panel']}
    shell={shell as unknown as ToolbarProps['shell']}
  />,
);
(window as unknown as FixtureWindow).translationReady = true;
