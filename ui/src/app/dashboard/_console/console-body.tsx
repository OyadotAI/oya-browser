/**
 * The console below the header: banners, tabs, the fleet strip, the open
 * tab's content and the selected browser's panel.
 */
'use client';

import { useEffect, useRef } from 'react';
import { HelpCircle } from 'lucide-react';
import FleetStrip from '@/components/dashboard/fleet-strip';
import FleetTable from '@/components/dashboard/fleet-table';
import BrowserPanel from '@/components/dashboard/browser-panel';
import PersonasTab from '@/components/dashboard/personas-tab';
import ControlTab from '@/components/dashboard/control-tab';
import PlaybooksTab from '@/components/dashboard/playbooks-tab';
import RoutinesTab from '@/components/dashboard/routines-tab';
import DesktopBanner from '@/components/dashboard/desktop-banner';
import { isOyaProvider } from '@/components/dashboard/config';
import type { Console } from './use-console';
import { TABS } from './view';

/** Every part gets the whole console. */
interface Props {
  /** The console's state and actions. */
  c: Console;
}

/** The desktop banner (for an Oya provider the desktop has not been seen on) and the refresh error. */
function Notices({ c }: Props) {
  const cloudProvider = !!c.config && isOyaProvider(c.config.browser_provider);
  const needsDesktop = cloudProvider && !c.config?.desktop_seen_at && !c.view.bannerDismissed;
  return (
    <>
      {needsDesktop && <DesktopBanner apiKey={c.apiKey} onDismiss={() => c.patch({ bannerDismissed: true })} />}
      {c.loadError && (
        <div
          role="alert"
          className="flex items-center gap-3 border-b border-red/30 bg-red/10 px-6 py-2 text-sm text-red"
        >
          <span className="flex-1">Could not refresh browsers: {c.loadError}. Showing the last received state.</span>
          <button className="btn-ghost" onClick={c.fetchBrowsers}>
            Retry
          </button>
        </div>
      )}
    </>
  );
}

/** Keeps the open tab in view when the tab row scrolls sideways on a phone. */
function useActiveTabInView(tab: string) {
  const row = useRef<HTMLDivElement>(null);
  useEffect(() => {
    // jsdom has no scrollIntoView, hence the optional call.
    row.current?.querySelector('[aria-current="true"]')?.scrollIntoView?.({ inline: 'nearest', block: 'nearest' });
  }, [tab]);
  return row;
}

/** The tab bar and the shortcut-help button; the tabs scroll sideways on narrow screens, help stays pinned. */
function TabBar({ c }: Props) {
  const row = useActiveTabInView(c.view.tab);
  return (
    <div className="flex items-center gap-1 border-b border-border px-4 lg:px-6">
      <div ref={row} className="flex min-w-0 flex-1 gap-1 overflow-x-auto">
        {TABS.map((t) => (
          <button
            key={t.key}
            onClick={() => c.patch({ tab: t.key })}
            aria-current={c.view.tab === t.key ? 'true' : undefined}
            className={`flex shrink-0 items-center gap-2 whitespace-nowrap border-b-2 px-3 py-2.5 text-[13.5px] font-medium transition-colors ${
              c.view.tab === t.key
                ? 'border-accent text-text'
                : 'border-transparent text-text-muted hover:text-text-secondary'
            }`}
          >
            <t.icon className="h-4 w-4 shrink-0" />
            {t.label}
          </button>
        ))}
      </div>
      <button
        className="btn-icon shrink-0"
        onClick={() => c.patch({ showHelp: true })}
        title="Keyboard shortcuts (?)"
        aria-label="Keyboard shortcuts"
      >
        <HelpCircle className="h-4 w-4" />
      </button>
    </div>
  );
}

/** The browsers tab: the fleet table. */
function BrowsersTab({ c }: Props) {
  const { view, patch } = c;
  return (
    <FleetTable
      rows={c.browsers}
      selectedId={view.selected}
      onSelect={(selected) => patch({ selected })}
      checked={view.checked}
      onChecked={(checked) => patch({ checked })}
      filter={view.filter}
      onFilter={c.onFilter}
      onStop={c.requestStop}
      onStart={() => patch({ showStart: true })}
      onConnect={(connectId) => patch({ connectId })}
      onScreenshot={c.screenshotOf}
      onCode={() => patch({ showCode: true })}
      apiKey={c.apiKey}
      filterRef={c.filterRef}
      now={c.now}
    />
  );
}

/** The open tab's content. */
function TabContent({ c }: Props) {
  const { view, patch, apiKey, project } = c;
  const scope = project ?? apiKey;
  return (
    <div className="min-w-0 flex-1">
      {view.tab === 'browsers' && <BrowsersTab c={c} />}
      {view.tab === 'personas' && (
        <PersonasTab
          key={scope}
          apiKey={apiKey}
          browsers={c.browsers}
          personas={c.personas}
          status={c.personasStatus}
          refresh={c.fetchPersonas}
          openId={view.openPersona}
          onOpen={(openPersona) => patch({ openPersona })}
          onShowBrowsers={c.showBrowsersFor}
          now={c.now}
        />
      )}
      {view.tab === 'playbooks' && (
        <PlaybooksTab key={scope} apiKey={apiKey} browsers={c.browsers} personas={c.personas} now={c.now} />
      )}
      {view.tab === 'routines' && <RoutinesTab key={scope} apiKey={apiKey} now={c.now} personas={c.personas} />}
      {view.tab === 'control' && (
        <div className="h-full min-w-0 overflow-hidden">
          <ControlTab key={scope} apiKey={apiKey} />
        </div>
      )}
    </div>
  );
}

/** The selected browser's panel; on narrow screens an overlay that closes on a backdrop click. */
function SelectedBrowser({ c }: Props) {
  const { view, patch } = c;
  if (!(view.tab === 'browsers' && view.selected)) return null;
  const scope = c.project ?? c.apiKey;
  return (
    <div
      className="fixed inset-0 z-40 flex justify-end bg-ink/50 lg:static lg:z-auto lg:bg-transparent"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) patch({ selected: null });
      }}
    >
      <BrowserPanel
        key={`${scope}:${view.selected}`}
        apiKey={c.apiKey}
        browserId={view.selected}
        onClose={() => patch({ selected: null })}
        onStop={c.requestStop}
        onOpenPersona={(openPersona) => patch({ openPersona })}
        onConnect={(connectId) => patch({ connectId })}
        urlRef={c.urlRef}
        now={c.now}
      />
    </div>
  );
}

/** Everything below the header once onboarding is done. */
export function ConsoleBody({ c }: Props) {
  return (
    <>
      <Notices c={c} />
      <TabBar c={c} />
      {c.view.tab === 'browsers' && (
        <FleetStrip fleet={c.fleet} rate={c.rate} filter={c.view.filter} onFilter={c.onFilter} />
      )}
      <div className="flex min-h-0 min-w-0 flex-1">
        <TabContent c={c} />
        <SelectedBrowser c={c} />
      </div>
    </>
  );
}
