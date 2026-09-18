/**
 * The "Browsers" panel: the default browser provider, its credentials, and a
 * way to connect Oya Desktop for signed-in sessions.
 */
'use client';

import { ArrowUpRight, Loader2, Monitor } from 'lucide-react';
import { isOyaProvider, type KeyConfig } from '../config';
import { CREDENTIAL_LABELS, SAVED_PLACEHOLDER } from './constants';
import { Row, Secret, Select } from './fields';
import { SectionHeading } from './heading';
import type { SettingsForm } from './use-draft';

/** A provider as GET /config lists it. */
type Provider = KeyConfig['providers'][number];

/** What the browsers panel edits. */
interface Props {
  /** The saved settings. */
  config: KeyConfig;
  /** The draft. */
  form: SettingsForm;
  /** The desktop sign-in link is being made. */
  pairing: boolean;
  /** Opens the desktop app. */
  openDesktop: () => void;
}

/** The default-provider select, with a readiness note for the chosen one. */
function ProviderRow({
  config,
  form,
  provider,
}: Omit<Props, 'pairing' | 'openDesktop'> & { /** The chosen provider. */ provider?: Provider }) {
  return (
    <Row id="settings-browser-provider" label="Default provider" hint="Used when you start a browser.">
      <Select
        id="settings-browser-provider"
        value={form.value('browser_provider')}
        onChange={(v) => form.set('browser_provider', v)}
      >
        <option value="">Server default</option>
        {config.providers.map((p) => (
          <option key={p.id} value={p.id}>
            {p.label}
          </option>
        ))}
      </Select>
      {provider && (
        <p
          className={`mt-2 flex items-center gap-1.5 text-[11.5px] ${provider.configured ? 'text-accent' : 'text-text-muted'}`}
        >
          <span className={`h-1.5 w-1.5 rounded-full ${provider.configured ? 'bg-accent' : 'bg-yellow'}`} />
          {provider.configured ? 'Connected and ready' : 'Setup required'}
        </p>
      )}
    </Row>
  );
}

/** One secret row per credential the chosen provider needs. */
function CredentialRows({
  form,
  provider,
}: {
  /** The draft. */ form: SettingsForm;
  /** The chosen provider. */ provider?: Provider;
}) {
  return provider?.needs.map((f) => (
    <Row key={f} id={`settings-${f}`} label={CREDENTIAL_LABELS[f] || f.replace(/_/g, ' ')}>
      <Secret
        id={`settings-${f}`}
        value={form.draft[f] ?? ''}
        onChange={(v) => form.set(f, v)}
        placeholder={form.saved(f) ? SAVED_PLACEHOLDER : 'Enter connection details'}
      />
    </Row>
  ));
}

/** Oya browsers inherit a desktop's logins: offer to connect one. */
function DesktopPrompt({ pairing, openDesktop }: Pick<Props, 'pairing' | 'openDesktop'>) {
  return (
    <div className="mt-7 border-t border-border pt-5">
      <div className="flex items-start gap-3">
        <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border border-border bg-bg-sunken">
          <Monitor className="h-4 w-4 text-text-secondary" />
        </div>
        <div>
          <h4 className="text-[13px] font-medium">Bring your signed-in accounts.</h4>
          <p className="mt-1 text-[12px] leading-5 text-text-muted">
            Connect Oya Desktop to save your sessions to a profile.
          </p>
          <div className="mt-3 flex flex-wrap items-center gap-3">
            <button type="button" className="btn-ghost" onClick={openDesktop} disabled={pairing}>
              {pairing && <Loader2 className="h-3.5 w-3.5 animate-spin" />}Open desktop
            </button>
            <a
              href="/downloads"
              target="_blank"
              rel="noreferrer"
              className="inline-flex items-center gap-1 text-[12px] text-text-muted hover:text-text"
            >
              Download
              <ArrowUpRight className="h-3.5 w-3.5" />
            </a>
          </div>
        </div>
      </div>
    </div>
  );
}

/** The panel. */
export default function BrowsersSection({ config, form, pairing, openDesktop }: Props) {
  const provider = config.providers.find((p) => p.id === form.value('browser_provider'));
  return (
    <>
      <SectionHeading eyebrow="Execution" title="A home for your browsers.">
        Choose where new browser sessions run.
      </SectionHeading>
      <div className="space-y-5">
        <ProviderRow config={config} form={form} provider={provider} />
        <CredentialRows form={form} provider={provider} />
      </div>
      {isOyaProvider(form.value('browser_provider')) && <DesktopPrompt pairing={pairing} openDesktop={openDesktop} />}
      {provider && !provider.configured && !provider.needs.length && (
        <p className="mt-5 rounded-lg border border-yellow/20 bg-yellow/5 p-3 text-[12px] leading-5 text-text-secondary">
          Cloud setup must be completed on this server before new sessions can start. Your connected desktop is still
          available.
        </p>
      )}
    </>
  );
}
