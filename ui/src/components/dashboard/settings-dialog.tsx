'use client';

import { useEffect, useState, type ReactNode } from 'react';
import { ArrowUpRight, Bell, Check, ChevronDown, Cpu, Eye, EyeOff, KeyRound, Loader2, Monitor, RotateCcw, ShieldCheck } from 'lucide-react';
import Dialog from '@/components/ui/dialog';
import { useToast } from './toast';
import { loadConfig, saveConfig, desktopSignInUrl, LLM_PRESETS, isOyaProvider, type KeyConfig } from './config';
import SlackSection from './slack-section';

type Props = { open: boolean; onClose: () => void; apiKey: string; onRerunSetup?: () => void; initialSection?: Section };
type Section = 'model' | 'browsers' | 'verification' | 'alerts';
const SECTIONS = [
  { id: 'model' as const, label: 'AI model', icon: Cpu },
  { id: 'browsers' as const, label: 'Browsers', icon: Monitor },
  { id: 'verification' as const, label: 'Verification', icon: ShieldCheck },
  { id: 'alerts' as const, label: 'Alerts', icon: Bell },
];
const MODELS: Record<string, { id: string; label: string }[]> = {
  openai: [{ id: 'gpt-4o-mini', label: 'GPT-4o mini' }, { id: 'gpt-4.1-mini', label: 'GPT-4.1 mini' }, { id: 'gpt-4.1', label: 'GPT-4.1' }],
  anthropic: [{ id: 'claude-sonnet-4-5', label: 'Claude Sonnet 4.5' }, { id: 'claude-haiku-4-5', label: 'Claude Haiku 4.5' }],
  gemini: [{ id: 'gemini-3.8-flash', label: 'Gemini 3.8 Flash' }, { id: 'gemini-3.5-flash-lite', label: 'Gemini 3.5 Flash-Lite' }, { id: 'gemini-3.1-pro-preview', label: 'Gemini 3.1 Pro (preview)' }],
  vertex: [{ id: 'gemini-2.5-flash', label: 'Gemini 2.5 Flash' }, { id: 'gemini-2.5-pro', label: 'Gemini 2.5 Pro' }],
};
const CREDENTIAL_LABELS: Record<string, string> = {
  anchor_api_key: 'Anchor API key', browserbase_api_key: 'Browserbase API key',
  browserbase_project_id: 'Project ID', steel_api_key: 'Steel API key',
  browseruse_api_key: 'Browser Use API key', cdp_ws_url: 'Chrome connection URL',
};

function Select({ id, value, onChange, children }: { id: string; value: string; onChange: (value: string) => void; children: ReactNode }) {
  return <div className="relative"><select id={id} className="settings-input appearance-none pr-10" value={value} onChange={e => onChange(e.target.value)}>{children}</select><ChevronDown aria-hidden="true" className="pointer-events-none absolute right-3.5 top-1/2 h-4 w-4 -translate-y-1/2 text-text-muted" /></div>;
}

function Secret({ id, value, onChange, placeholder }: { id: string; value: string; onChange: (value: string) => void; placeholder: string }) {
  const [visible, setVisible] = useState(false);
  return <div className="relative"><input id={id} className="settings-input pr-11 font-mono text-[12px]" type={visible ? 'text' : 'password'} autoComplete="off" spellCheck={false} placeholder={placeholder} value={value} onChange={e => onChange(e.target.value)} /><button type="button" onClick={() => setVisible(!visible)} className="absolute right-1 top-1 flex h-9 w-9 items-center justify-center rounded-md text-text-muted hover:bg-text/5 hover:text-text" aria-label={visible ? 'Hide credential' : 'Show credential'}>{visible ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}</button></div>;
}

function Row({ id, label, hint, children }: { id?: string; label: string; hint?: string; children: ReactNode }) {
  return <div className="settings-row"><div className="pt-0.5"><label htmlFor={id} className="text-[13px] font-medium text-text">{label}</label>{hint && <p className="mt-1 text-[12px] leading-5 text-text-muted">{hint}</p>}</div><div className="min-w-0">{children}</div></div>;
}

export default function SettingsDialog(props: Props) {
  // A new editing session starts with saved values. Cancel never leaves a
  // hidden draft behind, and switching API keys cannot carry credentials over.
  return props.open ? <SettingsEditor key={props.apiKey} {...props} /> : null;
}

