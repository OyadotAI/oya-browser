'use client';

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { CircleDot, Code, ExternalLink, Pencil, Play, Trash2, Workflow } from 'lucide-react';
import { ago, api, errorMessage } from '@/lib/api-client';
import Dialog, { Confirm } from '@/components/ui/dialog';
import SyntaxCode from '@/components/ui/syntax-code';
import LiveView from './live-view';
import { subscribeFrames } from '@/lib/live-stream';
import { useToast } from './toast';
import type { BrowserRow, Persona } from './types';

interface PlaybookBody { name: string; variables: string[]; defaults: Record<string, string>; steps: number; code: string }
interface PlaybookInfo extends PlaybookBody {
  createdAt: string | null;
  promotedAt: string | null;
  draft: (PlaybookBody & { healedAt: string; healedFrom: number }) | null;
}
interface RunInfo {
  id: string;
  status: 'running' | 'needs_attention' | 'succeeded' | 'failed';
  attention: { id: string; reason: 'captcha' | 'login' | 'mfa' | 'agent' | 'heal_failed'; message: string; liveViewUrl?: string } | null;
  result?: { steps?: number; total?: number; fellBack?: boolean; healed?: boolean; draft?: string; text?: string };
  error?: string;
}

const RECORD_SNIPPET = `await browser.ask('Fill the order for {{name}}', { data: { name: 'Ada' } });
await browser.toPlaybook('order');           // saved here
await browser.play('order', { name: 'Alan' }); // replayed without the LLM`;

/**
 * Flows recorded from ask() runs. Each replays without the LLM; a replay that
 * breaks can heal itself into a draft, which waits here for review.
 */
