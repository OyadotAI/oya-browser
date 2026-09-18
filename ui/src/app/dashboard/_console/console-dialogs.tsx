/**
 * The console's dialogs and drawers: persona drawer, start a browser, stop
 * confirmation, shortcut help, code snippets, screenshot and settings.
 */
'use client';

import PersonaDrawer from '@/components/dashboard/persona-drawer';
import SettingsDialog from '@/components/dashboard/settings-dialog';
import StartBrowser from '@/components/dashboard/start-browser';
import ShortcutHelp from '@/components/dashboard/shortcut-help';
import SnippetsDialog, { browserSnippets, fleetSnippets } from '@/components/dashboard/snippets';
import Dialog, { Confirm } from '@/components/ui/dialog';
import type { Console } from './use-console';

/** Every part gets the whole console. */
interface Props {
  /** The console's state and actions. */
  c: Console;
}

/** What the stop dialog says about cloud browsers, whose sandboxes are destroyed. */
function stopBody(cloud: number, total: number) {
  if (!cloud) {
    return <>The session ends now. A CDP browser is handed back to its provider; a desktop browser just disconnects.</>;
  }
  return (
    <>
      {cloud === total ? 'This' : `${cloud} of these`}{' '}
      {cloud === 1
        ? 'is a cloud browser: its sandbox is destroyed'
        : 'are cloud browsers: their sandboxes are destroyed'}{' '}
      and billing stops. Anything unsaved in the page is gone.
    </>
  );
}

/** Confirms a stop, spelling out what it destroys. */
function StopConfirm({ c }: Props) {
  const { stopIds } = c.view;
  const targets = stopIds ? c.browsers.filter((b) => stopIds.includes(b.id)) : [];
  const cloud = targets.filter((b) => b.provider === 'oya-cloud').length;
  return (
    <Confirm
      open={!!stopIds}
      onClose={() => c.patch({ stopIds: null })}
      onConfirm={c.doStop}
      danger
      busy={c.view.stopping}
      title={targets.length === 1 ? `Stop ${targets[0].name}?` : `Stop ${targets.length} browsers?`}
      confirmLabel={targets.length === 1 ? 'Stop browser' : `Stop ${targets.length}`}
      body={stopBody(cloud, targets.length)}
    />
  );
}

/** Connect snippets for one browser. */
function ConnectDialog({ c }: Props) {
  const b = c.view.connectId ? c.browsers.find((x) => x.id === c.view.connectId) : null;
  const kind = b?.clientType === 'cdp' ? 'CDP-backed — Playwright can attach' : 'Oya client — drive it over the API';
  return (
    <SnippetsDialog
      open={!!b}
      onClose={() => c.patch({ connectId: null })}
      apiKey={c.apiKey}
      title={b ? `Connect to ${b.name}` : 'Connect'}
      description={b ? `${b.id} · ${kind}` : undefined}
      snippets={b ? browserSnippets(b) : []}
    />
  );
}

/** The screenshot viewer. */
function ScreenshotDialog({ c }: Props) {
  const { shot } = c.view;
  return (
    <Dialog open={!!shot} onClose={() => c.patch({ shot: null })} title="Screenshot" size="lg" description={shot?.id}>
      {/* eslint-disable-next-line @next/next/no-img-element */}
      {shot && <img src={shot.src} alt="Screenshot" className="w-full rounded-md border border-border" />}
    </Dialog>
  );
}

/** Start a browser, with the providers this key has. */
function StartDialog({ c }: Props) {
  if (!c.view.showStart) return null;
  const providers = (c.config?.providers || []).map((p) => ({ id: p.id, label: p.label, configured: p.configured }));
  return (
    <StartBrowser
      open
      onClose={() => c.patch({ showStart: false })}
      apiKey={c.apiKey}
      personas={c.personas}
      defaultProvider={c.config?.browser_provider || 'cdp'}
      providers={providers}
      onStarted={c.refresh}
    />
  );
}

/** Settings; closing it reloads the configuration. */
function Settings({ c }: Props) {
  return (
    <SettingsDialog
      open={c.view.showSettings}
      initialSection={c.view.settingsSection}
      onClose={() => (c.patch({ showSettings: false, settingsSection: undefined }), c.fetchConfig())}
      apiKey={c.apiKey}
      onRerunSetup={() => c.patch({ showOnboarding: true })}
    />
  );
}

/** Every dialog and drawer the console can open. */
export function ConsoleDialogs({ c }: Props) {
  const { view, patch } = c;
  return (
    <>
      {/* The persona drawer can open from the browser panel too. */}
      {view.tab !== 'personas' && (
        <PersonaDrawer
          persona={c.personas.find((p) => p.id === view.openPersona) || null}
          onClose={() => patch({ openPersona: null })}
          apiKey={c.apiKey}
          browsers={c.browsers}
          onChanged={c.fetchPersonas}
          onShowBrowsers={c.showBrowsersFor}
          now={c.now}
        />
      )}
      <StartDialog c={c} />
      <StopConfirm c={c} />
      <ShortcutHelp open={view.showHelp} onClose={() => patch({ showHelp: false })} shortcuts={c.shortcuts} />
      <ConnectDialog c={c} />
      <SnippetsDialog
        open={view.showCode}
        onClose={() => patch({ showCode: false })}
        apiKey={c.apiKey}
        title="Use this fleet from code"
        description="Every snippet targets this deployment and your current key."
        snippets={fleetSnippets()}
      />
      <ScreenshotDialog c={c} />
      <Settings c={c} />
    </>
  );
}