function SettingsEditor({ onClose, apiKey, onRerunSetup, initialSection }: Props) {
  const toast = useToast();
  const [section, setSection] = useState<Section>(initialSection || 'model');
  const [config, setConfig] = useState<KeyConfig | null>(null);
  const [draft, setDraft] = useState<Record<string, string>>({});
  const [saving, setSaving] = useState(false);
  const [pairing, setPairing] = useState(false);
  const [error, setError] = useState('');
  const [attempt, setAttempt] = useState(0);
  const [customModel, setCustomModel] = useState(false);

  useEffect(() => {
    let cancelled = false;
    loadConfig(apiKey).then(data => { if (!cancelled) { setConfig(data); setError(''); } })
      .catch(err => { if (!cancelled) setError(err instanceof Error ? err.message : 'Could not load settings.'); });
    return () => { cancelled = true; };
  }, [apiKey, attempt]);

  const saved = (field: string) => typeof config?.[field as keyof KeyConfig] === 'string' ? config[field as keyof KeyConfig] as string : '';
  const value = (field: string) => draft[field] ?? saved(field);
  const set = (field: string, next: string) => setDraft(previous => {
    const updated = { ...previous };
    if (next === saved(field)) delete updated[field]; else updated[field] = next;
    return updated;
  });
  const baseUrl = config?.effective.baseUrl || '';
  const originalProvider = config?.llm_provider || (baseUrl.includes('anthropic.com') ? 'anthropic' : baseUrl.includes('aiplatform.googleapis.com') ? 'vertex' : baseUrl.includes('generativelanguage.googleapis.com') ? 'gemini' : 'openai');
  const provider = value('llm_provider') || originalProvider;
  const providerChanged = provider !== originalProvider;
  const models = MODELS[provider] || [];
  const model = value('chat_model') || config?.effective.model || '';
  const custom = customModel || !models.some(m => m.id === model);
  const browserProvider = config?.providers.find(p => p.id === value('browser_provider'));
  const dirty = Object.keys(draft).length > 0;
  const needsModelKey = providerChanged && !draft.openai_api_key?.trim();

  const chooseProvider = (next: string) => {
    if (provider === next) return;
    setCustomModel(false);
    setDraft(previous => {
      const updated = { ...previous, llm_provider: next, chat_model: LLM_PRESETS.find(p => p.id === next)!.model, openai_base_url: '' };
      delete (updated as Record<string, string>).openai_api_key;
      return updated;
    });
  };
  const close = () => { if (!saving) onClose(); };
  const save = async () => {
    if (!dirty || saving || needsModelKey || !model.trim()) return;
    setSaving(true); setError('');
    try { await saveConfig(apiKey, draft); toast('Settings saved', 'success'); onClose(); }
    catch (err) { setError(err instanceof Error ? err.message : 'Could not save settings.'); }
    finally { setSaving(false); }
  };
  const openDesktop = async () => {
    setPairing(true);
    try { window.location.href = await desktopSignInUrl(apiKey); }
    catch (err) { setError(err instanceof Error ? err.message : 'Could not open desktop.'); }
    finally { setPairing(false); }
  };

  return (
    <Dialog open onClose={close} title="Settings" description="Your defaults. Every browser, ready to work." size="lg"
      footer={<><div className="mr-auto flex items-center gap-2 text-[12px] text-text-muted"><span className={`h-1.5 w-1.5 rounded-full ${dirty ? 'bg-yellow' : 'bg-text-dim/50'}`} /><span className="hidden sm:inline">{dirty ? 'Unsaved changes' : 'Changes apply to this API key'}</span></div><button className="btn-ghost h-9" onClick={close} disabled={saving}>Cancel</button><button className="btn-primary h-9 min-w-[100px]" onClick={save} disabled={!config || !dirty || saving || needsModelKey || !model.trim()}>{saving ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Check className="h-3.5 w-3.5" />}Save</button></>}>
      <div className="-mx-5 -my-4 grid min-h-[390px] min-w-0 md:grid-cols-[156px_minmax(0,1fr)]">
        <nav aria-label="Settings sections" className="flex flex-col border-b border-border bg-bg-sunken/60 md:border-b-0 md:border-r">
          <div role="tablist" aria-label="Settings" className="flex gap-1 p-3 md:flex-col md:py-5" onKeyDown={e => {
            if (!['ArrowRight','ArrowLeft','ArrowUp','ArrowDown','Home','End'].includes(e.key)) return;
            e.preventDefault();
            const index = SECTIONS.findIndex(s => s.id === section);
            const last = SECTIONS.length - 1;
            const next = e.key === 'Home' ? 0 : e.key === 'End' ? last : (index + (['ArrowRight','ArrowDown'].includes(e.key) ? 1 : last)) % SECTIONS.length;
            setSection(SECTIONS[next].id);
            e.currentTarget.querySelectorAll<HTMLButtonElement>('[role="tab"]')[next].focus();
          }}>
            {SECTIONS.map(s => <button key={s.id} role="tab" id={`settings-tab-${s.id}`} aria-selected={section === s.id} aria-controls={`settings-panel-${s.id}`} tabIndex={section === s.id ? 0 : -1} onClick={() => setSection(s.id)} className={`flex flex-1 items-center gap-2.5 rounded-lg px-3 py-2.5 text-left text-[12.5px] font-medium md:flex-none ${section === s.id ? 'bg-bg-card text-text shadow-sm ring-1 ring-border' : 'text-text-muted hover:bg-text/5 hover:text-text'}`}><s.icon className={`hidden h-4 w-4 shrink-0 sm:block ${section === s.id ? 'text-accent' : ''}`} />{s.label}</button>)}
          </div>
          {onRerunSetup && <button onClick={() => { close(); onRerunSetup(); }} disabled={saving} className="m-3 mt-auto hidden items-center gap-2 rounded-md px-3 py-2 text-left text-[11.5px] text-text-muted hover:bg-text/5 hover:text-text md:flex"><RotateCcw className="h-3.5 w-3.5" />Run setup again</button>}
        </nav>
        <div className="min-w-0 px-5 py-6 sm:px-7">
          {error && <div role="alert" className="mb-5 rounded-lg border border-red/25 bg-red/5 p-3 text-[13px] text-red">{error}{!config && <button className="ml-3 underline" onClick={() => setAttempt(a => a + 1)}>Retry</button>}</div>}
          {!config ? <div className="flex min-h-[270px] items-center justify-center gap-2 text-sm text-text-muted" role="status"><Loader2 className="h-4 w-4 animate-spin" />Loading your preferences…</div> : (
            <fieldset disabled={saving} role="tabpanel" id={`settings-panel-${section}`} aria-labelledby={`settings-tab-${section}`} className="min-w-0">
              {section === 'model' && <>
                <div className="mb-7"><p className="mb-2 text-[10px] font-semibold uppercase tracking-[0.18em] text-accent">Intelligence</p><h3 className="text-[22px] font-semibold tracking-[-0.035em]">Choose how Oya thinks.</h3><p className="mt-1.5 text-[13px] leading-6 text-text-muted">The model behind your browser conversations.</p></div>
                <div className="space-y-5">
                  <Row label="Provider" hint="Connect your AI account."><div role="group" aria-label="AI provider" className="grid grid-cols-3 gap-2">{LLM_PRESETS.map(p => <button key={p.id} type="button" aria-pressed={provider === p.id} onClick={() => chooseProvider(p.id)} className={`flex h-11 items-center justify-between rounded-lg border px-3.5 text-[13px] font-medium ${provider === p.id ? 'border-accent/50 bg-accent/[0.06] text-text' : 'border-border bg-bg-sunken text-text-muted hover:border-text-dim'}`}><span>{p.label}</span>{provider === p.id && <Check className="h-3.5 w-3.5 text-accent" />}</button>)}</div></Row>
                  <Row id="settings-model" label="Model" hint="Use a preset or your own model ID."><Select id="settings-model" value={custom ? '__custom' : model} onChange={next => { if (next === '__custom') setCustomModel(true); else { setCustomModel(false); set('chat_model', next); } }}>{models.map(m => <option key={m.id} value={m.id}>{m.label}</option>)}<option value="__custom">Custom model…</option></Select>{custom && <input aria-label="Custom model ID" className="settings-input mt-2 font-mono text-[12px]" value={model} placeholder="Enter a model ID" onChange={e => set('chat_model', e.target.value)} />}</Row>
                  <Row id="settings-model-key" label="API key" hint={config.inherited && !providerChanged ? 'Using the server’s shared key.' : 'Your credential stays private.'}><Secret id="settings-model-key" value={draft.openai_api_key ?? ''} onChange={v => set('openai_api_key', v)} placeholder={!providerChanged && config.openai_api_key ? 'Saved · leave blank to keep' : LLM_PRESETS.find(p => p.id === provider)?.hint || 'Enter your API key'} />{needsModelKey && <p className="mt-2 text-[12px] leading-5 text-yellow">Enter a key for {LLM_PRESETS.find(p => p.id === provider)?.label} to switch providers.</p>}</Row>
                </div>
                <details className="mt-6 border-t border-border pt-4"><summary className="cursor-pointer text-[12px] font-medium text-text-muted hover:text-text">Advanced connection</summary><div className="mt-4"><Row id="settings-base-url" label="API base URL" hint="For a compatible gateway."><input id="settings-base-url" className="settings-input font-mono text-[12px]" type="url" value={value('openai_base_url')} onChange={e => set('openai_base_url', e.target.value)} placeholder="Use provider default" /><p className="mt-2 text-[11.5px] leading-5 text-text-muted">A custom endpoint requires your own API key.</p></Row></div></details>
              </>}
              {section === 'browsers' && <>
                <div className="mb-7"><p className="mb-2 text-[10px] font-semibold uppercase tracking-[0.18em] text-accent">Execution</p><h3 className="text-[22px] font-semibold tracking-[-0.035em]">A home for your browsers.</h3><p className="mt-1.5 text-[13px] leading-6 text-text-muted">Choose where new browser sessions run.</p></div>
                <div className="space-y-5"><Row id="settings-browser-provider" label="Default provider" hint="Used when you start a browser."><Select id="settings-browser-provider" value={value('browser_provider')} onChange={v => set('browser_provider', v)}><option value="">Server default</option>{config.providers.map(p => <option key={p.id} value={p.id}>{p.label}</option>)}</Select>{browserProvider && <p className={`mt-2 flex items-center gap-1.5 text-[11.5px] ${browserProvider.configured ? 'text-accent' : 'text-text-muted'}`}><span className={`h-1.5 w-1.5 rounded-full ${browserProvider.configured ? 'bg-accent' : 'bg-yellow'}`} />{browserProvider.configured ? 'Connected and ready' : 'Setup required'}</p>}</Row>
                {browserProvider?.needs.map(f => <Row key={f} id={`settings-${f}`} label={CREDENTIAL_LABELS[f] || f.replace(/_/g, ' ')}><Secret id={`settings-${f}`} value={draft[f] ?? ''} onChange={v => set(f,v)} placeholder={saved(f) ? 'Saved · leave blank to keep' : 'Enter connection details'} /></Row>)}</div>
                {isOyaProvider(value('browser_provider')) && <div className="mt-7 border-t border-border pt-5"><div className="flex items-start gap-3"><div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border border-border bg-bg-sunken"><Monitor className="h-4 w-4 text-text-secondary" /></div><div><h4 className="text-[13px] font-medium">Bring your signed-in accounts.</h4><p className="mt-1 text-[12px] leading-5 text-text-muted">Connect Oya Desktop to save your sessions to a profile.</p><div className="mt-3 flex flex-wrap items-center gap-3"><button type="button" className="btn-ghost" onClick={openDesktop} disabled={pairing}>{pairing && <Loader2 className="h-3.5 w-3.5 animate-spin" />}Open desktop</button><a href="/downloads" target="_blank" rel="noreferrer" className="inline-flex items-center gap-1 text-[12px] text-text-muted hover:text-text">Download<ArrowUpRight className="h-3.5 w-3.5" /></a></div></div></div></div>}
                {browserProvider && !browserProvider.configured && !browserProvider.needs.length && <p className="mt-5 rounded-lg border border-yellow/20 bg-yellow/5 p-3 text-[12px] leading-5 text-text-secondary">Cloud setup must be completed on this server before new sessions can start. Your connected desktop is still available.</p>}
              </>}
              {section === 'alerts' && <SlackSection apiKey={apiKey} Row={Row} />}
              {section === 'verification' && <>
                <div className="mb-7"><p className="mb-2 text-[10px] font-semibold uppercase tracking-[0.18em] text-accent">Continuity</p><h3 className="text-[22px] font-semibold tracking-[-0.035em]">Keep the session moving.</h3><p className="mt-1.5 text-[13px] leading-6 text-text-muted">Choose how browsers handle verification prompts.</p></div>
                <div className="space-y-5"><Row id="settings-captcha" label="CAPTCHA solver" hint="Optional automatic solving."><Select id="settings-captcha" value={value('captcha_solver')} onChange={v => set('captcha_solver',v)}><option value="">Manual · ask for help</option><option value="capsolver">CapSolver</option></Select></Row>{value('captcha_solver') && <Row id="settings-captcha-key" label="Solver API key" hint="From your solver account."><Secret id="settings-captcha-key" value={draft.captcha_api_key ?? ''} onChange={v => set('captcha_api_key',v)} placeholder={config.captcha_api_key ? 'Saved · leave blank to keep' : 'Enter solver API key'} /></Row>}</div>
                <div className="mt-7 flex gap-3 border-t border-border pt-5"><KeyRound className="mt-0.5 h-4 w-4 shrink-0 text-text-muted" /><div><h4 className="text-[13px] font-medium">Two-factor authentication</h4><p className="mt-1 text-[12px] leading-5 text-text-muted">Manage MFA inside each profile. If a prompt needs your attention, finish it in the live browser.</p></div></div>
              </>}
            </fieldset>
          )}
        </div>
      </div>
    </Dialog>
  );
}