export default function PlaybooksTab({ apiKey, browsers, personas, now }: { apiKey: string; browsers: BrowserRow[]; personas: Persona[]; now: number }) {
  const toast = useToast();
  const [playbooks, setPlaybooks] = useState<PlaybookInfo[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [code, setCode] = useState<PlaybookBody | null>(null);
  const [running, setRunning] = useState<PlaybookInfo | null>(null);
  const [removing, setRemoving] = useState<string | null>(null);
  const [renaming, setRenaming] = useState<PlaybookInfo | null>(null);
  const [busy, setBusy] = useState(false);
  const [recording, setRecording] = useState(false);

  const refresh = useCallback(async () => {
    if (!apiKey) return;
    try {
      setPlaybooks((await api<{ playbooks: PlaybookInfo[] }>('/playbooks', { key: apiKey })).playbooks);
      setLoadError(null);
    } catch (err) { setLoadError(errorMessage(err)); }
  }, [apiKey]);

  useEffect(() => {
    refresh();
    const timer = setInterval(refresh, 15000);
    return () => clearInterval(timer);
  }, [refresh]);

  const promote = async (name: string) => {
    try {
      await api(`/playbooks/${encodeURIComponent(name)}/promote`, { key: apiKey, method: 'POST', body: {} });
      toast(`${name} now uses the healed steps`, 'success');
      refresh();
    } catch (err) { toast(errorMessage(err), 'error'); }
  };

  const doRemove = async () => {
    if (!removing) return;
    setBusy(true);
    try {
      await api(`/playbooks/${encodeURIComponent(removing)}`, { key: apiKey, method: 'DELETE' });
      toast(removing.endsWith(':draft') ? 'Draft discarded' : `Deleted ${removing}`, 'success');
      refresh();
    } catch (err) { toast(errorMessage(err), 'error'); }
    finally { setBusy(false); setRemoving(null); }
  };

  const doRename = async (name: string) => {
    if (!renaming) return;
    setBusy(true);
    try {
      await api(`/playbooks/${encodeURIComponent(renaming.name)}`, { key: apiKey, method: 'PATCH', body: { name } });
      toast(`Renamed to ${name}`, 'success');
      setRenaming(null);
      refresh();
    } catch (err) { toast(errorMessage(err), 'error'); }
    finally { setBusy(false); }
  };

  const list = playbooks || [];

  return (
    <div className="workspace-list flex h-full min-h-0 min-w-0 flex-col">
      <div className="flex shrink-0 flex-wrap items-center gap-3 px-4 py-5 lg:px-6">
        <div className="mr-auto">
          <h2 className="text-[22px] font-medium tracking-tight text-text">Playbooks <span className="ml-2 text-[14px] text-text-dim">{list.length}</span></h2>
          <p className="text-[12px] text-text-muted">Flows recorded from an ask() run, or from you doing it yourself. Replayed without the LLM; values you enter or select are variables you can override.</p>
        </div>
        <button className="btn-primary" onClick={() => setRecording(true)} disabled={!browsers.length}
          title={browsers.length ? 'Do the task yourself in a live browser and keep it as a playbook' : 'Start a browser first'}>
          <CircleDot className="h-4 w-4" /> Record a flow
        </button>
      </div>

      {loadError && <div role="alert" className="mx-4 mb-3 rounded-lg border border-red/30 bg-red/10 px-3 py-2 text-sm text-red lg:mx-6">Could not load playbooks: {loadError}</div>}

      <div className="data-scroll mx-4 mb-4 min-h-0 flex-initial overflow-auto rounded-xl border border-border bg-bg-card/25 lg:mx-6 lg:mb-6">
        <table className="data-table w-full table-fixed border-collapse text-[13px]" aria-label="Playbooks">
          <thead className="sticky top-0 z-10 bg-bg-elevated">
            <tr className="border-b border-border text-left text-[11px] font-medium uppercase tracking-[0.1em] text-text-muted">
              <th className="w-[22%] px-4 py-3">Playbook</th>
              <th className="px-3 py-3">Variables</th>
              <th className="w-[70px] px-2 py-3 text-right">Steps</th>
              <th className="w-[28%] px-3 py-3">Healed draft</th>
              <th className="w-[90px] px-2 py-3 text-right">Created</th>
              <th className="w-[150px] px-2 py-3" aria-label="Actions" />
            </tr>
          </thead>
          <tbody>
            {list.map((p) => (
              <tr key={p.name} className="border-b border-border/60 hover:bg-text/[0.035]">
                <td className="px-2 py-3 pl-4">
                  <div className="truncate font-medium text-text" title={p.name}>{p.name}</div>
                  {p.promotedAt && <div className="mt-1 text-[11px] text-text-dim">healed {ago(p.promotedAt, now)} ago</div>}
                </td>
                <td className="px-3 py-3">
                  <div className="flex flex-wrap gap-1">
                    {p.variables.length
                      ? p.variables.map((v) => <span key={v} className="rounded border border-border px-1.5 py-0.5 font-mono text-[11px] text-text-secondary">{v}</span>)
                      : <span className="text-text-dim">none</span>}
                  </div>
                </td>
                <td className="px-2 py-3 text-right num text-text-secondary">{p.steps}</td>
                <td className="px-3 py-3">
                  {p.draft ? (
                    <div className="flex flex-wrap items-center gap-2">
                      <span className="text-[12px] text-yellow" title={`The replay broke at step ${p.draft.healedFrom + 1}; the agent finished it.`}>
                        {p.draft.steps} steps · {ago(p.draft.healedAt, now)}
                      </span>
                      <button className="btn-ghost h-7 px-2 text-[12px]" onClick={() => setCode(p.draft)}>Review</button>
                      <button className="btn-primary h-7 px-2 text-[12px]" onClick={() => promote(p.name)}>Promote</button>
                      <button className="btn-ghost h-7 px-2 text-[12px]" onClick={() => setRemoving(`${p.name}:draft`)}>Discard</button>
                    </div>
                  ) : <span className="text-text-dim">—</span>}
                </td>
                <td className="px-2 py-3 text-right num text-text-muted">{ago(p.createdAt, now)}</td>
                <td className="px-2 py-3">
                  <div className="flex justify-end gap-1">
                    <button className="btn-icon" title="Playwright code" aria-label={`Playwright code for ${p.name}`} onClick={() => setCode(p)}><Code className="h-4 w-4" /></button>
                    <button className="btn-icon" title="Rename" aria-label={`Rename ${p.name}`} onClick={() => setRenaming(p)}><Pencil className="h-4 w-4" /></button>
                    <button className="btn-icon" title="Run" aria-label={`Run ${p.name}`} onClick={() => setRunning(p)}><Play className="h-4 w-4" /></button>
                    <button className="btn-icon" title="Delete" aria-label={`Delete ${p.name}`} onClick={() => setRemoving(p.name)}><Trash2 className="h-4 w-4" /></button>
                  </div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        {playbooks && list.length === 0 && (
          <div className="flex flex-col items-center justify-center gap-3 px-6 py-24 text-center">
            <div className="flex h-12 w-12 items-center justify-center rounded-xl border border-border bg-bg-card"><Workflow className="h-5 w-5 text-text-muted" /></div>
            <p className="text-[15px] font-medium text-text">No playbooks yet</p>
            <p className="max-w-md text-[13px] text-text-muted">Record one yourself with the button above, or run a task with ask() and save it from code:</p>
            <pre className="max-w-full overflow-x-auto rounded-lg border border-border bg-bg-card px-4 py-3 text-left text-[12px]"><SyntaxCode code={RECORD_SNIPPET} language="typescript" /></pre>
          </div>
        )}
      </div>

      <Dialog open={!!code} onClose={() => setCode(null)} size="lg" title={code?.name} description="The same flow as a Playwright module. Pass the variables as vars.">
        {code && <pre className="max-h-[60vh] overflow-auto rounded-lg border border-border bg-bg-card px-4 py-3 text-[12px]"><SyntaxCode code={code.code} language="typescript" /></pre>}
      </Dialog>

      {renaming && <RenameDialog current={renaming.name} busy={busy} onClose={() => setRenaming(null)} onRename={doRename} />}

      {running && <RunDialog apiKey={apiKey} playbook={running} browsers={browsers} personas={personas} onClose={() => setRunning(null)} onFinished={refresh} />}

      {recording && <RecordDialog apiKey={apiKey} browsers={browsers} onClose={() => setRecording(false)} onSaved={refresh} />}

      <Confirm open={!!removing} onClose={() => setRemoving(null)} onConfirm={doRemove} danger busy={busy}
        title={removing?.endsWith(':draft') ? 'Discard the healed draft?' : `Delete ${removing}?`}
        confirmLabel={removing?.endsWith(':draft') ? 'Discard' : 'Delete'}
        body={removing?.endsWith(':draft')
          ? 'The playbook keeps its current steps. The next replay that breaks will heal again.'
          : 'The playbook and any healed draft are gone. Code calling play() with this name will fail.'} />
    </div>
  );
}

/** Rename a saved playbook. Code calling play() with the old name breaks, so the dialog says so. */
function RenameDialog({ current, busy, onClose, onRename }: {
  current: string; busy: boolean; onClose: () => void; onRename: (name: string) => void;
}) {
  const [name, setName] = useState(current);
  const trimmed = name.trim();
  const ok = /^[\w-]{1,64}$/.test(trimmed) && trimmed !== current;
  return (
    <Dialog open onClose={onClose} title={`Rename ${current}`} description="Any healed draft moves with it. Code calling play() with the old name will fail."
      footer={<>
        <button className="btn-ghost" onClick={onClose} disabled={busy}>Cancel</button>
        <button className="btn-primary" onClick={() => onRename(trimmed)} disabled={!ok || busy}>{busy ? 'Renaming…' : 'Rename'}</button>
      </>}>
      <form onSubmit={(e) => { e.preventDefault(); if (ok && !busy) onRename(trimmed); }}>
        <label className="label" htmlFor="pb-rename">Name</label>
        <input id="pb-rename" className="field font-mono" value={name} autoComplete="off" spellCheck={false}
          onChange={(e) => setName(e.target.value)} />
        <p className="mt-1 text-[12px] text-text-muted">1-64 letters, digits, _ or -.</p>
      </form>
    </Dialog>
  );
}

const ATTENTION: Record<NonNullable<RunInfo['attention']>['reason'], string> = {
  captcha: 'CAPTCHA',
  login: 'Sign-in',
  mfa: 'MFA',
  agent: 'Agent question',
  heal_failed: 'Could not heal',
};

interface RecordedStep {
  action: string;
  url?: string; text?: string; option?: string; key?: string;
  el?: { text?: string; name?: string; domId?: string; testId?: string; tag?: string };
}
interface RecordState { recording: boolean; steps: RecordedStep[]; secrets: string[] }

const describeStep = (s: RecordedStep) => {
  const label = s.el ? (s.el.text || s.el.name || s.el.domId || s.el.testId || s.el.tag || '') : '';
  if (s.action === 'navigate') return `navigate ${s.url}`;
  if (s.action === 'type') return `type ${label} ← ${s.text}`;
  if (s.action === 'select_option') return `select ${label} ← ${s.option}`;
  if (s.action === 'press_key') return `key ${s.key}`;
  return `${s.action} ${label}`;
};

/**
 * Record a flow by doing it. The page watches what the person does in the live view
 * and reports it as steps, so coordinate clicks come back as elements — the same
 * shape an ask() run leaves behind, saved as a playbook by the same route.
 */
function RecordDialog({ apiKey, browsers, onClose, onSaved }: {
  apiKey: string; browsers: BrowserRow[]; onClose: () => void; onSaved: () => void;
}) {
  const toast = useToast();
  const [browserId, setBrowserId] = useState(browsers[0]?.id || '');
  const [state, setState] = useState<RecordState | null>(null);
  const [frame, setFrame] = useState<string | null>(null);
  const [fps, setFps] = useState(0);
  const [frameAt, setFrameAt] = useState<number | null>(null);
  const frames = useRef(0);
  const [busy, setBusy] = useState<'start' | 'stop' | 'save' | null>(null);
  const [held, setHeld] = useState(false);
  const [url, setUrl] = useState('');
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');

  const acquired = useRef(false);
  const mounted = useRef(true);
  const revision = useRef(0);
  const recording = !!state?.recording;
  const steps = state?.steps || [];

  // Stop capture and hand control back on every exit, including navigation
  // away from this tab. The server retains the final steps for recovery.
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      if (acquired.current) {
        void api(`/control/sessions/${encodeURIComponent(browserId)}/record`, {
          key: apiKey, method: 'POST', body: { mode: 'stop', resume: true },
        }).catch(() => undefined);
      }
    };
  }, [browserId, apiKey]);

  useEffect(() => {
    let cancelled = false;
    const version = revision.current;
    void api<RecordState>(`/control/sessions/${encodeURIComponent(browserId)}/record`, {
      key: apiKey, method: 'POST', body: { mode: 'status' },
    }).then((saved) => {
      if (!cancelled && version === revision.current && (saved.recording || saved.steps.length)) {
        // An active flow is rejoined through Start, which acquires control.
        if (!saved.recording) setState(saved);
      }
    }).catch(() => undefined);
    return () => { cancelled = true; };
  }, [browserId, apiKey]);

  // `started`, not `state`: the poll below replaces that object every 800ms, and
  // depending on it tore the frame stream down and rebuilt it just as often — the
  // view never got a frame, and with no frame there is nothing to map a click onto.
  const started = !!state;
  useEffect(() => {
    if (!started || !browserId) return;
    const stop = subscribeFrames(browserId, apiKey, (f) => { setFrame(f); frames.current++; setFrameAt(Date.now()); }, () => setFrame(null));
    // The indicator reads "connecting" until a frame lands, which is the difference
    // between a view that is merely slow and one that is not there at all.
    const fpsTimer = setInterval(() => { setFps(frames.current); frames.current = 0; }, 1000);
    return () => { stop(); clearInterval(fpsTimer); };
  }, [started, browserId, apiKey]);

  // Never overlap status requests or let a response resurrect a stopped flow.
  useEffect(() => {
    if (!recording || busy) return;
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      try {
        const next = await api<RecordState>(`/control/sessions/${encodeURIComponent(browserId)}/record`, {
          key: apiKey, method: 'POST', body: { mode: 'status' },
        });
        if (!cancelled) setState(next);
      } catch (err) { if (!cancelled) toast(errorMessage(err), 'error'); }
      if (!cancelled) timer = setTimeout(poll, 800);
    };
    timer = setTimeout(poll, 800);
    return () => { cancelled = true; clearTimeout(timer); };
  }, [recording, busy, browserId, apiKey, toast]);

  const send = useCallback(async (action: string, params: Record<string, unknown> = {}) => {
    const r = await api<{ ok: boolean; error?: string }>(`/control/sessions/${encodeURIComponent(browserId)}/input`, {
      key: apiKey, method: 'POST', body: { action, params },
    }).catch((err) => ({ ok: false, error: errorMessage(err) }));
    if (r.ok === false) toast(r.error || `${action} failed`, 'error');
    return r;
  }, [browserId, apiKey, toast]);

  /** Where the flow starts. Typed here rather than clicked, and recorded as the first step. */
  const go = async () => {
    const target = url.trim();
    if (!target || busy) return;
    await send('navigate', { url: /^https?:\/\//i.test(target) ? target : `https://${target}` });
  };

  // Human input is refused unless a person holds the browser, and the hold expires
  // in five minutes — shorter than plenty of flows, so recording renews it.
  const takeControl = useCallback((force = false) => api(`/control/sessions/${encodeURIComponent(browserId)}/control`, {
    key: apiKey, method: 'POST', body: { action: 'acquire', force },
  }), [browserId, apiKey]);

  useEffect(() => {
    if (!recording) return;
    const timer = setInterval(() => { void takeControl().catch(() => undefined); }, 120000);
    return () => clearInterval(timer);
  }, [recording, takeControl]);

  const record = async (mode: 'start' | 'stop', force = false) => {
    setBusy(mode);
    revision.current++;
    try {
      if (mode === 'start') { await takeControl(force); acquired.current = true; }
      if (!mounted.current) {
        await api(`/control/sessions/${encodeURIComponent(browserId)}/record`, { key: apiKey, method: 'POST', body: { mode: 'stop', resume: true } });
        acquired.current = false;
        return;
      }
      const next = await api<RecordState>(`/control/sessions/${encodeURIComponent(browserId)}/record`, { key: apiKey, method: 'POST', body: { mode, resume: mode === 'stop' } });
      if (mode === 'stop') acquired.current = false;
      if (mounted.current) setState(next);
    } catch (err) {
      if (mode === 'start' && acquired.current) {
        await api(`/control/sessions/${encodeURIComponent(browserId)}/record`, { key: apiKey, method: 'POST', body: { mode: 'stop', resume: true } }).then(() => { acquired.current = false; }).catch(() => undefined);
      }
      const code = (err as { body?: { code?: string } }).body?.code;
      setHeld(code === 'control_busy');
      toast(code === 'control_busy' ? 'Another tab or operator is holding this browser.'
        : code === 'commands_pending' ? 'The browser is still finishing a command. Try again in a moment.'
        : errorMessage(err), 'error');
    }
    finally { setBusy(null); }
  };

  const save = async () => {
    setBusy('save');
    try {
      const saved = await api<PlaybookBody>(`/browsers/${encodeURIComponent(browserId)}/playbooks`, {
        key: apiKey, method: 'POST',
        body: { name: name.trim(), prompt: description.trim(), steps, secrets: state?.secrets || [] },
      });
      await api(`/control/sessions/${encodeURIComponent(browserId)}/record`, { key: apiKey, method: 'POST', body: { mode: 'discard' } });
      toast(`${saved.name} saved — ${saved.steps} steps`, 'success');
      onSaved();
      onClose();
    } catch (err) { toast(errorMessage(err), 'error'); }
    finally { setBusy(null); }
  };

  const close = async () => {
    if (busy) return;
    if (recording || acquired.current) {
      setBusy('stop');
      revision.current++;
      try {
        const final = await api<RecordState>(`/control/sessions/${encodeURIComponent(browserId)}/record`, {
          key: apiKey, method: 'POST', body: { mode: 'stop', resume: true },
        });
        acquired.current = false;
        setState(final);
      } catch (err) { toast(errorMessage(err), 'error'); return; }
      finally { setBusy(null); }
    }
    onClose();
  };

  const nameOk = /^[\w-]{1,64}$/.test(name.trim());

  return (
    <Dialog open onClose={() => { void close(); }} size="lg" title="Record a flow"
      description="Do the task yourself in the live view. What you click and type becomes a playbook of named fields, and a Playwright module."
      footer={
        <>
          <button className="btn-ghost" onClick={close} disabled={!!busy}>Close</button>
          {state && !recording && <button className="btn-ghost" onClick={() => record('start')} disabled={!!busy}>Start new recording</button>}
          {!state
            ? <button className="btn-primary" onClick={() => record('start', held)} disabled={!browserId || busy === 'start'}>
                {busy === 'start' ? 'Starting…' : held ? 'Take over and record' : 'Start recording'}
              </button>
            : recording
              ? <button className="btn-primary" onClick={() => record('stop')} disabled={busy === 'stop'}>{busy === 'stop' ? 'Stopping…' : 'Stop'}</button>
              : <button className="btn-primary" onClick={save} disabled={!nameOk || !description.trim() || !steps.length || busy === 'save'}>{busy === 'save' ? 'Saving…' : 'Save playbook'}</button>}
        </>
      }>
      <div className="flex flex-col gap-4">
        {!state ? (
          <div>
            <label className="label" htmlFor="rec-browser">Browser</label>
            {browsers.length
              ? <select id="rec-browser" className="field" disabled={!!busy} value={browserId} onChange={(e) => setBrowserId(e.target.value)}>
                  {browsers.map((b) => <option key={b.id} value={b.id}>{b.name} · {b.id}</option>)}
                </select>
              : <p className="text-sm text-text-muted">No browser is running. Start one from the Browsers tab.</p>}
            <p className="mt-2 text-[12px] text-text-muted">Recording takes control of the browser so your input reaches the page. Passwords are masked in the page and saved as variables.</p>
            {held && <p className="mt-2 text-[12px] text-yellow">Someone else is holding this browser — another dashboard tab, or one that was closed without releasing. Take over to record anyway; their live view stops driving it.</p>}
          </div>
        ) : (
          <>
            <div className="flex items-center gap-2 text-[12px]">
              <span className={recording ? 'inline-flex items-center gap-1.5 text-red' : 'text-text-muted'}>
                {recording && <span className="h-2 w-2 animate-pulse rounded-full bg-red" />}
                {recording ? 'Recording' : 'Stopped'}
              </span>
              <span className="text-text-dim">·</span>
              <span className="text-text-secondary">{steps.length} step{steps.length === 1 ? '' : 's'}</span>
            </div>

            {recording && (
              <>
                <form className="flex gap-2" onSubmit={(e) => { e.preventDefault(); go(); }}>
                  <input className="field flex-1 font-mono text-[12px]" value={url} onChange={(e) => setUrl(e.target.value)}
                    placeholder="Go to a page — example.com/login" aria-label="Address" spellCheck={false} autoComplete="off" disabled={!!busy} />
                  <button className="btn-ghost" type="submit" disabled={!!busy || !url.trim()}>Go</button>
                </form>
                <LiveView frameSrc={frame} fps={fps} frameAgeMs={frameAt ? Date.now() - frameAt : null} send={send} interactive={!busy} />
              </>
            )}

            <div className="max-h-40 overflow-auto rounded-lg border border-border bg-bg-card px-3 py-2 font-mono text-[11px] text-text-secondary">
              {steps.length
                ? steps.map((s, i) => <div key={i} className="truncate">{i + 1}. {describeStep(s)}</div>)
                : <span className="text-text-dim">Nothing yet — click and type in the view above.</span>}
            </div>

            {!recording && (
              <>
                <div>
                  <label className="label" htmlFor="rec-name">Name</label>
                  <input id="rec-name" className="field font-mono" value={name} autoComplete="off" placeholder="portal-login"
                    onChange={(e) => setName(e.target.value)} />
                </div>
                <div>
                  <label className="label" htmlFor="rec-desc">What does this flow do?</label>
                  <input id="rec-desc" className="field" value={description} autoComplete="off" placeholder="Log into the portal and open the eligibility screen"
                    onChange={(e) => setDescription(e.target.value)} />
                  <p className="mt-1 text-[12px] text-text-muted">Used to finish the job if a replay breaks. Every value you typed, picked or clicked is already a variable.</p>
                </div>
              </>
            )}
          </>
        )}
      </div>
    </Dialog>
  );
}

/** How long a browser started from here gets to dial in — cloud ones take up to ~90s. */
const CONNECT_TIMEOUT_MS = 150_000;

/** Run a playbook on a live browser and follow it, answering it if it stops for a person. */
function RunDialog({ apiKey, playbook, browsers, personas, onClose, onFinished }: {
  apiKey: string; playbook: PlaybookInfo; browsers: BrowserRow[]; personas: Persona[];
  onClose: () => void; onFinished: () => void;
}) {
  const toast = useToast();
  // A profile is chosen when a browser starts, never at replay, so picking one here
  // means picking a browser already running it — or starting one that does.
  const [persona, setPersona] = useState('');
  const matching = useMemo(() => (persona ? browsers.filter((b) => b.persona === persona) : browsers), [browsers, persona]);
  const [browserId, setBrowserId] = useState(browsers[0]?.id || '');
  // Prefilled with what the recording used, which is what the replay does with an
  // untouched field anyway — so the form shows the run it is about to make.
  const [values, setValues] = useState<Record<string, string>>(() => ({ ...playbook.defaults }));
  const [autoHeal, setAutoHeal] = useState(true);
  const [run, setRun] = useState<RunInfo | null>(null);
  const [reply, setReply] = useState('');
  const [starting, setStarting] = useState(false);
  const [pending, setPending] = useState<{ id: string; until: number } | null>(null);

  const runId = run?.id;
  const ended = run?.status === 'succeeded' || run?.status === 'failed';
  useEffect(() => {
    if (!runId || ended) return;
    const timer = setInterval(async () => {
      try {
        const next = await api<RunInfo>(`/runs/${encodeURIComponent(runId)}`, { key: apiKey });
        setRun(next);
        if (next.status === 'succeeded' || next.status === 'failed') onFinished();
      } catch { /* keep following */ }
    }, 2000);
    return () => clearInterval(timer);
  }, [runId, ended, apiKey, onFinished]);

  const start = useCallback(async (on: string) => {
    setStarting(true);
    try {
      setRun(await api<RunInfo>(`/browsers/${encodeURIComponent(on)}/runs`, {
        key: apiKey, method: 'POST', body: { playbook: playbook.name, data: values, autoHeal },
      }));
    } catch (err) { toast(errorMessage(err), 'error'); }
    finally { setStarting(false); }
  }, [apiKey, playbook.name, values, autoHeal, toast]);

  // Keep the browser choice inside the profile filter.
  useEffect(() => {
    if (!matching.some((b) => b.id === browserId)) setBrowserId(matching[0]?.id || '');
  }, [matching, browserId]);

  /** Nothing is running on this profile: start one, then replay on it once it dials in. */
  const startAndRun = async () => {
    setStarting(true);
    try {
      const { id } = await api<{ id: string }>('/browsers/start', { key: apiKey, method: 'POST', body: { persona } });
      setPending({ id, until: Date.now() + CONNECT_TIMEOUT_MS });
    } catch (err) { toast(errorMessage(err), 'error'); setStarting(false); }
  };

  // The dashboard already polls GET /browsers every 3s, so watching the prop is the
  // whole wait — no second poller, and it ends on its own deadline.
  useEffect(() => {
    if (!pending) return;
    if (browsers.some((b) => b.id === pending.id)) { setPending(null); void start(pending.id); }
    else if (Date.now() > pending.until) {
      setPending(null);
      setStarting(false);
      toast('The browser did not connect in time. It may still come up — check the Browsers tab.', 'error');
    }
  }, [pending, browsers, start, toast]);

  const respond = async () => {
    if (!run) return;
    try {
      await api(`/runs/${encodeURIComponent(run.id)}/respond`, { key: apiKey, method: 'POST', body: { response: reply.trim() || 'done' } });
      setReply('');
      setRun({ ...run, status: 'running', attention: null });
    } catch (err) { toast(errorMessage(err), 'error'); }
  };

  const result = run?.result;

  return (
    <Dialog open onClose={onClose} title={`Run ${playbook.name}`} description="Replays on a running browser without the LLM."
      footer={run
        ? <button className="btn-ghost" onClick={onClose}>Close</button>
        : <>
            <button className="btn-ghost" onClick={onClose}>Cancel</button>
            {matching.length
              ? <button className="btn-primary" onClick={() => start(browserId)} disabled={!browserId || starting}>{starting ? 'Starting…' : 'Run'}</button>
              : <button className="btn-primary" onClick={startAndRun} disabled={!persona || starting}>{starting ? 'Starting a browser…' : 'Start one and run'}</button>}
          </>}>
      {!run ? (
        <div className="flex flex-col gap-4">
          <div>
            <label className="label" htmlFor="pb-persona">Profile</label>
            <select id="pb-persona" className="field" value={persona} onChange={(e) => setPersona(e.target.value)} disabled={starting}>
              <option value="">Any profile</option>
              {personas.map((p) => <option key={p.id} value={p.id}>{p.name}{p.isDefault ? ' · default' : ''}</option>)}
            </select>
          </div>
          <div>
            <label className="label" htmlFor="pb-browser">Browser</label>
            {matching.length
              ? <select id="pb-browser" className="field" value={browserId} onChange={(e) => setBrowserId(e.target.value)} disabled={starting}>
                  {matching.map((b) => <option key={b.id} value={b.id}>{b.name} · {b.id}</option>)}
                </select>
              : persona
                ? <p className="text-sm text-yellow">No browser is running this profile. Start one and the replay follows it there — its logins, fingerprint and exit IP come with it.</p>
                : <p className="text-sm text-text-muted">No browser is running. Start one from the Browsers tab.</p>}
          </div>
          {playbook.variables.map((v) => (
            <div key={v}>
              <label className="label" htmlFor={`pb-var-${v}`}>{v}</label>
              <input id={`pb-var-${v}`} className="field font-mono" value={values[v] || ''} autoComplete="off"
                onChange={(e) => setValues((cur) => ({ ...cur, [v]: e.target.value }))} />
            </div>
          ))}
          <label className="flex items-start gap-2 text-sm text-text-secondary">
            <input type="checkbox" className="mt-0.5 h-4 w-4" checked={autoHeal} onChange={(e) => setAutoHeal(e.target.checked)} />
            <span>Auto-heal: if the page changed, the agent finishes the run and saves its fix as a draft to review.</span>
          </label>
        </div>
      ) : (
        <div className="flex flex-col gap-4 text-sm">
          <p>
            Status:{' '}
            <span className={run.status === 'failed' ? 'text-red' : run.status === 'needs_attention' ? 'text-yellow' : 'text-accent'}>
              {run.status === 'needs_attention' ? 'needs a person' : run.status}
            </span>
          </p>

          {run.attention && (
            <div className="flex flex-col gap-3 rounded-lg border border-yellow/40 bg-yellow/10 p-3">
              <div className="flex items-center gap-2">
                <span className="rounded border border-yellow/40 px-1.5 text-[11px] uppercase tracking-wider text-yellow">{ATTENTION[run.attention.reason]}</span>
                {run.attention.liveViewUrl && (
                  <a className="ml-auto inline-flex items-center gap-1 text-[12px] text-accent hover:underline" href={run.attention.liveViewUrl} target="_blank" rel="noreferrer">
                    Open live view <ExternalLink className="h-3 w-3" />
                  </a>
                )}
              </div>
              <p className="text-text-secondary">{run.attention.message}</p>
              <div className="flex gap-2">
                <input className="field flex-1" value={reply} onChange={(e) => setReply(e.target.value)} aria-label="Reply"
                  placeholder={run.attention.reason === 'agent' ? 'Your answer'
                    : run.attention.reason === 'mfa' ? 'Paste the code, or finish in the live view and type done'
                    : 'done'}
                  onKeyDown={(e) => { if (e.key === 'Enter') respond(); }} />
                <button className="btn-primary" onClick={respond}>{run.attention.reason === 'agent' ? 'Reply' : 'Done, continue'}</button>
              </div>
            </div>
          )}

          {run.status === 'succeeded' && result && (
            <div className="flex flex-col gap-1 text-text-secondary">
              {result.total !== undefined && <p>Replayed {result.steps} of {result.total} steps without the LLM.</p>}
              {result.healed && <p className="text-yellow">The page had changed. The agent finished the run and saved its fix as a draft for review.</p>}
              {result.fellBack && !result.healed && <p className="text-yellow">A person finished the run.</p>}
              {result.text && <p className="whitespace-pre-wrap">{result.text}</p>}
            </div>
          )}
          {run.status === 'failed' && <p className="text-red">{run.error}</p>}
        </div>
      )}
    </Dialog>
  );
}
