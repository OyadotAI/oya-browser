/**
 * The shell page: every region in the order the page lays them out, each a
 * feature's view over the ViewModels the composition root built. Its imports
 * also order the stylesheets: the design system's, the panel's, then each
 * feature's, after the page-wide ones app/main.tsx loads first.
 */
import { NotificationsButton, NotificationsCenter } from '../features/notifications/index.ts';
import { ProfilesCard } from '../features/profiles/index.ts';
import { BrowserFooter } from './browser-footer.tsx';
import { useViewModel } from '../hooks/index.ts';
import { IconButton } from '../ui/index.ts';
import type { ShellViewModels } from './view-models.ts';
import { useRootEffects } from './use-root-effects.ts';
import { PanelFrame } from './panel/panel-frame.tsx';
import {
  SetupScreen,
  ReconnectOverlay,
  ShellDialog,
  UpdatePill,
  ProfileButton,
  AppVersion,
} from '../features/connection/index.ts';
import { ControlBar, blockedActions } from '../features/control/index.ts';
import { BrandMark, ThemeSelect, PageBackdrop, Launch, useChromeRoot } from '../features/chrome/index.ts';
import { TabBar, TabCard } from '../features/tabs/index.ts';
import { BookmarksBar, FindBar, ShareChipButton, Toolbar } from '../features/toolbar/index.ts';
import { LibraryPage } from '../features/library/index.tsx';
import { StartPage } from '../features/start/index.ts';
import { PlaybooksPane } from '../features/playbooks/index.ts';
import { AskPane } from '../features/ask/index.ts';
import { RecordPane } from '../features/studio/index.ts';
import { RoutinesPane } from '../features/routines/index.ts';
import { ActionsPane, NetworkPane, SourcePane } from '../features/inspect/index.ts';
import { PageFormatSelect } from './page-format-select.tsx';

/** What the root is given. */
export interface ShellRootProps {
  /** Every ViewModel. */
  vms: ShellViewModels;
}

/** The commands button at the end of the tab bar: opens the command palette. */
function CommandsButton({ vms }: ShellRootProps) {
  return (
    <IconButton
      className="nav-btn"
      id="btn-commands"
      aria-label="Commands and settings"
      title="Commands (⌘/Ctrl K)"
      data-icon="menu"
      onClick={() => void vms.connection.dialog.open()}
      icon="menu"
    />
  );
}

/** The toolbar's end, as in Chrome: notifications, then the profile with its connection dot. */
function ToolbarEnd({ vms }: ShellRootProps) {
  return (
    <>
      <NotificationsButton vm={vms.notifications} />
      <ProfileButton
        pill={vms.connection.pill}
        account={vms.connection.account}
        onOpen={() => vms.connection.profiles.toggle()}
      />
    </>
  );
}

/** The tab bar, the toolbar under it, the bookmarks bar under that, and the find bar when it is open. */
function BrowserChrome({ vms }: ShellRootProps) {
  const { tabs, chrome, connection: c } = vms;
  const brand = <BrandMark vm={chrome.activity} />;
  return (
    <>
      <TabBar
        strip={tabs.strip}
        drag={tabs.drag}
        card={tabs.card}
        brand={brand}
        commands={<CommandsButton vms={vms} />}
      />
      <TabCard vm={tabs.card} />
      <Toolbar vm={vms.toolbar} panel={vms.panel} shell={vms.shell} trailing={<ToolbarEnd vms={vms} />}>
        <ShareChipButton vm={vms.toolbar.share} />
        <UpdatePill vm={c.updates} />
        <ControlBar vm={vms.control} />
      </Toolbar>
      <BookmarksBar vm={vms.toolbar.bookmarks} />
      <FindBar vm={vms.toolbar.find} platform={vms.env.platform} />
    </>
  );
}

/** The workspace panel and its panes. */
function Workspace({ vms }: ShellRootProps) {
  // The panel is the shown tab's agent: its own conversation, run and orb.
  const { current: ask } = useViewModel(vms.ask);
  const { orb } = useViewModel(ask.run);
  const { pane } = useViewModel(vms.panel);
  const recordBlocked = useRecordBlocked(vms);
  return (
    <PanelFrame panel={vms.panel} orb={orb}>
      <AskPane
        key={ask.tabId ?? 'none'}
        ask={ask}
        panel={vms.panel}
        onViewPlaybook={(name) => void vms.playbooks.open(name)}
      />
      <PlaybooksPane vm={vms.playbooks} panel={vms.panel} />
      <RoutinesPane routines={vms.routines} panel={vms.panel} />
      <ActionsPane actions={vms.inspect.actions} panel={vms.panel} />
      <NetworkPane log={vms.inspect.netLog} panel={vms.panel} />
      <RecordPane
        vm={vms.studio}
        active={pane === 'record'}
        controlBlocked={recordBlocked}
        onViewPlaybook={(name) => void vms.playbooks.open(name)}
      />
      <SourcePane source={vms.inspect.source} panel={vms.panel} />
    </PanelFrame>
  );
}

/** Whether Start recording is refused right now: watch-only, and control cannot be taken. */
function useRecordBlocked(vms: ShellViewModels): boolean {
  const state = useViewModel(vms.control);
  return !!state.control && blockedActions(state).recordBlocked;
}

/** The commands and account dialog, with the appearance and format settings in its footer. */
function Dialogs({ vms }: ShellRootProps) {
  const c = vms.connection;
  const preferences = (
    <>
      <div className="settings-preference">
        <ThemeSelect vm={vms.chrome.theme} />
      </div>
      <div className="settings-preference">
        <PageFormatSelect source={vms.inspect.source} />
        <p>How your agents read web pages.</p>
      </div>
    </>
  );
  const footer = <AppVersion vm={c.updates} />;
  const models = { dialog: c.dialog, account: c.account, sync: c.sync, imports: c.imports, profile: c.profile };
  return (
    <>
      <NotificationsCenter vm={vms.notifications} />
      <ProfilesCard vm={vms.connection.profiles} account={vms.connection.account} />
      <ReconnectOverlay vm={c.reconnect} />
      <ShellDialog palette={c.palette} reconnect={c.reconnect} footer={footer} preferences={preferences} {...models} />
    </>
  );
}

/** The whole shell page. */
export function ShellRoot({ vms }: ShellRootProps) {
  useRootEffects(vms.shell, vms.panel);
  useChromeRoot({ theme: vms.chrome.theme, activity: vms.chrome.activity, shell: vms.shell });
  return (
    <>
      <SetupScreen vm={vms.connection.setup} checklist={vms.connection.checklist} />
      <BrowserChrome vms={vms} />
      <PageBackdrop vm={vms.chrome.backdrop} />
      <StartPage vm={vms.start} />
      <LibraryPage vms={vms} />
      <Launch vm={vms.chrome.launch} />
      <Workspace vms={vms} />
      <BrowserFooter vms={vms} />
      <Dialogs vms={vms} />
    </>
  );
}
