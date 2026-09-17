'use client';

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Monitor, Users, Activity, HelpCircle, Workflow } from 'lucide-react';
import { useToast } from '@/components/dashboard/toast';
import { api, errorMessage } from '@/lib/api-client';
import { consoleCredential } from '@/lib/api';
import { useShortcuts, type Shortcut } from '@/lib/shortcuts';

import Header from '@/components/dashboard/header';
import FleetStrip, { type FleetFilter } from '@/components/dashboard/fleet-strip';
import FleetTable from '@/components/dashboard/fleet-table';
import BrowserPanel from '@/components/dashboard/browser-panel';
import PersonasTab from '@/components/dashboard/personas-tab';
import PersonaDrawer from '@/components/dashboard/persona-drawer';
import ControlTab from '@/components/dashboard/control-tab';
import PlaybooksTab from '@/components/dashboard/playbooks-tab';
import SettingsDialog from '@/components/dashboard/settings-dialog';
import Onboarding from '@/components/dashboard/onboarding';
import StartBrowser from '@/components/dashboard/start-browser';
import DesktopBanner from '@/components/dashboard/desktop-banner';
import ShortcutHelp from '@/components/dashboard/shortcut-help';
import SnippetsDialog, { browserSnippets, fleetSnippets } from '@/components/dashboard/snippets';
import Dialog from '@/components/ui/dialog';
import { Confirm } from '@/components/ui/dialog';
import { loadConfig, isOyaProvider, type KeyConfig } from '@/components/dashboard/config';
import type { BrowserRow, Fleet, Persona } from '@/components/dashboard/types';

type MainTab = 'browsers' | 'personas' | 'playbooks' | 'control';

const TABS: { key: MainTab; label: string; icon: typeof Monitor }[] = [
  { key: 'browsers', label: 'Browsers', icon: Monitor },
  { key: 'personas', label: 'Profiles', icon: Users },
  { key: 'playbooks', label: 'Playbooks', icon: Workflow },
  { key: 'control', label: 'Control', icon: Activity },
];

const NO_FILTER: FleetFilter = { health: null, provider: null, persona: null, text: '' };

/**
 * The fleet console. A thousand browsers in a table with their health, one
 * of them open on the right, and a keyboard to move between them.
 */
