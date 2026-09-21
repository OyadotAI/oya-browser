/**
 * The console's view state: which tab, which browsers are selected or
 * checked, the filter, and which dialogs are open. One object, changed by
 * patches.
 */
import { Monitor, Users, Activity, Workflow } from 'lucide-react';
import type { FleetFilter } from '@/components/dashboard/fleet-strip';

/** The console's tabs. */
export type MainTab = 'browsers' | 'personas' | 'playbooks' | 'control';

/** A tab's key, label and icon. */
export interface TabSpec {
  /** Which tab. */
  key: MainTab;
  /** Its label. */
  label: string;
  /** Its icon. */
  icon: typeof Monitor;
}

/** The tabs in order. */
export const TABS: TabSpec[] = [
  { key: 'browsers', label: 'Browsers', icon: Monitor },
  { key: 'personas', label: 'Profiles', icon: Users },
  { key: 'playbooks', label: 'Playbooks', icon: Workflow },
  { key: 'control', label: 'Control', icon: Activity },
];

/** The unfiltered fleet. */
export const NO_FILTER: FleetFilter = { health: null, provider: null, persona: null, text: '' };

/** A screenshot on show. */
export interface Shot {
  /** The browser it was taken from. */
  id: string;
  /** The image as a data URL. */
  src: string;
}

/** Everything the console shows that is not fetched data. */
export interface ConsoleView {
  /** The open tab. */
  tab: MainTab;
  /** The browser open in the side panel. */
  selected: string | null;
  /** Browsers ticked for a bulk action. */
  checked: Set<string>;
  /** What the table is narrowed to. */
  filter: FleetFilter;
  /** The persona open in the drawer. */
  openPersona: string | null;
  /** Whether the setup wizard is showing. */
  showOnboarding: boolean;
  /** Whether settings are open. */
  showSettings: boolean;
  /** The settings section to open at. */
  settingsSection: 'alerts' | undefined;
  /** Whether the start-a-browser dialog is open. */
  showStart: boolean;
  /** Whether the shortcut sheet is open. */
  showHelp: boolean;
  /** Browsers awaiting stop confirmation. */
  stopIds: string[] | null;
  /** Whether a stop is running. */
  stopping: boolean;
  /** Whether the desktop banner was dismissed. */
  bannerDismissed: boolean;
  /** The browser whose connect snippets are open. */
  connectId: string | null;
  /** Whether the fleet code snippets are open. */
  showCode: boolean;
  /** A screenshot being shown. */
  shot: Shot | null;
}

/** The view on first load. */
export const INITIAL_VIEW: ConsoleView = {
  tab: 'browsers',
  selected: null,
  checked: new Set(),
  filter: NO_FILTER,
  openPersona: null,
  showOnboarding: false,
  showSettings: false,
  settingsSection: undefined,
  showStart: false,
  showHelp: false,
  stopIds: null,
  stopping: false,
  bannerDismissed: false,
  connectId: null,
  showCode: false,
  shot: null,
};

/** What opening another project resets; the tab and open dialogs stay. */
export const projectReset = (): Partial<ConsoleView> => ({
  selected: null,
  checked: new Set(),
  filter: NO_FILTER,
  openPersona: null,
  stopIds: null,
  connectId: null,
  bannerDismissed: false,
});