export default function DashboardPage() {
  const toast = useToast();

  const [apiKey, setApiKey] = useState('');
  const [tab, setTab] = useState<MainTab>('browsers');
  const [browsers, setBrowsers] = useState<BrowserRow[]>([]);
  const [fleet, setFleet] = useState<Fleet | null>(null);
  const [personas, setPersonas] = useState<Persona[]>([]);
  const [config, setConfig] = useState<KeyConfig | null>(null);
  const [now, setNow] = useState(Date.now());
  const [loadError, setLoadError] = useState<string | null>(null);

  const [selected, setSelected] = useState<string | null>(null);
  const [checked, setChecked] = useState<Set<string>>(new Set());
  const [filter, setFilter] = useState<FleetFilter>(NO_FILTER);
  const [openPersona, setOpenPersona] = useState<string | null>(null);

  const [showOnboarding, setShowOnboarding] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [settingsSection, setSettingsSection] = useState<'alerts' | undefined>(undefined);
  const [showStart, setShowStart] = useState(false);
  const [showHelp, setShowHelp] = useState(false);
  const [stopIds, setStopIds] = useState<string[] | null>(null);
  const [stopping, setStopping] = useState(false);
  const [bannerDismissed, setBannerDismissed] = useState(false);
  const [connectId, setConnectId] = useState<string | null>(null);
  const [showCode, setShowCode] = useState(false);
  const [shot, setShot] = useState<{ id: string; src: string } | null>(null);

  const filterRef = useRef<HTMLInputElement>(null);
  const urlRef = useRef<HTMLInputElement>(null);
  const rateRef = useRef<{ at: number; commands: number; errors: number } | null>(null);
  const [rate, setRate] = useState<{ commandsPerMin: number; errorPct: number } | null>(null);
  const hidden = useRef(false);

  // The project the credential opens. Credentials renew hourly; only a change of project resets the console.
  const [project, setProject] = useState<string | null>(null);
  const projectRef = useRef<string | null>(null);
  // The credential in force now, so a response for a previous project is dropped instead of painted over the new one.
  const keyRef = useRef('');
  const openProject = useCallback((credential: string, id: string | null) => {
    keyRef.current = credential;
    setApiKey(credential);
    if (projectRef.current === id) return;
    projectRef.current = id;
    setProject(id);
    setBrowsers([]); setFleet(null); setPersonas([]); setConfig(null); setLoadError(null); setRate(null); rateRef.current = null;
    setSelected(null); setChecked(new Set()); setFilter(NO_FILTER); setOpenPersona(null); setStopIds(null); setConnectId(null); setBannerDismissed(false);
  }, []);

  // ── Data ──
  const fetchBrowsers = useCallback(async () => {
    if (!apiKey || hidden.current) return;
    try {
      const rows = await api<BrowserRow[]>('/browsers', { key: apiKey });
      if (keyRef.current === apiKey) { setBrowsers(rows); setLoadError(null); }
    } catch (err) { if (keyRef.current === apiKey) setLoadError(errorMessage(err)); }
  }, [apiKey]);

  const fetchFleet = useCallback(async () => {
    if (!apiKey || hidden.current) return;
    try {
      const f = await api<Fleet>('/fleet', { key: apiKey });
      if (keyRef.current !== apiKey) return;
      setFleet(f);
      const prev = rateRef.current;
      const cur = { at: Date.now(), commands: f.browsers.commands, errors: f.browsers.errors };
      if (prev && cur.at > prev.at) {
        const dc = Math.max(0, cur.commands - prev.commands), de = Math.max(0, cur.errors - prev.errors);
        const mins = (cur.at - prev.at) / 60000;
        setRate({ commandsPerMin: Math.round(dc / mins), errorPct: dc ? (de / dc) * 100 : 0 });
      }
      rateRef.current = cur;
    } catch { /* strip shows dashes */ }
  }, [apiKey]);

  const fetchPersonas = useCallback(async () => {
    if (!apiKey || hidden.current) return;
    try {
      const { personas } = await api<{ personas: Persona[] }>('/personas', { key: apiKey });
      if (keyRef.current === apiKey) setPersonas(personas || []);
    } catch { /* keep last */ }
  }, [apiKey]);

  // The wizard decision is made once per project, on first load. Later refreshes
  // (after Settings, after Skip, after a credential renewal) must not re-open it.
  const decidedFor = useRef<string | null>(null);
  const fetchConfig = useCallback(async () => {
    if (!apiKey) { setConfig(null); return; }
    try {
      const cfg = await loadConfig(apiKey);
      if (keyRef.current !== apiKey) return;
      setConfig(cfg);
      const scope = project || apiKey;
      if (decidedFor.current !== scope) { decidedFor.current = scope; setShowOnboarding(!cfg.onboarded); }
    } catch { /* an invalid key already shows as an empty fleet */ }
  }, [apiKey, project]);

  useEffect(() => {
    const saved = consoleCredential();
    let id: string | null = null;
    try { id = sessionStorage.getItem('oya_project_id'); } catch { /* storage blocked */ }
    if (saved) openProject(saved, id);
    setSelected(new URLSearchParams(window.location.search).get('browser'));
  }, [openProject]);

  useEffect(() => { fetchConfig(); }, [fetchConfig]);

  // Coming back from the Slack install. Land the person on the channel picker
  // rather than an unchanged dashboard, and drop the parameter so a refresh is quiet.
  useEffect(() => {
    const outcome = new URLSearchParams(window.location.search).get('slack');
    if (!outcome) return;
    history.replaceState(null, '', window.location.pathname);
    if (outcome === 'pick-channel') { setSettingsSection('alerts'); setShowSettings(true); toast('Slack connected — choose a channel', 'success'); }
    else if (outcome === 'connected') toast('Slack connected', 'success');
    else toast(`Slack install failed: ${outcome.replace(/_/g, ' ')}`, 'error');
  }, [toast]);

  useEffect(() => {
    if (!apiKey) return;
    fetchBrowsers(); fetchFleet(); fetchPersonas();
    const a = setInterval(fetchBrowsers, 3000);
    const b = setInterval(fetchFleet, 5000);
    const c = setInterval(fetchPersonas, 10000);
    const d = setInterval(() => { if (!document.hidden) setNow(Date.now()); }, 5000);
    const onVis = () => { hidden.current = document.hidden; if (!document.hidden) { fetchBrowsers(); fetchFleet(); } };
    document.addEventListener('visibilitychange', onVis);
    return () => { clearInterval(a); clearInterval(b); clearInterval(c); clearInterval(d); document.removeEventListener('visibilitychange', onVis); };
  }, [apiKey, fetchBrowsers, fetchFleet, fetchPersonas]);

  // A browser that leaves the fleet leaves the selection too.
  useEffect(() => {
    if (selected && !loadError && !browsers.some((b) => b.id === selected)) setSelected(null);
    if (checked.size) {
      const alive = new Set(browsers.map((b) => b.id));
      const next = new Set([...checked].filter((id) => alive.has(id)));
      if (next.size !== checked.size) setChecked(next);
    }
  }, [browsers, selected, checked, loadError]);

  // ── Actions ──
  const requestStop = useCallback((ids: string[]) => { if (ids.length) setStopIds(ids); }, []);

  const doStop = async () => {
    if (!stopIds) return;
    setStopping(true);
    try {
      const r = await api<{ stopped: number; results: { id: string; ok: boolean; sandboxRemoved: boolean | null; error?: string }[] }>(
        '/browsers/stop', { key: apiKey, method: 'POST', body: { ids: stopIds } });
      const failedSandbox = r.results.filter((x) => x.sandboxRemoved === false).length;
      toast(`Stopped ${r.stopped}${failedSandbox ? ` — ${failedSandbox} sandbox${failedSandbox > 1 ? 'es' : ''} could not be removed` : ''}`, failedSandbox ? 'error' : 'success');
      if (selected && stopIds.includes(selected)) setSelected(null);
      setChecked(new Set());
      fetchBrowsers(); fetchFleet();
    } catch (err) { toast(errorMessage(err), 'error'); }
    finally { setStopping(false); setStopIds(null); }
  };

  const screenshotOf = useCallback(async (id: string) => {
    try {
      const r = await api<{ ok: boolean; data?: { screenshot?: string }; error?: string }>(`/browsers/${id}/command`, { key: apiKey, method: 'POST', body: { action: 'screenshot' } });
      if (r.data?.screenshot) setShot({ id, src: r.data.screenshot }); else toast(r.error || 'No screenshot', 'error');
    } catch (err) { toast(errorMessage(err), 'error'); }
  }, [apiKey, toast]);

  const moveSelection = useCallback((dir: 1 | -1) => {
    const rows = [...document.querySelectorAll<HTMLElement>('tbody [data-id]')].map((el) => el.dataset.id!);
    if (!rows.length) return;
    const i = selected ? rows.indexOf(selected) : -1;
    const next = rows[Math.min(rows.length - 1, Math.max(0, i + dir))];
    setSelected(next);
  }, [selected]);

  const showBrowsersFor = useCallback((personaId: string) => {
    const p = personas.find((x) => x.id === personaId);
    setFilter({ ...NO_FILTER, persona: p?.name || personaId });
    setOpenPersona(null);
    setTab('browsers');
  }, [personas]);

  // ── Shortcuts ──
  const shortcuts = useMemo<Shortcut[]>(() => [
    { keys: 'mod+1', label: 'Browsers', group: 'Navigate', global: true, handler: () => setTab('browsers') },
    { keys: 'mod+2', label: 'Personas', group: 'Navigate', global: true, handler: () => setTab('personas') },
    { keys: 'mod+3', label: 'Control', group: 'Navigate', global: true, handler: () => setTab('control') },
    { keys: 'mod+4', label: 'Playbooks', group: 'Navigate', global: true, handler: () => setTab('playbooks') },
    { keys: '?', label: 'This help', group: 'Navigate', handler: () => setShowHelp(true) },
    { keys: 'n', label: 'Start a browser', group: 'Fleet', handler: () => setShowStart(true) },
    { keys: '/', label: 'Filter the fleet', group: 'Fleet', handler: () => { setTab('browsers'); filterRef.current?.focus(); } },
    { keys: 'down', label: 'Next browser', group: 'Fleet', handler: () => moveSelection(1) },
    { keys: 'up', label: 'Previous browser', group: 'Fleet', handler: () => moveSelection(-1) },
    { keys: 'j', label: 'Next browser', group: 'Fleet', handler: () => moveSelection(1) },
    { keys: 'k', label: 'Previous browser', group: 'Fleet', handler: () => moveSelection(-1) },
    { keys: 'escape', label: 'Close panel / clear', group: 'Fleet', global: true, handler: () => {
      if (showStart || showHelp || showSettings || stopIds || openPersona) return;   // the dialog handles it
      if (selected) setSelected(null); else if (checked.size) setChecked(new Set());
    } },
    { keys: 'x', label: 'Stop selected', group: 'Browser', handler: () => requestStop(checked.size ? [...checked] : selected ? [selected] : []) },
    { keys: 'l', label: 'Focus the URL bar', group: 'Browser', handler: () => urlRef.current?.focus() },
    { keys: 'r', label: 'Reload', group: 'Browser', handler: () => { if (selected) document.querySelector<HTMLButtonElement>('[title^="Reload"]')?.click(); } },
    { keys: 's', label: 'Screenshot', group: 'Browser', handler: () => { if (selected) [...document.querySelectorAll<HTMLButtonElement>('aside button')].find((b) => /Screenshot/.test(b.textContent || ''))?.click(); } },
  ], [moveSelection, requestStop, selected, checked, showStart, showHelp, showSettings, stopIds, openPersona]);

  useShortcuts(shortcuts, !!apiKey && !showOnboarding);

  // ── Derived ──
  const onboarding = showOnboarding && config && apiKey;
  const cloudProvider = !!config && isOyaProvider(config.browser_provider);
  const needsDesktop = cloudProvider && !config?.desktop_seen_at && !bannerDismissed;
  const stopTargets = stopIds ? browsers.filter((b) => stopIds.includes(b.id)) : [];
  const stopCloud = stopTargets.filter((b) => b.provider === 'oya-cloud').length;
  const providersForStart = (config?.providers || []).map((p) => ({ id: p.id, label: p.label, configured: p.configured }));

  return (
    <div className="flex h-dvh flex-col overflow-hidden bg-bg">
      <Header apiKey={apiKey} setApiKey={openProject}onOpenSettings={() => setShowSettings(true)} />

      {onboarding ? (
        <Onboarding apiKey={apiKey} config={config} personas={personas} browsers={browsers} onDone={() => { setShowOnboarding(false); fetchConfig(); }} />
      ) : (
        <>
          {needsDesktop && <DesktopBanner apiKey={apiKey} onDismiss={() => setBannerDismissed(true)} />}
          {loadError && <div role="alert" className="flex items-center gap-3 border-b border-red/30 bg-red/10 px-6 py-2 text-sm text-red"><span className="flex-1">Could not refresh browsers: {loadError}. Showing the last received state.</span><button className="btn-ghost" onClick={fetchBrowsers}>Retry</button></div>}

          {/* Tabs */}
          <div className="flex items-center gap-1 border-b border-border px-4 lg:px-6">
            {TABS.map((t) => (
              <button key={t.key} onClick={() => setTab(t.key)}
                className={`flex items-center gap-2 border-b-2 px-3 py-2.5 text-[13.5px] font-medium transition-colors ${
                  tab === t.key ? 'border-accent text-text' : 'border-transparent text-text-muted hover:text-text-secondary'}`}>
                <t.icon className="h-4 w-4 shrink-0" />{t.label}
              </button>
            ))}
            <button className="btn-icon ml-auto" onClick={() => setShowHelp(true)} title="Keyboard shortcuts (?)" aria-label="Keyboard shortcuts">
              <HelpCircle className="h-4 w-4" />
            </button>
          </div>

          {tab === 'browsers' && <FleetStrip fleet={fleet} rate={rate} filter={filter} onFilter={(n) => setFilter((f) => ({ ...f, ...n }))} />}

          <div className="flex min-h-0 min-w-0 flex-1">
            <div className="min-w-0 flex-1">
              {tab === 'browsers' && (
                <FleetTable rows={browsers} selectedId={selected} onSelect={setSelected} checked={checked} onChecked={setChecked}
                  filter={filter} onFilter={(n) => setFilter((f) => ({ ...f, ...n }))} onStop={requestStop} onStart={() => setShowStart(true)}
                  onConnect={setConnectId} onScreenshot={screenshotOf} onCode={() => setShowCode(true)} apiKey={apiKey}
                  filterRef={filterRef} now={now} />
              )}
              {tab === 'personas' && (
                <PersonasTab apiKey={apiKey} browsers={browsers} personas={personas} refresh={fetchPersonas}
                  openId={openPersona} onOpen={setOpenPersona} onShowBrowsers={showBrowsersFor} now={now} />
              )}
              {tab === 'playbooks' && <PlaybooksTab key={project ?? ''} apiKey={apiKey} browsers={browsers} personas={personas} now={now} />}
              {tab === 'control' && <div className="h-full min-w-0 overflow-hidden"><ControlTab key={project ?? ''} apiKey={apiKey} /></div>}
            </div>

            {tab === 'browsers' && selected && (
              <div className="fixed inset-0 z-40 flex justify-end bg-black/50 lg:static lg:z-auto lg:bg-transparent" onMouseDown={(e) => { if (e.target === e.currentTarget) setSelected(null); }}>
                <BrowserPanel key={`${project}:${selected}`} apiKey={apiKey} browserId={selected} onClose={() => setSelected(null)} onStop={requestStop}
                  onOpenPersona={setOpenPersona} onConnect={setConnectId} urlRef={urlRef} now={now} />
              </div>
            )}
          </div>
        </>
      )}

      {/* The persona drawer can open from the browser panel too. */}
      {tab !== 'personas' && (
        <PersonaDrawer persona={personas.find((p) => p.id === openPersona) || null} onClose={() => setOpenPersona(null)} apiKey={apiKey}
          browsers={browsers} onChanged={fetchPersonas} onShowBrowsers={showBrowsersFor} now={now} />
      )}

      {showStart && <StartBrowser open onClose={() => setShowStart(false)} apiKey={apiKey} personas={personas}
        defaultProvider={config?.browser_provider || 'cdp'} providers={providersForStart}
        onStarted={() => { fetchBrowsers(); fetchFleet(); }} />}

      <Confirm open={!!stopIds} onClose={() => setStopIds(null)} onConfirm={doStop} danger busy={stopping}
        title={stopTargets.length === 1 ? `Stop ${stopTargets[0].name}?` : `Stop ${stopTargets.length} browsers?`}
        confirmLabel={stopTargets.length === 1 ? 'Stop browser' : `Stop ${stopTargets.length}`}
        body={stopCloud
          ? <>{stopCloud === stopTargets.length ? 'This' : `${stopCloud} of these`} {stopCloud === 1 ? 'is a cloud browser: its sandbox is destroyed' : 'are cloud browsers: their sandboxes are destroyed'} and billing stops. Anything unsaved in the page is gone.</>
          : <>The session ends now. A CDP browser is handed back to its provider; a desktop browser just disconnects.</>} />

      <ShortcutHelp open={showHelp} onClose={() => setShowHelp(false)} shortcuts={shortcuts} />

      {(() => { const b = connectId ? browsers.find((x) => x.id === connectId) : null; return (
        <SnippetsDialog open={!!b} onClose={() => setConnectId(null)} apiKey={apiKey}
          title={b ? `Connect to ${b.name}` : 'Connect'}
          description={b ? `${b.id} · ${b.clientType === 'cdp' ? 'CDP-backed — Playwright can attach' : 'Oya client — drive it over the API'}` : undefined}
          snippets={b ? browserSnippets(b) : []} />
      ); })()}
      <SnippetsDialog open={showCode} onClose={() => setShowCode(false)} apiKey={apiKey}
        title="Use this fleet from code" description="Every snippet targets this deployment and your current key." snippets={fleetSnippets()} />
      <Dialog open={!!shot} onClose={() => setShot(null)} title="Screenshot" size="lg" description={shot?.id}>
        {/* eslint-disable-next-line @next/next/no-img-element */}
        {shot && <img src={shot.src} alt="Screenshot" className="w-full rounded-md border border-border" />}
      </Dialog>

      <SettingsDialog open={showSettings} initialSection={settingsSection} onClose={() => { setShowSettings(false); setSettingsSection(undefined); fetchConfig(); }} apiKey={apiKey} onRerunSetup={() => setShowOnboarding(true)} />
    </div>
  );
}
